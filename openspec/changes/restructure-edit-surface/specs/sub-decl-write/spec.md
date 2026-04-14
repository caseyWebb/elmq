## ADDED Requirements

### Requirement: Upsert a let binding with `set let`

The `set let <FILE> <DECL>` command SHALL upsert a let binding within the enclosing top-level declaration `<DECL>`. The binding is specified via structured flags: `--name <NAME>` (required), `--body <EXPR>` (required; stdin alternative accepted, exactly-one-of), optional `--type <TYPE>`, optional `--params <SPACE_SEPARATED_NAMES>`, optional `--no-type` (remove existing signature), optional `--after <NAME>` / `--before <NAME>` (sibling positioning), optional `--line <N>` (ambiguity escalation).

The `--body` content is the right-hand side expression only (the text that would appear after the `=`), not the full binding source.

On update (existing binding with the given `--name`):
- If `--type` is provided, the existing signature is replaced with the new type.
- If `--type` is absent and `--no-type` is absent, the existing signature is preserved unchanged.
- If `--no-type` is passed, the existing signature (if any) is removed.
- If `--params` is provided, the parameter list is replaced.
- If `--params` is absent, existing parameters are preserved.
- The body is replaced with the `--body` value.
- If `--after` or `--before` is provided, the binding is moved to the specified position within its let block.

On insert (no binding with the given `--name` in the targeted let block):
- The binding is appended to the end of the outermost let block in `<DECL>`, unless `--after`/`--before` selects a specific sibling position or `--line` selects a specific let block.
- If `--type` is provided, the binding is inserted with a type signature.
- If `--params` is provided, the binding is inserted as a function; if absent, as a value binding.

Addressing: if multiple let blocks within `<DECL>` contain a binding named `<NAME>`, the command SHALL error and list candidate absolute file lines. The agent retries with `--line <N>` to resolve.

If `--name` is passed and the `--body` content parses as a let binding with a different name, the command SHALL error (no silent rename-via-upsert).

Output on success: `ok`.

#### Scenario: Body-only edit on a typed binding preserves the signature

- **GIVEN** `Main.elm` contains `update msg model = let helper : Int -> Int\n helper n = n + 1 in ...`
- **WHEN** `elmq set let Main.elm update --name helper --body "n + 2"` is run
- **THEN** the `helper : Int -> Int` signature SHALL be preserved byte-for-byte and the definition SHALL become `helper n = n + 2`, and the command SHALL exit `0` with stdout `ok`.

#### Scenario: Insert a new value binding

- **GIVEN** `Main.elm` has a `processItem` function with a let block containing `a = 1`
- **WHEN** `elmq set let Main.elm processItem --name b --body "2"` is run
- **THEN** the let block SHALL contain `a = 1` and `b = 2` in that order, with `b` appended.

#### Scenario: Insert a new typed function binding

- **WHEN** `elmq set let Main.elm update --name helper --type "Int -> Int" --params "n" --body "n + 1"` is run
- **THEN** the outermost let block SHALL contain a new binding `helper : Int -> Int\n helper n = n + 1`.

#### Scenario: `--type` on an existing typed binding replaces the signature

- **GIVEN** `helper : Int -> Int\n helper n = n + 1` exists
- **WHEN** `elmq set let Main.elm update --name helper --type "Int -> String" --body "String.fromInt n"` is run
- **THEN** the signature SHALL become `helper : Int -> String` and the body SHALL become `String.fromInt n`.

#### Scenario: `--no-type` removes an existing signature

- **GIVEN** `helper : Int -> Int\n helper n = n + 1` exists
- **WHEN** `elmq set let Main.elm update --name helper --no-type --body "n + 1"` is run
- **THEN** the signature SHALL be removed, leaving `helper n = n + 1`.

#### Scenario: Positioning via `--after` on insert

- **GIVEN** a let block with bindings `initial`, `current`, `final`
- **WHEN** `elmq set let Main.elm update --name cached --body "expensive model" --after initial` is run
- **THEN** the resulting let block SHALL have the binding order `initial`, `cached`, `current`, `final`.

#### Scenario: Upsert with positioning moves the existing binding

- **GIVEN** a let block with bindings `a`, `helper`, `b`
- **WHEN** `elmq set let Main.elm update --name helper --body "n + 2" --after b` is run
- **THEN** the resulting let block SHALL have the binding order `a`, `b`, `helper`, with `helper`'s body updated.

#### Scenario: Ambiguous binding name with multiple let blocks

- **GIVEN** `Main.elm` contains `outer x = let helperA = let h = ... in h in ... ; helperB = let h = ... in h in ...` (two bindings named `h` in separate nested let blocks)
- **WHEN** `elmq set let Main.elm outer --name h --body "42"` is run
- **THEN** the command SHALL exit with a non-zero status and the stderr SHALL list the two candidate absolute file lines and suggest `--line <N>` for disambiguation.

#### Scenario: Disambiguation via `--line`

- **WHEN** the agent re-runs with `elmq set let Main.elm outer --name h --body "42" --line 5` where line 5 is one of the candidates
- **THEN** only the `h` binding at line 5 SHALL be updated.

#### Scenario: `--name` mismatch with parsed content is an error

- **WHEN** `elmq set let Main.elm update --name helper --body "betterHelper n = n + 1"` is run (the content parses as a binding named `betterHelper`, not `helper`)
- **THEN** the command SHALL exit with a non-zero status and stderr SHALL state that `--name` does not match the parsed name in the content.

### Requirement: Remove let bindings with `rm let`

The `rm let <FILE> <DECL> <NAMES>…` command SHALL remove one or more let bindings from the enclosing top-level declaration `<DECL>` by name. Each binding SHALL be removed along with its type signature if present. Multiple names MAY be passed in a single invocation; all names SHALL be resolved up front and the command SHALL error with every failure listed if any name is missing or ambiguous. Ambiguous names MUST be removed individually with `--line <N>`.

Output on success: `ok`.

#### Scenario: Remove a single let binding

- **WHEN** `elmq rm let Main.elm update helper` is run
- **THEN** the `helper` binding SHALL be removed from `update`'s outermost let block, along with its signature if present.

#### Scenario: Remove multiple let bindings in one call

- **WHEN** `elmq rm let Main.elm update helper cached stale` is run and all three bindings exist unambiguously
- **THEN** all three SHALL be removed in a single atomic write and the command SHALL exit `0`.

#### Scenario: All-or-nothing validation on multi-target

- **WHEN** `elmq rm let Main.elm update helper nonexistent cached` is run and `nonexistent` does not exist
- **THEN** the command SHALL exit with a non-zero status, neither `helper` nor `cached` SHALL be removed, and stderr SHALL list `nonexistent` as the failure.

#### Scenario: Ambiguous binding name errors and requires single-target retry

- **WHEN** `elmq rm let Main.elm outer h` is run and two bindings named `h` exist in separate let blocks
- **THEN** the command SHALL exit non-zero, stderr SHALL list the candidate absolute file lines, and the agent SHALL retry with `elmq rm let Main.elm outer h --line <N>`.

### Requirement: Rename a let binding with `rename let`

The `rename let <FILE> <DECL> --from <OLD> --to <NEW>` command SHALL rename a let binding and update every reference to it within the enclosing top-level declaration `<DECL>`. The new name MUST NOT collide with any binder in scope at the binding's location; if it does, the command SHALL error without modifying the file.

Because Elm 0.19 disallows shadowing, within-declaration references are name-exact: every occurrence of `<OLD>` in the declaration refers to the same binding. If a file violates this invariant (invalid Elm that `elm make` would reject), `rename let` SHALL error without attempting a rewrite, because the name-exact rewrite cannot be scoped to a single binding site. `--line <N>` is accepted for API parity with `set let` / `rm let` but is not used for disambiguation here — the spec assumes valid Elm where the name is already unambiguous.

Output on success: `ok`.

#### Scenario: Rename a let binding and its references

- **GIVEN** `update` contains `let h = expensive model in view h model.count h`
- **WHEN** `elmq rename let Main.elm update --from h --to helper` is run
- **THEN** the binding SHALL become `helper = expensive model` and the body SHALL become `view helper model.count helper`, with every reference to `h` within the declaration rewritten.

#### Scenario: New name collides with an in-scope binder

- **GIVEN** `update` contains function parameter `model` and let binding `helper`
- **WHEN** `elmq rename let Main.elm update --from helper --to model` is run
- **THEN** the command SHALL exit non-zero, the file SHALL be unchanged, and stderr SHALL state that `model` is already in scope.

#### Scenario: Duplicate old name (invalid Elm)

- **GIVEN** a file with two let bindings named `h` in different scopes (invalid Elm 0.19; `elm make` would reject it)
- **WHEN** `elmq rename let Main.elm outer --from h --to helper` is run
- **THEN** the command SHALL exit non-zero, stderr SHALL list the candidate absolute file lines, and SHALL state that `--line <N>` cannot disambiguate because the rewrite cannot be scoped — the agent SHALL fix the shadowing first or rewrite the enclosing declaration with `set decl`.

### Requirement: Upsert a case branch with `set case`

The `set case <FILE> <DECL>` command SHALL upsert a branch in a case expression within the enclosing declaration `<DECL>`. Required: `--pattern <PAT>`, `--body <EXPR>` (stdin alternative accepted, exactly-one-of). Optional: `--on <EXPR>` (scrutinee selector), `--line <N>`.

Pattern matching is byte-exact against existing branch patterns after trimming surrounding whitespace. If the pattern matches an existing branch, its body SHALL be replaced with `--body`. If no existing branch matches, a new branch SHALL be appended to the target case expression, immediately before any wildcard (`_`) branch if one exists.

If the enclosing declaration has exactly one case expression, `--on` MAY be omitted. If the declaration has multiple case expressions, `--on` SHALL be used to select by scrutinee text. If multiple case expressions share a scrutinee, `--line` SHALL disambiguate.

If the enclosing declaration has no case expression, the command SHALL error — `set case` does not create case expressions from scratch. The agent SHALL use `set decl` to rewrite the declaration with a case expression.

Output on success: `ok`.

#### Scenario: Replace an existing branch body

- **GIVEN** `update` contains `case msg of Increment -> model + 1`
- **WHEN** `elmq set case Main.elm update --pattern Increment --body "model + 2"` is run
- **THEN** the `Increment` branch body SHALL become `model + 2`.

#### Scenario: Add a new branch

- **GIVEN** `update` contains `case msg of Increment -> ... ; Decrement -> ...`
- **WHEN** `elmq set case Main.elm update --pattern Reset --body "0"` is run
- **THEN** a new branch `Reset -> 0` SHALL be appended to the case expression.

#### Scenario: New branch inserted before wildcard

- **GIVEN** `case n of 0 -> "zero" ; _ -> "other"`
- **WHEN** `elmq set case Main.elm toLabel --pattern "1" --body "\"one\""` is run
- **THEN** the resulting branches SHALL be `0 -> "zero" ; 1 -> "one" ; _ -> "other"`, with the new branch inserted before the wildcard.

#### Scenario: Scrutinee required when multiple case expressions exist

- **GIVEN** `view` contains two case expressions, one on `state` and one on `model.route`
- **WHEN** `elmq set case Main.elm view --pattern Loaded --body "viewLoaded data"` is run (no `--on`)
- **THEN** the command SHALL error listing the two candidate case expressions by scrutinee text and absolute file line.
- **WHEN** retried as `elmq set case Main.elm view --on state --pattern Loaded --body "viewLoaded data"`
- **THEN** the case expression on `state` SHALL be updated.

#### Scenario: No case expression to target

- **WHEN** `elmq set case Main.elm simpleFn --pattern X --body "..."` is run and `simpleFn` has no case expression in its body
- **THEN** the command SHALL exit non-zero and stderr SHALL state that no case expression was found in `simpleFn`.

### Requirement: Remove case branches with `rm case`

The `rm case <FILE> <DECL>` command SHALL remove one or more branches from a case expression in the enclosing declaration. Required: `--pattern <PAT>` (repeatable). Optional: `--on <EXPR>`, `--line <N>`.

Branch matching is byte-exact after trimming whitespace. All patterns SHALL be resolved up front; if any pattern does not match or the case expression is ambiguous, the command SHALL error with every failure listed. The command SHALL error if removing branches would leave the case expression empty.

Output on success: `ok`.

#### Scenario: Remove a single branch

- **WHEN** `elmq rm case Main.elm update --pattern Increment` is run
- **THEN** the `Increment` branch SHALL be removed from `update`'s case expression.

#### Scenario: Remove multiple branches in one call

- **WHEN** `elmq rm case Main.elm update --pattern Increment --pattern Decrement` is run
- **THEN** both branches SHALL be removed in one atomic write.

#### Scenario: Refusing to empty a case expression

- **GIVEN** a case expression with branches `A ->` and `B ->`
- **WHEN** `elmq rm case Main.elm update --pattern A --pattern B` is run
- **THEN** the command SHALL exit non-zero because removing both leaves the case empty, and no modifications SHALL occur.

### Requirement: Add a function argument with `add arg`

The `add arg <FILE> <DECL> --at <N> --name <NAME> [--type <TYPE>]` command SHALL add a new parameter to the function declaration `<DECL>`. The parameter is inserted at 1-indexed position `<N>` in the parameter list.

If the declaration has a type signature, `--type` SHALL be required — the new type is inserted into the signature's arrow chain at position `<N>`. If the declaration has no type signature, `--type` is optional; if passed, it is silently ignored (no signature is created). Only the parameter list is updated when there is no signature.

Position boundary rules:
- `--at 1` prepends the parameter at the start of the parameter list.
- `--at N+1` where `N` is the current parameter count appends at the end.
- `--at N+2` or higher SHALL error — no sparse insertion.

Call sites across the project are not updated. Broken callers are surfaced by `elm make` and fixed by the agent.

Output on success: `ok`.

#### Scenario: Add a typed parameter to a typed function

- **GIVEN** `update : Msg -> Model -> Model` with definition `update msg model = ...`
- **WHEN** `elmq add arg Main.elm update --at 2 --name flag --type Bool` is run
- **THEN** the signature SHALL become `update : Msg -> Bool -> Model -> Model` and the definition SHALL become `update msg flag model = ...`.

#### Scenario: Add a parameter to a typed function without `--type` errors

- **GIVEN** `update : Msg -> Model -> Model`
- **WHEN** `elmq add arg Main.elm update --at 2 --name flag` is run (no `--type`)
- **THEN** the command SHALL error: `'update' has a type signature; --type is required`.

#### Scenario: Add a parameter to an untyped function

- **GIVEN** `logImpl level msg = ...` with no type signature
- **WHEN** `elmq add arg Util.elm logImpl --at 1 --name tag` is run
- **THEN** the definition SHALL become `logImpl tag level msg = ...` and no signature SHALL be created.

#### Scenario: Out-of-range position

- **GIVEN** a function with 3 parameters
- **WHEN** `elmq add arg Main.elm fn --at 5 --name x --type Int` is run
- **THEN** the command SHALL error because `--at 5` exceeds the allowed range (`--at 4` is the maximum).

### Requirement: Remove function arguments with `rm arg`

The `rm arg <FILE> <DECL>` command SHALL remove one or more parameters from a function declaration. Targets are specified via mutually-exclusive repeatable flags: `--at <N>` (1-indexed) or `--name <NAME>`.

Removal happens from both the type signature (if present) and the parameter list. Multi-target removal by position SHALL use the original parameter indices (resolved up front before any mutation); rear-to-front internal processing avoids index-shift bugs. Multi-target removal by name resolves each name independently.

Call sites are not updated. References to the removed parameter within the function body are left for `elm make` to flag.

Output on success: `ok`.

#### Scenario: Remove a single parameter by position

- **WHEN** `elmq rm arg Main.elm update --at 2` is run
- **THEN** the second parameter SHALL be removed from both the signature and the definition.

#### Scenario: Remove multiple parameters by position

- **GIVEN** a function with 4 parameters
- **WHEN** `elmq rm arg Main.elm fn --at 2 --at 4` is run
- **THEN** parameters originally at positions 2 and 4 SHALL both be removed, regardless of the order the flags appear.

#### Scenario: Remove by name

- **WHEN** `elmq rm arg Main.elm update --name flag --name verbose` is run
- **THEN** the `flag` and `verbose` parameters SHALL be removed from the signature and definition.

#### Scenario: Mixing `--at` and `--name` in one call errors

- **WHEN** `elmq rm arg Main.elm update --at 2 --name flag` is run
- **THEN** the command SHALL error — only one addressing mode per invocation.

### Requirement: Rename a function argument with `rename arg`

The `rename arg <FILE> <DECL> --from <OLD> --to <NEW>` command SHALL rename a parameter in the function definition and update every reference to it within the function body. The type signature is not modified (Elm type signatures use types, not parameter names).

The new name MUST NOT collide with any other binder in scope within the function. If it does, the command SHALL error without modifying the file.

Output on success: `ok`.

#### Scenario: Rename a parameter

- **GIVEN** `update m model = m + model.count`
- **WHEN** `elmq rename arg Main.elm update --from m --to msg` is run
- **THEN** the definition SHALL become `update msg model = msg + model.count` and the type signature (if present) SHALL be unchanged.

#### Scenario: Collision with an in-scope name

- **GIVEN** `update msg model = let model = ... in ...`
- **WHEN** `elmq rename arg Main.elm update --from msg --to model` is run
- **THEN** the command SHALL error — `model` is already in scope as a let binding.

### Requirement: Addressing discipline across sub-decl commands

All sub-declaration commands SHALL follow a uniform addressing discipline:

- **Primary address**: a semantic flag derived from information the agent already has from a prior `get` — `--name`, `--pattern`, `--on`, or a positional name arg.
- **Escalation**: when the primary address is ambiguous (multiple matches within the enclosing declaration), the command SHALL error and list the candidates by absolute file line in the stderr output. The agent retries with `--line <N>` where `<N>` is one of the listed lines.
- **`--line` is always absolute file lines** matching the output of `elmq get`, so no mental arithmetic is required when copying numbers between command invocations.
- The command SHALL NOT require a separate preflight read call for addressing; all disambiguation information is emitted inline in the error message so the retry happens in a single round-trip.

#### Scenario: Ambiguity error includes inline candidates

- **WHEN** a sub-decl command's primary address matches multiple sites
- **THEN** stderr SHALL list each candidate with its absolute file line and optional human-readable context (e.g., `line 42 (inside helperA)`), and SHALL suggest the retry command with `--line <N>`

#### Scenario: Chained content-addressed ops do not suffer line drift

- **GIVEN** a function with two let bindings `helperA` and `helperB`, each with distinct names
- **WHEN** the agent chains `elmq set let ... --name helperA ... && elmq set let ... --name helperB ...`
- **THEN** both commands SHALL succeed regardless of any line shifts between them, because `--name` is content-addressed

### Requirement: Sub-decl write commands emit `ok` on success

Every sub-declaration write command (`set let`, `set case`, `rm let`, `rm case`, `rm arg`, `rename let`, `rename arg`, `add arg`) SHALL print `ok` to stdout on success and exit `0`. On failure, the command SHALL print a structured error to stderr with file, operation, and enough location information to diagnose the problem, and exit non-zero.

#### Scenario: Successful write emits `ok`

- **WHEN** `elmq set let Main.elm update --name helper --body "n + 2"` succeeds
- **THEN** stdout SHALL contain the single line `ok` and stderr SHALL be empty

#### Scenario: Failure emits a structured error

- **WHEN** `elmq set let Main.elm update --name helper --body "not valid elm $$$"` fails to parse
- **THEN** stdout SHALL be empty, stderr SHALL name the file, the operation (`set let`), and the location of the parse failure, and the exit code SHALL be non-zero
