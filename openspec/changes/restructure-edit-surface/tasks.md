## 1. CLI scaffolding: verb-first command tree

- [x] 1.1 Restructure `src/cli.rs` `Command` enum: introduce subcommand groups for `Set`, `Rm`, `Rename`, `Add` with inner scope subcommands (`Decl`, `Let`, `Case`, `Arg`, `Variant`, `Import` as applicable). `Patch`, `Expose`, `Unexpose`, `Mv`, `MoveDecl`, and all read commands stay flat.
- [x] 1.2 Define flag structs for each new inner subcommand: `SetLet`, `SetCase`, `RmLet`, `RmCase`, `RmArg`, `RenameLet`, `RenameArg`, `AddArg`. Each flag set matches the proposal's documented shape.
- [x] 1.3 Remove bare-form `Set`, `Rm`, `Rename` variants (they become `Set { decl | let | case }`, `Rm { decl | let | case | arg | variant | import }`, `Rename { decl | let | arg }`).
- [x] 1.4 Remove `Variant` and `Import` top-level command groups; fold their add/rm subcommands into `Add` and `Rm`. `variant cases` becomes a bare top-level `VariantCases` command (or equivalent) — it's the only remaining noun-first op and it's a read op.
- [x] 1.5 Add `--content` flag to `set decl` as stdin alternative; enforce exactly-one-of stdin/`--content` at parse time or in the dispatcher.
- [x] 1.6 Add `--body` flag to `set let` and `set case` as stdin alternative; same exactly-one-of rule.
- [x] 1.7 Snapshot-test or manually verify `elmq --help` and each subcommand `--help` render correctly and read clearly.

## 2. `src/analysis.rs` module

- [x] 2.1 Create `src/analysis.rs`, add to `lib.rs`.
- [x] 2.2 Move `compute_site_keys` from `src/variant.rs` to `src/analysis.rs`; update `variant.rs` to import it. No behavior change.
- [x] 2.3 Implement `collect_let_sites(decl_node, source) -> Vec<LetSite>` where `LetSite { name, line, scope_path, node_span }`. Walks `let_in_expr` nodes within the decl; for each `value_declaration` child, records the binding name, the absolute file line of the binding's start, and the chain of enclosing binding names.
- [x] 2.4 Implement `collect_case_sites_generic(decl_node, source) -> Vec<CaseSite>` where `CaseSite { scrutinee_text, line, branches: Vec<CaseBranch> }` and `CaseBranch { pattern_text, body_span, line }`. Walks `case_of_expr` nodes within the decl.
- [x] 2.5 Implement `collect_binders_in_scope(decl_node, position) -> HashSet<String>` that returns every name visible at `position` within the declaration: function args, outer let bindings, case pattern vars. Flat set (no-shadowing makes this sufficient).
- [x] 2.6 Unit tests in `src/analysis.rs`: (a) nested let bindings with duplicate names produce distinct sites, (b) case sites with tuple scrutinees and nested patterns, (c) binders-in-scope reports function args and outer let bindings but not sibling let bindings in separate scopes.

## 3. Parser helpers for sub-decl content validation

- [x] 3.1 Add `parser::parse_let_binding(source: &str) -> Result<LetBindingInfo>` that parses a standalone let-binding source string (optional sig + definition) and returns the parsed name, params, type annotation, and body expression range. Used by `set let` to validate `--body` content and to extract the parsed name for `--name` mismatch checks.
- [x] 3.2 Add `parser::parse_case_branch_body(source: &str) -> Result<()>` that verifies a case branch body expression parses cleanly. Used by `set case` to validate `--body` content pre-flight.
- [x] 3.3 Add `parser::parse_arg_type(source: &str) -> Result<()>` that verifies a type expression parses cleanly. Used by `add arg` to validate `--type` before splicing into the signature's arrow chain.
- [x] 3.4 Unit tests for each parser helper covering clean inputs, syntactically invalid inputs, and edge cases (multi-line expressions, trailing whitespace, type annotations with constraints).

## 4. Writer functions: `set let` / `rm let` / `rename let`

- [x] 4.1 `writer::upsert_let_binding(source, summary, decl_name, binding_spec) -> Result<String>` where `binding_spec` carries `name`, optional `type`, optional `params`, `body`, and positioning hints (`after`/`before`/`line`). Handles update (existing binding) and insert (new binding) paths. Preserves sig when `type` is None on update; removes sig when `no_type` flag is set.
- [x] 4.2 `writer::remove_let_binding(source, summary, decl_name, binding_name, line_hint) -> Result<String>`. Removes the binding's signature (if present) and definition. Handles whitespace cleanup.
- [x] 4.3 `writer::rename_let_binding(source, summary, decl_name, old, new, line_hint) -> Result<String>`. Renames the binding and rewrites every reference within the enclosing decl. Uses `analysis::collect_binders_in_scope` to check for new-name collisions before writing.
- [x] 4.4 `writer::remove_let_bindings_batch(source, summary, decl_name, names) -> Result<String>` for the multi-target `rm let` case. Resolves all names up front (all-or-nothing validation), processes rear-to-front by line to avoid byte-offset drift.
- [x] 4.5 Unit tests for each function covering: value bindings, function bindings, typed vs untyped, positioning flags on insert, positioning flags on upsert (move semantic), signature preservation, sig removal via `--no-type`, ambiguity errors with candidate lines.

## 5. Writer functions: `set case` / `rm case`

- [x] 5.1 `writer::upsert_case_branch(source, summary, decl_name, case_spec) -> Result<String>` where `case_spec` carries `on` (scrutinee), `pattern`, `body`, and `line`. Handles pattern-match-existing (replace body) and no-match (append new branch before wildcard if present) paths. Pattern matching is byte-exact after whitespace trim.
- [x] 5.2 `writer::remove_case_branch(source, summary, decl_name, on, pattern, line) -> Result<String>`. Removes the branch and cleans up excess whitespace. Errors if removing the branch would leave the case empty.
- [x] 5.3 `writer::remove_case_branches_batch(source, summary, decl_name, on, patterns) -> Result<String>` for multi-target `rm case`. All-or-nothing validation.
- [x] 5.4 Unit tests: simple variant patterns, patterns with binders (`Just x`), tuple patterns, wildcard-branch handling, scrutinee ambiguity errors with candidate lines.

## 6. Writer functions: `add arg` / `rm arg` / `rename arg`

- [x] 6.1 `writer::add_function_arg(source, summary, decl_name, at, name, type_opt) -> Result<String>`. Inserts the parameter name into the definition at position `at` (1-indexed); if the declaration has a type signature, inserts `type_opt` into the signature's arrow chain at the same position. If no signature and `type_opt` is Some, record a note but do not modify. Errors if `at` > current-arg-count + 1.
- [x] 6.2 `writer::remove_function_arg(source, summary, decl_name, target) -> Result<String>` where `target` is either `Position(n)` or `Name(s)`. Removes from both signature (if present) and definition. Errors if no match.
- [x] 6.3 `writer::remove_function_args_batch(source, summary, decl_name, targets) -> Result<String>` — all positions or all names (not mixed), all-or-nothing validation, processed rear-to-front for positions.
- [x] 6.4 `writer::rename_function_arg(source, summary, decl_name, old, new) -> Result<String>`. Renames the parameter in the definition and every reference in the function body. Checks collision with other binders in scope via `analysis::collect_binders_in_scope`.
- [x] 6.5 Unit tests: typed and untyped functions, `--at 1` (prepend), `--at N+1` (append), `--at N+2` (error), multi-arg positional remove with index-shift correctness, multi-arg name remove, rename collision detection.

## 7. Dispatch rewrite in `src/main.rs`

- [x] 7.1 Match the new `Command` enum shape. Split write-path dispatch into `handle_set`, `handle_rm`, `handle_rename`, `handle_add` functions, each taking its subcommand enum and dispatching to the appropriate writer function.
- [x] 7.2 `set decl` dispatch: read content from `--content` or stdin (error if both or neither); parse name; if `--name` given, compare to parsed name and error on mismatch; call `writer::upsert_declaration`.
- [x] 7.3 `set let` dispatch: read content from `--body` or stdin; parse as a let binding via `parser::parse_let_binding`; check `--name` mismatch; call `writer::upsert_let_binding`.
- [x] 7.4 `set case` dispatch: read content from `--body` or stdin; validate via `parser::parse_case_branch_body`; call `writer::upsert_case_branch`.
- [x] 7.5 `rm let`/`rm case`/`rm arg` dispatch: handle multi-target validation and call the batch writers.
- [x] 7.6 `rename let`/`rename arg`/`add arg` dispatch: validate inputs, call the writers.
- [x] 7.7 `add variant`/`rm variant`/`add import`/`rm import`/`rm decl`/`rename decl` dispatch: call the existing writer functions (variant.rs, writer.rs) — no behavior change, just the new command name.
- [x] 7.8 All write dispatches end with `println!("ok")` on success. Confirmation-only output.
- [x] 7.9 Remove the existing `Command::Set`, `Command::Rm`, `Command::Rename`, `Command::Variant`, `Command::Import` match arms and their helper functions (`run_rm`, `run_import_add`, etc.) — replaced by the new dispatch structure.

## 8. Preserve `rm variant` advisory output exception

- [x] 8.1 Under the new `rm variant` command (was `variant rm`), continue emitting the `references_not_rewritten` advisory section on stdout as today. Update the spec and docs to note this as the sole exception to the confirmation-only write-output rule.
- [x] 8.2 Verify no other existing write command emits advisory-class output that would be lost by the confirmation-only rule. Audit `expose`/`unexpose`/`mv`/`move-decl`/`variant add` output and confirm they can be reduced to `ok`.

## 9. Integration tests — new sub-decl commands

- [x] 9.1 `tests/sub_decl_let.rs`: full coverage of `set let`, `rm let`, `rename let`. Include:
  - (a) body-only edit on typed binding preserves sig
  - (b) `--type` on update replaces sig
  - (c) `--no-type` removes sig
  - (d) value binding insert
  - (e) function binding insert with `--params`
  - (f) `--after`/`--before` on insert and on upsert (move semantic)
  - (g) ambiguity error with candidate absolute lines
  - (h) `--line` resolves ambiguous target
  - (i) `rm let` multi-target all-or-nothing validation
  - (j) `rename let` collision with in-scope name errors
- [x] 9.2 `tests/sub_decl_case.rs`: full coverage of `set case`, `rm case`. Include:
  - (a) replace existing branch body
  - (b) add new branch (appended before wildcard)
  - (c) scrutinee ambiguity with `--on` resolution
  - (d) scrutinee + `--line` disambiguation
  - (e) multi-target `rm case`
  - (f) pattern with spaces / nested constructors
- [x] 9.3 `tests/sub_decl_arg.rs`: full coverage of `add arg`, `rm arg`, `rename arg`. Include:
  - (a) typed function requires `--type`, errors without
  - (b) untyped function accepts `--at` alone
  - (c) multi-position `rm arg` with rear-to-front correctness
  - (d) multi-name `rm arg`
  - (e) `rename arg` updates body references
  - (f) `--at` boundary conditions (1, N+1, N+2 error)

## 10. Integration tests — restructured existing commands

- [x] 10.1 Update every existing test file to use the new command shape: `tests/set.rs`, `tests/rm.rs`, `tests/rename.rs`, `tests/variant.rs`, `tests/import.rs`. Mechanical rewrite — change invocation lines, no logic changes.
- [x] 10.2 Add `--content` vs stdin tests in `tests/set.rs` for `set decl`: both work, both error if both provided, error if neither.
- [x] 10.3 Add `--name` mismatch error tests in `tests/set.rs` for both `set decl` and `set let`.
- [x] 10.4 Update output assertions in every existing write-command test to expect `ok` on success. Remove verbose-output assertions.
- [x] 10.5 Preserve `tests/variant.rs`'s advisory-output assertions for `rm variant` — those are intentionally not reduced to `ok`.

## 11. Documentation

- [x] 11.1 Update `CLAUDE.md` — every mention of the renamed commands (`set` → `set decl`, `variant add` → `add variant`, etc.), architecture notes for the new sub-decl commands, mention of `src/analysis.rs`.
- [x] 11.2 Update `README.md` — command overview table, quick-start examples, any subcommand references.
- [x] 11.3 Rewrite `src/guide.md` (the agent integration guide) — describe the new verb-first surface, the `<verb> <scope>` rule, the decision tree for picking between `patch`/`set let`/`rename let`, addressing discipline (`--name`/`--pattern` primary, `--line` escalation with absolute file lines), `--content` vs `--body` flag distinction, confirmation-only output rule, migration guidance from the old surface.
- [x] 11.4 Release notes: one line per breaking change. Title the PR with `feat!:` for release-please.
- [x] 11.5 Verify `CONTRIBUTING.md` and `ROADMAP.md` are unaffected or update if mentioned.

## 12. Verification

- [x] 12.1 `cargo fmt`
- [x] 12.2 `cargo clippy -- -D warnings`
- [x] 12.3 `cargo test` — all new and existing tests green.
- [ ] 12.4 Manual smoke test: build the binary, run every new command against a scratch project, exercise ambiguity flows, verify error messages read cleanly.
- [ ] 12.5 Run the existing benchmark treatment arm against the new surface; verify token usage does not regress. Record numbers in the release notes or an adjacent comment.
