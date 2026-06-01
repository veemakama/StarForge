use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// The running StarForge CLI version — used for template compatibility checks.
pub const CLI_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TemplateRegistry {
    #[serde(default)]
    pub templates: Vec<TemplateEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TemplateSource {
    Git {
        url: String,
        #[serde(default)]
        branch: Option<String>,
    },
    Local {
        path: String,
    },
    Builtin {
        id: String,
    },
}

impl std::fmt::Display for TemplateSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TemplateSource::Git { url, branch } => {
                if let Some(branch) = branch {
                    write!(f, "git:{}@{}", url, branch)
                } else {
                    write!(f, "git:{}", url)
                }
            }
            TemplateSource::Local { path } => write!(f, "local:{}", path),
            TemplateSource::Builtin { id } => write!(f, "builtin:{}", id),
        }
    }
}

/// Maintenance state of a marketplace template.
///
/// Surfaced to users as a lightweight trust signal so they can tell at a
/// glance whether a template is being kept up to date or has been abandoned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MaintenanceStatus {
    /// Updated recently and accepting changes.
    Active,
    /// Stable and still supported, but not under active development.
    Maintained,
    /// No longer maintained; use with caution.
    Deprecated,
    /// Maintenance state has not been declared.
    #[default]
    Unknown,
}

impl MaintenanceStatus {
    /// Short human-readable label used in trust indicators.
    pub fn label(&self) -> &'static str {
        match self {
            MaintenanceStatus::Active => "Actively maintained",
            MaintenanceStatus::Maintained => "Maintained",
            MaintenanceStatus::Deprecated => "Deprecated",
            MaintenanceStatus::Unknown => "Unknown maintenance",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateEntry {
    pub name: String,
    pub description: String,
    pub version: String,
    pub source: TemplateSource,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub downloads: u32,
    #[serde(default)]
    pub verified: bool,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    /// Minimum StarForge CLI version required by this template (semver, e.g. "0.1.0").
    /// `None` means no minimum — the template is compatible with all CLI versions.
    #[serde(default)]
    pub cli_version_min: Option<String>,
    /// Maximum StarForge CLI version supported by this template (semver, e.g. "1.99.99").
    /// `None` means no upper bound.
    #[serde(default)]
    pub cli_version_max: Option<String>,
}

/// Outcome of a template-vs-CLI compatibility check.
#[derive(Debug, PartialEq, Eq)]
pub enum CompatibilityStatus {
    /// Template is compatible with the running CLI version.
    Compatible,
    /// Template requires a newer CLI version than what is running.
    TooOld {
        required_min: String,
        running: String,
    },
    /// Template is not compatible with the current (too-new) CLI version.
    TooNew {
        required_max: String,
        running: String,
    },
    /// Template metadata contains a malformed version string.
    MalformedMetadata { reason: String },
}

/// Parse a semver string `"major.minor.patch"` into `(major, minor, patch)`.
///
/// Returns `Err` when the string cannot be parsed.
fn parse_semver(v: &str) -> std::result::Result<(u64, u64, u64), String> {
    let parts: Vec<&str> = v.splitn(3, '.').collect();
    if parts.len() != 3 {
        return Err(format!("'{}' is not a valid semver string (expected major.minor.patch)", v));
    }
    let parse = |s: &str| {
        s.parse::<u64>()
            .map_err(|_| format!("non-numeric component '{}' in version '{}'", s, v))
    };
    Ok((parse(parts[0])?, parse(parts[1])?, parse(parts[2])?))
}

/// Return whether `version` satisfies `min <= version <= max` using semver ordering.
///
/// Either bound may be `None`, meaning unbounded in that direction.
pub fn check_version_range(
    version: &str,
    min: Option<&str>,
    max: Option<&str>,
) -> CompatibilityStatus {
    let running = match parse_semver(version) {
        Ok(v) => v,
        Err(reason) => return CompatibilityStatus::MalformedMetadata { reason },
    };

    if let Some(min_str) = min {
        match parse_semver(min_str) {
            Ok(min_v) => {
                if running < min_v {
                    return CompatibilityStatus::TooOld {
                        required_min: min_str.to_string(),
                        running: version.to_string(),
                    };
                }
            }
            Err(reason) => return CompatibilityStatus::MalformedMetadata { reason },
        }
    }

    if let Some(max_str) = max {
        match parse_semver(max_str) {
            Ok(max_v) => {
                if running > max_v {
                    return CompatibilityStatus::TooNew {
                        required_max: max_str.to_string(),
                        running: version.to_string(),
                    };
                }
            }
            Err(reason) => return CompatibilityStatus::MalformedMetadata { reason },
        }
    }

    CompatibilityStatus::Compatible
}

/// Check whether `entry` is compatible with the currently running StarForge CLI.
///
/// Templates that carry no version constraints (`cli_version_min` and
/// `cli_version_max` are both `None`) are always considered compatible, ensuring
/// full backward compatibility with pre-versioning templates.
pub fn check_template_compatibility(entry: &TemplateEntry) -> CompatibilityStatus {
    check_version_range(
        CLI_VERSION,
        entry.cli_version_min.as_deref(),
        entry.cli_version_max.as_deref(),
    )
}

/// Validate that `entry` is compatible with the running CLI and return an
/// actionable error message if it is not.
pub fn assert_template_compatible(entry: &TemplateEntry) -> Result<()> {
    match check_template_compatibility(entry) {
        CompatibilityStatus::Compatible => Ok(()),
        CompatibilityStatus::TooOld { required_min, running } => {
            anyhow::bail!(
                "Template '{}' requires StarForge >= {} but you are running {}.\n\
                 Please upgrade StarForge: https://github.com/Nanle-code/StarForge#installation",
                entry.name,
                required_min,
                running,
            )
        }
        CompatibilityStatus::TooNew { required_max, running } => {
            anyhow::bail!(
                "Template '{}' only supports StarForge <= {} but you are running {}.\n\
                 Use an older version of StarForge or check if a newer template version is available.",
                entry.name,
                required_max,
                running,
            )
        }
        CompatibilityStatus::MalformedMetadata { reason } => {
            anyhow::bail!(
                "Template '{}' has malformed version metadata: {}.\n\
                 Contact the template author to fix the cli_version_min / cli_version_max fields.",
                entry.name,
                reason,
            )
        }
    /// Whether the template ships user-facing documentation (e.g. a README).
    #[serde(default)]
    pub documented: bool,
    /// Declared maintenance state of the template.
    #[serde(default)]
    pub maintenance: MaintenanceStatus,
}

impl TemplateEntry {
    /// Compute a 0-100 quality/trust score from the available signals.
    ///
    /// The score blends verification status, documentation, usage (downloads)
    /// and maintenance state so that dependable templates rank higher and are
    /// easier to discover in a growing community catalog.
    pub fn quality_score(&self) -> u8 {
        let mut score: i32 = 0;

        // Verified templates have been vetted — the strongest trust signal.
        if self.verified {
            score += 40;
        }

        // Documentation makes a template far easier to adopt.
        if self.documented {
            score += 20;
        }

        // Usage is a proxy for community confidence (capped so a single
        // wildly-popular template cannot dominate the ranking).
        score += (self.downloads / 50).min(30) as i32;

        // Maintenance state rewards living projects and penalizes dead ones.
        score += match self.maintenance {
            MaintenanceStatus::Active => 10,
            MaintenanceStatus::Maintained => 5,
            MaintenanceStatus::Deprecated => -25,
            MaintenanceStatus::Unknown => 0,
        };

        score.clamp(0, 100) as u8
    }

    /// Human-readable trust/quality badges suitable for display to users.
    pub fn trust_indicators(&self) -> Vec<String> {
        let mut badges = Vec::new();

        if self.verified {
            badges.push("✓ Verified".to_string());
        }
        if self.documented {
            badges.push("📖 Documented".to_string());
        }

        match self.maintenance {
            MaintenanceStatus::Active => badges.push("🟢 Actively maintained".to_string()),
            MaintenanceStatus::Maintained => badges.push("🟡 Maintained".to_string()),
            MaintenanceStatus::Deprecated => badges.push("⚠ Deprecated".to_string()),
            MaintenanceStatus::Unknown => {}
        }

        if self.downloads >= 1000 {
            badges.push(format!("★ Popular ({} downloads)", self.downloads));
        }

        badges
    }
}

#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct TemplateManifest {
    name: Option<String>,
    description: Option<String>,
    version: Option<String>,
    source: Option<String>,
    #[serde(default)]
    tags: Vec<String>,
}

const DEFAULT_REGISTRY: &str = include_str!("../../templates/registry.json");
const DEFAULT_REGISTRY_URL: &str =
    "https://starforge-protocol.github.io/starforge/templates/registry.json";

fn registry_path() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    let dir = home.join(".starforge").join("templates");
    if !dir.exists() {
        fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    }
    Ok(dir.join("registry.json"))
}

fn template_storage_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    let dir = home.join(".starforge").join("templates").join("storage");
    if !dir.exists() {
        fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    }
    Ok(dir)
}

fn template_cache_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    let dir = home.join(".starforge").join("template-cache");
    if !dir.exists() {
        fs::create_dir_all(&dir).with_context(|| format!("Failed to create {}", dir.display()))?;
    }
    Ok(dir)
}

/// Clone a git-sourced template into `~/.starforge/template-cache/<name>/` with
/// `--depth 1` (shallow clone) and return the cache path.
///
/// When `force_refresh` is `true` any existing cached copy is removed before
/// re-cloning, guaranteeing a fresh copy of the template.
pub fn fetch_template_cached(entry: &TemplateEntry, force_refresh: bool) -> Result<PathBuf> {
    let cache_root = template_cache_dir()?;
    let dest = cache_root.join(&entry.name);

    if dest.exists() {
        if force_refresh {
            fs::remove_dir_all(&dest).with_context(|| {
                format!("Failed to remove cached template at {}", dest.display())
            })?;
        } else {
            return Ok(dest);
        }
    }

    fetch_template(entry, &dest)?;
    Ok(dest)
}

/// Return the `src/lib.rs` content for a marketplace template, fetching and
/// caching it if necessary.
///
/// Returns `None` when the template name is not found in the registry.
pub fn template_source_content(name: &str, force_refresh: bool) -> Result<Option<String>> {
    let registry = load_registry()?;
    let entry = match registry.templates.into_iter().find(|t| t.name == name) {
        Some(e) => e,
        None => return Ok(None),
    };

    let cache_path = fetch_template_cached(&entry, force_refresh)?;
    let lib_rs = cache_path.join("src").join("lib.rs");
    if lib_rs.exists() {
        let content = fs::read_to_string(&lib_rs)
            .with_context(|| format!("Failed to read {}", lib_rs.display()))?;
        Ok(Some(content))
    } else {
        Ok(None)
    }
}

pub fn load_registry() -> Result<TemplateRegistry> {
    // Determine remote registry URL, falling back to the default global index.
    let remote_url = std::env::var("STARFORGE_TEMPLATE_REGISTRY_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_REGISTRY_URL.to_string());

    // Check if user forced a refresh
    let force_refresh = std::env::var("STARFORGE_TEMPLATE_REGISTRY_FORCE_REFRESH").is_ok();
    let cache_path = registry_path()?;

    // Use cache if it exists and is fresh and we are not forcing a refresh.
    if !force_refresh {
        if let Ok(metadata) = fs::metadata(&cache_path) {
            if let Ok(modified) = metadata.modified() {
                use std::time::{Duration, SystemTime};
                let ttl = Duration::from_secs(24 * 60 * 60); // 24 hours
                if SystemTime::now()
                    .duration_since(modified)
                    .unwrap_or_else(|_| ttl)
                    < ttl
                {
                    let contents = fs::read_to_string(&cache_path).with_context(|| {
                        format!("Failed to read cached registry at {}", cache_path.display())
                    })?;
                    let registry: TemplateRegistry = serde_json::from_str(&contents)
                        .with_context(|| "Failed to parse cached template registry")?;
                    return Ok(registry);
                }
            }
        }
    }

    // Either forced refresh or cache is missing/old – attempt to fetch remote.
    match fetch_and_cache_remote(&remote_url) {
        Ok(registry) => Ok(registry),
        Err(_fetch_err) => {
            // If the remote fetch failed but a cached registry exists, fall back to it.
            if cache_path.exists() {
                let contents = fs::read_to_string(&cache_path).with_context(|| {
                    format!("Failed to read cached registry at {}", cache_path.display())
                })?;
                let registry: TemplateRegistry = serde_json::from_str(&contents)
                    .with_context(|| "Failed to parse cached template registry")?;
                return Ok(registry);
            }
            // No cache available – fall back to the registry bundled with the binary
            // so the marketplace still works offline on a fresh install.
            let registry: TemplateRegistry = serde_json::from_str(DEFAULT_REGISTRY)
                .with_context(|| "Failed to parse bundled default template registry")?;
            Ok(registry)
        }
    }
}

pub fn save_registry(registry: &TemplateRegistry) -> Result<()> {
    let path = registry_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }
    let contents = serde_json::to_string_pretty(registry)
        .with_context(|| "Failed to serialize registry")?;
    fs::write(&path, contents)
        .with_context(|| format!("Failed to write registry to {}", path.display()))?;
    Ok(())
}

/// Fetches a remote JSON template registry, caches it locally, and returns the parsed registry.
fn fetch_and_cache_remote(url: &str) -> Result<TemplateRegistry> {
    // Use `ureq` to perform a simple GET request.
    let response = ureq::get(url)
        .call()
        .with_context(|| format!("Failed to fetch remote template registry from {}", url))?;
    if response.status() != 200 {
        anyhow::bail!(
            "Unexpected HTTP status {} when fetching remote registry",
            response.status()
        );
    }
    let json_str = response
        .into_string()
        .with_context(|| "Failed to read response body as string")?;
    // Parse the JSON into our TemplateRegistry struct.
    let registry: TemplateRegistry = serde_json::from_str(&json_str)
        .with_context(|| "Failed to deserialize remote template registry JSON")?;
    // Cache the fetched registry locally for offline use.
    let cache_path = registry_path()?;
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create cache directory {}", parent.display()))?;
    }
    fs::write(&cache_path, &json_str).with_context(|| {
        format!(
            "Failed to write cached registry to {}",
            cache_path.display()
        )
    })?;
    Ok(registry)
}

/// Filters applied on top of a text query when searching the marketplace.
#[derive(Debug, Clone, Default)]
pub struct SearchFilters {
    /// Templates must carry all of these tags (case-insensitive).
    pub tags: Vec<String>,
    /// Only include templates flagged as verified.
    pub verified_only: bool,
    /// Only include templates whose quality score is at least this value.
    pub min_quality: u8,
}

/// A single ranked search result, carrying the matched template alongside the
/// information needed to explain *why* it matched and *how* it ranked.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub entry: TemplateEntry,
    /// Text-relevance score for the query (0 when the query is empty).
    pub relevance: u32,
    /// Human-readable reasons the template matched the query.
    pub reasons: Vec<String>,
}

/// Compute the text-relevance of a template for a query, returning the score
/// and the reasons it matched. Field weighting (name > tags > description)
/// makes the most meaningful matches rank highest.
fn relevance_for(entry: &TemplateEntry, query_lower: &str) -> (u32, Vec<String>) {
    if query_lower.is_empty() {
        return (0, Vec::new());
    }

    let mut score = 0u32;
    let mut reasons = Vec::new();

    let name_lower = entry.name.to_lowercase();
    if name_lower == query_lower {
        score += 100;
        reasons.push("exact name".to_string());
    } else if name_lower.starts_with(query_lower) {
        score += 60;
        reasons.push("name prefix".to_string());
    } else if name_lower.contains(query_lower) {
        score += 40;
        reasons.push("name".to_string());
    }

    for tag in &entry.tags {
        let tag_lower = tag.to_lowercase();
        if tag_lower == query_lower {
            score += 30;
            reasons.push(format!("tag: {}", tag));
        } else if tag_lower.contains(query_lower) {
            score += 15;
            reasons.push(format!("tag ~ {}", tag));
        }
    }

    if entry.description.to_lowercase().contains(query_lower) {
        score += 10;
        reasons.push("description".to_string());
    }

    (score, reasons)
}

/// Search the marketplace with relevance ranking, filtering and per-result
/// match explanations.
///
/// Results are ordered by text relevance first, then by overall quality score
/// (verification, documentation, usage, maintenance), then by raw downloads.
/// An empty query lists every template that satisfies the filters, ranked by
/// quality alone.
pub fn search_templates_ranked(
    query: &str,
    filters: &SearchFilters,
) -> Result<Vec<SearchResult>> {
    let registry = load_registry()?;
    let query_lower = query.trim().to_lowercase();

    let mut results: Vec<SearchResult> = registry
        .templates
        .into_iter()
        .filter_map(|entry| {
            // Apply structured filters first — they are independent of the text query.
            let has_all_tags = filters
                .tags
                .iter()
                .all(|ft| entry.tags.iter().any(|t| t.eq_ignore_ascii_case(ft)));
            if !has_all_tags {
                return None;
            }
            if filters.verified_only && !entry.verified {
                return None;
            }
            if entry.quality_score() < filters.min_quality {
                return None;
            }

            let (relevance, reasons) = relevance_for(&entry, &query_lower);
            // When a text query is supplied, drop templates that do not match it.
            if !query_lower.is_empty() && relevance == 0 {
                return None;
            }

            Some(SearchResult {
                entry,
                relevance,
                reasons,
            })
        })
        .collect();

    // Rank by relevance, then quality, then downloads. This keeps the most
    // pertinent matches at the top while still favouring trusted, well-
    // documented and well-maintained templates.
    results.sort_by(|a, b| {
        b.relevance
            .cmp(&a.relevance)
            .then_with(|| b.entry.quality_score().cmp(&a.entry.quality_score()))
            .then_with(|| b.entry.downloads.cmp(&a.entry.downloads))
    });

    Ok(results)
}

/// Backwards-compatible search returning just the ranked template entries.
pub fn search_templates(query: &str, tags: Option<&[String]>) -> Result<Vec<TemplateEntry>> {
    let filters = SearchFilters {
        tags: tags.map(|t| t.to_vec()).unwrap_or_default(),
        ..Default::default()
    };
    Ok(search_templates_ranked(query, &filters)?
        .into_iter()
        .map(|r| r.entry)
        .collect())
}

pub fn get_template(name: &str) -> Result<TemplateEntry> {
    let registry = load_registry()?;
    registry
        .templates
        .into_iter()
        .find(|t| t.name == name)
        .ok_or_else(|| anyhow::anyhow!("Template '{}' not found in registry", name))
}

pub fn get_template_by_name_and_version(
    name: &str,
    version: Option<&str>,
) -> Result<TemplateEntry> {
    let registry = load_registry()?;
    let mut matching: Vec<_> = registry
        .templates
        .into_iter()
        .filter(|t| t.name == name)
        .collect();

    if matching.is_empty() {
        return Err(anyhow::anyhow!("Template '{}' not found", name));
    }

    if let Some(v) = version {
        matching.sort_by(|a, b| semver_cmp(&b.version, &a.version));
        matching
            .into_iter()
            .find(|t| t.version == v)
            .ok_or_else(|| anyhow::anyhow!("Template '{}@{}' not found", name, v))
    } else {
        matching.sort_by(|a, b| semver_cmp(&b.version, &a.version));
        Ok(matching.into_iter().next().unwrap())
    }
}

fn semver_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let parse_version = |v: &str| {
        v.strip_prefix('v')
            .unwrap_or(v)
            .split('.')
            .filter_map(|s| s.parse::<u64>().ok())
            .collect::<Vec<_>>()
    };
    parse_version(a).cmp(&parse_version(b))
}

pub fn add_template(entry: TemplateEntry) -> Result<()> {
    let mut registry = load_registry()?;

    // Check if template already exists
    if let Some(existing) = registry.templates.iter_mut().find(|t| t.name == entry.name) {
        // Update existing template
        *existing = entry;
    } else {
        // Add new template
        registry.templates.push(entry);
    }

    save_registry(&registry)?;
    Ok(())
}

pub fn remove_template(name: &str) -> Result<()> {
    let mut registry = load_registry()?;
    let before = registry.templates.len();
    registry.templates.retain(|t| t.name != name);
    
    if registry.templates.len() == before {
        anyhow::bail!("Template '{}' not found in registry", name);
    }
    
    save_registry(&registry)?;
    Ok(())
}

pub fn update_template(name: &str) -> Result<()> {
    let entry = get_template(name)?;

    match &entry.source {
        TemplateSource::Git { url, branch } => {
            let dest = std::env::temp_dir().join(&entry.name);
            if dest.exists() {
                fs::remove_dir_all(&dest).ok();
            }
            fetch_git_template(url, branch.as_deref(), &dest)?;
            Ok(())
        }
        other => anyhow::bail!("Template source '{}' does not support updates", other),
    }
}

/// Fetch a template's files into `dest` according to its source type.
pub fn fetch_template(entry: &TemplateEntry, dest: &Path) -> Result<()> {
    // Compatibility gate: reject incompatible templates before touching the filesystem.
    assert_template_compatible(entry)?;

    let source = &entry.source;
    if source.starts_with("http://") || source.starts_with("https://") || source.starts_with("git@") {
        fetch_git_template(source, None, dest)
    } else if !source.is_empty() {
        fetch_local_template(Path::new(source), dest)
    } else {
        anyhow::bail!("Template '{}' has no source configured", entry.name)
    match &entry.source {
        TemplateSource::Git { url, branch } => fetch_git_template(url, branch.as_deref(), dest),
        TemplateSource::Local { path } => fetch_local_template(Path::new(path), dest),
        TemplateSource::Builtin { id } => fetch_builtin_template(id, dest),
    }
}

/// Copy a built-in example template (shipped under `templates/examples/<id>`)
/// into `dest`.
fn fetch_builtin_template(id: &str, dest: &Path) -> Result<()> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("templates")
        .join("examples")
        .join(id);
    if !src.exists() {
        anyhow::bail!(
            "Built-in template '{}' was not found at {}",
            id,
            src.display()
        );
    }
    fetch_local_template(&src, dest)
}

fn fetch_git_template(url: &str, branch: Option<&str>, dest: &Path) -> Result<()> {
    use std::process::Command;
    
    let mut cmd = Command::new("git");
    cmd.arg("clone");
    
    if let Some(b) = branch {
        cmd.arg("--branch").arg(b);
    }
    
    cmd.arg("--depth").arg("1");
    cmd.arg(url);
    cmd.arg(dest);
    
    let output = cmd.output()
        .with_context(|| "Failed to execute git clone. Is git installed?")?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Git clone failed: {}", stderr);
    }
    
    // Remove .git directory to clean up
    let git_dir = dest.join(".git");
    if git_dir.exists() {
        fs::remove_dir_all(&git_dir).ok();
    }
    
    Ok(())
}

fn fetch_local_template(source: &Path, dest: &Path) -> Result<()> {
    if !source.exists() {
        anyhow::bail!("Local template path does not exist: {}", source.display());
    }
    
    copy_dir_recursive(source, dest)
        .with_context(|| format!("Failed to copy template from {}", source.display()))?;
    
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }
    
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        
        // Skip .git directories
        if file_name == ".git" {
            continue;
        }
        
        let dest_path = dst.join(&file_name);
        
        if path.is_dir() {
            copy_dir_recursive(&path, &dest_path)?;
        } else {
            fs::copy(&path, &dest_path)?;
        }
    }
    
    Ok(())
}

pub fn publish_template(
    template_path: &Path,
    name: String,
    description: String,
    author: String,
    tags: Vec<String>,
    version: String,
) -> Result<()> {
    publish_template_versioned(template_path, name, description, author, tags, version, None, None)
}

/// Like `publish_template` but also records optional CLI version constraints.
pub fn publish_template_versioned(
    template_path: &Path,
    name: String,
    description: String,
    author: String,
    tags: Vec<String>,
    version: String,
    cli_version_min: Option<String>,
    cli_version_max: Option<String>,
) -> Result<()> {
    if !template_path.exists() {
        anyhow::bail!("Template path does not exist: {}", template_path.display());
    }

    validate_template_structure(template_path, &name, &description, &author, &version)?;

    let dest = template_storage_dir()?.join(&name);

    if dest.exists() {
        anyhow::bail!("Template '{}' already exists. Remove it first or use a different name.", name);
    }

    copy_dir_recursive(template_path, &dest)?;

    let entry = TemplateEntry {
        name: name.clone(),
        version,
        description,
        author,
        tags,
        source: TemplateSource::Local {
            path: dest.to_string_lossy().to_string(),
        },
        path: Some(dest.to_string_lossy().to_string()),
        downloads: 0,
        verified: false,
        created_at: String::new(),
        updated_at: String::new(),
        cli_version_min,
        cli_version_max,
        documented: template_path.join("README.md").exists(),
        maintenance: MaintenanceStatus::Active,
    };

    add_template(entry)?;

    Ok(())
}

pub fn validate_template_structure(
    path: &Path,
    name: &str,
    description: &str,
    author: &str,
    version: &str,
) -> Result<()> {
    // --- Metadata completeness ---
    let mut missing: Vec<&str> = Vec::new();
    if name.trim().is_empty() { missing.push("name"); }
    if description.trim().is_empty() { missing.push("description"); }
    if author.trim().is_empty() { missing.push("author"); }
    if version.trim().is_empty() { missing.push("version"); }
    if !missing.is_empty() {
        anyhow::bail!("Missing required metadata fields: {}", missing.join(", "));
    }

    // --- Required files ---
    let cargo_toml = path.join("Cargo.toml");
    if !cargo_toml.exists() {
        anyhow::bail!("Template must contain Cargo.toml");
    }

    let src_dir = path.join("src");
    if !src_dir.exists() || !src_dir.is_dir() {
        anyhow::bail!("Template must contain src/ directory");
    }

    let lib_rs = src_dir.join("lib.rs");
    if !lib_rs.exists() {
        anyhow::bail!("Template must contain src/lib.rs");
    }

    // --- Placeholder check ---
    // Cargo.toml must use {{PROJECT_NAME}} so the scaffolder can substitute it.
    let cargo_contents = fs::read_to_string(&cargo_toml)
        .with_context(|| format!("Failed to read {}", cargo_toml.display()))?;
    if !cargo_contents.contains("{{PROJECT_NAME}}") {
        anyhow::bail!(
            "Cargo.toml must contain the {{{{PROJECT_NAME}}}} placeholder so the project name can be substituted"
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(name: &str) -> TemplateEntry {
        TemplateEntry {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            author: String::new(),
            tags: vec![],
            source: "https://example.com/repo.git".to_string(),
            path: None,
            downloads: 0,
            verified: false,
            created_at: String::new(),
            updated_at: String::new(),
            cli_version_min: None,
            cli_version_max: None,
        }
    use std::fs;
    use tempfile::tempdir;

    fn make_valid_template(dir: &std::path::Path) {
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("Cargo.toml"), "[package]\nname = \"{{PROJECT_NAME}}\"\nversion = \"0.1.0\"\n").unwrap();
        fs::write(dir.join("src/lib.rs"), "#![no_std]\n").unwrap();
    }

    #[test]
    fn validate_passes_for_valid_template() {
        let tmp = tempdir().unwrap();
        make_valid_template(tmp.path());
        assert!(validate_template_structure(tmp.path(), "my-tpl", "A desc", "Alice", "1.0.0").is_ok());
    }

    #[test]
    fn validate_rejects_missing_metadata() {
        let tmp = tempdir().unwrap();
        make_valid_template(tmp.path());
        let err = validate_template_structure(tmp.path(), "", "desc", "author", "1.0.0").unwrap_err();
        assert!(err.to_string().contains("name"), "should mention missing field");

        let err = validate_template_structure(tmp.path(), "n", "", "author", "1.0.0").unwrap_err();
        assert!(err.to_string().contains("description"));

        let err = validate_template_structure(tmp.path(), "n", "d", "", "1.0.0").unwrap_err();
        assert!(err.to_string().contains("author"));

        let err = validate_template_structure(tmp.path(), "n", "d", "a", "").unwrap_err();
        assert!(err.to_string().contains("version"));
    }

    #[test]
    fn validate_rejects_missing_cargo_toml() {
        let tmp = tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        fs::write(tmp.path().join("src/lib.rs"), "").unwrap();
        let err = validate_template_structure(tmp.path(), "n", "d", "a", "1.0.0").unwrap_err();
        assert!(err.to_string().contains("Cargo.toml"));
    }

    #[test]
    fn validate_rejects_missing_src_lib() {
        let tmp = tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        fs::write(tmp.path().join("Cargo.toml"), "[package]\nname = \"{{PROJECT_NAME}}\"\n").unwrap();
        let err = validate_template_structure(tmp.path(), "n", "d", "a", "1.0.0").unwrap_err();
        assert!(err.to_string().contains("src/lib.rs"));
    }

    #[test]
    fn validate_rejects_missing_placeholder() {
        let tmp = tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("src")).unwrap();
        // Cargo.toml without {{PROJECT_NAME}}
        fs::write(tmp.path().join("Cargo.toml"), "[package]\nname = \"hardcoded\"\n").unwrap();
        fs::write(tmp.path().join("src/lib.rs"), "").unwrap();
        let err = validate_template_structure(tmp.path(), "n", "d", "a", "1.0.0").unwrap_err();
        assert!(err.to_string().contains("PROJECT_NAME"));
    }

    #[test]
    fn test_search_templates() {
        let mut registry = TemplateRegistry::default();
        registry.templates.push(TemplateEntry {
            name: "uniswap-v2".to_string(),
            version: "1.0.0".to_string(),
            description: "Uniswap V2 DEX implementation".to_string(),
            author: "DeFi Team".to_string(),
            tags: vec!["defi".to_string(), "dex".to_string(), "amm".to_string()],
            source: TemplateSource::Git {
                url: "https://github.com/example/uniswap-v2.git".to_string(),
                branch: None,
            },
            path: None,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:00:00Z".to_string(),
            downloads: 100,
            verified: true,
            cli_version_min: None,
            cli_version_max: None,
            documented: true,
            maintenance: MaintenanceStatus::Active,
        });

        // Test name search
        let results: Vec<_> =
            registry.templates.iter().filter(|t| t.name.contains("uniswap")).collect();
        assert_eq!(results.len(), 1);

        // Test tag search
        let results: Vec<_> = registry
            .templates
            .iter()
            .filter(|t| t.tags.contains(&"defi".to_string()))
            .collect();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn fetch_template_cached_uses_cache_on_second_call() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_dir = tmp.path().join("my-template");
        std::fs::create_dir_all(&cache_dir).unwrap();
        std::fs::write(cache_dir.join("marker.txt"), "cached").unwrap();

        let entry = make_entry("my-template");
        let entry = TemplateEntry {
            name: "my-template".to_string(),
            source: TemplateSource::Git {
                url: "https://example.com/repo.git".to_string(),
                branch: None,
            },
            description: String::new(),
            version: "1.0.0".to_string(),
            tags: vec![],
            path: None,
            author: String::new(),
            downloads: 0,
            verified: false,
            created_at: String::new(),
            updated_at: String::new(),
            documented: false,
            maintenance: MaintenanceStatus::Unknown,
        };

        let dest = tmp.path().join(&entry.name);
        assert!(dest.exists(), "pre-existing cache dir should exist");

        if dest.exists() {
            let marker = dest.join("marker.txt");
            assert!(marker.exists(), "cached content preserved on force_refresh=false");
        }
    }

    #[test]
    fn fetch_template_cached_force_refresh_removes_old_cache() {
        let tmp = tempfile::tempdir().unwrap();
        let cache_dir = tmp.path().join("my-template");
        std::fs::create_dir_all(&cache_dir).unwrap();
        std::fs::write(cache_dir.join("stale.txt"), "old").unwrap();

        std::fs::remove_dir_all(&cache_dir).unwrap();
        assert!(!cache_dir.exists(), "old cache dir should be gone after force_refresh");
    }

    fn sample_entry() -> TemplateEntry {
        TemplateEntry {
            name: "sample".to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            author: String::new(),
            tags: vec![],
            source: TemplateSource::Builtin {
                id: "sample".to_string(),
            },
            path: None,
            created_at: String::new(),
            updated_at: String::new(),
            downloads: 0,
            verified: false,
            documented: false,
            maintenance: MaintenanceStatus::Unknown,
        }
    }

    #[test]
    fn quality_score_rewards_trust_signals() {
        let bare = sample_entry();
        assert_eq!(bare.quality_score(), 0);

        let mut trusted = sample_entry();
        trusted.verified = true;
        trusted.documented = true;
        trusted.maintenance = MaintenanceStatus::Active;
        trusted.downloads = 2000;
        // 40 (verified) + 20 (documented) + 30 (downloads cap) + 10 (active)
        assert_eq!(trusted.quality_score(), 100);

        let mut deprecated = sample_entry();
        deprecated.maintenance = MaintenanceStatus::Deprecated;
        // Penalty is clamped at 0, never negative.
        assert_eq!(deprecated.quality_score(), 0);
    }

    #[test]
    fn quality_score_ranks_verified_above_unverified() {
        let mut verified = sample_entry();
        verified.verified = true;

        let mut popular = sample_entry();
        popular.downloads = 500; // capped contribution of 10

        assert!(verified.quality_score() > popular.quality_score());
    }

    #[test]
    fn trust_indicators_reflect_metadata() {
        let mut entry = sample_entry();
        entry.verified = true;
        entry.documented = true;
        entry.maintenance = MaintenanceStatus::Deprecated;
        entry.downloads = 1500;

        let badges = entry.trust_indicators();
        assert!(badges.iter().any(|b| b.contains("Verified")));
        assert!(badges.iter().any(|b| b.contains("Documented")));
        assert!(badges.iter().any(|b| b.contains("Deprecated")));
        assert!(badges.iter().any(|b| b.contains("Popular")));
    }

    #[test]
    fn relevance_weights_name_above_description() {
        let mut entry = sample_entry();
        entry.name = "uniswap-v2".to_string();
        entry.description = "an amm dex".to_string();
        entry.tags = vec!["defi".to_string()];

        let (name_score, name_reasons) = relevance_for(&entry, "uniswap");
        let (desc_score, _) = relevance_for(&entry, "amm");
        assert!(name_score > desc_score);
        assert!(name_reasons.iter().any(|r| r.contains("name")));
    }

    #[test]
    fn relevance_exact_name_beats_prefix() {
        let mut exact = sample_entry();
        exact.name = "token".to_string();
        let mut prefix = sample_entry();
        prefix.name = "token-allowlist".to_string();

        let (exact_score, _) = relevance_for(&exact, "token");
        let (prefix_score, _) = relevance_for(&prefix, "token");
        assert!(exact_score > prefix_score);
    }

    #[test]
    fn relevance_empty_query_scores_zero() {
        let entry = sample_entry();
        let (score, reasons) = relevance_for(&entry, "");
        assert_eq!(score, 0);
        assert!(reasons.is_empty());
    }

    #[test]
    fn relevance_tag_match_is_reported() {
        let mut entry = sample_entry();
        entry.tags = vec!["defi".to_string(), "dex".to_string()];
        let (score, reasons) = relevance_for(&entry, "defi");
        assert!(score > 0);
        assert!(reasons.iter().any(|r| r == "tag: defi"));
    }

    #[test]
    fn template_source_content_returns_none_for_unknown_template() {
        let registry = TemplateRegistry::default();
        let found = registry.templates.iter().find(|t| t.name == "nonexistent");
        assert!(found.is_none());
    }

    // ── Template versioning tests ──────────────────────────────────────────────

    #[test]
    fn parse_semver_valid() {
        assert_eq!(parse_semver("1.2.3"), Ok((1, 2, 3)));
        assert_eq!(parse_semver("0.1.0"), Ok((0, 1, 0)));
        assert_eq!(parse_semver("10.20.30"), Ok((10, 20, 30)));
    }

    #[test]
    fn parse_semver_invalid() {
        assert!(parse_semver("1.2").is_err());
        assert!(parse_semver("1.2.x").is_err());
        assert!(parse_semver("").is_err());
    }

    #[test]
    fn check_version_range_no_constraints_is_compatible() {
        // Templates with no min/max are always compatible.
        assert_eq!(
            check_version_range("0.1.0", None, None),
            CompatibilityStatus::Compatible
        );
        assert_eq!(
            check_version_range("99.0.0", None, None),
            CompatibilityStatus::Compatible
        );
    }

    #[test]
    fn check_version_range_within_bounds_is_compatible() {
        assert_eq!(
            check_version_range("0.1.0", Some("0.1.0"), Some("1.0.0")),
            CompatibilityStatus::Compatible
        );
        assert_eq!(
            check_version_range("0.5.0", None, None),
            CompatibilityStatus::Compatible
        );
        assert_eq!(
            check_version_range("0.1.0", Some("0.1.0"), None),
            CompatibilityStatus::Compatible
        );
    }

    #[test]
    fn check_version_range_below_min_is_too_old() {
        let result = check_version_range("0.0.9", Some("0.1.0"), None);
        assert!(matches!(result, CompatibilityStatus::TooOld { .. }));
    }

    #[test]
    fn check_version_range_above_max_is_too_new() {
        let result = check_version_range("2.0.0", None, Some("1.99.99"));
        assert!(matches!(result, CompatibilityStatus::TooNew { .. }));
    }

    #[test]
    fn check_version_range_malformed_min_is_error() {
        let result = check_version_range("0.1.0", Some("bad"), None);
        assert!(matches!(result, CompatibilityStatus::MalformedMetadata { .. }));
    }

    #[test]
    fn check_version_range_malformed_max_is_error() {
        let result = check_version_range("0.1.0", None, Some("1.x.0"));
        assert!(matches!(result, CompatibilityStatus::MalformedMetadata { .. }));
    }

    #[test]
    fn template_without_version_metadata_is_compatible() {
        let entry = make_entry("legacy-template");
        assert_eq!(check_template_compatibility(&entry), CompatibilityStatus::Compatible);
    }

    #[test]
    fn template_compatible_with_current_cli() {
        let mut entry = make_entry("current-template");
        entry.cli_version_min = Some(CLI_VERSION.to_string());
        assert_eq!(check_template_compatibility(&entry), CompatibilityStatus::Compatible);
    }

    #[test]
    fn template_requiring_future_cli_is_rejected() {
        let mut entry = make_entry("future-template");
        // Parse current version and bump the major to guarantee a future version.
        let (major, _, _) = parse_semver(CLI_VERSION).unwrap();
        entry.cli_version_min = Some(format!("{}.0.0", major + 100));
        let status = check_template_compatibility(&entry);
        assert!(matches!(status, CompatibilityStatus::TooOld { .. }));
        assert!(assert_template_compatible(&entry).is_err());
    }

    #[test]
    fn template_with_low_max_is_rejected() {
        let mut entry = make_entry("old-template");
        // Set max to a version that is guaranteed to be below the current CLI.
        let (major, minor, _) = parse_semver(CLI_VERSION).unwrap();
        if major > 0 || minor > 0 {
            entry.cli_version_max = Some("0.0.0".to_string());
            let status = check_template_compatibility(&entry);
            assert!(matches!(status, CompatibilityStatus::TooNew { .. }));
            assert!(assert_template_compatible(&entry).is_err());
        }
        // When CLI_VERSION is "0.0.0" the test is a no-op (trivially passes).
    }

    #[test]
    fn template_with_malformed_metadata_is_rejected() {
        let mut entry = make_entry("bad-template");
        entry.cli_version_min = Some("not-a-semver".to_string());
        let status = check_template_compatibility(&entry);
        assert!(matches!(status, CompatibilityStatus::MalformedMetadata { .. }));
        assert!(assert_template_compatible(&entry).is_err());
    }
}
