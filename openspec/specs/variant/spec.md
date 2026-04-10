### Requirement: Add a variant constructor
The system SHALL provide a `variant add` command that appends a constructor to a custom type declaration and inserts `Debug.todo` branches in all matching case expressions project-wide.

#### Scenario: Add variant to type and case expressions
- **GIVEN** `Types.elm` defines `type Msg = Increment | Decrement` and `Main.elm` has a case expression matching `Msg`
- **WHEN** `elmq variant add Types.elm --type Msg "Reset"` is run
- **THEN** `Types.elm` SHALL contain `| Reset` and `Main.elm`'s case expression SHALL gain a branch `Reset -> Debug.todo "Reset"`

#### Scenario: Variant with arguments
- **WHEN** `elmq variant add Types.elm --type Msg "SetCount Int"` is run
- **THEN** the inserted branch SHALL be `SetCount _ -> Debug.todo "SetCount"` with one wildcard per type argument

#### Scenario: Complex type arguments
- **WHEN** the definition is `"GotResponse (Result String Int)"`
- **THEN** the system SHALL parse it correctly via tree-sitter (one argument, parenthesized) and insert `GotResponse _ -> Debug.todo "GotResponse"`

#### Scenario: Wildcard branch present
- **GIVEN** a case expression has a `_ -> ...` catch-all branch
- **WHEN** a variant is added
- **THEN** the case expression SHALL NOT be modified, and an info message SHALL be emitted indicating the wildcard covers the new variant

#### Scenario: Multiple case expressions across project
- **GIVEN** three files each contain a case expression matching `Msg`
- **WHEN** a variant is added
- **THEN** all three case expressions SHALL be updated (or skipped with info if wildcard)

#### Scenario: Same-file case expressions
- **GIVEN** the type definition and case expressions are in the same file
- **WHEN** a variant is added
- **THEN** both the type and all case expressions in that file SHALL be updated

#### Scenario: Dry run
- **WHEN** `--dry-run` is passed
- **THEN** no files SHALL be written, but the output SHALL report what would change

#### Scenario: Constructor already exists
- **WHEN** the constructor name already exists in the target type
- **THEN** the system SHALL error with a message indicating the constructor already exists

### Requirement: Remove a variant constructor
The system SHALL provide a `variant rm` command that removes a constructor from a custom type declaration and removes matching branches from all case expressions project-wide.

#### Scenario: Remove variant from type and case expressions
- **GIVEN** `Types.elm` defines `type Msg = Increment | Decrement | Reset` and `Main.elm` has a case expression with a `Reset -> ...` branch
- **WHEN** `elmq variant rm Types.elm --type Msg Reset` is run
- **THEN** `Reset` SHALL be removed from the type declaration and the `Reset -> ...` branch SHALL be removed from `Main.elm`'s case expression

#### Scenario: Remove first variant
- **GIVEN** a type `type Msg = Increment | Decrement | Reset`
- **WHEN** `elmq variant rm Types.elm --type Msg Increment` is run
- **THEN** the `= Increment` line SHALL be removed and `Decrement` SHALL become the first variant (with `=`)

#### Scenario: Wildcard branch covers removed variant
- **GIVEN** a case expression has a `_ -> ...` branch and no explicit branch for the removed constructor
- **WHEN** a variant is removed
- **THEN** the case expression SHALL NOT be modified, and an info message SHALL be emitted

#### Scenario: Last variant removal
- **WHEN** the type has only one constructor and `variant rm` is run
- **THEN** the system SHALL error with a message suggesting `elmq rm` to remove the entire type

#### Scenario: Dry run
- **WHEN** `--dry-run` is passed
- **THEN** no files SHALL be written, but the output SHALL report what would change

#### Scenario: Constructor not found
- **WHEN** the constructor name does not exist in the target type
- **THEN** the system SHALL error with a message indicating the constructor was not found

### Requirement: Constructor resolution
The system SHALL resolve constructors through import contexts to identify which case expressions match the target type. Elm constructors are unique within a module — there is no shadowing or ambiguity.

#### Scenario: Bare exposed constructors
- **GIVEN** a file has `import Types exposing (Msg(..))`
- **WHEN** a case expression uses bare `Increment`
- **THEN** the system SHALL resolve it to `Types.Msg` and update the case expression

#### Scenario: Qualified constructors
- **GIVEN** a file has `import Types` and a case uses `Types.Increment`
- **WHEN** a variant is added or removed
- **THEN** the system SHALL resolve the qualified reference and update the case expression

#### Scenario: Aliased constructors
- **GIVEN** a file has `import Types as T` and a case uses `T.Increment`
- **WHEN** a variant is added or removed
- **THEN** the system SHALL resolve through the alias and update the case expression

#### Scenario: Same-module constructors
- **GIVEN** the type and case expression are in the same module
- **WHEN** a variant is added or removed
- **THEN** bare constructors SHALL be resolved without requiring an import

#### Scenario: Unrelated types not affected
- **GIVEN** a file has case expressions for both `Msg` and `Color` types
- **WHEN** a variant is added to `Msg`
- **THEN** only `Msg` case expressions SHALL be modified; `Color` case expressions SHALL be untouched

### Requirement: Result reporting
Both `variant add` and `variant rm` SHALL return structured results including the file, module name, enclosing function name (bare, not qualified), and line number for each edit and skip.

#### Scenario: Compact output
- **WHEN** default format is used
- **THEN** output SHALL show one line per edit/skip:
  ```
  added Reset to Msg in src/Types.elm
    src/Update.elm:22  update  — inserted branch
    src/Main.elm:31    update  — skipped (wildcard branch covers new variant)
  ```

#### Scenario: JSON output
- **WHEN** `--format json` is used
- **THEN** output SHALL be a JSON object with `dry_run`, `type_file`, `type_name`, `variant_name`, `edits`, and `skipped` arrays

### Requirement: Atomic writes
All file modifications SHALL be collected before any writes occur. If any transformation fails, no files SHALL be modified.


### Requirement: Inspect case sites for a type
The system SHALL provide a read-only `elmq variant cases` command that walks the project for every case expression matching a given custom type and emits each site with its enclosing function body and a stable site key. The command SHALL NOT modify any files.

#### Scenario: List case sites in one file
- **GIVEN** `src/Page/Home.elm` defines `type FeedTab = YourFeed Cred | GlobalFeed | TagFeed String` and has two functions (`viewTabs`, `fetchFeed`) that each contain one case expression matching `FeedTab`
- **WHEN** `elmq variant cases src/Page/Home.elm --type FeedTab` is run
- **THEN** the output SHALL list both sites, each with its file, function name, line, key, and the full enclosing function body (signature and implementation)

#### Scenario: List case sites across multiple files
- **GIVEN** `src/Types.elm` defines `type Msg` and three other files (`src/Main.elm`, `src/Page.elm`, `src/Update.elm`) each contain one case expression matching `Msg`
- **WHEN** `elmq variant cases src/Types.elm --type Msg` is run
- **THEN** the output SHALL list all three sites grouped by file, each with its computed key and enclosing function body

#### Scenario: Wildcard-covered sites appear in skipped list
- **GIVEN** a case expression has a `_ -> ...` catch-all branch
- **WHEN** `elmq variant cases` is run on the enclosing type
- **THEN** the site SHALL appear in a `skipped` section with reason `wildcard branch covers type` and SHALL NOT appear in the active sites list

#### Scenario: JSON output format
- **WHEN** `--format json` is passed
- **THEN** the output SHALL be a JSON object with `type`, `type_file`, `sites` (array of `{ file, function, key, line, body }` objects), and `skipped` (array of `{ file, function, line, reason }` objects)

#### Scenario: No case sites found
- **WHEN** no file in the project contains a case expression matching the given type
- **THEN** the command SHALL exit successfully with an empty `sites` array (JSON) or a `no case sites found for type <TypeName>` message (compact)

#### Scenario: Type not found
- **WHEN** the type name does not exist in the given file
- **THEN** the command SHALL error with `type <TypeName> not found in <file>` and exit non-zero

### Requirement: Stable site keys with progressive qualification
The system SHALL compute a unique key for each case site returned by `variant cases`, using the shortest form that is unambiguous across the result set. Keys SHALL be usable verbatim as identifiers in `variant add --fill`.

Key grammar, in order of precedence (shortest wins):
1. `<function>` — bare function name.
2. `<function>#<N>` — function plus 1-indexed ordinal, source-ordered, scoped to `(file, function, type)`.
3. `<file>:<function>` — file-qualified.
4. `<file>:<function>#<N>` — both qualifiers.

#### Scenario: Bare function key when unambiguous
- **GIVEN** the project has exactly one function named `viewTabs` containing a case expression on the target type
- **WHEN** `variant cases` computes keys
- **THEN** the site's key SHALL be `viewTabs`

#### Scenario: Ordinal disambiguation within one function
- **GIVEN** function `update` in `src/Main.elm` contains two distinct case expressions, both matching the target type
- **WHEN** `variant cases` computes keys
- **THEN** the first case (lower byte offset) SHALL have key `update#1` and the second SHALL have key `update#2`

#### Scenario: File qualification across files
- **GIVEN** `src/Main.elm` and `src/Page.elm` each contain a function named `update` with one case expression on the target type
- **WHEN** `variant cases` computes keys
- **THEN** the sites SHALL have keys `src/Main.elm:update` and `src/Page.elm:update` respectively

#### Scenario: Full qualification
- **GIVEN** the same function name appears in two files AND one of those files has two cases in that function
- **WHEN** `variant cases` computes keys
- **THEN** the two-case file's sites SHALL have keys like `src/Main.elm:update#1` and `src/Main.elm:update#2`, and the single-case file's site SHALL have key `src/Page.elm:update`

#### Scenario: Keys match between cases and add --fill
- **GIVEN** `variant cases --type Msg src/Types.elm` emits a site with key `view`
- **WHEN** the same project state is passed to `variant add src/Types.elm --type Msg 'Reset' --fill view='Reset -> text "reset"'`
- **THEN** the fill SHALL be applied at the site that `cases` reported

### Requirement: Fill branch bodies when adding a variant
The system SHALL accept a repeatable `--fill <key>=<body>` flag on `variant add` that replaces the default `Debug.todo "<VariantName>"` branch with the provided body text at the matching site. Unmatched sites SHALL receive the default `Debug.todo` stub (graceful degradation).

#### Scenario: Single fill replaces Debug.todo stub
- **GIVEN** `variant cases` has reported one site with key `view` on type `Msg`
- **WHEN** `elmq variant add src/Types.elm --type Msg 'Reset' --fill 'view=Reset -> text "reset"'` is run
- **THEN** the `view` function's case expression SHALL gain the branch `Reset -> text "reset"` (instead of `Reset -> Debug.todo "Reset"`), and the type declaration SHALL gain the `| Reset` variant

#### Scenario: Multiple fills applied in one invocation
- **GIVEN** `variant cases` has reported three sites with keys `update`, `view`, `subscriptions`
- **WHEN** `variant add` is run with three `--fill` flags covering all three keys
- **THEN** each site SHALL receive its corresponding filled body and no `Debug.todo` stubs SHALL be emitted

#### Scenario: Partial fills degrade to Debug.todo for unfilled sites
- **GIVEN** `variant cases` has reported three sites with keys `update`, `view`, `subscriptions`
- **WHEN** `variant add` is run with `--fill` only for `view`
- **THEN** the `view` site SHALL receive its filled body, and the `update` and `subscriptions` sites SHALL receive `Debug.todo "<VariantName>"` stubs as if no `--fill` had been passed

#### Scenario: Fill body split on first equals
- **WHEN** the fill argument is `'update=UpdateMsg x -> ( { model | counter = x }, Cmd.none )'` (body contains multiple `=` characters)
- **THEN** the key SHALL be `update` and the body SHALL be the entire text after the first `=`, preserving all remaining `=` characters

#### Scenario: Fill key matches an unused site key
- **WHEN** the user passes `--fill nosuchfunction=...` and no site has that key
- **THEN** the system SHALL error with `no case site matched fill key: nosuchfunction` and list the valid site keys for the current invocation, exiting non-zero without writing any files

#### Scenario: Ambiguous bare fill key
- **GIVEN** function `update` has two case expressions on the target type, so its sites have keys `update#1` and `update#2`
- **WHEN** the user passes `--fill update=...` (bare, ambiguous)
- **THEN** the system SHALL error with a message listing the disambiguated keys (`update#1`, `update#2`) the user should use, and SHALL NOT write any files

#### Scenario: Fill applied via tuple pattern case
- **GIVEN** a case expression matches on `( msg, model )` (tuple pattern) and the target type appears in one tuple position
- **WHEN** a fill body is provided keyed to that site
- **THEN** the inserted branch SHALL use the tuple pattern shape (same as when `Debug.todo` would have been inserted) and the fill body SHALL be placed in the branch's body position

#### Scenario: Dry run with fills
- **WHEN** `variant add --fill <key>=<body> --dry-run` is run
- **THEN** no files SHALL be written, but the reported preview SHALL reflect the filled body at the matching site rather than `Debug.todo`

#### Scenario: Backward compatibility without --fill
- **WHEN** `variant add` is run without any `--fill` flags
- **THEN** the behavior SHALL be identical to the pre-change `variant add`: every matching case expression receives `Debug.todo "<VariantName>"`
