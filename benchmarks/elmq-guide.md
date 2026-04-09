# Working with .elm files: use `elmq`, not built-in tools

This is an Elm project. The `elmq` CLI is on PATH and is a tree-sitter-aware tool for reading, searching, and editing `.elm` files. It is dramatically cheaper than `Read`/`Write`/`Edit`/`Grep` for anything inside a `.elm` file.

## Hard rules — read these before touching any .elm file

1. **Do NOT use `Read` on .elm files.** `elmq list <file>` returns the module structure, imports, and every declaration with line ranges in ~10% of Read's tokens. It is your reconnaissance tool — use it every time, not Read.
2. **Do NOT use `Read` to look at a specific function or type.** `elmq get <file> <name>` returns that declaration's full source in isolation.
3. **Do NOT use `Edit` on .elm files.** Use `elmq set`, `elmq patch`, or `elmq rm` instead.
4. **Do NOT use `Grep` to search Elm code.** `elmq refs <file> [<name>]` resolves qualified, aliased, and exposed references through the import graph — `Grep` misses all of those.
5. **Do NOT invoke `node`, `python3`, `sed`, `awk`, or `perl` via Bash to rewrite .elm files.** That is the anti-pattern elmq exists to replace. Use `elmq patch` or `elmq set`.

The only acceptable built-in operations on an existing .elm file are `Bash(elm make …)`, `Bash(elm-format …)`, `Bash(elm-test …)`, and the discovery/glob/find tools that return file *lists* (not contents).

## Workflow

**Phase 1 — Explore.** Use `Bash(find src -name '*.elm')` or `Glob` to enumerate files. For each file you need to understand, run `elmq list <file>` to see its structure. Do not `Read` any .elm file during exploration.

**Phase 2 — Dive.** When `elmq list` tells you there's a specific function or type you need to understand, run `elmq get <file> <name>` to pull that declaration's source. Do not `Read` the surrounding file.

**Phase 3 — Search.** When you need to find where something is used, run `elmq refs <file> <name>`. Do not `Grep`.

**Phase 4 — Edit.** Use the appropriate `elmq` subcommand (see the table below). Do not `Edit`.

## Intent → command

| Intent | Command |
|---|---|
| See a file's structure | `elmq list <file>` (add `--docs` for doc comments) |
| Get one declaration's source | `elmq get <file> <name>` |
| Find references to a module | `elmq refs <file>` |
| Find references to a declaration | `elmq refs <file> <name>` |
| Upsert a declaration | `elmq set <file>` (source read from stdin; use a heredoc) |
| Find-replace inside a declaration | `elmq patch <file> <name> --old '…' --new '…'` |
| Remove a declaration | `elmq rm <file> <name>` |
| Add an import | `elmq import add <file> 'Html exposing (Html, div)'` |
| Remove an import | `elmq import remove <file> Html` |
| Add to exposing list | `elmq expose <file> update` (or `'Msg(..)'`) |
| Remove from exposing list | `elmq unexpose <file> helper` |
| Rename/move a module project-wide | `elmq mv <file> <new_path>` (add `--dry-run` to preview) |
| Rename a declaration project-wide | `elmq rename <file> <old_name> <new_name>` |
| Move declarations between modules | `elmq move-decl <file> --name foo --name bar --to <target>` |
| Add a type variant | `elmq variant add <file> --type Msg 'SetName String'` |
| Remove a type variant | `elmq variant rm <file> --type Msg Decrement` |

`elmq set` reads source from stdin. Pipe via heredoc:

```
elmq set src/Main.elm << 'ELM'
update : Msg -> Model -> ( Model, Cmd Msg )
update msg model = ...
ELM
```

## Creating a brand-new .elm file

When the file does not yet exist, use `Write` **once** with the full module source. Do not try `cat > ... << EOF`, `touch` + `elmq set`, `python3 -c`, or `node -e` — they all fail or flail. Just call `Write` directly with the complete file content.

After the file exists, switch back to `elmq` for any further edits.

## Everything else

`Bash` is for running `elm make`, `elm-format`, `elm-test`, `elm-review`, and other shell tools. `Glob` and `find` are for listing files. Everything that looks inside a `.elm` file goes through `elmq`.
