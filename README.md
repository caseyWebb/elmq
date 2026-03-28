---
update-when: CLI commands, output format, or installation steps change
---

# elmq

A CLI and MCP server for querying and editing Elm files — like jq for Elm.

Designed as a next-gen LSP for agents and scripts, not editors. Optimized for token efficiency and structured tool calling.

> **Status:** Active development. Supports reading and writing Elm declarations, imports, and module lines. MCP server available via `elmq mcp`. See [ROADMAP.md](ROADMAP.md) for what's planned.

## Install

### Homebrew

```sh
brew install caseyWebb/tap/elmq
```

### From source

Requires [Rust](https://rustup.rs/):

```sh
cargo install --path .
```

## Usage

### File summary

```sh
elmq list src/Main.elm
```

```
module Main exposing (Model, Msg(..), update, view)

imports:
  Html exposing (Html, div, text)
  Html.Attributes as Attr

type aliases:
  Model  L4-8

types:
  Msg  L11-15

functions:
  update  Msg -> Model -> Model  L18-28
  view    Model -> Html Msg      L31-34
  helper                         L37-38
```

### With doc comments

```sh
elmq list src/Main.elm --docs
```

```
type aliases:
  Model  L4-8
    The model for our app

types:
  Msg  L11-15
    Messages for the update function
...
```

### Extract a declaration

```sh
elmq get src/Main.elm update
```

```
update : Msg -> Model -> Model
update msg model =
    case msg of
        Increment ->
            { model | count = model.count + 1 }

        Decrement ->
            { model | count = model.count - 1 }

        Reset ->
            { model | count = 0 }
```

Includes doc comments and type annotations when present. Returns non-zero exit code if the declaration is not found.

### Upsert a declaration

```sh
echo 'helper x =
    x + 42' | elmq set src/Main.elm
```

Reads a full declaration from stdin, parses the name, and replaces the existing declaration (or appends if new). Use `--name` to override:

```sh
echo 'renamed x = x + 1' | elmq set src/Main.elm --name helper
```

### Patch a declaration

```sh
elmq patch src/Main.elm update --old "model.count + 1" --new "model.count + 2"
```

Surgical find-and-replace scoped to a single declaration. The `--old` string must match exactly once.

### Remove a declaration

```sh
elmq rm src/Main.elm helper
```

Removes the declaration, its type annotation, and doc comment. Cleans up excess blank lines.

### Manage imports

```sh
elmq import add src/Main.elm "Browser exposing (element)"
elmq import remove src/Main.elm Html
```

`import add` inserts in alphabetical order or replaces an existing import with the same module name.

### Manage exposing list

```sh
elmq expose src/Main.elm update
elmq expose src/Main.elm "Msg(..)"
elmq unexpose src/Main.elm helper
```

Granularly add or remove items from the module's exposing list. If the module has `exposing (..)`, `unexpose` auto-expands to an explicit list then removes the target. `expose` is a no-op when `exposing (..)`. Neither command ever produces `exposing (..)`.

### Rename/move a module

```sh
elmq mv src/Foo/Bar.elm src/Foo/Baz.elm
```

```
renamed src/Foo/Bar.elm -> src/Foo/Baz.elm
updated src/Main.elm
updated src/Page/Home.elm
```

Renames the file, updates the module declaration, and rewrites all imports and qualified references (`Foo.Bar.something` -> `Foo.Baz.something`) across the project. Requires `elm.json` in a parent directory. Use `--dry-run` to preview changes without writing.

### Rename a declaration

```sh
elmq rename src/Main.elm helper newHelper
```

```
renamed helper -> newHelper
updated src/Main.elm
updated src/Page/Home.elm
```

Renames a declaration (function, type, type alias, port, or variant) in the defining file and updates all references across the project — including qualified (`Module.helper`), aliased (`M.helper`), and exposed references. Requires `elm.json` in a parent directory. Use `--dry-run` to preview changes without writing.

### Find references

```sh
elmq refs src/Lib/Utils.elm
```

```
src/Main.elm:3
src/Page/Home.elm:5
src/Page/Settings.elm:3
```

Find all files that import a module. Add a declaration name to find specific usage sites:

```sh
elmq refs src/Lib/Utils.elm helper
```

```
src/Page/Home.elm:3: import Lib.Utils exposing (helper)
src/Page/Settings.elm:5: LU.helper config
src/Main.elm:7: Lib.Utils.helper model
```

Resolves fully-qualified references (`Lib.Utils.helper`), aliased references (`LU.helper`), and explicitly-exposed names. Requires `elm.json` in a parent directory.

### Rename a declaration

```sh
elmq rename src/Main.elm helper helperV2
```

```
renamed helper -> helperV2
updated src/Page/Home.elm
```

Renames a declaration (or type constructor) and updates all references across the project. Handles qualified, aliased, and bare-exposed references. Use `--dry-run` to preview changes.

### Move declarations between modules

```sh
elmq move-decl src/Page/Home.elm --name viewHeader --name viewFooter --to src/Shared/Layout.elm
```

```
moved viewHeader
moved viewFooter
auto-included renderNav
updated src/Page/Home.elm
updated src/Shared/Layout.elm
updated src/Main.elm
```

Moves declarations from one module to another, rewriting the declaration bodies to match the target file's import conventions (aliases, exposed names). Automatically includes unexposed helpers used only by the moved declarations. Creates the target file if it doesn't exist. Auto-upgrades the target to a `port module` when moving ports.

Use `--copy-shared-helpers` to duplicate (not move) helpers that are used by both moved and non-moved declarations. Use `--dry-run` to preview changes.

### Add/remove type variant constructors

```sh
elmq variant add src/Types.elm --type Msg "SetName String"
```

```
added SetName to Msg in src/Types.elm
  src/Update.elm:22  update      — inserted branch
  src/View.elm:15    label       — inserted branch
  src/Main.elm:31    update      — skipped (wildcard branch covers new variant)
```

Appends a constructor to a custom type and inserts `Debug.todo` branches in all matching case expressions project-wide. Case expressions with wildcard (`_`) branches are skipped with an info message.

```sh
elmq variant rm src/Types.elm --type Msg Decrement
```

```
removed Decrement from Msg in src/Types.elm
  src/Update.elm:22  update  — removed branch
  src/View.elm:15    label   — removed branch
```

Removes a constructor and its matching branches from all case expressions. Errors if removing the last variant (use `elmq rm` instead). Use `--dry-run` to preview changes.

### JSON output

```sh
elmq list src/Main.elm --format json
```

```json
{
  "module_line": "module Main exposing (Model, Msg(..), update, view)",
  "imports": ["Html exposing (Html, div, text)"],
  "declarations": [
    {
      "name": "update",
      "kind": "function",
      "type_annotation": "Msg -> Model -> Model",
      "start_line": 18,
      "end_line": 28
    }
  ]
}
```

## MCP Server

Start the MCP server (stdio transport):

```sh
elmq mcp
```

Exposes 4 tools optimized for LLM agents:

| Tool | Description |
|------|-------------|
| `elm_summary` | File overview: module, imports, declarations with types and line numbers |
| `elm_get` | Extract full source text of a declaration by name |
| `elm_edit` | All file mutations: `set`, `patch`, `rm`, `mv`, `rename`, `move_decl`, `add_variant`, `rm_variant`, `add_import`, `remove_import`, `expose`, `unexpose` |
| `elm_refs` | Find all references to a module or declaration across the project |

### Claude Code Plugin

For best results with Claude Code, install the elmq plugin:

```sh
/plugin marketplace add caseyWebb/elmq
/plugin install elmq@elmq
```

Or load directly for testing:

```sh
claude --plugin-dir path/to/elmq/.claude-plugin
```

The plugin auto-registers the MCP server and includes a SessionStart hook that detects Elm projects (via `elm.json`) and guides Claude to prefer elmq tools over built-in Read/Write/Edit for `.elm` files. The guidance survives context compaction.

### Manual MCP Configuration

Alternatively, configure the MCP server directly in your MCP client (e.g. Claude Code `settings.json`):

```json
{
  "mcpServers": {
    "elmq": {
      "command": "elmq",
      "args": ["mcp"]
    }
  }
}
```

## Roadmap

See [ROADMAP.md](ROADMAP.md) for the phased development plan.
