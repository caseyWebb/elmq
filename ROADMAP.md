---
update-when: features are completed or new phases are planned
---

# elmq Roadmap

Query and edit Elm files — like jq for Elm. A next-gen LSP for agents and scripts, optimized for token efficiency.

## Phase 0: Foundation ✓

- Rust CLI scaffold with mise-managed toolchain
- tree-sitter-elm parsing
- `elmq list <file>` — basic declaration listing
- Compact and JSON output formats

## Phase 0.5: File Summary ✓

- Evolve `list` into a grouped file summary (module, imports, types, functions, ports)
- Absorb `list-imports` and `list-exposing` into `list`
- Add `--docs` flag for inline doc comments
- Omit empty sections, remove redundant per-declaration exposed status

## Phase 1: Read Tools ✓

- ~~`list-modules`~~ — deferred (agents find files effectively; not needed yet)
- `get` — extract the full source text of a declaration by name

## Phase 2: Write Tools ✓

- `set` — upsert a top-level declaration (full source from stdin, name parsed or `--name` override)
- `patch` — surgical `--old`/`--new` find-and-replace scoped to a declaration
- `rm` — remove a declaration (including doc comment and type annotation)
- `import add` / `import remove` — manage import clauses
- `expose` / `unexpose` — granularly manage module exposing list
- Round-trip formatting preservation (comments, whitespace outside edited region)
- Atomic file writes (write-to-temp, rename-over-original)

## Phase 3: MCP Server (retired)

Built and later removed. The elmq MCP stdio server (`elmq mcp`, `src/mcp.rs`, `rmcp` SDK) was retired as part of the `drop-mcp-server` change because a Claude Code upstream bug (anthropics/claude-code#24762, #36914) prevents local stdio MCP servers from registering their tools in Claude Code sessions, making the entire surface unreachable from the only harness that matters today. The benchmark harness in `benchmarks/` was reshaped to answer "does elmq save tokens?" via a CLI-oriented oracle arm instead of via MCP.

LLM-harness packaging (MCP, skill, plugin, npm) will return as a dedicated phase once the benchmark proves the token-savings thesis and the data informs which packaging model is worth building.

## Phase 4: Multi-File Operations ✓

- Project-aware mode (elm.json, source-directories) ✓
- `mv` — rename a module and update imports and qualified references across the project ✓
- `refs` — find all references to a module or declaration across the project ✓
- `rename` — rename a declaration and update all references across the project ✓
- `move-decl` — move declarations between modules with import-aware body rewriting, automatic helper detection, and project-wide reference updates ✓
- `variant add`/`variant rm` — add or remove type constructors with project-wide case expression propagation ✓

## Phase 5: Benchmarks and validation (in progress)

The benchmark harness in `benchmarks/` runs Claude Code against `rtfeldman/elm-spa-example` on five sequential Elm editing scenarios (add feature, rename module, extract module, add route, add variant). Two arms:

- **`control`** — Claude works with built-in Read/Write/Edit/Grep, no elmq guidance
- **`treatment`** — Claude gets elmq CLI guidance delivered as `CLAUDE.md` in the treatment workdir (project memory, propagates to spawned `Task`/`Agent` subagents). The guide lives at `.claude-plugin/elmq-guide.md` and is also delivered at runtime via a Claude Code plugin SessionStart hook. The v1 and v2-first-draft runs used `--append-system-prompt-file` instead, which does not reach subagents — see `openspec/changes/elmq-guide-v2/design.md` §"Delivery mechanism" for the evidence that motivated the swap.

The thesis under test (**Q1**): given Claude knows how to use elmq, does it actually save tokens on Elm editing tasks? A positive answer unlocks the follow-up question (**Q2**): which delivery mechanism (plugin, skill, hook, MCP, etc.) is worth investing in to deliver elmq's guidance at runtime?

Planned follow-ups once Q1 has preliminary data:

- `benchmark-independent-scenarios` — replace sequential scenarios with git-tag-based reference checkpoints per scenario (option B from the explore session), eliminating survivorship bias in multi-run averages
- Q2 packaging experiments — skill arm, hook arm, maybe reintroduce MCP if the upstream Claude Code stdio registration bug is fixed

## Phase 6: v2 — guide and CLI improvements from benchmark findings

The first benchmark run (5 scenarios × 3 runs, control vs. treatment with `elmq-guide.md` v1) showed treatment losing overall by ~61% (3.26M vs 2.02M tokens) while winning decisively on the one scenario with a perfect one-shot command (`02-rename-module`, −84% via `elmq mv`). Analysis of the tool-call traces surfaced a set of guide rules and CLI affordances that should flip the losing scenarios. Each bullet below will get its own OpenSpec change as we pick it up.

### 6a. Guide v2 (`.claude-plugin/elmq-guide.md`) and delivery-mechanism change

The initial Phase 6a draft (preserved in git history) proposed an eight-rule guide organized into tiers. Mid-implementation the guide was substantially tightened under a scope constraint: *the guide describes elmq — what it does, what it replaces, how to invoke each subcommand, and factual gotchas about elmq's runtime behavior. It does not prescribe agent workflow (planning, subagent trust, shell hygiene) and it does not expose the benchmark's metrics (tool-call counts, cache-read cost models) to the agent.* The `elmq-guide-v2` change captures the full derivation and the rules-considered-and-cut rationale.

What actually shipped in v2 (66 lines, ~914 tokens, smaller than v1 by ~60 tokens after the scope cut and then slightly expanded for failure-mode-B fixes):

- **The load-bearing edit.** Replace v1's *"`elmq list` is your reconnaissance tool — use it every time"* with *"`elmq list` and `elmq get` are targeted exploration tools — use them on files and declarations you need to understand, not carte blanche on the whole project."* The v1 "every time" language produced 40+ pre-mutation reconnaissance calls on losing scenarios.
- **Widened Grep rule** covering `grep` / `rg` via `Bash` on `.elm` files, not just the `Grep` tool.
- **Expanded decision table** with precise flag syntax for `move-decl`, `variant add`/`rm`, and `rename`.
- **`move-decl` extraction phrasing.** The decision-table entry for `elmq move-decl` now reads "Extract declarations into a new module, or move declarations between modules" and notes that `move-decl` creates the `<target>` if it doesn't exist. Added after the `T224012-treatment-1` run showed the main agent skipping `move-decl` on scenario 03 because the original phrasing "Move declarations between modules" didn't match the task's mental framing as extraction.
- **Concrete `move-decl` worked example** after the decision table, showing a four-`--name`-flag extraction into a new target file. Factual description of what the command does.
- **One `variant add` gotcha:** inserted branches are `Debug.todo "<VariantName>"`, so `get` the destination before trying to `patch` them.
- **Demoted from v1**: `patch`-vs-`set` framing reduced to one sentence in the appendix (~8% of the v1 gap per the per-call-cost analysis).
- **Dropped from v1**: the "no python/node fallback" rule (obsoleted by `benchmark-results-dir-rename`), and the Phase 1/2/3/4 workflow section.
- **Considered but cut on scope grounds**: plan-then-execute gate, trust-the-Explore-subagent rule, trust-exit-codes, batch-with-`&&`, always-variant-add as a separate rule, all tool-call-count / cache-read cost-model exposition. Each of these targets a real observed failure mode but prescribes agent behavior rather than describing elmq. See `openspec/changes/elmq-guide-v2/design.md` §"Rules considered and cut" for the per-rule rationale.

Delivery-mechanism change (new in v2, not present in the original Phase 6a plan):

- **CLAUDE.md instead of `--append-system-prompt-file`.** Mid-implementation smoke test on `2026-04-09T224012-treatment-1` revealed that `--append-system-prompt-file` does not propagate to subagents spawned via the `Task` tool — the Explore subagent in treatment 01/04 made 5 `Read` calls each on `.elm` files because it never saw the guide. Structural fix, not a wording fix: write the guide as `$work_dir/CLAUDE.md` for the treatment arm in `run.sh`, because CLAUDE.md is project memory loaded from cwd on every Claude invocation (including subagents). Verified on `2026-04-09T230100-treatment-1/01-add-feature`: subagent made 14 elmq calls, zero Reads. Control arm byte-for-byte unchanged (the `cp` is gated on `$arm == "treatment"`).

### 6b. CLI improvements (ordered by expected impact)

- ✓ **`elmq grep <PATTERN> [PATH]` — landed.** Step-0 discovery locator: regex search over Elm sources (Rust regex, like ripgrep) that annotates each hit with its enclosing top-level declaration, so `elmq grep | elmq get` closes the find-then-retrieve loop without falling back to `rg` + whole-file reads. Filters comments and string literals by default; honors `.gitignore` and walks up for `elm.json` (or recursively walks CWD when none is found). Exit codes match rg. Captured in `openspec/changes/add-grep-command`.
- ✓ **`elmq variant cases` + `variant add --fill` — landed.** Driven by the `2026-04-10T001716` benchmark run, which showed treatment burning +104% raw tokens on `05-add-variant` because the model made ~14 follow-up `elmq patch` calls to fill each `Debug.todo` stub inserted by `variant add`. New read-only `variant cases <file> --type <T>` walks the project and emits every case expression on the target type with its enclosing function body (signature + implementation) and a stable per-site key, using progressive qualification (`function` → `function#N` → `file:function` → `file:function#N`) so bare function names work in the common case and ambiguity surfaces as a pre-write error listing the disambiguated alternatives. New repeatable `--fill <key>=<branch>` flag on `variant add` replaces the default `Debug.todo "<Variant>"` stub with the provided branch text at matching sites; unfilled sites fall back to `Debug.todo` (graceful degradation), unknown or ambiguous fill keys error before any file is touched. Collapses the common "add variant → patch N stubs → compile" flow from ~14 turns to 3 (`cases` → `add --fill` → `elm make`). Captured in `openspec/changes/variant-fill`.
- ✓ **`elmq get -f <FILE> <NAME>... [-f ...]`** — landed. Multi-file get in one invocation. Repeatable `-f`/`--file` flag groups a file path with one or more declaration names; multiple groups span files. Output framed as `## Module.decl` when `elm.json` is discoverable, `## file:decl` fallback otherwise. Collapses the `list` → N×`get` read-phase into a single turn. Backward-compatible with existing `get FILE NAME...` form. Captured in `openspec/changes/add-get-multi-file`.
- ✓ **`elmq grep --definitions --source`** — landed. Two composable flags on `grep`. `--definitions` filters to matches at the declaration name site (byte-range coincidence with the identifier node, not call sites). `--source` replaces per-match locator lines with full declaration source blocks, deduped by `(file, decl)`, framed `## Module.decl` (single block bare). Together: `grep --definitions --source 'update'` is a one-call definition lookup. Captured in `openspec/changes/add-grep-source-flags`.
- ✓ **Constructor-aware `elmq refs` + `variant rm` advisory — landed.** `variant rm` under the old contract only handled `case_of_expr` branches and silently ignored every other reference site — expression-position construction (`Increment 1`, `List.map Wrap xs`, `msg == Increment 0`), refutable patterns in function/lambda/let arguments, and nested `union_pattern` branches like `Just Increment -> ...` (a latent bug where `find_constructor_in_pattern` did not recurse through `nullary_constructor_argument_pattern` wrappers). New shared `collect_constructor_sites()` classifier walks every `upper_case_qid` with an ancestor-field context that resets at expression-body boundaries and is set by pattern-hosting fields (`case_of_branch.pattern`, `function_declaration_left.pattern`, `anonymous_function_expr.param`, `value_declaration.pattern`), classifying every resolved site into `case-branch`, `case-wildcard-covered`, `function-arg-pattern`, `lambda-arg-pattern`, `let-binding-pattern`, or `expression-position`. A second targeted pass detects `case_of_expr` nodes whose wildcard branch already covers the removed constructor. `variant rm` consumes the classifier twice: the cleanly-removable sites feed the existing iterative re-parse branch-removal loop (and the nested-pattern bug fix means it now catches `Just Increment` too), and every non-clean site is surfaced as a `references_not_rewritten` advisory section in the same call so the agent gets the same data without a follow-up tool call. The top-level `elmq refs <file> <name>...` command now auto-routes each name: decls continue through today's `find_refs_batch` path, while constructors of a type declared in the file route through the classifier and emit the same classified report the rm advisory uses. Decl and constructor names can be mixed in a single call, framed under `## <arg>` headers. No separate `variant refs` surface — one refs command, one concept. Deliberately **advisory, not gating** — `variant rm` still writes its files when non-clean references exist, because the Elm compiler is the authority on type correctness and a refusal-to-mutate design would (a) duplicate `elm make`, (b) force an extra tool round-trip on every non-clean removal, and (c) break down on projects that are already mid-refactor with unrelated errors. The advisory is elmq's structured hint; `elm make` is the verification. One- or two-elmq-touch flow at most (one when no blockers, two when the agent fixes the advisory list in between). Captured in `openspec/changes/archive/2026-04-13-variant-refs`.
- ✓ **Write-safety gate on every mutation — landed.** Every elmq command that mutates Elm source (`set`, `patch`, `rm`, `import add`/`remove`, `expose`/`unexpose`, `mv`, `rename`, `move-decl`, `variant add`/`rm`) now performs two tree-sitter checks at the tool boundary: an input-side gate that refuses to edit any file with pre-existing ERROR/MISSING nodes, and an output-side gate via a new `writer::validated_write` choke point that re-parses every buffer before `atomic_write` and rejects ones that would not parse, naming the file, operation, and first-error `line:col`. Read commands (`list`, `get`, `grep`, `refs`, `variant cases`, `guide`) keep their existing warn-and-continue behavior so broken files remain inspectable. Multi-file commands validate per file and may leave partial writes on failure (explicit non-goal: no cross-file staging). No new dependencies, no new flags, no subprocesses — pure in-process tree-sitter, consistent with elmq's Unix-philosophy stance that downstream pipelines (`elm make`, `elm-format --validate`, tests) own semantic validation. Uncovered and fixed three pre-existing splicing bugs in the same pass: `move-decl` produced invalid `exposing ()` when every originally-exposed decl was moved; `variant rm`'s branch removal ate the newline between `case msg of` and the next branch; `variant add` appended new variants at column 1 for single-line type declarations. **BREAKING** for any workflow that relied on elmq editing a file with pre-existing parse errors. Captured in `openspec/changes/reject-invalid-write-output`.
- **`elmq new <Module.Path> [--expose ...]`** — create a new module file with correct header. Eliminates the `Write`→`cat`→`touch`→`python`→`node` file-creation loop observed in both arms on 01/03/04.
- **Multi-arg read and write commands** — `elmq list file1 file2 ...`, `elmq get file decl1 decl2 ...`, `elmq refs file decl1 decl2 ...`, `elmq rm file decl1 decl2 ...`, `elmq expose file sym1 sym2 ...`, `elmq unexpose file sym1 sym2 ...`, `elmq import add file 'Mod1' 'Mod2 as M2' ...`, `elmq import remove file Mod1 Mod2 ...`, and `elmq move-decl src/Api.elm --to src/Api/Cred.elm decl1 decl2 decl3` (multi-decl extract, positional names replacing the `--name` flag — **breaking**). Output uses `## <arg>` header blocks per item, with single-arg calls keeping today's bare output. Batching is **best-effort per argument**, not all-or-nothing: per-item errors render inline as `error: <msg>` in that item's block, successful items still land, and the process exits `2` if any argument failed. This is a deliberate reversal of the original "all-or-nothing" framing in an earlier draft of this roadmap — fail-fast on writes would force the agent to retry the rest of the batch after a single typo, giving back the round-trip savings the batching is supposed to capture. `refs` batching additionally walks the project exactly once regardless of name count, turning the per-call-cost win into a real parse-work win. Writes are one parse + one atomic write per file. Captured in `openspec/changes/batch-positional-args`.
- **Better `patch` match-failure errors.** On `--old` mismatch, show the closest candidate region in the file so the agent can correct without re-`get`ing and guessing.
- **`elmq set` soft-warning when it looks like a `patch`.** If the new content differs from the existing decl by only a small insertion/replacement, emit a stderr hint: `consider 'elmq patch' — only N lines differ`. Non-blocking.
- **Idempotent `expose`/`unexpose` and `import remove`.** `expose Cred(..)` when `Cred` is already exposed should upgrade cleanly; no error, no duplicate. Reverse for `unexpose`. `import remove` on a missing import is likewise a no-op. Bundled into `batch-positional-args` because the batching design depends on the idempotent write path — a fail-fast bail on a missing item would poison any batch. This is a **breaking** change to the single-argument surface for `unexpose` and `import remove`, which previously errored on missing items.
- **`elmq refs --unused <file>`** — list imports in a file no longer referenced. Helps post-extraction cleanup.

Multi-arg `patch` and `set` are deferred — both consume stdin, which would need new framing. Not part of `batch-positional-args`.

### 6c. Benchmark harness fix

- ✓ **New-file-creation flakiness — resolved.** Root cause was a Claude Code false-positive in the `Write` tool's path-safety heuristic: the results directory timestamp `T19:29:08` matched the Windows drive-letter pattern (`T:`), causing every `Write` call to be silently rejected as "contains a suspicious Windows path pattern that requires manual approval." The agent then fell through to a `Bash(cat > EOF)` → `python3` → `node` loop. Fix was to remove colons from the timestamp format (`%Y-%m-%dT%H:%M:%S` → `%Y-%m-%dT%H%M%S`) in `benchmarks/run.sh`. Smoke-tested on `2026-04-09T202526-treatment-1/01-add-feature`: `Write` succeeds, zero fallback-loop fingerprints, 65 → 38 tool calls and ~−19% all-billable tokens vs the pre-fix run on the same scenario. See `openspec/changes/benchmark-results-dir-rename`.

### Sequencing

Based on the per-call-cost analysis of the first benchmark run, sequencing has been revised. Phase 6's assumption that output-token savings (e.g. `patch` vs `set`) would dominate was incorrect — treatment and control output tokens are within 8% of each other. The entire gap is tool-call count × cache-read-per-call (~15–20k per call). Two load-bearing things come out of this: (1) the `elmq list` / `elmq get` framing in the guide ("targeted, not carte blanche"), and (2) the delivery-mechanism swap to CLAUDE.md so the guide reaches spawned subagents at all. See `openspec/changes/elmq-guide-v2/design.md` for the per-call-cost derivation and the subagent-propagation smoke test evidence.

1. ✓ Fix new-file-creation flakiness (`benchmark-results-dir-rename`) — landed, Write tool unblocked.
2. Ship guide v2 (`elmq-guide-v2`) with reprioritized rules and re-run. Expected to flip overall delta from +61% to approximately −10% to −25%, driven mostly by 01 and 04 via the reconnaissance discipline rule.
3. Add a held-out scenario (`benchmark-heldout-scenario`) after guide v2 stabilizes, to guard Q1 against silent overfitting in future iterations.
4. ✓ **`batch-positional-args` — landed.** Multi-arg read/write commands, `move-decl --name` → positional (breaking), idempotent `unexpose`/`import remove` (breaking), `refs` single-walk batching (normative), `## <arg>` multi-arg output framing with N=1 bare. 38 tasks / 230 tests green. Pending archive. Follow-ups still on the Phase 6b list: better `patch` mismatch errors, `elmq new`, `elmq refs --unused`. Batching is expected to widen the win on 03-extract-module and compound with guide v2's reconnaissance-discipline rule across 01 and 04 — benchmark re-run is gated on the guide being updated to use the new forms.

## Phase 7: Advanced

- Type-aware queries (find functions matching a type signature)
- Unused import/declaration detection
- elm-format integration
- Shell completions
