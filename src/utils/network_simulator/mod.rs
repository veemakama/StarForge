//! # Network Simulation and Testing Environment
//!
//! This module provides a local, in-process Stellar/Soroban network simulator
//! for testing contracts under controlled, deterministic conditions:
//!
//! - **`simulator`**: Core Soroban RPC simulator (accounts, contracts, ledgers)
//! - **`deterministic`**: Seeded RNG and deterministic execution parameters
//! - **`state`**: State snapshot/restore, export/import
//! - **`time`**: Ledger time control (advance, freeze, jump)
//! - **`failure`**: Failure injection (RPC errors, transaction failures, network faults)
//! - **`scenarios`**: Pre-built test scenarios (simple counter, token, escrow, …)
//!
//! ## Quick-start
//!
//! ```ignore
//! use starforge::utils::network_simulator::*;
//!
//! let mut sim = simulator::NetworkSimulator::new()
//!     .with_deterministic_seed(42)
//!     .start();
//!
//! // Deploy a contract, invoke it, inspect state…
//! ```

pub mod simulator;
pub mod deterministic;
pub mod state;
pub mod time;
pub mod failure;
pub mod scenarios;

// ── Re-exports for convenience ────────────────────────────────────────────────

pub use simulator::{
    NetworkSimulator,
    SimulatorConfig,
    SimulatorMode,
    AccountInfo,
    ContractInstance,
    LedgerInfo,
    TransactionReceipt,
    SimulationOutcome,
};
pub use deterministic::{DeterministicConfig, SeededRng};
pub use state::{StateSnapshot, SnapshotManager};
pub use time::{TimeController, LedgerTime};
pub use failure::{FailureInjector, FailureMode, FailureRule};
pub use scenarios::{Scenario, ScenarioRunner, BuiltInScenario};
