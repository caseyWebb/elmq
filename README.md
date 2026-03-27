---
update-when: CLI commands, output format, or installation steps change
---

# elmq

A CLI and MCP server for querying and editing Elm files — like jq for Elm.

Designed as a next-gen LSP for agents and scripts, not editors. Optimized for token efficiency and structured tool calling.

> **Status:** Active development. Supports reading and writing Elm declarations, imports, and module lines. See [ROADMAP.md](ROADMAP.md) for what's planned.

## Install

Requires Rust. If you use [mise](https://mise.jdx.dev/):

```sh
mise install
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

## Roadmap

See [ROADMAP.md](ROADMAP.md) for the phased development plan.
