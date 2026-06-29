# Template Contribution Guidelines

Thank you for contributing a smart contract template to the StarForge library!
This document covers every step from writing your template to getting it accepted.

---

## Quick-start checklist

Before opening a pull request, confirm each item:

- [ ] Template compiles with `cargo build` targeting `wasm32-unknown-unknown`
- [ ] Template passes its own test suite (`cargo test`)
- [ ] Template source uses `{{PROJECT_NAME_PASCAL}}` as the contract struct name
- [ ] A `README.md` is included describing the contract and its public functions
- [ ] `registry.json` entry is present with all required fields
- [ ] `security_review` field is present (status `"pending"` is acceptable for new submissions)
- [ ] `changelog` field has at least one entry for the initial version
- [ ] License is declared via the `license` field (MIT or Apache-2.0 preferred)
- [ ] `TEMPLATE_CONTRIBUTING.md` checklist items have all been addressed

---

## Template structure

Every template lives under `templates/examples/<template-name>/` and follows this layout:

```
templates/examples/<template-name>/
├── Cargo.toml          # crate manifest — uses {{project_name_snake}}
├── README.md           # user-facing documentation
└── src/
    └── lib.rs          # contract source
```

### Cargo.toml requirements

```toml
[package]
name = "{{project_name_snake}}"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
soroban-sdk = { version = "22.0.0", features = ["alloc"] }

[dev-dependencies]
soroban-sdk = { version = "22.0.0", features = ["testutils"] }
```

Use `{{project_name_snake}}` as the crate name — StarForge replaces this with the
user's project name when scaffolding.

### Contract source requirements

- Must start with `#![no_std]`
- Must use `{{PROJECT_NAME_PASCAL}}` as the contract struct name (double-brace placeholder)
- Must include a module-level doc comment (`//! …`) explaining the contract
- Must include a `#[cfg(test)] mod test { … }` block with at least two meaningful tests
- Must compile without warnings

```rust
#![no_std]
//! Brief description of the contract.
use soroban_sdk::{contract, contractimpl, Env};

#[contract]
pub struct {{PROJECT_NAME_PASCAL}};

#[contractimpl]
impl {{PROJECT_NAME_PASCAL}} {
    // ...
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_happy_path() { /* ... */ }

    #[test]
    #[should_panic(expected = "…")]
    fn test_error_case() { /* ... */ }
}
```

---

## Registry entry

Every template must have a corresponding entry in `templates/registry.json`.
Below is the minimum required shape:

```json
{
  "name": "my-template",
  "version": "1.0.0",
  "description": "One-line description of what the contract does",
  "author": "Your Name",
  "tags": ["defi", "my-category"],
  "source": { "type": "builtin", "id": "my-template" },
  "created_at": "2025-01-01T00:00:00Z",
  "updated_at": "2025-01-01T00:00:00Z",
  "verified": false,
  "documented": true,
  "maintenance": "active",
  "license": "MIT",
  "security_review": {
    "status": "pending",
    "audited_at": null,
    "auditor": null,
    "findings": null,
    "score": null
  },
  "changelog": [
    { "version": "1.0.0", "date": "2025-01-01", "notes": "Initial release" }
  ]
}
```

### Field reference

| Field | Required | Description |
|---|---|---|
| `name` | ✓ | Unique kebab-case identifier |
| `version` | ✓ | Semver string (`major.minor.patch`) |
| `description` | ✓ | One-line summary (≤ 120 chars) |
| `author` | ✓ | Author name or GitHub handle |
| `tags` | ✓ | At least one tag from the [tag taxonomy](#tag-taxonomy) |
| `source` | ✓ | `builtin`, `git`, or `local` source descriptor |
| `license` | recommended | SPDX identifier (`MIT`, `Apache-2.0`, …) |
| `security_review` | recommended | Audit status — `pending` is fine initially |
| `changelog` | recommended | At least one entry |
| `maintenance` | recommended | `active`, `maintained`, `deprecated`, or `unknown` |

### Tag taxonomy

Use at least one of these standard tags so search works reliably:

| Tag | Used for |
|---|---|
| `token` | Fungible token contracts |
| `nft` | Non-fungible token contracts |
| `defi` | DeFi primitives (AMM, lending, staking, …) |
| `dao` | Governance / DAO contracts |
| `governance` | Voting and proposal contracts |
| `multisig` | Multi-signature wallets and vaults |
| `security` | Auth, access-control, and audit-focused contracts |
| `payments` | Payment channels, escrow, and invoicing |
| `staking` | Yield / staking contracts |
| `standard` | SEP-conformant contracts |

---

## Security review process

The StarForge Security Team reviews every new template before setting
`verified: true`. Until review is complete, the template is published with
`"status": "pending"`.

### What gets reviewed

1. **Authorization checks** — every mutating function calls `require_auth()` on
   the right principal; no function can be called by an arbitrary address.
2. **Re-entrancy** — state is updated before external token transfers.
3. **Integer arithmetic** — no unchecked arithmetic that could overflow or wrap.
4. **Initialization guards** — contracts cannot be re-initialized.
5. **Storage hygiene** — correct use of `instance`, `persistent`, and `temporary`
   storage lifetimes.
6. **Panic messages** — descriptive error strings, no empty panics.

### Review SLA

| Priority | Target turnaround |
|---|---|
| Security fix | 48 hours |
| New template | 7 days |
| Version bump | 5 days |

### Requesting an expedited review

Open a GitHub issue with the label `security-review-request` and link your PR.
The Security Team triages these daily.

---

## Version bumping

When updating an existing template:

1. Increment the `version` field in the `registry.json` entry following semver:
   - **Patch** (`x.y.Z`) — bug fixes, doc improvements, no API changes.
   - **Minor** (`x.Y.0`) — new optional functions, backward-compatible changes.
   - **Major** (`X.0.0`) — breaking changes to the public API.
2. Add a new entry at the **top** of the `changelog` array.
3. Update `updated_at` to the current date.
4. Reset `security_review.status` to `"pending"` if the change affects contract logic.

---

## Testing your template

Run the template's own tests from the StarForge CLI before submitting:

```bash
# Run tests via cargo directly
cargo test --manifest-path templates/examples/my-template/Cargo.toml

# Or via the CLI (once registered)
starforge template test my-template
```

All tests must pass with zero warnings.

---

## Generating documentation

After adding your registry entry you can preview the generated Markdown docs:

```bash
starforge template docs my-template
# write to a file
starforge template docs my-template --output docs/templates/my-template.md
```

---

## Pull request process

1. Fork the repo and create a branch: `git checkout -b feat/template-my-template`
2. Add your template files and registry entry.
3. Run `cargo test` from the repo root to verify nothing is broken.
4. Open a PR against `main` with the title `feat(templates): add my-template`.
5. Fill in the PR description template, including the checklist above.
6. The Security Team will review and either approve or request changes within 7 days.

We appreciate every contribution — thank you for making the Stellar ecosystem stronger!
