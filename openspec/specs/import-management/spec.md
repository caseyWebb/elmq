## ADDED Requirements

### Requirement: Add an import
The `import add` command SHALL take a file path and one or more positional import clauses, adding each in input order. If an import for a given module already exists in the file (or was added earlier in the same batch), it SHALL be replaced. Multi-clause invocations SHALL frame output per the shared `## <arg>` block rule in `cli-scaffold`; single-clause invocations SHALL produce bare output. The file SHALL be parsed once and written once per invocation.

#### Scenario: Add new import
- **WHEN** `elmq import add File.elm "Html exposing (Html, div, text)"` is run and no `Html` import exists
- **THEN** an `import Html exposing (Html, div, text)` line SHALL be added to the import block

#### Scenario: Replace existing import
- **WHEN** `elmq import add File.elm "Html exposing (Html, div, text, span)"` is run and `import Html exposing (Html, div, text)` already exists
- **THEN** the existing Html import SHALL be replaced with the new one

#### Scenario: Import placement
- **WHEN** an import is added to a file with existing imports
- **THEN** the import SHALL be inserted in alphabetical order among existing imports

#### Scenario: Import placement in file with no imports
- **WHEN** an import is added to a file with no existing imports
- **THEN** the import SHALL be placed after the module declaration, separated by two blank lines

#### Scenario: Add multiple imports
- **WHEN** `elmq import add File.elm "Html exposing (div)" "Html.Attributes exposing (class)" "Http"` is run
- **THEN** all three imports SHALL be added in alphabetical order among existing imports, the file SHALL be parsed once and written once, and the output SHALL contain `## Html exposing (div)`, `## Html.Attributes exposing (class)`, and `## Http` blocks in that order

#### Scenario: Duplicate-module clauses in one batch are last-wins
- **WHEN** `elmq import add File.elm "Html exposing (div)" "Html exposing (text)"` is run
- **THEN** the file SHALL end with `import Html exposing (text)` (the later clause replaces the earlier), and both `## Html exposing (div)` and `## Html exposing (text)` blocks SHALL appear in the output reporting success for each

### Requirement: Remove an import
The `import remove` command SHALL take a file path and one or more positional module names, removing each import by module name in input order. Removing an import that does not exist SHALL be a successful no-op. Multi-module invocations SHALL frame output per the shared `## <arg>` block rule in `cli-scaffold`; single-module invocations SHALL produce bare output. The file SHALL be parsed once and written once per invocation.

#### Scenario: Remove existing import
- **WHEN** `elmq import remove File.elm "Html"` is run and `import Html exposing (Html, div, text)` exists
- **THEN** the entire import line SHALL be removed

#### Scenario: Remove nonexistent import is a no-op
- **WHEN** `elmq import remove File.elm "NonExistent"` is run and no `NonExistent` import exists
- **THEN** the command SHALL succeed with no changes to the file and exit with status `0`

#### Scenario: Remove multiple imports
- **WHEN** `elmq import remove File.elm "Html" "Http" "Json.Decode"` is run and all three imports exist
- **THEN** all three import lines SHALL be removed, the file SHALL be parsed once and written once, and the process SHALL exit with status `0`

#### Scenario: Partial no-op in batch is still success
- **WHEN** `elmq import remove File.elm "Html" "NonExistent" "Http"` is run and only `Html` and `Http` exist
- **THEN** `Html` and `Http` SHALL be removed, `NonExistent` SHALL be a no-op (no error), the file SHALL be written once, and the process SHALL exit with status `0`
