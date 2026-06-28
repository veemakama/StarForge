use crate::utils::{config, documentation, print as p};
use anyhow::Result;
use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum DocsCommands {
    /// Generate documentation from a contract
    Generate(GenerateDocsArgs),
    /// Search for contracts in the documentation portal
    Search(SearchDocsArgs),
    /// View documentation for a specific contract
    View(ViewDocsArgs),
    /// List all documented contracts
    List,
    /// Generate HTML documentation portal
    Portal(PortalArgs),
    /// Manage documentation versions
    #[command(subcommand)]
    Version(VersionCommands),
}

#[derive(Args)]
pub struct GenerateDocsArgs {
    /// Path to the WASM file
    #[arg(long)]
    pub wasm: PathBuf,
    /// Contract ID
    pub contract_id: String,
    /// Contract name
    #[arg(long)]
    pub name: String,
    /// Contract description
    #[arg(long)]
    pub description: String,
    /// Author's wallet name
    #[arg(long)]
    pub wallet: String,
    /// Network (testnet/mainnet)
    #[arg(long, default_value = "testnet")]
    pub network: String,
    /// Deployment transaction hash
    #[arg(long)]
    pub deployment_tx: Option<String>,
}

#[derive(Args)]
pub struct SearchDocsArgs {
    /// Search query
    pub query: String,
}

#[derive(Args)]
pub struct ViewDocsArgs {
    /// Contract ID
    pub contract_id: String,
    /// Output format (html/json)
    #[arg(long, default_value = "json")]
    pub format: String,
    /// Output file path
    #[arg(long)]
    pub output: Option<PathBuf>,
}

#[derive(Args)]
pub struct PortalArgs {
    /// Output directory for HTML portal
    #[arg(long, default_value = "./docs-portal")]
    pub output: PathBuf,
}

#[derive(Subcommand)]
pub enum VersionCommands {
    /// Create a new documentation version
    Create(CreateVersionArgs),
    /// List versions for a contract
    List(ListVersionsArgs),
    /// View a specific version
    View(ViewVersionArgs),
}

#[derive(Args)]
pub struct CreateVersionArgs {
    /// Contract ID
    pub contract_id: String,
    /// Version number
    pub version: String,
    /// Changelog for this version
    #[arg(long)]
    pub changelog: String,
}

#[derive(Args)]
pub struct ListVersionsArgs {
    /// Contract ID
    pub contract_id: String,
}

#[derive(Args)]
pub struct ViewVersionArgs {
    /// Contract ID
    pub contract_id: String,
    /// Version number
    pub version: String,
}

pub fn handle(cmd: DocsCommands) -> Result<()> {
    match cmd {
        DocsCommands::Generate(args) => handle_generate(args),
        DocsCommands::Search(args) => handle_search(args),
        DocsCommands::View(args) => handle_view(args),
        DocsCommands::List => handle_list(),
        DocsCommands::Portal(args) => handle_portal(args),
        DocsCommands::Version(version_cmd) => handle_version(version_cmd),
    }
}

fn handle_generate(args: GenerateDocsArgs) -> Result<()> {
    let cfg = config::load()?;
    let wallet = cfg.wallets.iter()
        .find(|w| &w.name == &args.wallet)
        .ok_or_else(|| anyhow::anyhow!("Wallet '{}' not found", args.wallet))?;
    
    let home_dir = dirs::home_dir().context("Failed to get home directory")?;
    let docs_dir = home_dir.join(".starforge").join("documentation");
    
    p::header("Generate Contract Documentation");
    p::kv("Contract ID", &args.contract_id);
    p::kv("Name", &args.name);
    p::kv("Description", &args.description);
    p::kv("WASM", &args.wasm.display().to_string());
    p::kv("Network", &args.network);
    
    let metadata = documentation::DocumentationMetadata {
        network: args.network.clone(),
        deployer: wallet.public_key.clone(),
        deployment_tx: args.deployment_tx.unwrap_or_else(|| "unknown".to_string()),
        soroban_version: "22.0.0".to_string(),
        stellar_version: "20.0.0".to_string(),
    };
    
    let generator = documentation::DocumentationGenerator::new(docs_dir);
    let mut doc = generator.generate_from_wasm(&args.wasm, &args.contract_id, metadata)?;
    
    // Update with provided metadata
    doc.name = args.name;
    doc.description = args.description;
    doc.author = wallet.public_key.clone();
    
    generator.save_documentation(&doc)?;
    
    p::success("Documentation generated successfully");
    p::kv("Documentation ID", &doc.contract_id);
    p::kv("Functions", &doc.functions.len().to_string());
    
    Ok(())
}

fn handle_search(args: SearchDocsArgs) -> Result<()> {
    let home_dir = dirs::home_dir().context("Failed to get home directory")?;
    let docs_dir = home_dir.join(".starforge").join("documentation");
    
    p::header("Search Documentation");
    p::kv("Query", &args.query);
    
    let portal = documentation::DocumentationPortal::new(docs_dir);
    let results = portal.search(&args.query)?;
    
    if results.is_empty() {
        p::info("No contracts found matching your search");
    } else {
        p::success(&format!("Found {} contract(s)", results.len()));
        
        println!();
        for result in results {
            p::kv_accent("Name", &result.name);
            p::kv("Contract ID", &result.contract_id);
            p::kv("Description", &result.description);
            p::kv("Version", &result.version);
            p::kv("Network", &result.network);
            println!();
        }
    }
    
    Ok(())
}

fn handle_view(args: ViewDocsArgs) -> Result<()> {
    let home_dir = dirs::home_dir().context("Failed to get home directory")?;
    let docs_dir = home_dir.join(".starforge").join("documentation");
    
    p::header("View Contract Documentation");
    p::kv("Contract ID", &args.contract_id);
    p::kv("Format", &args.format);
    
    let portal = documentation::DocumentationPortal::new(docs_dir);
    let doc = portal.get_documentation(&args.contract_id)?;
    
    match args.format.as_str() {
        "json" => {
            let json = serde_json::to_string_pretty(&doc)?;
            
            if let Some(output_path) = args.output {
                fs::write(&output_path, json)?;
                p::success(&format!("Documentation saved to {}", output_path.display()));
            } else {
                println!("{}", json);
            }
        }
        "html" => {
            let output_path = args.output.unwrap_or_else(|| PathBuf::from(format!("{}.html", args.contract_id)));
            portal.generate_contract_html(&args.contract_id, &output_path)?;
            p::success(&format!("HTML documentation saved to {}", output_path.display()));
        }
        _ => {
            // Default to pretty print
            println!();
            p::kv_accent("Contract Name", &doc.name);
            p::kv("Contract ID", &doc.contract_id);
            p::kv("Description", &doc.description);
            p::kv("Version", &doc.version);
            p::kv("Author", &doc.author);
            p::kv("Network", &doc.metadata.network);
            
            println!();
            p::header("Functions");
            for func in &doc.functions {
                println!();
                p::kv("Name", &func.name);
                p::kv("Description", &func.description);
                p::kv("Access", format!("{:?}", func.access));
                if !func.inputs.is_empty() {
                    p::kv("Inputs", &func.inputs.iter().map(|p| format!("{}: {}", p.name, p.param_type)).collect::<Vec<_>>().join(", "));
                }
            }
            
            println!();
            p::header("Usage Examples");
            for example in &doc.examples {
                println!();
                p::kv("Title", &example.title);
                p::kv("Description", &example.description);
                println!("Code:");
                println!("{}", example.code);
            }
        }
    }
    
    Ok(())
}

fn handle_list() -> Result<()> {
    let home_dir = dirs::home_dir().context("Failed to get home directory")?;
    let docs_dir = home_dir.join(".starforge").join("documentation");
    
    p::header("Documented Contracts");
    
    let portal = documentation::DocumentationPortal::new(docs_dir);
    let contracts = portal.list_contracts()?;
    
    if contracts.is_empty() {
        p::info("No documented contracts found");
    } else {
        p::success(&format!("Found {} contract(s)", contracts.len()));
        
        println!();
        for contract in contracts {
            p::kv_accent("Name", &contract.name);
            p::kv("Contract ID", &contract.contract_id);
            p::kv("Description", &contract.description);
            p::kv("Version", &contract.version);
            p::kv("Network", &contract.network);
            println!();
        }
    }
    
    Ok(())
}

fn handle_portal(args: PortalArgs) -> Result<()> {
    let home_dir = dirs::home_dir().context("Failed to get home directory")?;
    let docs_dir = home_dir.join(".starforge").join("documentation");
    
    p::header("Generate Documentation Portal");
    p::kv("Output directory", &args.output.display().to_string());
    
    fs::create_dir_all(&args.output)?;
    
    let portal = documentation::DocumentationPortal::new(docs_dir);
    
    // Generate main index
    let index_path = args.output.join("index.html");
    portal.generate_html_portal(&index_path)?;
    p::success(&format!("Generated index.html at {}", index_path.display()));
    
    // Generate individual contract pages
    let contracts = portal.list_contracts()?;
    for contract in contracts {
        let contract_path = args.output.join(format!("{}.html", contract.contract_id));
        if let Err(e) = portal.generate_contract_html(&contract.contract_id, &contract_path) {
            p::warn(&format!("Failed to generate page for {}: {}", contract.name, e));
        } else {
            p::kv("Generated", &contract_path.display().to_string());
        }
    }
    
    p::success("Documentation portal generated successfully");
    p::info(&format!("Open {} in your browser to view the portal", index_path.display()));
    
    Ok(())
}

fn handle_version(cmd: VersionCommands) -> Result<()> {
    match cmd {
        VersionCommands::Create(args) => handle_create_version(args),
        VersionCommands::List(args) => handle_list_versions(args),
        VersionCommands::View(args) => handle_view_version(args),
    }
}

fn handle_create_version(args: CreateVersionArgs) -> Result<()> {
    let home_dir = dirs::home_dir().context("Failed to get home directory")?;
    let docs_dir = home_dir.join(".starforge").join("documentation");
    let versions_dir = home_dir.join(".starforge").join("documentation-versions");
    
    p::header("Create Documentation Version");
    p::kv("Contract ID", &args.contract_id);
    p::kv("Version", &args.version);
    
    // Load current documentation
    let portal = documentation::DocumentationPortal::new(docs_dir);
    let doc = portal.get_documentation(&args.contract_id)?;
    
    let version_manager = documentation::DocumentationVersionManager::new(versions_dir);
    let version = version_manager.create_version(&args.contract_id, doc, &args.changelog)?;
    
    p::success("Documentation version created successfully");
    p::kv("Version", &version.version);
    p::kv("Created at", &version.created_at);
    
    Ok(())
}

fn handle_list_versions(args: ListVersionsArgs) -> Result<()> {
    let home_dir = dirs::home_dir().context("Failed to get home directory")?;
    let versions_dir = home_dir.join(".starforge").join("documentation-versions");
    
    p::header("Documentation Versions");
    p::kv("Contract ID", &args.contract_id);
    
    let version_manager = documentation::DocumentationVersionManager::new(versions_dir);
    let versions = version_manager.list_versions(&args.contract_id)?;
    
    if versions.is_empty() {
        p::info("No versions found for this contract");
    } else {
        p::success(&format!("Found {} version(s)", versions.len()));
        
        println!();
        for version in versions {
            p::kv_accent("Version", &version.version);
            p::kv("Created at", &version.created_at);
            p::kv("Changelog", &version.changelog.lines().next().unwrap_or("No changelog"));
            println!();
        }
    }
    
    Ok(())
}

fn handle_view_version(args: ViewVersionArgs) -> Result<()> {
    let home_dir = dirs::home_dir().context("Failed to get home directory")?;
    let versions_dir = home_dir.join(".starforge").join("documentation-versions");
    
    p::header("View Documentation Version");
    p::kv("Contract ID", &args.contract_id);
    p::kv("Version", &args.version);
    
    let version_manager = documentation::DocumentationVersionManager::new(versions_dir);
    let version = version_manager.get_version(&args.contract_id, &args.version)?;
    
    println!();
    p::kv_accent("Version", &version.version);
    p::kv("Created at", &version.created_at);
    p::kv("Contract name", &version.documentation.name);
    p::kv("Description", &version.documentation.description);
    
    println!();
    p::header("Changelog");
    println!("{}", version.changelog);
    
    println!();
    p::header("Functions");
    for func in &version.documentation.functions {
        println!();
        p::kv("Name", &func.name);
        p::kv("Description", &func.description);
    }
    
    Ok(())
}
