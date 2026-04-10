# Working with `.elm` files: use `elmq`

This is an Elm project. `elmq` is on PATH — a tree-sitter-aware CLI for reading and editing `.elm` files.

- **Never use `cat` on `.elm` files.** It dumps the entire file into context with no structure.
- Do not use `Grep`, or `grep` / `rg` via `Bash`, to search Elm code. Use `elmq grep` for text discovery (returns the enclosing decl for free) and `elmq refs` for structural references through the import graph.
- For **project-wide operations** (rename module, move declarations, add/remove variants), always use the dedicated `elmq` commands — they handle the entire dependency graph atomically.
- For **single-file edits**, choose by file size:
  - **Under ~200 lines** (check `elmq list` — it shows line counts): `Read` + `Edit` is simplest. Fewer round trips than `elmq get` + `elmq patch`.
  - **Over ~200 lines**: use `elmq get` to read specific declarations and `elmq patch` to edit them. Avoids pulling the full file into context.
- `Write` is fine for creating a new `.elm` file; switch to `elmq` or `Edit` for further edits.

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

**Fetch only what you need for the current step.** Fetch the 2-3 declarations relevant to your current edit, then fetch more as needed — don't bulk-read an entire module at once.

- `elmq list <file...>` — module header, imports, declarations with line ranges, exposing list. Add `--docs` for doc comments. Accepts one or more files in a single call.
- `elmq get <file> <name...>` — full source of one or more declarations from the same file.
- `elmq get -f <file> <name...> [-f <file> <name...> ...]` — read declarations across multiple files in one call. Each `-f` group is a file followed by one or more names. Output frames each block as `## Module.decl` (or `## file:decl` without `elm.json`). Use this after `elmq list` on several files: list, pick the decls you need, then fetch them all in one `get -f` call instead of N separate calls.
- `elmq refs <file>` — every project file that imports this module.
- `elmq refs <file> <name...>` — every project reference to one or more declarations in this file.

## Editing Elm code

**Work in two phases: plan all edits, then apply them in as few tool calls as possible.** Each tool call is a conversation turn that re-reads the full context. Fewer turns = lower cost.

1. **Plan**: read the code you need (`elmq list`, `elmq get`, `Read`), decide on all the changes
2. **Apply**: execute all edits in one or two Bash calls using `&&` chains, then `elm make` once at the end

**Trust your edits.** If `elmq patch`/`set`/`Edit` exited `0`, it applied exactly what you asked for — don't re-read to "verify." Only re-read when the compiler complains.

### Command reference

**Project-wide** — these handle the entire dependency graph atomically:

| Intent | Command |
|---|---|
| Rename/move a module | `elmq mv <file> <new_path>` (`--dry-run` to preview) |
| Rename a declaration | `elmq rename <file> <old_name> <new_name>` |
| Extract/move declarations between modules | `elmq move-decl <src> --to <target> <name...>` (creates `<target>` if needed) |
| Add a type variant + fill case branches | `elmq variant add --type <T> <file> '<Variant>' [--fill <key>=<branch>]...` |
| Remove a type variant | `elmq variant rm --type <T> <file> <Constructor>` |

**Single-file** — chain these with `&&` when you have several:

| Intent | Command |
|---|---|
| Find-replace inside a declaration | `elmq patch --old '…' --new '…' <file> <name>` |
| Upsert a declaration (stdin heredoc) | `elmq set <file> << 'ELM' ... ELM` |
| Remove declarations | `elmq rm <file> <name...>` |
| Add/remove imports | `elmq import add|remove <file> <arg...>` |
| Add/remove from exposing list | `elmq expose|unexpose <file> <item...>` |

### Chaining edits

You've already read the source — the old strings are deterministic. Chain confidently:

```bash
elmq patch --old '...' --new '...' src/Route.elm Route && \
elmq patch --old '...' --new '...' src/Route.elm parser && \
elmq patch --old '...' --new '...' src/Page.elm viewMenu && \
elmq import add src/Main.elm 'Page.Bookmarks as Bookmarks' && \
elmq expose src/Page.elm Bookmarks
```

### Extracting declarations into a new module

`elmq move-decl` handles extraction in one call — creates the target file, moves the declarations, updates `exposing (…)` lists, and rewrites every qualified reference project-wide:

```
elmq move-decl src/Types.elm \
  --to src/Types/Auth.elm \
  Token validate decode encode
```

Do not manually reconstruct this with `Write` + `rm` + `import add` + find-and-replace.

### Adding a variant (plan then apply)

This is the plan-then-apply pattern in action. Two turns:

**Turn 1 (plan):** `elmq variant cases <file> --type <T>` — returns every case site with its enclosing function body and a stable `key`. Read the output and compose your `--fill` arguments for every key. This also tells you which sites are case expressions (`--fill` handles those) vs list-based dispatch (you'll `elmq patch` those).

**Turn 2 (apply):** `variant add` with all `--fill` args, chained with `elmq patch` for non-case sites:

```bash
elmq variant add src/Theme.elm --type Theme 'HighContrast' \
    --fill 'toClass=HighContrast -> "theme-high-contrast"' \
    --fill 'toCssVars=HighContrast -> highContrastVars' && \
elmq patch --old '...' --new '...' src/Theme.elm allThemes
```

**Do not call `variant add` without `--fill` and then patch each `Debug.todo` stub afterward** — that turns 1 tool call into many.

`--fill` keys must come from `variant cases` output. `--fill` only targets case expressions — functions using list-based dispatch (e.g. building a list of items per variant, parser combinator lists) won't appear as case sites; chain `elmq patch` for those.

When one function has two case expressions on the same type, or two files both define a function with the same name, `cases` prints disambiguated keys (`handler#1`, `handler#2`, or `src/Other.elm:handler`) — pass those through.

`--fill` values are `KEY=BRANCH` pairs split on the **first** `=`, so branch bodies containing `=` (e.g. record updates) are fine. The branch text is inserted verbatim with proper indentation; write the full `Pattern -> body` as you would type it into the source.

## Reference notes

- **Multi-arg framing.** Commands accepting `<name...>` run best-effort per argument. With 2+ args, output is `## <arg>` blocks; errors are inline `error: …` on stdout. Exit `2` if any arg failed.
- **`unexpose` / `import remove` are idempotent** — removing something that isn't there is a no-op, not an error.
