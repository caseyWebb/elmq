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
