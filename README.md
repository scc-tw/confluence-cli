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

## Configuration

Default config path on Windows:

```text
%USERPROFILE%\.config\confluence-cli\config.json
```

If `USERPROFILE` is unavailable, the CLI falls back to `./config.json`.

### Environment variables

Supported runtime overrides:

- `CONFLUENCE_PROFILE`
- `CONFLUENCE_DOMAIN`
- `CONFLUENCE_PROTOCOL`
- `CONFLUENCE_API_PATH`
- `CONFLUENCE_AUTH_TYPE`
- `CONFLUENCE_EMAIL`
- `CONFLUENCE_USERNAME`
- `CONFLUENCE_API_TOKEN`
- `CONFLUENCE_PASSWORD`
- `CONFLUENCE_READ_ONLY`

### Precedence

Resolution order is:

1. explicit CLI flags such as `--config-path` and `--profile`
2. environment variables
3. selected profile from `config.json`
4. active profile from `config.json`
5. built-in defaults

### Auth expectations

- `basic` auth requires identity plus secret:
  - `email` or `username`
  - `api_token` or `password`
- `bearer` auth requires `api_token`
- `mtls` is not implemented yet

## Export behavior

`page export` currently writes a managed bundle:

- content file in the requested format, such as `page.md`
- `.confluence/page.json` metadata
- optional `attachments/` directory when attachments are included

Supported export formats today:

- `markdown`
- `storage`
- `html`
- `text`

Use `--skip-attachments` to avoid attachment download requests.

## Examples

```text
confluence config init --name work --domain your-domain.atlassian.net --auth-type bearer --api-token <token>
confluence profile add staging --domain staging.atlassian.net --auth-type bearer --api-token <token>
confluence page create --title "Design Doc" --body-file .\page.md --space-key ENG
confluence page export 123 --dest .\exports\design --format markdown
confluence attachment upload 123 --file .\spec.md --comment "refresh"
confluence property set 123 "release notes" --value-file .\release-notes.json
```

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

Current HTTP contract coverage includes:

- raw CQL pagination
- page move guards and parent move transport
- comment reply payloads and delete/list routes
- attachment list/download/upload/delete routes
- property list/get/set/delete routes
- export transport flow
- CLI smoke coverage for `--config-path`, `--profile`, and read-only mutation blocking

Current testing limits:

- attachment/property/export coverage is transport-focused, not full live-cloud integration
- CLI integration coverage is smoke-level, not exhaustive output snapshot coverage
- inline comment metadata is still treated as explicit caller input rather than inferred editor state

## Next high-value work

- expand HTTP fixture coverage for create, export, attachments, and properties
- paginate raw CQL searches across larger result sets
- deepen move and comment contract coverage
- improve README / examples / migration guidance
- add stronger secret handling for persisted profiles
