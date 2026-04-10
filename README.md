---
update-when: CLI commands, output format, or installation steps change
---

# elmq

A CLI for querying and editing Elm files — like jq for Elm.

Designed as a next-gen LSP for agents and scripts, not editors. Optimized for token efficiency and structured output.

> **Status:** Active development. Supports reading and writing Elm declarations, imports, and module lines, plus project-wide operations (rename, move, extract, add/remove variant). See [ROADMAP.md](ROADMAP.md) for what's planned.

## Install

### Homebrew

```sh
brew install caseyWebb/tap/elmq
```

### npm

```sh
npm install -g @caseywebb/elmq
```

Or run without installing:

```sh
npx @caseywebb/elmq <command>
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
module Main exposing (Model, Msg(..), update, view)  (38 lines)

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

Read across multiple files in one call with `-f`:

```sh
elmq get -f src/Page/Home.elm update view -f src/Update.elm main
```

Each `-f` group is a file followed by one or more names. Output is framed as `## Module.decl` blocks (falls back to `## file:decl` without `elm.json`).

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
elmq move-decl src/Page/Home.elm --to src/Shared/Layout.elm viewHeader viewFooter
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

To fill branch bodies in the same call, first survey the sites with `elmq variant cases`, then pass their keys to `--fill`:

```sh
elmq variant cases src/Types.elm --type Msg
```

```
## case sites for type Types.Msg (2 files, 2 functions)

### src/Update.elm

#### update (key: update, line 12)
update : Msg -> Model -> Model
update msg model =
    case msg of
        Increment -> ...
        Decrement -> ...

### src/View.elm

#### label (key: label, line 8)
...
```

```sh
elmq variant add src/Types.elm --type Msg "Reset" \
  --fill 'update=Reset -> { model | count = 0 }' \
  --fill 'label=Reset -> "reset"'
```

`variant cases` is read-only and returns every case expression project-wide that matches the target type, with its enclosing function body (including type annotation) and a stable site key. Pass those keys to `--fill <key>=<branch_text>` (repeatable) on `variant add` to replace the default `Debug.todo "<Variant>"` stub with real branch bodies in the same call. Unmatched fill keys fail validation before any file is touched; unfilled sites fall back to `Debug.todo` stubs (graceful degradation).

When one function contains multiple case expressions on the same type, or two files both define a function with the same name, `variant cases` disambiguates with `function#N` or `file:function` keys. Passing an ambiguous bare key to `--fill` errors with the valid alternatives listed.

```sh
elmq variant rm src/Types.elm --type Msg Decrement
```

```
removed Decrement from Msg in src/Types.elm
  src/Update.elm:22  update  — removed branch
  src/View.elm:15    label   — removed branch
```

Removes a constructor and its matching branches from all case expressions. Errors if removing the last variant (use `elmq rm` instead). Use `--dry-run` to preview changes.

### Search Elm sources

```sh
elmq grep "Http\.get"
```

```
src/Api.elm:42:fetchUser:    Http.get { url = userUrl, expect = Http.expectJson GotUser decoder }
src/Page/Home.elm:88:init:    Http.get { url = feedUrl, expect = Http.expectJson GotFeed feedDecoder }
```

Regex search over Elm files (Rust `regex` dialect, same as ripgrep) that annotates each hit with its **enclosing top-level declaration** — the discovery entry point that feeds into `elmq get`. Use `-F` for literal matching and `-i` for case-insensitive. Matches inside `--` / `{- -}` comments and string literals are filtered by default; pass `--include-comments` or `--include-strings` to opt back in independently.

Project discovery walks up for `elm.json` and honors its `source-directories`; if no `elm.json` is found, falls back to recursively walking the CWD. Both paths honor `.gitignore`. Exit codes match ripgrep: `0` on matches, `1` on none, `2` on error.

Two additional flags enable one-call definition lookup and source retrieval:

- `--definitions` — only emit matches at the declaration name site (filters out call sites)
- `--source` — emit full declaration source for each matched decl, deduped by `(file, decl)`. Output is framed `## Module.decl` (single result stays bare).

Combine them for definition lookup: `elmq grep --definitions --source 'update'` returns the full source of the `update` declaration without any call-site noise.

Pipe into `elmq get` for a find-then-retrieve workflow:

```sh
elmq grep --format json "Http\.get" \
  | jq -r 'select(.decl) | "\(.file) \(.decl)"' \
  | sort -u \
  | while read file decl; do elmq get "$file" "$decl"; done
```

Matches that land outside any top-level declaration (imports, module header) report `decl: null` in JSON and an empty decl slot in compact output.

### Agent integration guide

```sh
elmq guide
```

Prints the built-in agent integration guide to stdout. This is the guide that tells LLM coding agents how to use elmq effectively — when to prefer `elmq get` over `Read`, how to chain edits, etc. The Claude Code plugin (`/plugin install elmq@caseyWebb`) uses this automatically via a SessionStart hook.

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

## Using elmq with LLM coding agents

elmq is designed to be used from any coding agent that can shell out to a CLI (Claude Code, Cursor, Aider, Codex, etc.). The built-in agent guide (`elmq guide`) tells agents how to use elmq effectively.

**Claude Code**: Install the plugin with `/plugin install elmq@caseyWebb`. It automatically injects the guide into sessions working in Elm projects.

**Other agents**: Pipe `elmq guide` into your agent's system prompt or project instructions.

## Roadmap

See [ROADMAP.md](ROADMAP.md) for the phased development plan.
