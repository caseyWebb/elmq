# Working with `.elm` files: use `elmq`

This is an Elm project. `elmq` is on PATH — a tree-sitter-aware CLI for reading and editing `.elm` files.

- **Never use `cat` on `.elm` files.** It dumps the entire file into context with no structure.
- Do not use `Grep`, or `grep` / `rg` via `Bash`, to search Elm code. Use `elmq grep` for text discovery (returns the enclosing decl for free) and `elmq refs` for structural references through the import graph.
- For **project-wide operations** (rename module, move declarations, add/remove variants), always use the dedicated `elmq` commands — they handle the entire dependency graph atomically.
- For **single-file edits**, choose by file size:
  - **Under ~300 lines** (check `elmq list` — it shows line counts): `Read` + `Edit` is simplest. Fewer round trips than `elmq get` + `elmq patch`.
  - **Over ~300 lines**: use `elmq get` to read specific declarations and `elmq patch`/`set decl`/`set let`/`set case` to edit them. Avoids pulling the full file into context.
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
- `elmq refs <file> <name...>` — every project reference to one or more declarations in this file. Each name dispatches on what it resolves to: a top-level declaration produces a flat list of reference sites; a **constructor** of a custom type declared in the file produces a **classified** report (`case-branch`, `case-wildcard-covered`, `function-arg-pattern`, `lambda-arg-pattern`, `let-binding-pattern`, `expression-position`) with clean/blocking counts — the same data that `rm variant` surfaces in its `references not rewritten` advisory. You can mix decl and constructor names in one call; each is framed under a `## <arg>` header.

## Editing Elm code

**Every write command is `<verb> <scope>`.** One rule: "what verb, what scope?" The verbs are `set`, `rm`, `rename`, `add`; the scopes name what the verb acts on (`decl`, `let`, `case`, `arg`, `variant`, `import`). Writes without a natural scope — `patch`, `expose`, `unexpose`, `mv`, `move-decl` — stay flat.

**Confirmation-only writes.** Every write command prints `ok` on success and an explanatory error to stderr on failure. The sole exception is `rm variant`, which keeps its `references_not_rewritten` advisory block because the information is actionable and not recoverable from a subsequent read.

**Work in two phases: plan all edits, then apply them in as few tool calls as possible.** Each tool call is a conversation turn that re-reads the full context. Fewer turns = lower cost.

1. **Plan**: read the code you need (`elmq list`, `elmq get`, `Read`), decide on all the changes
2. **Apply**: execute all edits in one or two Bash calls using `&&` chains, then `elm make` once at the end

**Trust your edits.** If `elmq` exited `0`, it applied exactly what you asked for. **Never re-read a file or declaration after a successful edit** — each re-read is a wasted turn. Only re-read when the compiler reports an error you need to diagnose.

**Write commands refuse to touch a broken file.** Every write subcommand parses the target with tree-sitter before doing anything. If tree-sitter finds an ERROR/MISSING node, elmq exits non-zero with `refusing to edit <path>: file has pre-existing parse errors at <line>:<col>` and writes nothing. Write commands also re-parse their own output before committing it to disk; if an operation would produce syntactically invalid Elm, you get `rejected '<op>' write to <path>: output would not parse at <line>:<col>` and the file on disk is unchanged.

### Command reference

**Project-wide** — these handle the entire dependency graph atomically. **Never manually patch a type definition to add/remove a variant** — use `add variant` / `rm variant`, which propagate through every case expression in the project:

| Intent | Command |
|---|---|
| Rename/move a module | `elmq mv <file> <new_path>` (`--dry-run` to preview) |
| Rename a top-level declaration | `elmq rename decl <file> <old> <new>` |
| Extract/move declarations between modules | `elmq move-decl <src> --to <target> <name...>` |
| Add a type variant + fill case branches | `elmq add variant <file> --type <T> '<Variant>' [--fill <key>=<branch>]...` |
| Remove a type variant | `elmq rm variant <file> --type <T> <Constructor>` (emits an advisory list of any non-case references it could not rewrite) |

**Top-level decl edits** — chain these with `&&` when you have several:

| Intent | Command |
|---|---|
| Find-replace inside a declaration | `elmq patch --old '…' --new '…' <file> <name>` |
| Upsert a top-level declaration | `elmq set decl <file> [--name <N>] --content '<source>'` (or pipe source on stdin) |
| Remove top-level declarations | `elmq rm decl <file> <name...>` |
| Add imports | `elmq add import <file> '<clause>'...` |
| Remove imports | `elmq rm import <file> <module...>` |
| Add to the exposing list | `elmq expose <file> <item...>` |
| Remove from the exposing list | `elmq unexpose <file> <item...>` |

**Sub-declaration edits** — edit _inside_ a top-level declaration without rewriting it. These are the right tool for small surgical changes that would otherwise cost a full `set decl` round-trip:

| Intent | Command |
|---|---|
| Upsert a let binding (body-only edit preserves the sig automatically) | `elmq set let <file> <decl> --name <N> --body '<expr>' [--type <T>] [--params '<p1 p2>'] [--no-type] [--after <sib>\|--before <sib>] [--line <N>]` |
| Upsert a branch in a case expression | `elmq set case <file> <decl> --pattern '<P>' --body '<expr>' [--on <scrutinee>] [--line <N>]` |
| Remove let bindings | `elmq rm let <file> <decl> <name...> [--line <N>]` |
| Remove case branches | `elmq rm case <file> <decl> --pattern '<P>'... [--on <scrutinee>] [--line <N>]` |
| Remove function parameters | `elmq rm arg <file> <decl> --at <N>... \| --name <N>...` |
| Rename a let binding (updates refs within the enclosing decl) | `elmq rename let <file> <decl> --from <old> --to <new> [--line <N>]` |
| Rename a function parameter | `elmq rename arg <file> <decl> --from <old> --to <new>` |
| Add a function parameter | `elmq add arg <file> <decl> --at <N> --name <name> [--type <T>]` |

### `--content` vs `--body`

Two inline content flags, each for a different shape of input:

- **`--content <TEXT>`** — inline alternative to stdin for commands whose content is a whole source chunk. Used by `set decl`.
- **`--body <EXPR>`** — inline alternative to stdin for commands whose content is a single RHS expression. Used by `set let` and `set case`.

Exactly one of the flag or stdin must be provided per invocation. Inline is shorter for one-liners; stdin/heredoc is better for multi-line content.

```bash
# inline --content
elmq set decl src/Main.elm --content 'view = Html.text "hi"'

# stdin heredoc
elmq set decl src/Main.elm << 'ELM'
view : Model -> Html Msg
view model =
    Html.text (String.fromInt model.count)
ELM

# inline --body
elmq set let src/Main.elm update --name helper --body 'n + 2'
```

### `--name` mismatch is an error

On `set decl`, if `--name` is passed AND the content has a parseable name that differs from it, the command errors without writing. This prevents accidental rename-via-upsert. **To rename, use `rename decl` or `rename let`** — they update references project-wide (for `rename decl`) or within the enclosing decl (for `rename let`).

### Addressing discipline: name/pattern primary, --line escalation

Sub-decl commands address their target by name (`--name helper`), pattern (`--pattern Increment`), or scrutinee (`--on msg`) — all content-addressed and derivable from `elmq get` output you already have.

When addressing is ambiguous (multiple let blocks contain `helper`, multiple case expressions on `msg`, etc.), the command errors and lists candidate **absolute file lines** in stderr with a `retry with --line <N>` hint. Absolute lines match `elmq get`'s output exactly, so no mental arithmetic is needed when copying between commands.

**Chained content-addressed ops don't suffer line drift**, because `--name` / `--pattern` re-resolve after each edit. Use `--line` only when you specifically need to disambiguate.

### Decision tree: changing a let binding

- Body-only edit on a typed binding → `set let --name X --body '…'` (sig preserved automatically)
- Change the sig → `set let --name X --type '…' --body '…'`
- Remove the sig → `set let --name X --no-type --body '…'`
- Rename the binding → `rename let --from X --to Y`
- Find-replace inside the body → `patch` (still preferred for deterministic string substitutions)

### Decision tree: changing a case branch

- Replace an existing branch body → `set case --pattern X --body '…'`
- Add a new branch → `set case --pattern Y --body '…'` (appended before any wildcard branch)
- Remove a branch → `rm case --pattern X`
- `set case` errors if the enclosing decl has no case expression; use `set decl` to rewrite.

### Compiler-guided cleanup for `add arg` / `rm arg`

`add arg` and `rm arg` only update the target function's signature and definition. Call sites across the project are intentionally not touched — `elm make` flags them and you fix each with `patch` or `set decl`. Same discipline as `rm variant`'s advisory surface. If the function has a signature, `add arg --type` is required; if it doesn't, `--type` is optional (silently ignored).

### Chaining edits

You've already read the source — the old strings are deterministic. Chain confidently:

```bash
elmq patch --old '...' --new '...' src/Route.elm Route && \
elmq set let src/Main.elm update --name handler --body 'processMsg model msg' && \
elmq add import src/Main.elm 'Page.Bookmarks as Bookmarks' && \
elmq expose src/Page.elm Bookmarks
```

### Extracting declarations into a new module

`elmq move-decl` handles extraction in one call — creates the target file, moves the declarations, updates `exposing (…)` lists, and rewrites every qualified reference project-wide:

```
elmq move-decl src/Types.elm \
  --to src/Types/Auth.elm \
  Token validate decode encode
```

Do not manually reconstruct this with `Write` + `rm decl` + `add import` + find-and-replace.

### Adding a variant (plan then apply)

This is the plan-then-apply pattern in action. Two turns:

**Turn 1 (plan):** `elmq variant cases <file> --type <T>` — returns every case site with its enclosing function body and a stable `key`. (This is the only remaining noun-first command; it's a read op.) Read the output and compose your `--fill` arguments for every key. This also tells you which sites are case expressions (`--fill` handles those) vs list-based dispatch (you'll `elmq patch` those).

**Turn 2 (apply):** `add variant` with all `--fill` args, chained with `elmq patch` for non-case sites:

```bash
elmq add variant src/Theme.elm --type Theme 'HighContrast' \
    --fill 'toClass=HighContrast -> "theme-high-contrast"' \
    --fill 'toCssVars=HighContrast -> highContrastVars' && \
elmq patch --old '...' --new '...' src/Theme.elm allThemes
```

**Do not call `add variant` without `--fill` and then patch each `Debug.todo` stub afterward** — that turns 1 tool call into many.

`--fill` keys must come from `variant cases` output. `--fill` only targets case expressions — functions using list-based dispatch won't appear as case sites; chain `elmq patch` for those. When one function has two case expressions on the same type, or two files both define a function with the same name, `cases` prints disambiguated keys (`handler#1`, `handler#2`, or `src/Other.elm:handler`) — pass those through.

`--fill` values are `KEY=BRANCH` pairs split on the **first** `=`, so branch bodies containing `=` (e.g. record updates) are fine.

### Removing a variant

`rm variant` is a single self-contained call. It strips the constructor from the `type` declaration, removes every cleanly-removable case branch project-wide (including nested patterns like `Just Increment -> ...`), and — in the same call — emits a `references not rewritten` section listing every remaining reference it could not touch: construction sites, equality comparisons, partial applications, and refutable patterns in function/lambda/let arguments. Fix those by hand, then run `elm make` to verify.

This advisory block is the **documented exception** to the confirmation-only write-output rule — the classifier output is not recoverable from a `refs` call after the edit, so it has to ship inline.

```bash
$ elmq rm variant src/Types.elm --type Msg Increment
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

For "where is this constructor used?" outside the removal flow — reviewing whether a variant is still needed, planning a rename, or inspecting a type's call sites — use `elmq refs` with the constructor name. It classifies every reference into the same categories the `rm variant` advisory uses and does **not** mutate anything. Reach for it when you want the question answered independent of removal; **do not chain it into `rm variant`** — the rm advisory already gives you the same information in a single call.

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

- **Multi-arg framing.** Commands accepting `<name...>` (for example `rm decl <file> <name...>`) run best-effort per argument. With 2+ args, output is `## <arg>` blocks; errors are inline `error: …` on stdout. Exit `2` if any arg failed.
- **`unexpose` / `rm import` are idempotent** — removing something that isn't there is a no-op, not an error.
- **Absolute file lines** — every `--line` flag across every command is an absolute file line, matching `elmq get`'s output. No mental arithmetic when copying line numbers between commands.
- **Multi-target write commands** — `rm decl`, `rm let`, `rm case`, `rm arg`, `rm variant`, `rm import`, `expose`, `unexpose`, and `add import` accept multiple targets per invocation; `set`/`rename`/`add arg` are single-target per call. Batches are all-or-nothing for validation where that makes sense (sub-decl `rm`).
