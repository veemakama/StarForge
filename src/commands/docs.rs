use crate::utils::{doc_generator, docs, print as p};
use anyhow::Result;
use clap::Subcommand;
use colored::Colorize;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum DocsCommands {
    /// Generate documentation for a contract (metadata-driven)
    Generate {
        /// On-chain contract ID
        contract: String,
        /// Human-friendly contract name
        #[arg(long)]
        name: String,
        /// Short description of the contract
        #[arg(long)]
        description: String,
        /// Network (testnet / mainnet)
        #[arg(long, default_value = "testnet")]
        network: String,
        /// Documentation version
        #[arg(long, default_value = "1.0.0")]
        version: String,
    },

    /// Extract doc comments from a Rust source file or directory
    Extract {
        /// Path to a `.rs` file or a directory containing `.rs` files
        path: PathBuf,
        /// Save the extracted data to the docs store under this contract ID
        #[arg(long)]
        contract: Option<String>,
        /// Print a summary table instead of full JSON
        #[arg(long)]
        summary: bool,
    },

    /// Show stored documentation for a contract
    Show {
        /// Contract ID
        contract: String,
        /// Specific version (latest if omitted)
        #[arg(long)]
        version: Option<String>,
    },

    /// List all documented contracts
    List,

    /// Full-text search across all documentation
    Search {
        /// Search query
        query: String,
    },

    /// Show documentation versions for a contract
    Versions {
        /// Contract ID
        contract: String,
    },

    /// Export stored documentation as Markdown (printed to stdout)
    Export {
        /// Contract ID
        contract: String,
        /// Version to export (latest if omitted)
        #[arg(long)]
        version: Option<String>,
    },
    /// Extract doc comments from a Soroban contract source file
    Extract {
        /// Path to the contract source file (.rs) or directory
        source: String,
        /// Output file path for extracted JSON (stdout if omitted)
        #[arg(long)]
        output: Option<String>,
        /// Output format: json or markdown (default: json)
        #[arg(long, default_value = "json")]
        format: String,
    },
    /// Generate HTML documentation site from a contract source file
    Html {
        /// Contract ID (used for output directory naming)
        contract: String,
        /// Contract display name
        #[arg(long)]
        name: String,
        /// Path to the contract source file (.rs)
        #[arg(long)]
        source: String,
        /// Output directory for the generated HTML site
        #[arg(long)]
        output_dir: Option<String>,
        /// Custom template directory (overrides built-in templates)
        #[arg(long)]
        template_dir: Option<String>,
    },
    /// Generate an API reference (JSON + Markdown) from a contract source file
    ApiRef {
        /// Contract ID
        contract: String,
        /// Contract display name
        #[arg(long)]
        name: String,
        /// Path to the contract source file (.rs)
        #[arg(long)]
        source: String,
        /// Documentation version
        #[arg(long, default_value = "1.0.0")]
        version: String,
        /// Output directory (defaults to ~/.starforge/docs/<contract>/)
        #[arg(long)]
        output_dir: Option<String>,
    },
    /// Publish generated HTML documentation to a destination
    Publish {
        /// Contract ID
        contract: String,
        /// Source directory containing the generated HTML site
        #[arg(long)]
        source_dir: Option<String>,
        /// Destination path or remote rsync target (e.g. user@host:/var/www/docs)
        #[arg(long)]
        dest: Option<String>,
        /// Also write a deploy.sh script for manual deployment
        #[arg(long)]
        generate_script: bool,
    },
}

pub async fn handle(cmd: DocsCommands) -> Result<()> {
    match cmd {
        DocsCommands::Generate {
            contract,
            name,
            description,
            network,
            version,
        } => generate(contract, name, description, network, version),

        DocsCommands::Extract {
            path,
            contract,
            summary,
        } => extract(path, contract, summary),

        DocsCommands::Show { contract, version } => show(contract, version),
        DocsCommands::List => list(),
        DocsCommands::Search { query } => search(query),
        DocsCommands::Versions { contract } => versions(contract),
        DocsCommands::Export { contract, version } => export(contract, version),
        DocsCommands::Extract {
            source,
            output,
            format,
        } => extract(source, output, format),
        DocsCommands::Html {
            contract,
            name,
            source,
            output_dir,
            template_dir,
        } => generate_html(contract, name, source, output_dir, template_dir),
        DocsCommands::ApiRef {
            contract,
            name,
            source,
            version,
            output_dir,
        } => generate_api_ref(contract, name, source, version, output_dir),
        DocsCommands::Publish {
            contract,
            source_dir,
            dest,
            generate_script,
        } => publish(contract, source_dir, dest, generate_script),
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// generate
// ──────────────────────────────────────────────────────────────────────────────

fn generate(
    contract: String,
    name: String,
    description: String,
    network: String,
    version: String,
) -> Result<()> {
    p::header("Documentation Generator — Generate");

    p::step(1, 3, "Building documentation structure...");
    let functions = vec![
        docs::FunctionDoc {
            name: "initialize".to_string(),
            description: "Initialize the contract with an admin address.".to_string(),
            parameters: vec![docs::ParamDoc {
                name: "admin".to_string(),
                ty: "Address".to_string(),
                description: "The administrator address.".to_string(),
                required: true,
            }],
            returns: Some("bool".to_string()),
            examples: vec!["contract.initialize(&admin)".to_string()],
        },
        docs::FunctionDoc {
            name: "transfer".to_string(),
            description: "Transfer tokens between accounts.".to_string(),
            parameters: vec![
                docs::ParamDoc {
                    name: "from".to_string(),
                    ty: "Address".to_string(),
                    description: "Source address.".to_string(),
                    required: true,
                },
                docs::ParamDoc {
                    name: "to".to_string(),
                    ty: "Address".to_string(),
                    description: "Destination address.".to_string(),
                    required: true,
                },
                docs::ParamDoc {
                    name: "amount".to_string(),
                    ty: "i128".to_string(),
                    description: "Amount of tokens to transfer.".to_string(),
                    required: true,
                },
            ],
            returns: Some("bool".to_string()),
            examples: vec!["contract.transfer(&from, &to, 1_000)".to_string()],
        },
    ];

    let events = vec![docs::EventDoc {
        name: "Transfer".to_string(),
        description: "Emitted on every successful token transfer.".to_string(),
        topics: vec![
            docs::TopicDoc {
                name: "from".to_string(),
                ty: "Address".to_string(),
                description: "Source address.".to_string(),
            },
            docs::TopicDoc {
                name: "to".to_string(),
                ty: "Address".to_string(),
                description: "Destination address.".to_string(),
            },
        ],
    }];

    let storage = vec![
        docs::StorageDoc {
            key: "admin".to_string(),
            ty: "Address".to_string(),
            description: "Contract administrator.".to_string(),
        },
        docs::StorageDoc {
            key: "balances".to_string(),
            ty: "Map<Address, i128>".to_string(),
            description: "Token balances for all accounts.".to_string(),
        },
    ];

    let sections = vec![
        docs::DocSection {
            title: "Overview".to_string(),
            content: format!(
                "{} is a Soroban smart contract on {}. {}",
                name, network, description
            ),
            order: 0,
        },
        docs::DocSection {
            title: "Getting Started".to_string(),
            content: format!(
                "Deploy {} to {} and interact via the Soroban RPC.",
                name, network
            ),
            order: 1,
        },
        docs::DocSection {
            title: "Security".to_string(),
            content: "All state-changing operations require address-based authorization.".to_string(),
            order: 2,
        },
    ];

    p::step(2, 3, "Saving documentation...");
    let entry = docs::generate_documentation(
        &contract,
        &name,
        &description,
        &network,
        &version,
        functions,
        events,
        storage,
        sections,
    )?;

    p::step(3, 3, "Updating index...");
    println!();
    p::success(&format!("Documentation generated for '{}'", name));
    p::kv("Contract", &entry.contract_id);
    p::kv("Version", &entry.version);
    p::kv("Network", &entry.network);
    p::kv("Generated", &entry.generated_at[..10]);
    p::info("Use `starforge docs show <contract>` to view.");
    p::info("Use `starforge docs html <contract>` to build HTML.");
    p::info("Use `starforge docs api-ref <contract>` for the API reference.");
    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// extract
// ──────────────────────────────────────────────────────────────────────────────

fn extract(path: PathBuf, contract: Option<String>, summary: bool) -> Result<()> {
    p::header("Documentation Generator — Extract");

    p::step(1, 2, &format!("Extracting doc comments from {}...", path.display()));

    let extracted: Vec<doc_extractor::ExtractedDoc> = if path.is_dir() {
        doc_extractor::extract_from_directory(&path)?
    } else {
        vec![doc_extractor::extract_from_file(&path)?]
    };

    let total_fns: usize = extracted.iter().map(|e| e.functions.len()).sum();
    let total_structs: usize = extracted.iter().map(|e| e.structs.len()).sum();
    let total_enums: usize = extracted.iter().map(|e| e.enums.len()).sum();
    let total_examples: usize = extracted.iter().map(|e| e.examples.len()).sum();

    p::step(2, 2, "Extraction complete.");
    println!();
    p::kv("Files processed", &extracted.len().to_string());
    p::kv("Functions found", &total_fns.to_string());
    p::kv("Structs found", &total_structs.to_string());
    p::kv("Enums found", &total_enums.to_string());
    p::kv("Code examples", &total_examples.to_string());

    if summary {
        // Print summary table.
        println!();
        for doc in &extracted {
            println!(
                "  {} {}",
                "→".cyan(),
                doc.source_file.display().to_string().bright_white()
            );
            if let Some(ref md) = doc.module_doc {
                let first_line = md.lines().next().unwrap_or("");
                println!("    {}", first_line.dimmed());
            }
            for func in &doc.functions {
                println!("    {} fn {}", "•".dimmed(), func.name.cyan());
            }
        }
    } else {
        // Full JSON output.
        let json = serde_json::to_string_pretty(&extracted)?;
        println!("\n{}", json);
    }

    // Optionally persist into the docs store.
    if let Some(contract_id) = contract {
        p::info(&format!("Saving extracted docs under contract '{}'...", contract_id));

        // Build FunctionDoc list from extracted data.
        let functions: Vec<docs::FunctionDoc> = extracted
            .iter()
            .flat_map(|e| {
                e.functions.iter().map(|f| docs::FunctionDoc {
                    name: f.name.clone(),
                    description: f.doc.lines().next().unwrap_or("").to_string(),
                    parameters: f
                        .params
                        .iter()
                        .map(|p| docs::ParamDoc {
                            name: p.name.clone(),
                            ty: p.ty.clone(),
                            description: String::new(),
                            required: true,
                        })
                        .collect(),
                    returns: f.return_type.clone(),
                    examples: f.examples.clone(),
                })
            })
            .collect();

        let module_desc = extracted
            .first()
            .and_then(|e| e.module_doc.as_deref())
            .unwrap_or("")
            .lines()
            .next()
            .unwrap_or("")
            .to_string();

        docs::generate_documentation(
            &contract_id,
            &contract_id,
            &module_desc,
            "testnet",
            "1.0.0",
            functions,
            vec![],
            vec![],
            vec![],
        )?;

        p::success("Extracted documentation saved to docs store.");
    }

    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// show
// ──────────────────────────────────────────────────────────────────────────────

fn show(contract: String, version: Option<String>) -> Result<()> {
    p::header("Documentation Portal — View");

    let entry = docs::get_documentation(&contract, version.as_deref())?;

    p::separator();
    p::kv_accent("Contract", &entry.name);
    p::kv("ID", &entry.contract_id);
    p::kv("Version", &entry.version);
    p::kv("Network", &entry.network);
    p::kv("Generated", &entry.generated_at[..10]);
    p::separator();
    println!();

    for section in &entry.sections {
        println!("  {} {}", "##".dimmed(), section.title.bright_white());
        println!("  {}", section.content.dimmed());
        println!();
    }

    if !entry.api.functions.is_empty() {
        p::info("API Reference — Functions");
        for func in &entry.api.functions {
            println!("  {} `{}`", "→".cyan(), func.name.bright_white());
            println!("    {}", func.description);
            for param in &func.parameters {
                let req = if param.required { "required" } else { "optional" };
                println!(
                    "    • {} ({}): {} [{}]",
                    param.name, param.ty, param.description, req
                );
            }
            if let Some(ref ret) = func.returns {
                println!("    Returns: {}", ret);
            }
            println!();
        }
    }

    if !entry.api.events.is_empty() {
        p::info("API Reference — Events");
        for event in &entry.api.events {
            println!("  {} `{}`", "→".cyan(), event.name.bright_white());
            println!("    {}", event.description);
            for topic in &event.topics {
                println!("    • {} ({}): {}", topic.name, topic.ty, topic.description);
            }
            println!();
        }
    }

    if !entry.api.storage.is_empty() {
        p::info("Storage Layout");
        for s in &entry.api.storage {
            println!("  • {} ({}): {}", s.key, s.ty, s.description);
        }
    }

    println!();
    p::separator();
    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// list
// ──────────────────────────────────────────────────────────────────────────────

fn list() -> Result<()> {
    p::header("Documentation Portal — Index");

    let index = docs::list_documentation()?;

    if index.contracts.is_empty() {
        p::info("No documentation generated yet. Use `starforge docs generate` first.");
        return Ok(());
    }

    for contract in &index.contracts {
        println!(
            "  {} {} ({} versions)",
            "→".cyan(),
            contract.name.bright_white(),
            contract.versions.len()
        );
        p::kv("Contract ID", &contract.contract_id);
        if let Some(latest) = contract.versions.first() {
            p::kv("Latest", &latest.version);
        }
        println!();
    }

    p::kv("Total", &index.contracts.len().to_string());
    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// search
// ──────────────────────────────────────────────────────────────────────────────

fn search(query: String) -> Result<()> {
    p::header(&format!("Documentation Search: '{}'", query));

    let results = docs::search_documentation(&query)?;

    if results.is_empty() {
        p::info("No documentation matched your query.");
        return Ok(());
    }

    p::kv("Matches", &results.len().to_string());
    println!();

    for result in &results {
        println!(
            "  {} {} (score: {})",
            "→".cyan(),
            result.name.bright_white(),
            result.score
        );
        p::kv("Contract", &result.contract_id);
        p::kv("Version", &result.version);
        if !result.matched_sections.is_empty() {
            p::kv("Matched", &result.matched_sections.join(", "));
        }
        println!();
    }

    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// versions
// ──────────────────────────────────────────────────────────────────────────────

fn versions(contract: String) -> Result<()> {
    p::header("Documentation Portal — Versions");
    p::kv("Contract", &contract);

    let versions = docs::list_versions(&contract)?;

    if versions.is_empty() {
        p::info("No documentation versions found.");
        return Ok(());
    }

    println!();
    for v in &versions {
        println!("  {} v{}", "→".cyan(), v.bright_white());
    }

    println!();
    p::kv("Versions", &versions.len().to_string());
    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// export
// ──────────────────────────────────────────────────────────────────────────────

fn export(contract: String, version: Option<String>) -> Result<()> {
    p::header("Documentation Portal — Export Markdown");
    let md = docs::render_markdown(&contract, version.as_deref())?;
    println!("{}", md);
    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// html
// ──────────────────────────────────────────────────────────────────────────────

fn html(contract: String, output: PathBuf, templates: Option<PathBuf>) -> Result<()> {
    p::header("Documentation Generator — HTML Site");

    p::step(1, 3, "Loading documentation...");
    let entry = docs::get_documentation(&contract, None)?;

    p::step(2, 3, &format!("Rendering HTML to {}...", output.display()));
    let page_path =
        doc_html::generate_html_site(&entry, &output, templates.as_deref())?;

    p::step(3, 3, "HTML site ready.");
    println!();
    p::success("HTML documentation site generated.");
    p::kv("Contract page", &page_path.display().to_string());
    p::kv("Portal index", &output.join("index.html").display().to_string());
    p::info("Open index.html in a browser to view the portal.");
    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// api_ref
// ──────────────────────────────────────────────────────────────────────────────

fn api_ref(contract: String, output: PathBuf, json_only: bool, md_only: bool) -> Result<()> {
    p::header("Documentation Generator — API Reference");

    p::step(1, 3, "Loading documentation...");
    let entry = docs::get_documentation(&contract, None)?;

    p::step(2, 3, "Building API reference...");
    let api_reference = doc_api_ref::build_api_reference(&entry);

    p::step(3, 3, &format!("Writing to {}...", output.display()));

    let emit_json = !md_only;
    let emit_md = !json_only;

    if emit_json {
        doc_api_ref::write_json(&api_reference, &output)?;
        let path = output.join(format!(
            "{}_api.json",
            contract.replace('/', "_")
        ));
        p::kv("JSON ref", &path.display().to_string());
    }
    if emit_md {
        doc_api_ref::write_markdown(&api_reference, &output)?;
        let path = output.join(format!(
            "{}_api.md",
            contract.replace('/', "_")
        ));
        p::kv("Markdown ref", &path.display().to_string());
    }

    println!();
    p::success("API reference generated.");
    p::kv("Functions", &api_reference.functions.len().to_string());
    p::kv("Events", &api_reference.events.len().to_string());
    p::kv("Storage keys", &api_reference.storage.len().to_string());
    Ok(())
}

// ──────────────────────────────────────────────────────────────────────────────
// publish
// ──────────────────────────────────────────────────────────────────────────────

fn publish(
    contract: String,
    build_dir: PathBuf,
    target: String,
    dest: Option<PathBuf>,
    repo: Option<PathBuf>,
    endpoint: Option<String>,
    token: Option<String>,
    api_json: bool,
    api_md: bool,
) -> Result<()> {
    p::header("Documentation Generator — Publish Pipeline");

    p::step(1, 4, "Loading documentation...");
    let entry = docs::get_documentation(&contract, None)?;

    let publish_target = match target.as_str() {
        "github-pages" | "gh-pages" => {
            let repo_path = repo.unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
            doc_publisher::PublishTarget::GitHubPages {
                repo_path,
                commit_message: format!(
                    "docs: publish {} v{}",
                    entry.name, entry.version
                ),
            }
        }
        url if url.starts_with("http") => doc_publisher::PublishTarget::CustomHttp {
            endpoint: url.to_string(),
            auth_token: token,
        },
        _ => {
            let dest_path = dest.unwrap_or_else(|| PathBuf::from("docs-output"));
            doc_publisher::PublishTarget::Local { dest: dest_path }
        }
    };

    p::step(2, 4, "Configuring publish options...");
    let options = doc_publisher::PublishOptions {
        build_dir,
        target: publish_target,
        include_api_json: api_json,
        include_api_markdown: api_md,
        custom_template_dir: None,
    };

    p::step(3, 4, "Running build + publish pipeline...");
    let result = doc_publisher::publish(&entry, &options)?;

    p::step(4, 4, "Recording publish event...");
    let _ = doc_publisher::record_publish(&entry, &result);

    println!();
    p::success("Documentation published successfully.");
    p::kv("Published to", &result.published_to);
    p::kv("Files written", &result.files_written.to_string());
    p::info(&result.message);
    Ok(())
}

fn extract(source: String, output: Option<String>, format: String) -> Result<()> {
    p::header("Documentation — Extract Doc Comments");

    let source_path = PathBuf::from(&source);
    p::step(1, 3, &format!("Reading source: {}", source));

    let extracted = if source_path.is_dir() {
        // Merge docs from all .rs files in the directory
        let mut merged = doc_generator::ExtractedDocs {
            module_doc: String::new(),
            functions: Vec::new(),
            structs: Vec::new(),
            enums: Vec::new(),
            constants: Vec::new(),
            source_path: source.clone(),
        };
        for entry in std::fs::read_dir(&source_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                let docs = doc_generator::DocCommentExtractor::extract_from_file(&path)?;
                merged.functions.extend(docs.functions);
                merged.structs.extend(docs.structs);
                merged.enums.extend(docs.enums);
                merged.constants.extend(docs.constants);
                if merged.module_doc.is_empty() {
                    merged.module_doc = docs.module_doc;
                }
            }
        }
        merged
    } else {
        doc_generator::DocCommentExtractor::extract_from_file(&source_path)?
    };

    p::step(2, 3, "Formatting output...");

    let content = match format.as_str() {
        "markdown" | "md" => {
            let api = doc_generator::ApiReferenceGenerator::from_extracted(
                &extracted,
                &source,
                &source,
                "extracted",
            );
            doc_generator::ApiReferenceGenerator::render_markdown(&api)
        }
        _ => serde_json::to_string_pretty(&extracted)?,
    };

    p::step(3, 3, "Writing output...");

    if let Some(out_path) = output {
        std::fs::write(&out_path, &content)?;
        p::success(&format!("Extracted docs written to '{}'", out_path));
    } else {
        println!("{}", content);
    }

    println!();
    p::kv("Functions found", &extracted.functions.len().to_string());
    p::kv("Structs found", &extracted.structs.len().to_string());
    p::kv("Enums found", &extracted.enums.len().to_string());
    p::kv(
        "Public functions",
        &extracted
            .functions
            .iter()
            .filter(|f| f.visibility == doc_generator::Visibility::Public)
            .count()
            .to_string(),
    );

    Ok(())
}

fn generate_html(
    contract: String,
    name: String,
    source: String,
    output_dir: Option<String>,
    template_dir: Option<String>,
) -> Result<()> {
    p::header("Documentation — Generate HTML Site");

    p::step(1, 4, &format!("Extracting doc comments from '{}'", source));
    let source_path = PathBuf::from(&source);
    let extracted = doc_generator::DocCommentExtractor::extract_from_file(&source_path)?;

    let out_dir = output_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".starforge")
                .join("docs")
                .join("html")
                .join(&contract)
        });

    p::step(2, 4, "Initialising template engine...");
    let mut generator = doc_generator::HtmlDocGenerator::new();
    if let Some(tmpl_dir) = template_dir {
        generator = generator.with_template_dir(&PathBuf::from(tmpl_dir))?;
    }

    p::step(3, 4, &format!("Generating HTML site in '{}'", out_dir.display()));
    generator.generate_site(&extracted, &name, &contract, &out_dir)?;

    p::step(4, 4, "Writing publish manifest...");
    doc_generator::DocPublisher::write_manifest(&out_dir, &contract, "latest")?;

    println!();
    p::success(&format!("HTML documentation generated for '{}'", name));
    p::kv("Contract", &contract);
    p::kv("Output", &out_dir.display().to_string());
    p::kv("Functions documented", &extracted.functions.len().to_string());
    p::info(&format!(
        "Open '{}' to view the documentation.",
        out_dir.join("index.html").display()
    ));

    Ok(())
}

fn generate_api_ref(
    contract: String,
    name: String,
    source: String,
    version: String,
    output_dir: Option<String>,
) -> Result<()> {
    p::header("Documentation — Generate API Reference");

    p::step(1, 3, &format!("Extracting from '{}'", source));
    let source_path = PathBuf::from(&source);
    let extracted = doc_generator::DocCommentExtractor::extract_from_file(&source_path)?;

    p::step(2, 3, "Building API reference...");
    let api_ref = doc_generator::ApiReferenceGenerator::from_extracted(
        &extracted,
        &contract,
        &name,
        &version,
    );

    let out_dir = output_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".starforge")
                .join("docs")
                .join(&contract)
        });

    std::fs::create_dir_all(&out_dir)?;

    p::step(3, 3, "Writing JSON and Markdown...");
    let json_path = out_dir.join("api-reference.json");
    let md_path = out_dir.join("api-reference.md");

    doc_generator::ApiReferenceGenerator::save_json(&api_ref, &json_path)?;
    std::fs::write(&md_path, doc_generator::ApiReferenceGenerator::render_markdown(&api_ref))?;

    println!();
    p::success(&format!("API reference generated for '{}' v{}", name, version));
    p::kv("JSON", &json_path.display().to_string());
    p::kv("Markdown", &md_path.display().to_string());
    p::kv("Functions", &api_ref.functions.len().to_string());
    p::kv("Events", &api_ref.events.len().to_string());

    Ok(())
}

fn publish(
    contract: String,
    source_dir: Option<String>,
    dest: Option<String>,
    generate_script: bool,
) -> Result<()> {
    p::header("Documentation — Publish");

    let src = source_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".starforge")
                .join("docs")
                .join("html")
                .join(&contract)
        });

    if !src.exists() {
        p::error(&format!(
            "Source directory '{}' does not exist. Run `starforge docs html` first.",
            src.display()
        ));
        return Ok(());
    }

    p::step(1, 3, &format!("Source: {}", src.display()));

    if let Some(ref destination) = dest {
        p::step(2, 3, &format!("Copying to '{}'", destination));
        let dest_path = PathBuf::from(destination);
        doc_generator::DocPublisher::publish_to_dir(&src, &dest_path)?;
        p::success(&format!("Documentation published to '{}'", destination));
    } else {
        p::step(2, 3, "No --dest specified; skipping file copy");
    }

    p::step(3, 3, "Finalising...");
    doc_generator::DocPublisher::write_manifest(&src, &contract, "latest")?;

    if generate_script {
        let endpoint = dest.as_deref().unwrap_or("user@host:/var/www/docs");
        let script = doc_generator::DocPublisher::generate_deploy_script(&src, endpoint)?;
        p::kv("Deploy script", &script.display().to_string());
    }

    println!();
    p::kv("Contract", &contract);
    p::kv("Source", &src.display().to_string());
    if let Some(d) = &dest {
        p::kv("Destination", d);
    }
    p::info("Manifest written to manifest.json in the source directory.");

    Ok(())
}
