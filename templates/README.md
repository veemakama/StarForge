# StarForge Template Marketplace

This directory contains the template registry for the StarForge CLI marketplace feature.

## Overview

The template marketplace allows developers to discover, share, and use community-contributed Soroban smart contract templates.

## Registry Structure

The `registry.json` file contains metadata for all available templates:

```json
{
  "version": "1",
  "templates": [
    {
      "name": "template-name",
      "version": "1.0.0",
      "description": "Template description",
      "author": "Author Name",
      "tags": ["tag1", "tag2"],
      "source": {
        "type": "git",
        "url": "https://github.com/user/repo",
        "branch": "main"
      },
      "created_at": "2025-01-01T00:00:00Z",
      "updated_at": "2025-01-01T00:00:00Z",
      "downloads": 0,
      "verified": false
    }
  ]
}
```

## Template Sources

Templates can come from three sources:

1. **Git Repository**: Clone from a Git URL
2. **Local Path**: Copy from a local directory
3. **Built-in**: Pre-packaged templates in StarForge

## Using Templates

### Search for templates
```bash
starforge template search defi
starforge template search --tags defi,dex
```

### List all templates
```bash
starforge template list
```

### View template details
```bash
starforge template show uniswap-v2
```

### Use a template
```bash
starforge new contract my-dex --template uniswap-v2 --from marketplace
```

### Publish your own template
```bash
starforge template publish ./my-template \
  --name my-awesome-template \
  --description "An awesome Soroban contract" \
  --author "Your Name" \
  --tags "defi,custom" \
  --version "1.0.0"
```

## Template Requirements

To be valid, a template must contain:

- `Cargo.toml` - Rust package manifest
- `src/` directory - Source code
- `src/lib.rs` - Main contract file

## Example Templates

Built-in example templates are provided under `templates/examples/`:

- `simple-counter`: A basic smart contract demonstrating storage usage by incrementing, getting, and resetting a counter.
- `token-allowlist`: A smart contract for managing an allowlist of approved addresses, controlled by an administrator.
- `escrow`: A DeFi token escrow with buyer, seller, and arbiter roles for marketplaces, freelance payments, and OTC trades.
- `dao-governance`: A minimal DAO governance contract with member proposals and one-member-one-vote tallying.
- `multisig-vault`: A threshold (M-of-N) multi-signature vault for shared-custody token transfers and treasuries.

## Template Placeholders

Templates can use placeholders that will be replaced during scaffolding:

- `{{PROJECT_NAME}}` - Project name (e.g., "my-project")
- `{{PROJECT_NAME_SNAKE}}` - Snake case (e.g., "my_project")
- `{{PROJECT_NAME_PASCAL}}` - Pascal case (e.g., "MyProject")

## Contributing Templates

To contribute a template to the official registry:

1. Create your template following the structure requirements
2. Test it locally with `starforge template publish`
3. Submit a PR to add it to `templates/registry.json`
4. Include documentation and examples

## Verified Templates

Templates marked as `verified: true` have been reviewed by the StarForge maintainers for:

- Code quality and security
- Proper documentation
- Working examples
- Best practices compliance
