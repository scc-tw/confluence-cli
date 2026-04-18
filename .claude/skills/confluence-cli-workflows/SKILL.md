---
name: confluence-cli-workflows
description: Uses confluence-cli through explicit one-liner commands for pages, folders, attachments, properties, comments, profiles, and shell-adjacent workflows. Use when the task requires concrete confluence command invocations or repo-specific command guidance. Prefer one-liners by default and avoid confluence shell unless the user explicitly asks for shell interaction.
---

# Confluence CLI one-liner workflow

Use this skill when you need to operate `confluence-cli` or explain how to use it.

## Default rule

Prefer **one-liner commands**.

Do **not** use `confluence shell` by default.

Only use `confluence shell` if the user explicitly asks for shell interaction or a shell-only workflow.

## Command selection

- profile/config/auth work → `confluence login`, `confluence profile ...`, `confluence config ...`
- read/search/list work → `confluence page read|info|search|children`
- page mutations → `confluence page create|create-child|update|patch|move|archive|delete`
- attachments → `confluence attachment ...`
- properties → `confluence property ...`
- comments → `confluence comment ...`
- local content conversion → `confluence convert ...`

## One-liner defaults

### Auth and profile

```text
confluence login
confluence profile show
confluence profile list
confluence profile use work
```

### Read and search

```text
confluence page info 12345
confluence page read 12345
confluence page read https://your-site.atlassian.net/wiki/spaces/SPACE/pages/12345/Page+Title
confluence page read https://your-site.atlassian.net/wiki/spaces/SPACE/overview
confluence page search "release notes"
confluence page search 'type=page and space=SPACE' --cql
confluence page children 12345
```

### Create and update

```text
confluence page create --space-key SPACE --title "Draft" --body "# Hello"
confluence page create-child 12345 --title "Child" --body "# Hello"
confluence page update 12345 --title "Doc" --storage-body "<p>Hello</p>" --version 7
confluence page patch 12345 --base-file .\page.storage --patch-file .\page.diff
confluence page move 12345 --to-parent 67890
confluence page archive 12345
confluence page delete 12345 --mode trash
```

### Attachments, properties, comments

```text
confluence attachment list 12345
confluence attachment upload 12345 --file .\diagram.png
confluence property list 12345
confluence property get 12345 owner
confluence comment list 12345
confluence comment create 12345 --body "Looks good"
```

### Conversion

```text
confluence convert --from markdown --to storage --input-file .\page.md
confluence convert --from storage --to markdown --input "<p>Hello</p>"
```

## VFS-style shell commands exist, but are not the default

The repo also has shell commands such as `ls`, `cd`, `file`, `stat`, `cat`, `tail`, `grep`, `find`, `mkdir`, `rm`, `rmdir`, `mv`, `cp`, `whoami`, `id`, `seq`, and `sleep`.

Do not switch to shell mode just because those commands exist.

Use shell mode only when the user explicitly wants an interactive session.

## Working rules

1. Choose the narrowest one-liner that solves the request.
2. Prefer explicit ids/URLs over inferred shell context.
3. If unsure about syntax, use `confluence --help` or `confluence <group> --help`.
4. For destructive commands, keep defaults conservative and show the exact command.
5. Keep examples de-identified: use `SPACE`, `12345`, `Workspace Alpha`, `People Docs`.

## Validation

When changing command behavior in this repo:

```text
- targeted tests first
- cargo test
- cargo build
```

## Repo boundaries

- clap tree: `src/cli/args.rs`
- one-liner dispatch: `src/cli/dispatch/*`
- shell: `src/cli/shell/*`
- VFS: `src/application/vfs/*`, `src/infrastructure/vfs/*`
- transport: `src/api/http/*`

If the task is about normal command usage, stay in the one-liner CLI surface.
