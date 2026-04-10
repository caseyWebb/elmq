# Working with `.elm` files: use `elmq`

This is an Elm project. `elmq` is on PATH — a tree-sitter-aware CLI for reading and editing `.elm` files. Use it instead of the built-in tools on `.elm` files:

- Do not use `Read`. Use `elmq list` / `elmq get`.
- Do not use `Edit`. Use `elmq patch` / `elmq set` / `elmq rm`.
- Do not use `Grep`, or `grep` / `rg` via `Bash`, to search Elm code. Use `elmq grep` for text discovery (returns the enclosing decl for free) and `elmq refs` for structural references through the import graph.
- `Write` is fine for creating a new `.elm` file; switch to `elmq` for any further edits.

## Discovery (step 0)

When you don't already know the name of the declaration you need to touch, start with `elmq grep`. It regex-searches `.elm` files and, for each hit, reports the enclosing top-level declaration — so you can pipe directly into `elmq get` without ever reading a whole file.

- `elmq grep <regex> [path]` — compact output is `file:line:decl:line_text` (`-` in the decl slot means the match is outside any top-level decl, e.g. imports or the module header).
- Flags: `-F` literal, `-i` ignore case, `--format json` for machine pipelines, `--definitions` (only emit matches at the declaration name site, filtering out call sites), `--source` (emit full declaration source for each matched decl, deduped — replaces the locator lines with actual code). Combine `--definitions --source` for one-call definition lookup: `elmq grep --definitions --source 'submitForm'`.
- **Comments and string literals are filtered by default.** That is the whole point — it keeps discovery signal clean. Opt back in only when you specifically want them: `--include-comments` for `TODO` / docstring hunts, `--include-strings` for user-facing error messages.
- Project discovery is automatic: walks ancestors for `elm.json` (works from monorepo subdirs), falls back to walking CWD recursively if none is found, honors `.gitignore` in both paths.
- Exit codes match `rg`: `0` match, `1` no match, `2` error — safe in pipelines.

One-call definition lookup:

```
$ elmq grep --definitions --source 'fetchUsers'
fetchUsers : Cmd msg
fetchUsers =
    Http.get { url = "/users" }
```

Or the two-step flow when you want locator lines first, then selective retrieval:

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

### Adding a variant with branch bodies (the three-turn pattern)

`elmq variant add` inserts `Debug.todo "<Variant>"` stubs into every matching case expression by default. Patching them one-by-one afterward is a turn sink — a single call with `--fill` can write real branch bodies in the same operation. The workflow is:

1. **`elmq variant cases <file> --type <T>`** — read-only. Returns every case expression in the project that matches on `T`, with the enclosing function body and a stable `key` per site. Read it once to plan bodies.
2. **`elmq variant add <file> --type <T> '<Variant>' --fill key=branch ...`** — atomic write. Inserts the variant into the type declaration and fills each case branch from `--fill`. Sites you omit from `--fill` fall back to `Debug.todo` stubs (graceful degradation).
3. **`elm make`** — verify.

Worked example for adding a `Bookmarks` page to a router:

```
# 1. Plan: see all case expressions on Route and Page.
$ elmq variant cases src/Route.elm --type Route
$ elmq variant cases src/Page.elm  --type Page

# 2. Write: add the variant and fill every branch in one shot.
$ elmq variant add src/Route.elm --type Route 'Bookmarks' \
    --fill 'routeToPieces=Bookmarks -> [ "bookmarks" ]' \
    --fill 'parser=Parser.map Bookmarks (s "bookmarks")'

$ elmq variant add src/Page.elm --type Page 'Bookmarks' \
    --fill 'viewMenu=Bookmarks -> ...' \
    --fill 'isActive=( Bookmarks, Route.Bookmarks ) -> True'

# 3. Verify once.
$ elm make src/Main.elm
```

Keys come verbatim from the `cases` output. In the common case the bare function name works (`update`, `view`, `parser`). When one function has two case expressions on the same type, or two files both define a function with the same name, `cases` prints disambiguated keys (`update#1`, `update#2`, or `src/Main.elm:update`) — pass those through. If you pass an ambiguous bare key, `variant add` errors with the valid disambiguated alternatives before touching any file.

`--fill` values are `KEY=BRANCH` pairs split on the **first** `=`, so branch bodies containing `=` (e.g. record updates) are fine. The branch text is inserted verbatim with proper indentation; write the full `Pattern -> body` as you would type it into the source.

### `elmq set` stdin

`elmq set` reads source from stdin via heredoc:

```
elmq set src/Main.elm << 'ELM'
update : Msg -> Model -> ( Model, Cmd Msg )
update msg model = ...
ELM
```

Prefer `elmq patch` for fragment edits (new case branch, record field, parameter, body tweak); reserve `elmq set` for whole-body rewrites.

## Working efficiently (minimize turns, not just tokens)

Every tool call is another assistant turn, and each turn re-pays the cache-read tax. Token wins come from **fewer turns**, not from clever syntax inside a turn. Two rules flow from that:

- **Trust your edits. Don't re-read after a successful `elmq patch` / `elmq set`.** If the command exited `0` it applied exactly what you asked for — re-running `elmq get` to "verify" is pure overhead. Only re-read when the Elm compiler complains and you need to see current state to debug. (If you need the post-edit text for a follow-up patch, you already had it: it's what you put in `--new`.)
- **Compile at the end, not after every edit.** For a multi-step change (add a variant, wire it through `update`/`view`/`subscriptions`, add an import, etc.), do all the edits first and run `elm make` once at the very end. Each intermediate `elm make` is a ~10s round-trip *and* a tool-result payload the model then has to re-read on the next turn. Batch the work, compile once, fix whatever the compiler actually reports. An exception: if you're unsure a structural change (new module file, new type) will even parse, one early compile is worth it — but stop there until the full change is in place.

## Gotchas

- **Multi-argument output is framed.** Commands that accept `<...>` positional rest (`list`, `get`, `rm`, `refs`, `import add`, `import remove`, `expose`, `unexpose`, `move-decl`) run best-effort per argument: a bad argument does not abort the others. With two or more arguments, output is `## <arg>` blocks in input order, with per-argument errors rendered inline as `error: …` on stdout (not stderr). Single-argument calls produce bare output unchanged. Exit `2` if any argument failed, `0` otherwise.
- **`unexpose` and `import remove` are idempotent.** Unexposing an item that isn't exposed, or removing an import that doesn't exist, is a successful no-op — not an error.
- **`variant add` inserts `Debug.todo "<VariantName>"` stubs by default.** Prefer `--fill` over patching stubs afterward — it collapses the write into one turn. Use `elmq variant cases` first to gather the context (enclosing function bodies + stable keys) you need to synthesize `--fill` bodies. Only fall back to `elmq patch` on the `Debug.todo` stubs when you genuinely need a second pass (e.g. the compiler revealed more context after the initial fill).
