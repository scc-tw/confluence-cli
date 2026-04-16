# confluence-cli

`confluence` is a command-line tool for reading, searching, and updating Confluence from pure CLI workflows.

It supports two styles of use:

- **one-liner commands** for direct CRUD/query work
- **`confluence shell`** for an interactive, context-aware workflow that feels closer to a db shell or debugger prompt

## What it is good at

- searching pages with plain text or raw CQL
- reading pages by id or common Confluence URLs
- creating, updating, exporting, patching, moving, archiving, and deleting pages
- listing, uploading, downloading, and deleting attachments
- listing, reading, setting, and deleting content properties
- listing, creating, replying to, resolving, reopening, and deleting comments
- converting Confluence content locally between markdown, storage, html, and text

## Build and run

```text
cargo build
cargo run -- --help
```

Compiled binary:

```text
target\debug\confluence --help
```

## Start here

### 1. Log in

Interactive login is the fastest way to get started:

```text
confluence login
```

That flow creates or updates a profile, stores secrets through the existing secret backend, and makes the profile active.

If you want the lower-level flow instead, you can still initialize config directly:

```text
confluence config init --domain example.atlassian.net --auth-type bearer --api-token <token>
```

### 2. Inspect the active profile

```text
confluence profile show
confluence profile list
confluence profile use work
```

### 3. Learn the command tree

Top-level help:

```text
confluence --help
```

Drill down by group:

```text
confluence page --help
confluence attachment --help
confluence property --help
confluence comment --help
confluence shell --help
```

Then drill down again when needed:

```text
confluence page create --help
confluence page delete --help
confluence comment reply --help
```

## Common one-liners

### Read and search

Search by text:

```text
confluence page search "release notes"
```

Search with raw CQL:

```text
confluence page search "type=page and space=ENG" --cql
```

Read a page by id:

```text
confluence page read 12345
```

Read a page by Confluence URL:

```text
confluence page read https://your-site.atlassian.net/wiki/spaces/ENG/pages/12345/Page+Title
confluence page read https://your-site.atlassian.net/wiki/pages/viewpage.action?pageId=12345
```

Read a space overview URL by resolving the space home page:

```text
confluence page read https://your-site.atlassian.net/wiki/spaces/SPACEKEY/overview
```

Read as markdown:

```text
confluence page read 12345 --format markdown
```

Show page metadata:

```text
confluence page info 12345
```

### Create and update pages

Create a page in a space:

```text
confluence page create --space-key ENG --title "Draft" --body "# Hello"
```

Create from file:

```text
confluence page create --space-key ENG --title "Draft" --body-file .\page.md
```

Create a child page:

```text
confluence page create-child 12345 --title "Child" --body "# Child"
```

Update with explicit storage body and version:

```text
confluence page update 12345 --title "Design Doc" --storage-body "<p>Hello</p>" --version 7
```

Patch from a local base file plus unified diff:

```text
confluence page patch 12345 --base-file .\page.storage --patch-file .\page.diff
```

Preview a patch without writing:

```text
confluence page patch 12345 --base-file .\page.storage --patch-file .\page.diff --dry-run
```

### Move, archive, and delete

Move under a new parent:

```text
confluence page move 12345 --to-parent 67890
```

Archive:

```text
confluence page archive 12345
```

Delete to trash:

```text
confluence page delete 12345 --mode trash
```

Purge permanently:

```text
confluence page delete 12345 --mode purge --yes-im-sure
```

### Export and convert

Export a page bundle as markdown:

```text
confluence page export 12345 --dest .\exports\design --format markdown
```

Export without attachments:

```text
confluence page export 12345 --dest .\exports\design --skip-attachments
```

Convert locally:

```text
confluence convert --from markdown --to storage --input-file .\page.md
confluence convert --from storage --to markdown --input "<p>Hello</p>"
```

### Attachments

```text
confluence attachment list 12345
confluence attachment upload 12345 --file .\diagram.png
confluence attachment download 12345
confluence attachment delete 12345 diagram.png
```

### Properties

```text
confluence property list 12345
confluence property get 12345 owner
confluence property set 12345 owner --value '{"team":"eng"}'
confluence property delete 12345 owner
```

### Comments

```text
confluence comment list 12345
confluence comment create 12345 --body "Looks good"
confluence comment reply 12345 c-1 --body "Ack"
confluence comment resolve c-1
confluence comment reopen c-1
confluence comment delete c-1
```

## Interactive shell

If you are doing a sequence of related operations, use the shell:

```text
confluence shell
```

The shell now behaves more like a small virtual filesystem over Confluence:

- `/` lists available spaces
- `cd <space>` enters a space root
- `cd <page>` enters a child page
- `cd ..` goes up
- `pwd` shows the current logical path
- `ls` lists the current directory
- `cat` reads page text
- `grep` searches page text in the current subtree
- `find` walks the current subtree
- `|` pipes shell-native text commands together

Built-ins:

```text
help
pwd
ls
cd SPACE
cd 12345
cd ..
cd /
cat
grep keyword
find SPACE --name '*Guide'
ls SPACE | grep Guide
use profile work
exit
quit
```

Example shell session:

```text
confluence/> ls
confluence/> cd SPACE
confluence/SPACE> ls
confluence/SPACE> cd 12345
confluence/SPACE/Project Notes> pwd
confluence/SPACE/Project Notes> cat
confluence/SPACE/Project Notes> grep keyword
confluence/SPACE> find . --name '*Guide'
confluence/SPACE> ls . | grep Guide
confluence/SPACE> page create --title "Draft" --body "# Hello"
```

Inside the shell, page-scoped commands still inherit the current page when that is unambiguous, and `page create` inherits the current space when you are at a space root. Explicit CLI arguments still win over shell-derived context.

Current pipe support is intentionally small and shell-native:

- `ls`, `cat`, `grep`, and `find` can participate in text pipelines
- stateful builtins such as `cd`, `use profile`, and `exit` are rejected in pipelines
- maximum pipeline depth is capped to avoid runaway recursion

## Profiles and configuration

Useful profile commands:

```text
confluence profile list
confluence profile show
confluence profile add work --domain example.atlassian.net --auth-type bearer --api-token <token>
confluence profile use work
confluence profile remove work
```

Global flags:

- `--config-path <PATH>` select a config file
- `--profile <NAME>` select a profile for one invocation
- `--output <human|json>` switch output format

Resolution order:

1. `--config-path`
2. `--profile`
3. `CONFLUENCE_PROFILE`
4. active profile in config
5. environment overrides for the selected profile
6. built-in defaults

Supported environment overrides:

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

Defaults:

- `protocol`: `https`
- `api_path`: `/wiki/rest/api` for `*.atlassian.net`, otherwise `/rest/api`
- `auth_type`: `basic`
- `read_only`: `false`

Auth modes:

- `basic` needs email/username plus api token/password
- `bearer` needs api token
- `mtls` parses as config but HTTP client setup is not implemented yet

## Output modes

Human output is the default.

Machine-readable output:

```text
confluence --output json profile show
confluence --output json page info 12345
```

## Safety and guardrails

- read-only profiles block mutating commands before any write request is sent
- `page delete --mode purge` requires `--yes-im-sure`
- `page patch` only writes when the base file matches the current remote storage body exactly
- inline comment create/reply with `--location inline` requires explicit inline metadata
- moving across spaces is rejected
- renaming during `page move --before` or `--after` is not implemented
- mTLS transport is not implemented yet

## Verification

```text
cargo test
cargo build
```

The current test suite covers profile/bootstrap behavior, HTTP contracts, one-liner page URL reads, and shell flows through the real binary.
