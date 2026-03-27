# elmq

A CLI and MCP server for querying and editing Elm files — like jq for Elm.

Designed as a next-gen LSP for agents and scripts, not editors. Optimized for token efficiency and structured tool calling.

> **Status:** Early development. Currently supports listing declarations in Elm files. See [ROADMAP.md](ROADMAP.md) for what's planned.

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
