# Working with `.elm` files: use `elmq`

This is an Elm project. `elmq` is on PATH — a tree-sitter-aware CLI for reading and editing `.elm` files.

- **Never use `cat` on `.elm` files.** It dumps the entire file into context with no structure.
- Do not use `Grep`, or `grep` / `rg` via `Bash`, to search Elm code. Use `elmq grep` for text discovery (returns the enclosing decl for free) and `elmq refs` for structural references through the import graph.
- For **project-wide operations** (rename module, move declarations, add/remove variants), always use the dedicated `elmq` commands — they handle the entire dependency graph atomically.
- For **single-file edits**, choose by file size:
  - **Under ~300 lines** (check `elmq list` — it shows line counts): `Read` + `Edit` is simplest. Fewer round trips than `elmq get` + `elmq patch`.
  - **Over ~300 lines**: use `elmq get` to read specific declarations and `elmq patch` to edit them. Avoids pulling the full file into context.
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

- `elmq list <file...>` — module header, imports, declarations with line ranges, exposing list. **Pass all files in one call**: `elmq list src/Route.elm src/Page.elm src/Main.elm`, not three separate calls.
- `elmq get -f <file> <name...> [-f <file> <name...> ...]` — read declarations from one or more files. **Combine all files into one call**: `elmq get -f src/Route.elm Route parser -f src/Page.elm Page viewMenu -f src/Main.elm Model Msg update`. Do not issue separate `get` calls per file.
- `elmq refs <file>` — every project file that imports this module.
- `elmq refs <file> <name...>` — every project reference to one or more declarations in this file. Each name dispatches on what it resolves to: a top-level declaration produces a flat list of reference sites; a **constructor** of a custom type declared in the file produces a **classified** report (`case-branch`, `case-wildcard-covered`, `function-arg-pattern`, `lambda-arg-pattern`, `let-binding-pattern`, `expression-position`) with clean/blocking counts — the same data that `variant rm` surfaces in its `references not rewritten` advisory. You can mix decl and constructor names in one call; each is framed under a `## <arg>` header.

## Editing Elm code

**Work in two phases: plan all edits, then apply them in as few tool calls as possible.** Each tool call is a conversation turn that re-reads the full context. Fewer turns = lower cost.

1. **Plan**: read the code you need (`elmq list`, `elmq get`, `Read`), decide on all the changes
2. **Apply**: execute all edits in one or two Bash calls using `&&` chains, then `elm make` once at the end

**Trust your edits.** If `elmq patch`/`set`/`variant add`/`Edit` exited `0`, it applied exactly what you asked for. **Never re-read a file or declaration after a successful edit** — each re-read is a wasted turn. Only re-read when the compiler reports an error you need to diagnose.

### Command reference

**Project-wide** — these handle the entire dependency graph atomically. **Never manually patch a type definition to add/remove a variant** — use `variant add`/`variant rm`, which propagate through every case expression in the project:

| Intent | Command |
|---|---|
| Rename/move a module | `elmq mv <file> <new_path>` (`--dry-run` to preview) |
| Rename a declaration | `elmq rename <file> <old_name> <new_name>` |
| Extract/move declarations between modules | `elmq move-decl <src> --to <target> <name...>` (creates `<target>` if needed) |
| Add a type variant + fill case branches | `elmq variant add --type <T> <file> '<Variant>' [--fill <key>=<branch>]...` |
| Remove a type variant | `elmq variant rm --type <T> <file> <Constructor>` (emits an advisory list of any non-case references it could not rewrite) |

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

### Removing a variant

`variant rm` is a single self-contained call. It strips the constructor from the `type` declaration, removes every cleanly-removable case branch project-wide (including nested patterns like `Just Increment -> ...`), and — in the same call — emits a `references not rewritten` section listing every remaining reference it could not touch: construction sites, equality comparisons, partial applications, and refutable patterns in function/lambda/let arguments. Fix those by hand, then run `elm make` to verify. **This is a one- or two-elmq-touch flow at most**: one call when nothing blocks, and the advisory already gives you everything you need for the follow-up edits — don't chain `elmq refs` into the removal loop.

```bash
$ elmq variant rm src/Types.elm --type Msg Increment
removed Increment from Msg in src/Types.elm
  src/Update.elm:47  update  — removed branch

references not rewritten (2):
  src/Init.elm:15  init      expression-position
      init = ( Model 0, Cmd.map Wrap (Increment 1) )
  src/Debug.elm:8  debugMsg  expression-position
      debugMsg m = m == Increment 0
  run `elm make` to confirm and fix these before continuing
```

When the advisory is empty (every reference was a case branch or a wildcard-covered case), the section is omitted and you can skip straight to `elm make`.

### Auditing a constructor (exploration)

For "where is this constructor used?" outside the removal flow — reviewing whether a variant is still needed, planning a rename, or inspecting a type's call sites — use the regular `elmq refs` command with the constructor name. It classifies every reference into the same categories the rm advisory uses and does **not** mutate anything. Reach for it when you want the question answered independent of removal; **do not chain it into `variant rm`** — the rm advisory already gives you the same information for that case, in the same call.

```bash
$ elmq refs src/Types.elm Increment
Msg.Increment — 3 references (1 clean, 2 blocking)
src/Update.elm
    47  update       case-branch
        Increment ->
src/Init.elm
    15  init         expression-position
        init = ( Model 0, Cmd.map Wrap (Increment 1) )
src/Debug.elm
     8  debugMsg     expression-position
        debugMsg m = m == Increment 0
```

Use `--format json` for machine consumption; the JSON payload has `total_sites`, `total_clean`, `total_blocking`, and a flat `sites` array with `file`, `line`, `column`, `declaration`, `kind`, and `snippet` per entry. Mix decl and constructor names in a single `elmq refs` call and each is framed under its own `## <arg>` header.

## Reference notes

- **Multi-arg framing.** Commands accepting `<name...>` run best-effort per argument. With 2+ args, output is `## <arg>` blocks; errors are inline `error: …` on stdout. Exit `2` if any arg failed.
- **`unexpose` / `import remove` are idempotent** — removing something that isn't there is a no-op, not an error.
