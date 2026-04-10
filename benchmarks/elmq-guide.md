# Working with `.elm` files: use `elmq`

This is an Elm project. `elmq` is on PATH — a tree-sitter-aware CLI for reading and editing `.elm` files. Use it instead of the built-in tools on `.elm` files:

- Do not use `Read`. Use `elmq list` / `elmq get`.
- Do not use `Edit`. Use `elmq patch` / `elmq set` / `elmq rm`.
- Do not use `Grep`, or `grep` / `rg` via `Bash`, to search Elm code. Use `elmq grep` for text discovery (returns the enclosing decl for free) and `elmq refs` for structural references through the import graph.
- `Write` is fine for creating a new `.elm` file; switch to `elmq` for any further edits.

## Discovery (step 0)

When you don't already know the name of the declaration you need to touch, start with `elmq grep`. It regex-searches `.elm` files and, for each hit, reports the enclosing top-level declaration — so you can pipe directly into `elmq get` without ever reading a whole file.

- `elmq grep <regex> [path]` — compact output is `file:line:decl:line_text` (`-` in the decl slot means the match is outside any top-level decl, e.g. imports or the module header).
- Flags: `-F` literal, `-i` ignore case, `--format json` for machine pipelines, `--definitions` (only emit matches at the declaration name site, filtering out call sites), `--source` (emit full declaration source for each matched decl, deduped — replaces the locator lines with actual code). Combine `--definitions --source` for one-call definition lookup: `elmq grep --definitions --source 'myFunction'`.
- **Comments and string literals are filtered by default.** That is the whole point — it keeps discovery signal clean. Opt back in only when you specifically want them: `--include-comments` for `TODO` / docstring hunts, `--include-strings` for user-facing error messages.
- Project discovery is automatic: walks ancestors for `elm.json` (works from monorepo subdirs), falls back to walking CWD recursively if none is found, honors `.gitignore` in both paths.
- Exit codes match `rg`: `0` match, `1` no match, `2` error — safe in pipelines.

One-call definition lookup:

```
$ elmq grep --definitions --source 'myFunction'
myFunction : Int -> String
myFunction n =
    String.fromInt n
```

Or the two-step flow when you want locator lines first, then selective retrieval:

```
$ elmq grep 'Http\.get'
src/Api.elm:42:fetchData:    Http.get { url = "/items" }
$ elmq get src/Api.elm fetchData
```

> **Use `rg` only on non-`.elm` files.** `elmq grep` gives you the enclosing decl for free and filters comment/string noise — `rg` can't do either.

## Reading Elm code

`elmq list` and `elmq get` are targeted exploration tools — use them on files and declarations you need to understand, not carte blanche on the whole project. If you already know the file but not the decl, `list` it; if `elmq grep` already told you the decl name, skip straight to `get`.

`elmq list` shows each file's line count. For files under ~200 lines, `Read`+`Edit` is simpler. For larger files, use `elmq get`/`patch` to avoid pulling the full source into context.

**Fetch only what you need for the current step.** Fetch the 2-3 declarations relevant to your current edit, then fetch more as needed — don't bulk-read an entire module at once.

- `elmq list <file...>` — module header, imports, declarations with line ranges, exposing list. Add `--docs` for doc comments. Accepts one or more files in a single call.
- `elmq get <file> <name...>` — full source of one or more declarations from the same file.
- `elmq get -f <file> <name...> [-f <file> <name...> ...]` — read declarations across multiple files in one call. Each `-f` group is a file followed by one or more names. Output frames each block as `## Module.decl` (or `## file:decl` without `elm.json`). Use this after `elmq list` on several files: list, pick the decls you need, then fetch them all in one `get -f` call instead of N separate calls.
- `elmq refs <file>` — every project file that imports this module.
- `elmq refs <file> <name...>` — every project reference to one or more declarations in this file.

## Editing Elm code

Reach for the highest-level command that matches the intent. The project-wide commands (`mv`, `rename`, `move-decl`, `variant add`, `variant rm`) update every call site in one call — do not reconstruct them manually with `Write` + `rm` + `import add` + fixups.

| Intent | Command |
|---|---|
| Rename/move a module project-wide | `elmq mv <file> <new_path>` (`--dry-run` to preview) |
| Rename a declaration project-wide | `elmq rename <file> <old_name> <new_name>` |
| Extract declarations into a new module, or move declarations between modules | `elmq move-decl <src> --to <target> <name...>` (creates `<target>` if it doesn't exist) |
| List the case sites a type has (planning step before `variant add`/`rm`) | `elmq variant cases <file> --type <TypeName>` |
| Add a type variant, optionally filling each case branch in the same call | `elmq variant add --type <TypeName> <file> '<Variant def>' [--fill <key>=<branch>]...` |
| Remove a type variant | `elmq variant rm --type <TypeName> <file> <Constructor>` |
| Find-replace inside a declaration | `elmq patch --old '…' --new '…' <file> <name>` |
| Upsert a declaration | `elmq set <file>` (source from stdin via heredoc) |
| Remove declarations | `elmq rm <file> <name...>` |
| Add imports | `elmq import add <file> 'Html exposing (Html, div)' ...` |
| Remove imports | `elmq import remove <file> Html ...` |
| Add to exposing list | `elmq expose <file> name ...` (or `'Type(..)'`) |
| Remove from exposing list | `elmq unexpose <file> name ...` |

### Extracting declarations into a new module

`elmq move-decl` handles extraction in one call — it creates the target file, moves the declarations, updates `exposing (…)` lists on both sides, and rewrites every qualified reference in the project:

```
elmq move-decl src/Types.elm \
  --to src/Types/Auth.elm \
  Token validate decode encode
```

Do not manually reconstruct this with `Write` + `rm` + `import add` + find-and-replace. `move-decl` handles the entire dependency graph in a single atomic operation.

### Adding a variant with branch bodies (two turns)

1. **`elmq variant cases <file> --type <T>`** — returns every case site with its enclosing function body and a stable `key`. This is all the context you need — don't grep or get the type definition separately.
2. **`elmq variant add <file> --type <T> '<Variant>' --fill key=branch ...`** — inserts the variant and fills each case branch. Sites you omit fall back to `Debug.todo` stubs. Then use `elmq patch`/`set` for any non-case dispatch (list-based, parser combinators, etc.) that `--fill` can't reach.

```
$ elmq variant cases src/Theme.elm --type Theme
$ elmq variant add src/Theme.elm --type Theme 'HighContrast' \
    --fill 'toClass=HighContrast -> "theme-high-contrast"' \
    --fill 'toCssVars=HighContrast -> highContrastVars'
$ elm make src/Main.elm
```

**`--fill` keys must come from `variant cases` output.** `--fill` only targets `case` expressions. Functions that reference the type through list-based dispatch (e.g. building a list of items per variant, parser combinator lists) will not appear as case sites — use `elmq patch` for those. Always run `variant cases` first so you know which keys are valid before calling `variant add`.

When one function has two case expressions on the same type, or two files both define a function with the same name, `cases` prints disambiguated keys (`handler#1`, `handler#2`, or `src/Other.elm:handler`) — pass those through. If you pass an ambiguous bare key, `variant add` errors with the valid disambiguated alternatives before touching any file.

`--fill` values are `KEY=BRANCH` pairs split on the **first** `=`, so branch bodies containing `=` (e.g. record updates) are fine. The branch text is inserted verbatim with proper indentation; write the full `Pattern -> body` as you would type it into the source.

### `elmq set` stdin

`elmq set` reads source from stdin via heredoc:

```
elmq set src/Helpers.elm << 'ELM'
clamp : comparable -> comparable -> comparable -> comparable
clamp low high value =
    max low (min high value)
ELM
```

Prefer `elmq patch` for fragment edits (new case branch, record field, parameter, body tweak); reserve `elmq set` for whole-body rewrites.

**Trust your edits.** If `elmq patch`/`set` exited `0`, it applied exactly what you asked for — don't re-read to "verify." Only re-read when the compiler complains. **Compile at the end, not after every edit** — batch all edits, then run `elm make` once.

## Reference notes

- **Multi-arg framing.** Commands accepting `<name...>` run best-effort per argument. With 2+ args, output is `## <arg>` blocks; errors are inline `error: …` on stdout. Exit `2` if any arg failed.
- **`unexpose` / `import remove` are idempotent** — removing something that isn't there is a no-op, not an error.
