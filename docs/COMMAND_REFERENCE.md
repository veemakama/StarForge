# StarForge Command Reference

Browse every top-level command and its most important flags. For wallet, template, and transaction details see also [API_REFERENCE.md](../API_REFERENCE.md).

## Global options

| Flag | Description |
|------|-------------|
| `-q, --quiet` | Suppress banner and decorative output |
| `--log-format human\|json` | Structured log format (default: `human`) |
| `--log-dir <PATH>` | Optional rotating log directory |
| `-h, --help` | Command help |
| `-V, --version` | CLI version |

## Quick workflow examples

```bash
# Environment check
starforge info

# Wallet + network
starforge wallet create deployer --fund
starforge network show

# Templates + deploy
starforge template list
starforge deploy --wasm ./contract.wasm --wallet deployer --simulate

# Guided tutorial
starforge tutorial start hello-world
starforge tutorial next
```

---

## `wallet`

| Subcommand | Purpose |
|------------|---------|
| `create <NAME>` | Create and store a keypair (`--fund`, `--encrypt`, `--mnemonic`) |
| `list` | List saved wallets |
| `show <NAME>` | Show wallet metadata and balance (`--reveal`) |
| `fund <NAME>` | Fund via Friendbot when configured |
| `remove <NAME>` | Delete a saved wallet |
| `rename <OLD> <NEW>` | Rename a wallet entry |
| `merge` | Account merge (`--from`, `--to`, `--yes`) |
| `rotate <NAME>` | Rotate keys in place (`--fund`, `--encrypt`, `--mem`, `--iterations`) |
| `export <NAME> --output <FILE>` | Export backup JSON |
| `import` | Import from file or `--mnemonic` |
| `sign` | Sign a payload with a saved wallet |
| `multisig` | Multisig helpers (create, add-signer, submit) |

---

## `multisig`

| Subcommand | Purpose |
|------------|---------|
| `wizard` | Interactive transaction proposal builder |
| `create` | Create a proposal with threshold, signers, metadata, and optional transaction XDR |
| `status <FILE>` | Show visual signature collection progress |
| `verify <FILE>` | Validate signatures, duplicates, pending signers, and threshold readiness |
| `notify <FILE>` | Queue signature request notifications for pending signers |
| `export <FILE>` / `import <FILE>` | Share proposal JSON between signers |
| `templates` / `from-template` | Use common scenarios like escrow, company treasury, DAO, vault, and payment |

```bash
starforge multisig wizard
starforge multisig create --threshold 2 --signers alice,bob,carol \
  --title "Treasury payment" --transaction-xdr <XDR>
starforge multisig status proposal.json
starforge multisig verify proposal.json
starforge multisig notify proposal.json --message "Please sign the treasury payment"
```

---

## `new`

| Subcommand | Purpose |
|------------|---------|
| `contract <NAME>` | Scaffold Soroban contract (`--template`) |
| `dapp <NAME>` | Scaffold Stellar dApp frontend |

---

## `contract` / `inspect` / `deploy`

| Command | Purpose |
|---------|---------|
| `contract invoke` | Invoke contract function (`--simulate`) |
| `contract inspect` | Inspect deployed contract metadata |
| `contract generate-bindings <WASM_FILE>` | Generate Rust or TypeScript wrappers (`--lang rust\|ts`) |
| `inspect storage` | Deep storage inspection |
| `deploy --wasm <FILE>` | Prepare Soroban deployment |

**`deploy` flags:** `--network`, `--wallet`, `--optimize`, `--simulate`, `--yes`, `--execute`

```bash
starforge deploy --wasm target/wasm32v1-none/release/token.wasm \
  --wallet deployer --network testnet --simulate

starforge deploy --wasm ./token.wasm --optimize --yes --execute

starforge contract generate-bindings ./token.wasm --lang rust
```

---

## `test`

| Flag | Purpose |
|------|---------|
| `--wasm <FILE>` | Compiled Soroban WASM under test |
| `--fixture <FILE>` | JSON/TOML contract test suite with fixtures, mocks, and assertions |
| `--source <FILE>` | Contract source used for generated tests or coverage |
| `--coverage` | Include source coverage summary |
| `--report html\|json\|junit` | Write a test report (`junit` is available for fixture suites) |
| `--testnet` | Validate Soroban testnet integration for the run |
| `--testnet-dry-run` | Validate testnet configuration without probing RPC health |

```bash
starforge test --wasm ./target/contract.wasm \
  --fixture ./contract-tests.json --coverage --source ./src/lib.rs --report html

starforge test --wasm ./target/contract.wasm \
  --fixture ./contract-tests.toml --testnet --testnet-dry-run
```

Fixture suites support named storage fixtures, mocked contract calls, and assertions such as `state_equals`, `state_exists`, `return_equals`, `event_emitted`, `fee_at_most`, and `mock_called`.

---

## `network` / `node`

| Command | Purpose |
|---------|---------|
| `network show` | Show configured networks |
| `network switch <NAME>` | Set active network |
| `network add` | Add custom Horizon/RPC/Friendbot endpoints |
| `network test` | Connectivity probe |
| `node start` | Start local quickstart devnet (`--port`) |

---

## `tx`

| Subcommand | Purpose |
|------------|---------|
| `tx send` | Payment (`--from`, `--to`, `--amount`, `--asset`) |
| `tx batch` | Batch operations from JSON (`--file`, `--from`) |
| `tx history <PUBKEY>` | Recent transactions (`--limit`, `--cursor`, `--successful`) |

---

## `template`

| Subcommand | Purpose |
|------------|---------|
| `template list` | List marketplace templates |
| `template search <QUERY>` | Search templates |
| `template show <ID>` | Template details |
| `template init <ID> <DIR>` | Scaffold from template |
| `template publish` | Publish template metadata |
| `template remove <ID>` | Remove local template entry |

---

## `gas`

| Subcommand | Purpose |
|------------|---------|
| `gas analyze <WASM>` | Heuristic gas/cpu report (`--network`) |
| `gas optimize --target <IN> --output <OUT>` | Lightweight WASM shrink pass |
| `gas diff <OLD> <NEW>` | Compare estimated costs |

---

## `security`

| Subcommand | Purpose |
|------------|---------|
| `audit <PATH>` | Run built-in Soroban analysis plus optional Slither/Mythril integrations |
| `audit --format json\|html --out <FILE>` | Generate machine-readable or HTML audit reports |
| `audit --ci --min-score <N>` | Fail when the audit score is below the CI threshold |
| `audit --ci-workflow-out <FILE>` | Generate a GitHub Actions workflow for security audits |
| `audit --track` | Create remediation tracker items for findings |
| `remediation list` | Review tracked audit and pentest remediation items |

```bash
starforge security audit ./contracts/token/src/lib.rs --format html --out audit.html
starforge security audit ./contracts/token/src/lib.rs --ci --min-score 85
starforge security audit ./contracts/token/src/lib.rs \
  --ci-workflow-out .github/workflows/starforge-security.yml
```

External tools are optional. StarForge runs built-in Soroban heuristics every time and records whether Slither/Mythril were completed, failed, skipped, or unavailable.

---

## `upgrade`

| Subcommand | Purpose |
|------------|---------|
| `upgrade prepare` | Validate upgrade WASM (`--contract-id`, `--wasm`) |
| `upgrade auto compat` | Compare old/new WASM ABI and storage layout (`--old-wasm`, `--new-wasm`) |
| `upgrade auto plan` | Generate compatibility-aware upgrade plan and migration template |
| `upgrade propose` | Create governance proposal |
| `upgrade list` / `status` | List pending proposals |
| `upgrade approve` | Approve proposal |
| `upgrade execute` | Execute approved upgrade |
| `upgrade rollback` | Roll back contract version |
| `upgrade history` | Show upgrade history |

---

## `governance`

Contract upgrade governance with voting, timelock, audit trail, and emergency upgrades.

| Subcommand | Purpose |
|------------|---------|
| `governance propose` | Create upgrade proposal (`--contract-id`, `--wasm`, `--threshold`, `--timelock`) |
| `governance list` | List proposals with optional filters |
| `governance show` | Show proposal details and votes |
| `governance vote` | Cast vote (`--for` or `--against`) |
| `governance reject` | Reject a proposal |
| `governance execute` | Execute after timelock and threshold met |
| `governance emergency` | Emergency upgrade (bypasses timelock) |
| `governance audit` | Show governance audit trail |
| `governance dashboard` | Governance summary dashboard |
| `governance config show/set` | View or update governance defaults |

See [GOVERNANCE.md](GOVERNANCE.md) for the full workflow.

---

## `tutorial`

| Subcommand | Purpose |
|------------|---------|
| `tutorial list` | List tutorials under `./tutorials/` |
| `tutorial start <SLUG>` | Begin guided flow (resets progress) |
| `tutorial next` | Mark step complete and show next milestone |
| `tutorial status` | Show active tutorial and current step |

---

## Utility commands

| Command | Purpose |
|---------|---------|
| `info` | Version, config path, network health, Stellar CLI detection |
| `shell` | Interactive local REPL with persistent history and tab completion |
| `monitor` | Live event/threshold monitoring |
| `benchmark` | CLI performance benchmarks |
| `test` | Soroban WASM test runner |
| `lint <PATH>` | Static Soroban source lint |
| `plugin install/list/run` | Dynamic plugin management |
| `completions <SHELL>` | bash/zsh/fish completions |

---

## External plugins

```bash
starforge plugin install my-plugin --path ./libmy_plugin.so
starforge my-plugin <args>
```

## See also

- [API_REFERENCE.md](../API_REFERENCE.md) — detailed per-command examples and output samples
- [DEVELOPER_GUIDE.md](../DEVELOPER_GUIDE.md) — contributing and local development
