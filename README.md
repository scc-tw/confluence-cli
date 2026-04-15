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

## Planned command shape

```text
confluence config init
confluence profile list|use|add|remove
confluence page read|info|find|search|children|create|update|patch|move|archive|delete|export
confluence attachment list|download|upload|delete
confluence property list|get|set|delete
confluence comment list|create|delete
confluence convert
```

## Status

This repository currently contains the initial Rust skeleton only. The command tree, API client, conversion pipeline, and feature-complete workflows are still to be implemented.
