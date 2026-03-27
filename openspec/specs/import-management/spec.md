## ADDED Requirements

### Requirement: Add an import
The `import add` command SHALL add an import clause to the file. If the import already exists (same module name), it SHALL be replaced.

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

### Requirement: Remove an import
The `import remove` command SHALL remove an import clause by module name.

#### Scenario: Remove existing import
- **WHEN** `elmq import remove File.elm "Html"` is run and `import Html exposing (Html, div, text)` exists
- **THEN** the entire import line SHALL be removed

#### Scenario: Import not found
- **WHEN** `elmq import remove File.elm "NonExistent"` is run
- **THEN** the command SHALL exit with a non-zero status and an error message
