# confluence-cli

Rust rewrite of the Confluence CLI.

## Goals

- Match the practical feature surface of the existing JavaScript `confluence-cli`
- Redesign the product instead of porting technical debt
- Make `patch` a first-class workflow
- Prefer `archive` over destructive deletion
- Invest in Markdown round-trip quality through a canonical internal document model
- Treat inline comments and move semantics as explicit design areas, not incidental side effects

## Design direction

- Rust 2024 edition
- Rust 1.94 minimum
- Version starts at `0.1.0`
- Breaking changes are allowed until `1.0.0`
- REST v2 first, with selective v1 fallback where Cloud APIs still require it

## Current command surface

```text
confluence config init
confluence profile list|use|add|remove
confluence page read|info|find|search|children|create|create-child|update|patch|move|archive|delete|export
confluence attachment list|download|upload|delete
confluence property list|get|set|delete
confluence comment list|create|reply|delete
confluence convert
```

## Current implementation status

The project is no longer a skeleton. It currently includes:

- CLI command tree with human and JSON output modes
- config persistence plus profile CRUD
- environment override support
- read-only profile enforcement for mutating commands
- page read/info/search/create/create-child/update/patch/move/archive/delete/export workflows
- attachment list/download/upload/delete workflows
- property list/get/set/delete workflows
- comment list/create/reply/delete workflows
- local Markdown/storage/text conversion
- managed export bundle metadata
- local unified-diff patch application

## Current constraints

The rewrite is usable, but some areas are intentionally conservative:

- `page patch` is a local diff workflow followed by a guarded full-page update; the base file must exactly match the current remote storage body
- `page move --before/--after` is guarded and refuses top-level targets
- `page move --before/--after --title` is not implemented yet
- inline comments require explicit inline properties; the CLI does not try to infer editor selection metadata
- raw CQL search is supported, but advanced transport coverage is still improving
- secrets are currently stored in the local config file; secure credential storage is still future work

## Testing and verification

The project currently verifies with:

- `cargo fmt --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test`
- `cargo build`

Coverage includes:

- CLI parsing tests
- config/profile persistence tests
- domain parsing tests
- app-layer workflow validation tests
- conversion and patch tests
- API helper tests
- HTTP contract tests for pagination, comment reply payloads, and move guard behavior

## Next high-value work

- expand HTTP fixture coverage for create, export, attachments, and properties
- paginate raw CQL searches across larger result sets
- deepen move and comment contract coverage
- improve README / examples / migration guidance
- add stronger secret handling for persisted profiles
