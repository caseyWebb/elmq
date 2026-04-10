# Working with `.elm` files: use `elmq`

This is an Elm project. `elmq` is on PATH — a tree-sitter-aware CLI for reading and editing `.elm` files. Use it instead of the built-in tools on `.elm` files:

- Do not use `Read`. Use `elmq list` / `elmq get`.
- Do not use `Edit`. Use `elmq patch` / `elmq set` / `elmq rm`.
- Do not use `Grep`, or `grep` / `rg` via `Bash`, to search Elm code. Use `elmq grep` for text discovery (returns the enclosing decl for free) and `elmq refs` for structural references through the import graph.
- `Write` is fine for creating a new `.elm` file; switch to `elmq` for any further edits.

## Discovery (step 0)

When you don't already know the name of the declaration you need to touch, start with `elmq grep`. It regex-searches `.elm` files and, for each hit, reports the enclosing top-level declaration — so you can pipe directly into `elmq get` without ever reading a whole file.

- `elmq grep <regex> [path]` — compact output is `file:line:decl:line_text` (`-` in the decl slot means the match is outside any top-level decl, e.g. imports or the module header).
- Flags: `-F` literal, `-i` ignore case, `--format json` for machine pipelines.
- **Comments and string literals are filtered by default.** That is the whole point — it keeps discovery signal clean. Opt back in only when you specifically want them: `--include-comments` for `TODO` / docstring hunts, `--include-strings` for user-facing error messages.
- Project discovery is automatic: walks ancestors for `elm.json` (works from monorepo subdirs), falls back to walking CWD recursively if none is found, honors `.gitignore` in both paths.
- Exit codes match `rg`: `0` match, `1` no match, `2` error — safe in pipelines.

Typical discovery → retrieval flow:

```
$ elmq grep 'Http\.get'
src/Api.elm:42:fetchUsers:    Http.get { url = "/users" }
$ elmq get src/Api.elm fetchUsers
```

> **Do not `rg` inside the Elm tree.** `rg` returns `file:line:text`, which forces you to read the whole file to figure out which function each hit belongs to — that is the exact token cost `elmq grep` exists to eliminate. `elmq grep` does the offset → enclosing-decl mapping for free via tree-sitter, and filters comment/string noise by default. Reach for `rg` only on non-`.elm` files.

## Reading Elm code

`elmq list` and `elmq get` are targeted exploration tools — use them on files and declarations you need to understand, not carte blanche on the whole project. If you already know the file but not the decl, `list` it; if `elmq grep` already told you the decl name, skip straight to `get`.

- `elmq list <file...>` — module header, imports, declarations with line ranges, exposing list. Add `--docs` for doc comments. Accepts one or more files in a single call.
- `elmq get <file> <name...>` — full source of one or more declarations from the same file.
- `elmq refs <file>` — every project file that imports this module.
- `elmq refs <file> <name...>` — every project reference to one or more declarations in this file.

## Editing Elm code

Reach for the highest-level command that matches the intent. The project-wide commands (`mv`, `rename`, `move-decl`, `variant add`, `variant rm`) update every call site in one call — do not reconstruct them manually with `Write` + `rm` + `import add` + fixups.

| Intent | Command |
|---|---|
| Rename/move a module project-wide | `elmq mv <file> <new_path>` (`--dry-run` to preview) |
| Rename a declaration project-wide | `elmq rename <file> <old_name> <new_name>` |
| Extract declarations into a new module, or move declarations between modules | `elmq move-decl <src> --to <target> <name...>` (creates `<target>` if it doesn't exist) |
| Add a type variant | `elmq variant add --type <TypeName> <file> '<Variant def>'` |
| Remove a type variant | `elmq variant rm --type <TypeName> <file> <Constructor>` |
| Find-replace inside a declaration | `elmq patch --old '…' --new '…' <file> <name>` |
| Upsert a declaration | `elmq set <file>` (source from stdin via heredoc) |
| Remove declarations | `elmq rm <file> <name...>` |
| Add imports | `elmq import add <file> 'Html exposing (Html, div)' ...` |
| Remove imports | `elmq import remove <file> Html ...` |
| Add to exposing list | `elmq expose <file> update ...` (or `'Msg(..)'`) |
| Remove from exposing list | `elmq unexpose <file> helper ...` |

### Extracting declarations into a new module

A task like *"extract `Cred`, `username`, `credHeader`, `credDecoder` from `src/Api.elm` into a new `src/Api/Cred.elm`"* is one `elmq move-decl` call:

```
elmq move-decl src/Api.elm \
  --to src/Api/Cred.elm \
  Cred username credHeader credDecoder
```

`move-decl` creates the target file, removes the declarations from the source, updates both files' `exposing (…)` lists, and rewrites every qualified reference in the project to use the new module path.

### `elmq set` stdin

`elmq set` reads source from stdin via heredoc:

```
elmq set src/Main.elm << 'ELM'
update : Msg -> Model -> ( Model, Cmd Msg )
update msg model = ...
ELM
```

Prefer `elmq patch` for fragment edits (new case branch, record field, parameter, body tweak); reserve `elmq set` for whole-body rewrites.

## Gotchas

- **Multi-argument output is framed.** Commands that accept `<...>` positional rest (`list`, `get`, `rm`, `refs`, `import add`, `import remove`, `expose`, `unexpose`, `move-decl`) run best-effort per argument: a bad argument does not abort the others. With two or more arguments, output is `## <arg>` blocks in input order, with per-argument errors rendered inline as `error: …` on stdout (not stderr). Single-argument calls produce bare output unchanged. Exit `2` if any argument failed, `0` otherwise.
- **`unexpose` and `import remove` are idempotent.** Unexposing an item that isn't exposed, or removing an import that doesn't exist, is a successful no-op — not an error.
- **`variant add` branches are `Debug.todo "<VariantName>"`.** If you want to fill them in with `elmq patch` afterward, `get` the destination first to see the exact placeholder text — do not guess.
