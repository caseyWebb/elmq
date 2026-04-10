## ADDED Requirements

### Requirement: Expose a declaration
The `expose` command SHALL take a file path and one or more positional items to add to the module's exposing list, in input order. Multi-item invocations SHALL frame output per the shared `## <arg>` block rule in `cli-scaffold`; single-item invocations SHALL produce bare output. The file SHALL be parsed once and written once per invocation.

#### Scenario: Expose a function
- **WHEN** `elmq expose File.elm update` is run on a file with `module Main exposing (view)`
- **THEN** the module declaration SHALL become `module Main exposing (view, update)`

#### Scenario: Expose a type with constructors
- **WHEN** `elmq expose File.elm "Msg(..)"` is run on a file with `module Main exposing (view)`
- **THEN** the module declaration SHALL become `module Main exposing (view, Msg(..))`

#### Scenario: Already exposed
- **WHEN** `elmq expose File.elm view` is run and `view` is already in the exposing list
- **THEN** the command SHALL succeed with no changes

#### Scenario: Expose when exposing (..)
- **WHEN** `elmq expose File.elm update` is run on a file with `exposing (..)`
- **THEN** the command SHALL succeed with no changes (everything is already exposed)

#### Scenario: No module declaration
- **WHEN** the file has no module declaration
- **THEN** the command SHALL exit with a non-zero status and an error message

#### Scenario: Expose multiple items
- **WHEN** `elmq expose File.elm update init "Msg(..)"` is run on a file with `module Main exposing (view)`
- **THEN** the module declaration SHALL become `module Main exposing (view, update, init, Msg(..))`, the file SHALL be written once, and the output SHALL contain `## update`, `## init`, `## Msg(..)` blocks in that order

### Requirement: Unexpose a declaration
The `unexpose` command SHALL take a file path and one or more positional items to remove from the module's exposing list, in input order. Unexposing an item that is not in the exposing list SHALL be a successful no-op. Multi-item invocations SHALL frame output per the shared `## <arg>` block rule in `cli-scaffold`; single-item invocations SHALL produce bare output. The file SHALL be parsed once and written once per invocation.

#### Scenario: Unexpose a function
- **WHEN** `elmq unexpose File.elm helper` is run on a file with `module Main exposing (view, helper)`
- **THEN** the module declaration SHALL become `module Main exposing (view)`

#### Scenario: Unexpose when exposing (..)
- **WHEN** `elmq unexpose File.elm helper` is run on a file with `exposing (..)`
- **THEN** the command SHALL auto-expand `(..)` to an explicit list of all declarations in the file, then remove `helper`

#### Scenario: Not in exposing list is a no-op
- **WHEN** `elmq unexpose File.elm helper` is run and `helper` is not in the exposing list
- **THEN** the command SHALL succeed with no changes to the file and exit with status `0`

#### Scenario: No module declaration
- **WHEN** the file has no module declaration
- **THEN** the command SHALL exit with a non-zero status and an error message

#### Scenario: Unexpose multiple items
- **WHEN** `elmq unexpose File.elm helper internal debug` is run on a file with `module Main exposing (view, helper, internal, debug)`
- **THEN** the module declaration SHALL become `module Main exposing (view)`, the file SHALL be written once, and the process SHALL exit with status `0`

#### Scenario: Partial no-op in batch is still success
- **WHEN** `elmq unexpose File.elm helper nonexistent debug` is run and only `helper` and `debug` are in the exposing list
- **THEN** `helper` and `debug` SHALL be removed, `nonexistent` SHALL be a no-op, the file SHALL be written once, and the process SHALL exit with status `0`

### Requirement: Never produce exposing (..)
The `expose` and `unexpose` commands SHALL never write `exposing (..)` to the module declaration.

#### Scenario: Expose does not produce expose-all
- **WHEN** any `expose` operation completes
- **THEN** the resulting exposing list SHALL be an explicit list, not `(..)`
