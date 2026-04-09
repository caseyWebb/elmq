# Working with .elm files: prefer `elmq` over built-in tools

This is an Elm project. The `elmq` CLI is on PATH and is a structured, tree-sitter-aware tool for reading and editing Elm files — it is cheaper and more reliable than generic `Read`/`Write`/`Edit`/`Grep` for anything inside a `.elm` file.

## Rule

For every operation on a `.elm` file, reach for `elmq` first. Use `Read`/`Write`/`Edit`/`Grep` only for the narrow exceptions listed at the end.

## Intent → command

**Understand a file's structure** (imports, types, functions, line ranges) — prefer over `Read`:
```
elmq list <file>               # compact digest, ~10% of a full Read
elmq list <file> --docs        # include doc comments
elmq list <file> --format json # structured output
```

**Extract one declaration's source** — prefer over `Read` of the whole file:
```
elmq get <file> <name>
```

**Find references to a module or declaration across the project** — prefer over `Grep`:
```
elmq refs <file>               # files importing this module
elmq refs <file> <name>        # every usage site of the declaration
```

**Upsert a declaration** (insert new or replace existing) — prefer over `Edit`:
```
elmq set <file> < <<'ELM'
update : Msg -> Model -> ( Model, Cmd Msg )
update msg model = ...
ELM
# or: elmq set <file> --name <name>  (override parsed name)
```
Source is read from stdin. Use a heredoc for multi-line bodies.

**Surgical find-and-replace scoped to one declaration** — prefer over `Edit`:
```
elmq patch <file> <name> --old '<text>' --new '<text>'
```

**Remove a declaration** (including its doc comment and type annotation):
```
elmq rm <file> <name>
```

**Manage imports** — prefer over manual `Edit` of the import block:
```
elmq import add <file> 'Html exposing (Html, div, text)'
elmq import remove <file> Html
```

**Manage the module's exposing list**:
```
elmq expose <file> update
elmq expose <file> 'Msg(..)'
elmq unexpose <file> helper
```

**Rename or move a module project-wide** (file rename + update every import and qualified reference):
```
elmq mv <file> <new_path>
elmq mv <file> <new_path> --dry-run   # preview first
```

**Rename a declaration project-wide** (all usage sites updated):
```
elmq rename <file> <old_name> <new_name>
elmq rename <file> <old_name> <new_name> --dry-run
```

**Move declarations from one module to another** (with import-aware body rewriting):
```
elmq move-decl <file> --name funcA --name typeB --to <target_file>
elmq move-decl <file> --name funcA --to <target> --copy-shared-helpers --dry-run
```

**Add or remove a type variant constructor** (propagates branches through every matching case expression project-wide):
```
elmq variant add <file> --type Msg 'SetName String'
elmq variant rm  <file> --type Msg Decrement
```
Both support `--dry-run`.

## Acceptable uses of built-in tools on `.elm` files

- **`Write`** — creating a brand-new `.elm` file from scratch when no similar file exists to base it on.
- **`Bash`** — running `elm make`, `elm-format`, `elm-test`, `elm-review`, and other shell tools.

Everything else (reading, editing, searching) should go through `elmq`.
