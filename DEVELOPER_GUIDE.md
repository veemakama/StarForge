# StarForge Developer Guide

Complete guide for developers contributing to or extending StarForge.

## Table of Contents

1. [Plugin Version Compatibility](#plugin-version-compatibility)
2. [Getting Started](#getting-started)
3. [Development Setup](#development-setup)
4. [Project Structure](#project-structure)
5. [Code Style Guide](#code-style-guide)
6. [Adding New Features](#adding-new-features)
7. [Testing](#testing)
8. [Documentation](#documentation)
9. [Common Tasks](#common-tasks)
10. [Debugging](#debugging)
11. [Release Process](#release-process)

---

## Plugin Version Compatibility

StarForge enforces version compatibility when loading plugins to prevent subtle
runtime failures caused by ABI or API mismatches.

### How it works

Every plugin shared library must export a `PLUGIN_DECLARATION` symbol (provided
automatically by the `export_plugin!` macro).  When `starforge plugin load` runs,
the loader checks two fields in that declaration:

| Field | What is checked | Failure behaviour |
|---|---|---|
| `rustc_version` | Must match the exact rustc version used to build StarForge | Hard error — load aborted |
| `core_version` | **Major** version must match StarForge's own `CARGO_PKG_VERSION` | Hard error — load aborted |

The compatibility rule for `core_version` follows semantic versioning:

- `0.x.y` plugins are **only** compatible with a `0.x.y` StarForge core (major `0`).
- `1.x.y` plugins are **only** compatible with a `1.x.y` StarForge core (major `1`).
- Minor and patch bumps within the same major are considered backwards-compatible.

### Error messages

When a plugin fails the version check you will see a clear message, for example:

```
Error: Plugin version incompatibility in 'libmy_plugin.so':
  Plugin was built for StarForge 0.1.0
  Running StarForge 1.0.0

  The major version must match. Rebuild the plugin against
  StarForge 1.0.0 or install a compatible StarForge version.
  See DEVELOPER_GUIDE.md § "Plugin Version Compatibility" for details.
```

### Writing a compatible plugin

1. **Pin the StarForge version** in your plugin's `Cargo.toml`:

   ```toml
   [dependencies]
   # Use the same major version as the StarForge binary your users will run.
   starforge = "0.1"
   ```

2. **Use the `export_plugin!` macro** — it embeds both `rustc_version` and
   `core_version` automatically at compile time:

   ```rust
   use starforge::export_plugin;

   export_plugin!(register);

   fn register(registrar: &mut dyn starforge::plugins::PluginRegistrar) {
       registrar.register_plugin(Box::new(MyPlugin));
   }
   ```

3. **Rebuild when StarForge's major version changes.**  Check the running version
   with `starforge --version` and compare it to the version your plugin was built
   against (shown in `starforge plugin load` output under "Built for StarForge").

4. **Use the same Rust toolchain** as the StarForge binary.  The easiest way is
   to keep a `rust-toolchain.toml` in your plugin repo that mirrors the one in
   the StarForge repo.

### Checking compatibility without loading

```bash
# See which StarForge version is running
starforge --version

# See which version each installed plugin was built for
starforge plugin load
```

The `load` command prints a "Built for StarForge" line for every successfully
loaded plugin, and a descriptive error for any that fail the check.

---

## Getting Started

### Prerequisites

- **Rust**: 1.80 or higher ([install via rustup](https://rustup.rs))
- **Git**: For version control
- **Stellar CLI**: For contract operations (optional)
- **Docker**: For containerized development (optional)

### Clone and Build

```bash
# Clone repository
git clone https://github.com/YOUR_USERNAME/starforge.git
cd starforge

# Build in debug mode
cargo build

# Build in release mode
cargo build --release

# Run tests
cargo test

# Run with logging
RUST_LOG=debug cargo run -- wallet list
```

---

## Development Setup

### IDE Configuration

#### VS Code

Recommended extensions:

- `rust-analyzer` - Rust language support
- `crates` - Cargo.toml dependency management
- `Better TOML` - TOML syntax highlighting
- `Error Lens` - Inline error display

`.vscode/settings.json`:

```json
{
  "rust-analyzer.checkOnSave.command": "clippy",
  "rust-analyzer.cargo.features": "all",
  "editor.formatOnSave": true
}
```

#### IntelliJ IDEA / CLion

Install the Rust plugin and configure:

- Enable Clippy for code analysis
- Set rustfmt as formatter
- Enable external linter

### Environment Variables

```bash
# Enable debug logging
export RUST_LOG=debug

# Disable telemetry during development
export STARFORGE_TELEMETRY=false

# Use custom config directory
export STARFORGE_CONFIG_DIR=~/.starforge-dev
```

### Development Workflow

```bash
# 1. Create feature branch
git checkout -b feature/my-feature

# 2. Make changes
# ... edit files ...

# 3. Run tests
cargo test

# 4. Check formatting
cargo fmt --check

# 5. Run clippy
cargo clippy -- -D warnings

# 6. Build
cargo build

# 7. Test manually
cargo run -- <command>

# 8. Run smoke tests (optional but recommended)
./scripts/e2e-smoke.sh

# 9. Commit
git add .
git commit -m "feat: add my feature"

# 10. Push and create PR
git push origin feature/my-feature
```

---

## Project Structure

### Source Code Organization

```
src/
├── main.rs              # Entry point
├── commands/            # User-facing commands
│   ├── mod.rs          # Module exports
│   ├── wallet.rs       # Wallet operations
│   ├── template.rs     # Template marketplace
│   └── ...
├── utils/               # Shared utilities
│   ├── mod.rs          # Module exports
│   ├── config.rs       # Configuration
│   ├── templates.rs    # Template system
│   └── ...
└── plugins/             # Plugin system
    ├── mod.rs          # Module exports
    ├── interface.rs    # Plugin traits
    └── ...
```

### File Naming Conventions

- **Commands**: `<noun>.rs` (e.g., `wallet.rs`, `network.rs`)
- **Utilities**: `<function>.rs` (e.g., `config.rs`, `crypto.rs`)
- **Tests**: `<module>_test.rs` or inline `#[cfg(test)] mod tests`

### Module Organization

Each module should follow this structure:

```rust
// 1. Imports
use crate::utils::config;
use anyhow::Result;

// 2. Type definitions
pub struct MyStruct { /* ... */ }
pub enum MyEnum { /* ... */ }

// 3. Public API
pub fn public_function() -> Result<()> { /* ... */ }

// 4. Private helpers
fn private_helper() { /* ... */ }

// 5. Tests
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_something() { /* ... */ }
}
```

---

## Code Style Guide

### Rust Style

Follow the [Rust Style Guide](https://doc.rust-lang.org/1.0.0/style/):

```rust
// ✅ Good
pub fn create_wallet(name: String, encrypt: bool) -> Result<()> {
    validate_name(&name)?;
    let keypair = generate_keypair();
    save_wallet(name, keypair, encrypt)
}

// ❌ Bad
pub fn CreateWallet(Name: String, Encrypt: bool) -> Result<()> {
    ValidateName(&Name)?;
    let KeyPair = GenerateKeypair();
    SaveWallet(Name, KeyPair, Encrypt)
}
```

### Naming Conventions

| Type      | Convention             | Example           |
| --------- | ---------------------- | ----------------- |
| Functions | `snake_case`           | `fetch_account()` |
| Types     | `PascalCase`           | `WalletEntry`     |
| Constants | `SCREAMING_SNAKE_CASE` | `MAX_RETRIES`     |
| Modules   | `snake_case`           | `hardware_wallet` |
| Lifetimes | `'lowercase`           | `'a`, `'static`   |

### Error Handling

```rust
// ✅ Use Result and ? operator
pub fn operation() -> Result<()> {
    let data = load_data()?;
    process_data(data)?;
    Ok(())
}

// ✅ Add context to errors
fs::read_to_string(&path)
    .with_context(|| format!("Failed to read {}", path.display()))?

// ❌ Don't use unwrap() in production code
let data = load_data().unwrap(); // Bad!

// ✅ Use expect() with clear message for programmer errors
let data = load_data()
    .expect("Config should be initialized in main()");
```

### Documentation

````rust
/// Fetches account information from Horizon API.
///
/// # Arguments
///
/// * `public_key` - The Stellar public key (G...)
/// * `network` - Network name ("testnet" or "mainnet")
///
/// # Returns
///
/// Returns `AccountResponse` with balance and sequence information.
///
/// # Errors
///
/// Returns error if:
/// - Account doesn't exist on the network
/// - Network is unreachable
/// - Response parsing fails
///
/// # Example
///
/// ```
/// let account = fetch_account("GABC...", "testnet")?;
/// println!("Balance: {}", account.balances[0].balance);
/// ```
pub fn fetch_account(public_key: &str, network: &str) -> Result<AccountResponse> {
    // Implementation
}
````

### Comments

```rust
// ✅ Explain WHY, not WHAT
// Use shallow clone to reduce bandwidth and disk usage
git_clone(&url, "--depth", "1");

// ❌ Don't state the obvious
// Clone the repository
git_clone(&url);

// ✅ TODO comments with context
// TODO(username): Add retry logic after implementing exponential backoff

// ❌ Vague TODOs
// TODO: fix this
```

---

## Adding New Features

### 1. Adding a New Command

**Step 1**: Create command file

```bash
touch src/commands/mycommand.rs
```

**Step 2**: Define command structure

```rust
// src/commands/mycommand.rs
use anyhow::Result;
use clap::Subcommand;
use crate::utils::print as p;

#[derive(Subcommand)]
pub enum MyCommands {
    /// Do something useful
    Action {
        /// Input parameter
        #[arg(long)]
        input: String,
    },
}

pub fn handle(cmd: MyCommands) -> Result<()> {
    match cmd {
        MyCommands::Action { input } => action(input),
    }
}

fn action(input: String) -> Result<()> {
    p::header("My Command");
    p::kv("Input", &input);

    // Your logic here

    p::success("Done!");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action() {
        // Test your command
    }
}
```

**Step 3**: Register in mod.rs

```rust
// src/commands/mod.rs
pub mod mycommand;
```

**Step 4**: Add to main CLI

```rust
// src/main.rs
#[derive(Subcommand)]
enum Commands {
    // ... existing commands

    /// My new command
    #[command(subcommand)]
    MyCommand(commands::mycommand::MyCommands),
}

// In main():
let result = match cli.command {
    // ... existing matches
    Commands::MyCommand(cmd) => commands::mycommand::handle(cmd),
};
```

**Step 5**: Update documentation

```bash
# Update README.md with new command
# Add examples to examples/ directory
# Update ARCHITECTURE.md if needed
```

### 2. Adding a New Utility Module

**Step 1**: Create utility file

```bash
touch src/utils/myutil.rs
```

**Step 2**: Implement functionality

```rust
// src/utils/myutil.rs
use anyhow::Result;

/// Does something useful
pub fn do_something(input: &str) -> Result<String> {
    // Implementation
    Ok(format!("Processed: {}", input))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_do_something() {
        let result = do_something("test").unwrap();
        assert_eq!(result, "Processed: test");
    }
}
```

**Step 3**: Register in mod.rs

```rust
// src/utils/mod.rs
pub mod myutil;
```

**Step 4**: Use in commands

```rust
use crate::utils::myutil;

fn my_command() -> Result<()> {
    let result = myutil::do_something("input")?;
    println!("{}", result);
    Ok(())
}
```

### 3. Adding Template Support

**Step 1**: Create template directory

```bash
mkdir -p templates/examples/my-template/src
```

**Step 2**: Add template files

```toml
# templates/examples/my-template/Cargo.toml
[package]
name = "{{PROJECT_NAME}}"
version = "0.1.0"
edition = "2021"

[dependencies]
soroban-sdk = "21.0.0"
```

```rust
// templates/examples/my-template/src/lib.rs
#![no_std]
use soroban_sdk::{contract, contractimpl, Env};

#[contract]
pub struct {{PROJECT_NAME_PASCAL}};

#[contractimpl]
impl {{PROJECT_NAME_PASCAL}} {
    pub fn hello(env: Env) -> String {
        String::from_str(&env, "Hello from {{PROJECT_NAME}}")
    }
}
```

**Step 3**: Add to registry

```json
// templates/registry.json
{
  "templates": [
    {
      "name": "my-template",
      "version": "1.0.0",
      "description": "My awesome template",
      "author": "Your Name",
      "tags": ["example"],
      "source": {
        "type": "local",
        "path": "templates/examples/my-template"
      },
      "created_at": "2025-01-01T00:00:00Z",
      "updated_at": "2025-01-01T00:00:00Z",
      "downloads": 0,
      "verified": false
    }
  ]
}
```

---

## Testing

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_functionality() {
        let result = my_function("input");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "expected");
    }

    #[test]
    fn test_error_case() {
        let result = my_function("");
        assert!(result.is_err());
    }

    #[test]
    #[should_panic(expected = "Invalid input")]
    fn test_panic() {
        panic_function();
    }
}
```

### Integration Tests

```rust
// tests/integration_test.rs
use starforge::utils::config;
use tempfile::TempDir;

#[test]
fn test_config_lifecycle() {
    let temp = TempDir::new().unwrap();
    let config_path = temp.path().join("config.toml");

    // Test save
    let config = config::Config::default();
    config::save(&config).unwrap();

    // Test load
    let loaded = config::load().unwrap();
    assert_eq!(loaded.version, config.version);
}
```

### CLI smoke tests

Fast regression checks for core commands live in `tests/cli_smoke.rs` and
`scripts/e2e-smoke.sh`. CI runs both after every build:

```bash
cargo test --test cli_smoke
./scripts/e2e-smoke.sh
STARFORGE_E2E=1 ./scripts/e2e-smoke.sh   # optional network checks
```

### Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture

# Run tests in specific module
cargo test config::tests

# Run integration tests only
cargo test --test integration_test

# Run with coverage (requires tarpaulin)
cargo tarpaulin --out Html
```

### End-to-End Smoke Tests

StarForge includes an end-to-end smoke test script that verifies basic functionality across all major commands.

**Location**: `scripts/e2e-smoke.sh`

**Running Smoke Tests**:

```bash
# Build the project first
cargo build --release

# Run smoke tests (without network tests)
./scripts/e2e-smoke.sh

# Run smoke tests with network tests (requires internet)
STARFORGE_E2E=1 ./scripts/e2e-smoke.sh
```

**What the smoke test covers**:

1. **Basic Commands**
   - `starforge info` - System information
   - `starforge --version` - Version display
   - `starforge --help` - Help text

2. **Wallet Operations**
   - `wallet create` - Create test wallet
   - `wallet list` - List wallets
   - `wallet show` - Display wallet details

3. **Network Operations**
   - `network show` - Display network configuration
   - `network test` - Test network connectivity (requires `STARFORGE_E2E=1`)
   - `wallet fund` - Fund testnet wallet (requires `STARFORGE_E2E=1`)

4. **Template Operations**
   - `template list` - List available templates
   - `template search` - Search templates

5. **Other Commands**
   - `completions` - Generate shell completions

**Network Test Gating**:

Network tests are gated behind the `STARFORGE_E2E=1` environment variable because they:
- Require internet connectivity
- Depend on external services (Stellar testnet, Friendbot)
- May be slow or flaky in CI environments
- Can hit rate limits

To skip network tests in CI:

```yaml
# .github/workflows/ci.yml
- name: Run smoke tests
  run: ./scripts/e2e-smoke.sh  # Skips network tests by default
```

To run full tests locally:

```bash
STARFORGE_E2E=1 ./scripts/e2e-smoke.sh
```

**Exit Codes**:
- `0` - All tests passed
- `1` - One or more tests failed

**Cleanup**:

The smoke test automatically cleans up test wallets on exit. If cleanup fails, you may need to manually remove test wallets:

```bash
# List wallets to find test wallets
starforge wallet list

# Remove test wallet (when delete command is implemented)
# starforge wallet delete smoke-test-<timestamp>
```

### Test Organization

```
tests/
├── integration_test.rs      # Integration tests
├── template_test.rs          # Template-specific tests
└── common/
    └── mod.rs               # Shared test utilities
```

---

## Documentation

### Code Documentation

```rust
/// Module-level documentation
///
/// This module handles wallet operations including creation,
/// listing, and management of Stellar keypairs.

/// Function documentation
///
/// Creates a new wallet with the given name.
///
/// # Arguments
///
/// * `name` - Wallet identifier
/// * `encrypt` - Whether to encrypt the secret key
///
/// # Returns
///
/// Returns `Ok(())` on success, or an error if:
/// - Wallet name already exists
/// - Keypair generation fails
/// - Config save fails
pub fn create_wallet(name: String, encrypt: bool) -> Result<()> {
    // Implementation
}
```

### User Documentation

Update these files when adding features:

1. **README.md** - Main documentation, usage examples
2. **ARCHITECTURE.md** - Architecture and design decisions
3. **DEVELOPER_GUIDE.md** - This file
4. **Feature-specific docs** - Detailed feature documentation

### Documentation Standards

- Use clear, concise language
- Include code examples
- Explain WHY, not just WHAT
- Keep examples up-to-date
- Add diagrams for complex flows
- Update [docs/COMMAND_REFERENCE.md](docs/COMMAND_REFERENCE.md) when adding or renaming CLI subcommands

---

## Common Tasks

### Adding a Dependency

```bash
# Add to Cargo.toml
cargo add <crate-name>

# Add with specific version
cargo add <crate-name>@1.0.0

# Add with features
cargo add <crate-name> --features feature1,feature2

# Add as dev dependency
cargo add --dev <crate-name>
```

### Updating Dependencies

```bash
# Update all dependencies
cargo update

# Update specific dependency
cargo update <crate-name>

# Check for outdated dependencies
cargo outdated
```

### Running Clippy

```bash
# Run clippy
cargo clippy

# Deny all warnings
cargo clippy -- -D warnings

# Fix automatically (when possible)
cargo clippy --fix
```

### Formatting Code

```bash
# Format all code
cargo fmt

# Check formatting without changing
cargo fmt --check

# Format specific file
rustfmt src/main.rs
```

### Regenerating Shell Completions

Shell completion scripts are generated by `build.rs` into the `completions/` directory.

```bash
# Regenerate completions (bash/zsh/fish)
cargo build
```

### Building Documentation

```bash
# Build docs
cargo doc

# Build and open in browser
cargo doc --open

# Include private items
cargo doc --document-private-items
```

### Benchmarking

```bash
# Run benchmarks
cargo bench

# Run specific benchmark
cargo bench benchmark_name

# Save baseline
cargo bench -- --save-baseline my-baseline

# Compare to baseline
cargo bench -- --baseline my-baseline
```

### Running with Docker Soroban Sandbox

The `shell` command supports connecting to a local Soroban sandbox via Docker:

```bash
# Start the interactive shell against a local Docker Soroban sandbox
starforge shell --contract ./target/wasm32-unknown-unknown/release/my_contract.wasm --network docker-testnet
```

When `--network docker-testnet` is used, StarForge:
1. Ensures the Docker containers defined in `docker-compose.yml` are running (includes `stellar-testnet` and `soroban-rpc`)
2. Runs contract invocations inside the Docker network where the Soroban RPC is available at `http://soroban-rpc:8000`
3. Routes all RPC calls through the local sandbox instead of Stellar testnet

The `docker-compose.yml` at the project root defines:
- **stellar-testnet**: A full Stellar + Soroban RPC node on `localhost:8000`
- **soroban-rpc**: Dedicated Soroban RPC endpoint on `localhost:8001`

Prerequisites:
- Docker and docker-compose installed
- Docker daemon running

---

## Debugging

### Debug Logging

```rust
// Add to Cargo.toml
[dependencies]
log = "0.4"
env_logger = "0.10"

// In main.rs
env_logger::init();

// In code
use log::{debug, info, warn, error};

debug!("Debug message: {:?}", value);
info!("Info message");
warn!("Warning message");
error!("Error message");
```

### Running with Debug Output

```bash
# Enable all debug logs
RUST_LOG=debug cargo run -- wallet list

# Enable specific module
RUST_LOG=starforge::commands::wallet=debug cargo run -- wallet list

# Multiple modules
RUST_LOG=starforge::commands=debug,starforge::utils=info cargo run
```

### Using rust-gdb

```bash
# Build with debug symbols
cargo build

# Run with gdb
rust-gdb target/debug/starforge

# Set breakpoint
(gdb) break src/main.rs:42

# Run
(gdb) run wallet list

# Step through
(gdb) step
(gdb) next

# Print variable
(gdb) print variable_name
```

### Common Issues

**Issue**: Compilation errors after updating dependencies

```bash
# Solution: Clean and rebuild
cargo clean
cargo build
```

**Issue**: Tests failing intermittently

```bash
# Solution: Run tests serially
cargo test -- --test-threads=1
```

**Issue**: Slow compilation

```bash
# Solution: Use sccache
cargo install sccache
export RUSTC_WRAPPER=sccache
```

---

## Release Process

### Version Bumping

1. Update version in `Cargo.toml`
2. Update version in `src/main.rs` (if hardcoded)
3. Update CHANGELOG.md
4. Commit: `git commit -m "chore: bump version to X.Y.Z"`

### Creating a Release

```bash
# 1. Tag the release
git tag -a v0.2.0 -m "Release v0.2.0"

# 2. Push tag
git push origin v0.2.0

# 3. Build release binaries
cargo build --release

# 4. Create GitHub release
# - Go to GitHub releases
# - Create new release from tag
# - Upload binaries
# - Add release notes
```

### Release Checklist

- [ ] All tests passing
- [ ] Documentation updated
- [ ] CHANGELOG.md updated
- [ ] Version bumped
- [ ] Release notes prepared
- [ ] Binaries built for all platforms
- [ ] GitHub release created
- [ ] Announcement posted

---

## Best Practices

### 1. Error Handling

```rust
// ✅ Use Result for fallible operations
pub fn operation() -> Result<()> {
    let data = load_data()?;
    process(data)?;
    Ok(())
}

// ✅ Add context to errors
load_data()
    .with_context(|| "Failed to load configuration")?

// ✅ Create custom error types for complex cases
#[derive(Debug, thiserror::Error)]
pub enum MyError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Network error")]
    Network(#[from] ureq::Error),
}
```

### 2. Configuration Management

```rust
// ✅ Load config once, pass as reference
let config = config::load()?;
process_with_config(&config)?;

// ❌ Don't reload config repeatedly
fn process() {
    let config = config::load().unwrap(); // Bad!
    // ...
}
```

### 3. User Feedback

```rust
// ✅ Provide progress indicators
p::step(1, 3, "Loading configuration...");
p::step(2, 3, "Processing data...");
p::step(3, 3, "Saving results...");

// ✅ Show helpful error messages
anyhow::bail!(
    "Wallet '{}' not found.\n\nTry: starforge wallet list",
    name
);
```

### 4. Testing

```rust
// ✅ Test edge cases
#[test]
fn test_empty_input() { /* ... */ }

#[test]
fn test_invalid_input() { /* ... */ }

#[test]
fn test_boundary_conditions() { /* ... */ }

// ✅ Use descriptive test names
#[test]
fn creates_wallet_with_encrypted_key_when_encrypt_flag_is_true() {
    // ...
}
```

---

## Contributing Guidelines

### Pull Request Process

1. **Fork** the repository
2. **Create** a feature branch
3. **Make** your changes
4. **Test** thoroughly
5. **Document** your changes
6. **Submit** a pull request

### PR Checklist

- [ ] Code follows style guide
- [ ] Tests added/updated
- [ ] Documentation updated
- [ ] Commit messages follow convention
- [ ] No merge conflicts
- [ ] CI passes

### Commit Message Convention

```
<type>(<scope>): <subject>

<body>

<footer>
```

Types:

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation
- `style`: Formatting
- `refactor`: Code restructuring
- `test`: Adding tests
- `chore`: Maintenance

Examples:

```
feat(wallet): add hardware wallet support

Implements Ledger and Trezor integration for secure key storage.

Closes #123
```

---

## Resources

### Documentation

- [Rust Book](https://doc.rust-lang.org/book/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Stellar Documentation](https://developers.stellar.org/)
- [Soroban Documentation](https://soroban.stellar.org/)

### Tools

- [rust-analyzer](https://rust-analyzer.github.io/) - IDE support
- [clippy](https://github.com/rust-lang/rust-clippy) - Linter
- [rustfmt](https://github.com/rust-lang/rustfmt) - Formatter
- [cargo-edit](https://github.com/killercup/cargo-edit) - Dependency management

### Community

- [Stellar Discord](https://discord.gg/stellar)
- [Rust Users Forum](https://users.rust-lang.org/)
- [GitHub Discussions](https://github.com/YOUR_USERNAME/starforge/discussions)

---

## Getting Help

- **Issues**: [GitHub Issues](https://github.com/YOUR_USERNAME/starforge/issues)
- **Discussions**: [GitHub Discussions](https://github.com/YOUR_USERNAME/starforge/discussions)
- **Discord**: Join the Stellar Discord
- **Email**: maintainer@example.com

---

**Happy Coding! 🚀**
