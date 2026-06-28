use crate::utils::{config, orchestration, print as p};
use anyhow::Result;
use clap::{Args, Subcommand};
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum OrchestrateCommands {
    /// Create a new deployment plan
    Create(CreatePlanArgs),
    /// Add a contract to a deployment plan
    AddContract(AddContractArgs),
    /// Add a dependency between contracts
    AddDependency(AddDependencyArgs),
    /// Finalize and validate a deployment plan
    Finalize(FinalizeArgs),
    /// Execute a deployment plan
    Execute(ExecuteArgs),
    /// Rollback a deployment
    Rollback(RollbackArgs),
    /// View a deployment plan
    View(ViewPlanArgs),
    /// List all deployment plans
    List,
    /// Visualize deployment plan
    Visualize(VisualizeArgs),
}

#[derive(Args)]
pub struct CreatePlanArgs {
    /// Plan name
    pub name: String,
    /// Plan description
    #[arg(long)]
    pub description: String,
}

#[derive(Args)]
pub struct AddContractArgs {
    /// Plan ID
    pub plan_id: String,
    /// Contract name
    pub name: String,
    /// Path to WASM file
    #[arg(long)]
    pub wasm: PathBuf,
    /// Network to deploy to
    #[arg(long, default_value = "testnet")]
    pub network: String,
    /// Wallet to use for deployment
    #[arg(long)]
    pub wallet: String,
}

#[derive(Args)]
pub struct AddDependencyArgs {
    /// Plan ID
    pub plan_id: String,
    /// Contract ID that depends on another
    pub contract_id: String,
    /// Contract ID that is depended upon
    pub depends_on: String,
}

#[derive(Args)]
pub struct FinalizeArgs {
    /// Plan ID
    pub plan_id: String,
}

#[derive(Args)]
pub struct ExecuteArgs {
    /// Plan ID
    pub plan_id: String,
}

#[derive(Args)]
pub struct RollbackArgs {
    /// Execution ID to rollback
    pub execution_id: String,
}

#[derive(Args)]
pub struct ViewPlanArgs {
    /// Plan ID
    pub plan_id: String,
}

#[derive(Args)]
pub struct VisualizeArgs {
    /// Plan ID
    pub plan_id: String,
    /// Output format (dot/txt)
    #[arg(long, default_value = "dot")]
    pub format: String,
    /// Output file path
    #[arg(long)]
    pub output: Option<PathBuf>,
}

pub fn handle(cmd: OrchestrateCommands) -> Result<()> {
    match cmd {
        OrchestrateCommands::Create(args) => handle_create(args),
        OrchestrateCommands::AddContract(args) => handle_add_contract(args),
        OrchestrateCommands::AddDependency(args) => handle_add_dependency(args),
        OrchestrateCommands::Finalize(args) => handle_finalize(args),
        OrchestrateCommands::Execute(args) => handle_execute(args),
        OrchestrateCommands::Rollback(args) => handle_rollback(args),
        OrchestrateCommands::View(args) => handle_view(args),
        OrchestrateCommands::List => handle_list(),
        OrchestrateCommands::Visualize(args) => handle_visualize(args),
    }
}

fn handle_create(args: CreatePlanArgs) -> Result<()> {
    let engine = orchestration::OrchestrationEngine::new()?;
    
    p::header("Create Deployment Plan");
    p::kv("Name", &args.name);
    p::kv("Description", &args.description);
    
    let plan = engine.create_plan(&args.name, &args.description)?;
    
    p::success("Deployment plan created successfully");
    p::kv("Plan ID", &plan.id);
    
    Ok(())
}

fn handle_add_contract(args: AddContractArgs) -> Result<()> {
    let cfg = config::load()?;
    let wallet = cfg.wallets.iter()
        .find(|w| &w.name == &args.wallet)
        .ok_or_else(|| anyhow::anyhow!("Wallet '{}' not found", args.wallet))?;
    
    let engine = orchestration::OrchestrationEngine::new()?;
    
    p::header("Add Contract to Plan");
    p::kv("Plan ID", &args.plan_id);
    p::kv("Contract name", &args.name);
    p::kv("WASM", &args.wasm.display().to_string());
    p::kv("Network", &args.network);
    p::kv("Wallet", &wallet.public_key);
    
    let contract = orchestration::ContractDeployment {
        id: format!("contract_{}", uuid::Uuid::new_v4()),
        name: args.name,
        wasm_path: args.wasm,
        contract_id: None,
        network: args.network,
        wallet: wallet.public_key,
        constructor_args: vec![],
        dependencies: vec![],
        status: orchestration::ContractDeploymentStatus::Pending,
        deployed_at: None,
        deployment_tx: None,
        error: None,
    };
    
    engine.add_contract(&args.plan_id, contract)?;
    
    p::success("Contract added to plan successfully");
    
    Ok(())
}

fn handle_add_dependency(args: AddDependencyArgs) -> Result<()> {
    let engine = orchestration::OrchestrationEngine::new()?;
    
    p::header("Add Dependency");
    p::kv("Plan ID", &args.plan_id);
    p::kv("Contract", &args.contract_id);
    p::kv("Depends on", &args.depends_on);
    
    engine.add_dependency(&args.plan_id, &args.contract_id, &args.depends_on)?;
    
    p::success("Dependency added successfully");
    
    Ok(())
}

fn handle_finalize(args: FinalizeArgs) -> Result<()> {
    let engine = orchestration::OrchestrationEngine::new()?;
    
    p::header("Finalize Deployment Plan");
    p::kv("Plan ID", &args.plan_id);
    
    engine.finalize_plan(&args.plan_id)?;
    
    let plan = engine.get_plan(&args.plan_id)?;
    
    p::success("Plan finalized successfully");
    p::kv("State", format!("{:?}", plan.state));
    p::kv("Contracts to deploy", &plan.deployment_order.len().to_string());
    
    println!();
    p::header("Deployment Order");
    for (i, contract_id) in plan.deployment_order.iter().enumerate() {
        p::kv(&format!("{}", i + 1), contract_id);
    }
    
    println!();
    p::header("Rollback Order");
    for (i, contract_id) in plan.rollback_plan.rollback_order.iter().enumerate() {
        p::kv(&format!("{}", i + 1), contract_id);
    }
    
    Ok(())
}

fn handle_execute(args: ExecuteArgs) -> Result<()> {
    let engine = orchestration::OrchestrationEngine::new()?;
    
    p::header("Execute Deployment Plan");
    p::kv("Plan ID", &args.plan_id);
    
    let plan = engine.get_plan(&args.plan_id)?;
    
    if !matches!(plan.state, orchestration::DeploymentState::Ready) {
        anyhow::bail!("Plan is not ready for execution. Current state: {:?}", plan.state);
    }
    
    p::info("Starting deployment execution...");
    
    let execution = engine.execute_plan(&args.plan_id)?;
    
    println!();
    p::kv_accent("Execution ID", &execution.execution_id);
    p::kv("Status", format!("{:?}", execution.status));
    p::kv("Started at", &execution.started_at);
    
    println!();
    p::header("Deployment Results");
    for result in &execution.results {
        let status = if result.success {
            "✓ Success".to_string()
        } else {
            "✗ Failed".to_string()
        };
        p::kv(&result.contract_id, &status);
        p::kv("Duration", &format!("{}ms", result.duration_ms));
        if let Some(address) = &result.contract_address {
            p::kv("Address", address);
        }
        println!();
    }
    
    if matches!(execution.status, orchestration::DeploymentState::Completed) {
        p::success("Deployment completed successfully");
    } else {
        p::warn("Deployment did not complete successfully");
    }
    
    Ok(())
}

fn handle_rollback(args: RollbackArgs) -> Result<()> {
    let engine = orchestration::OrchestrationEngine::new()?;
    
    p::header("Rollback Deployment");
    p::kv("Execution ID", &args.execution_id);
    
    p::warn("This will rollback the deployment. This action cannot be undone.");
    
    let execution = engine.rollback(&args.execution_id)?;
    
    println!();
    p::kv_accent("Execution ID", &execution.execution_id);
    p::kv("Status", format!("{:?}", execution.status));
    
    println!();
    p::header("Rollback Results");
    for result in &execution.results {
        let status = if result.success {
            "✓ Rolled back".to_string()
        } else {
            "✗ Failed".to_string()
        };
        p::kv(&result.contract_id, &status);
        println!();
    }
    
    if matches!(execution.status, orchestration::DeploymentState::RolledBack) {
        p::success("Rollback completed successfully");
    } else {
        p::warn("Rollback did not complete successfully");
    }
    
    Ok(())
}

fn handle_view(args: ViewPlanArgs) -> Result<()> {
    let engine = orchestration::OrchestrationEngine::new()?;
    
    p::header("Deployment Plan Details");
    p::kv("Plan ID", &args.plan_id);
    
    let plan = engine.get_plan(&args.plan_id)?;
    
    println!();
    p::kv_accent("Name", &plan.name);
    p::kv("Description", &plan.description);
    p::kv("State", format!("{:?}", plan.state));
    p::kv("Created at", &plan.created_at);
    p::kv("Updated at", &plan.updated_at);
    
    println!();
    p::header("Contracts");
    for contract in &plan.contracts {
        println!();
        p::kv("ID", &contract.id);
        p::kv("Name", &contract.name);
        p::kv("WASM", &contract.wasm_path.display().to_string());
        p::kv("Network", &contract.network);
        p::kv("Status", format!("{:?}", contract.status));
        if !contract.dependencies.is_empty() {
            p::kv("Dependencies", &contract.dependencies.join(", "));
        }
    }
    
    println!();
    p::header("Dependencies");
    if plan.dependencies.is_empty() {
        p::info("No dependencies defined");
    } else {
        for (contract_id, deps) in &plan.dependencies {
            p::kv(contract_id, &deps.join(", "));
        }
    }
    
    println!();
    p::header("Deployment Order");
    if plan.deployment_order.is_empty() {
        p::info("Deployment order not yet calculated. Run 'finalize' first.");
    } else {
        for (i, contract_id) in plan.deployment_order.iter().enumerate() {
            p::kv(&format!("{}", i + 1), contract_id);
        }
    }
    
    println!();
    p::header("Rollback Plan");
    p::kv("Enabled", &plan.rollback_plan.enabled.to_string());
    if plan.rollback_plan.rollback_order.is_empty() {
        p::info("Rollback order not yet calculated. Run 'finalize' first.");
    } else {
        for (i, contract_id) in plan.rollback_plan.rollback_order.iter().enumerate() {
            p::kv(&format!("{}", i + 1), contract_id);
        }
    }
    
    Ok(())
}

fn handle_list() -> Result<()> {
    let engine = orchestration::OrchestrationEngine::new()?;
    
    p::header("Deployment Plans");
    
    let plans = engine.list_plans()?;
    
    if plans.is_empty() {
        p::info("No deployment plans found");
    } else {
        p::success(&format!("Found {} plan(s)", plans.len()));
        
        println!();
        for plan in plans {
            p::kv_accent("Name", &plan.name);
            p::kv("ID", &plan.id);
            p::kv("Description", &plan.description);
            p::kv("State", format!("{:?}", plan.state));
            p::kv("Contracts", &plan.contracts.len().to_string());
            println!();
        }
    }
    
    Ok(())
}

fn handle_visualize(args: VisualizeArgs) -> Result<()> {
    let engine = orchestration::OrchestrationEngine::new()?;
    
    p::header("Visualize Deployment Plan");
    p::kv("Plan ID", &args.plan_id);
    p::kv("Format", &args.format);
    
    let plan = engine.get_plan(&args.plan_id)?;
    
    match args.format.as_str() {
        "dot" => {
            let dot = orchestration::OrchestrationVisualizer::generate_dependency_graph(&plan)?;
            
            if let Some(output_path) = args.output {
                std::fs::write(&output_path, dot)?;
                p::success(&format!("Dependency graph saved to {}", output_path.display()));
            } else {
                println!("{}", dot);
            }
        }
        "txt" => {
            let mut output = String::new();
            output.push_str(&format!("Deployment Plan: {}\n", plan.name));
            output.push_str(&format!("State: {:?}\n\n", plan.state));
            
            output.push_str("Contracts部署顺序:\n");
            for (i, contract_id) in plan.deployment_order.iter().enumerate() {
                output.push_str(&format!("  {}. {}\n", i + 1, contract_id));
            }
            
            output.push_str("\nDependencies:\n");
            for (contract_id, deps) in &plan.dependencies {
                output.push_str(&format!("  {} -> {}\n", contract_id, deps.join(", ")));
            }
            
            if let Some(output_path) = args.output {
                std::fs::write(&output_path, output)?;
                p::success(&format!("Visualization saved to {}", output_path.display()));
            } else {
                println!("{}", output);
            }
        }
        _ => {
            anyhow::bail!("Unsupported format. Use: dot or txt");
        }
    }
    
    Ok(())
}
