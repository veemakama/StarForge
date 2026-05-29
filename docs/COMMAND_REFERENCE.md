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
| `inspect storage` | Deep storage inspection |
| `deploy --wasm <FILE>` | Prepare Soroban deployment |

**`deploy` flags:** `--network`, `--wallet`, `--optimize`, `--simulate`, `--yes`, `--execute`

```bash
starforge deploy --wasm target/wasm32v1-none/release/token.wasm \
  --wallet deployer --network testnet --simulate

starforge deploy --wasm ./token.wasm --optimize --yes --execute
```

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

## `upgrade`

| Subcommand | Purpose |
|------------|---------|
| `upgrade prepare` | Validate upgrade WASM (`--contract-id`, `--wasm`) |
| `upgrade propose` | Create governance proposal |
| `upgrade list` / `status` | List pending proposals |
| `upgrade approve` | Approve proposal |
| `upgrade execute` | Execute approved upgrade |
| `upgrade rollback` | Roll back contract version |
| `upgrade history` | Show upgrade history |

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
| `shell` | Interactive local REPL |
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
