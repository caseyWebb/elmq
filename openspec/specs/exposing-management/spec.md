## ADDED Requirements

### Requirement: Expose a declaration
The `expose` command SHALL add an item to the module's exposing list.

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

### Requirement: Unexpose a declaration
The `unexpose` command SHALL remove an item from the module's exposing list.

#### Scenario: Unexpose a function
- **WHEN** `elmq unexpose File.elm helper` is run on a file with `module Main exposing (view, helper)`
- **THEN** the module declaration SHALL become `module Main exposing (view)`

#### Scenario: Unexpose when exposing (..)
- **WHEN** `elmq unexpose File.elm helper` is run on a file with `exposing (..)`
- **THEN** the command SHALL auto-expand `(..)` to an explicit list of all declarations in the file, then remove `helper`

#### Scenario: Not in exposing list
- **WHEN** `elmq unexpose File.elm helper` is run and `helper` is not in the exposing list
- **THEN** the command SHALL exit with a non-zero status and an error message

#### Scenario: No module declaration
- **WHEN** the file has no module declaration
- **THEN** the command SHALL exit with a non-zero status and an error message

### Requirement: Never produce exposing (..)
The `expose` and `unexpose` commands SHALL never write `exposing (..)` to the module declaration.

#### Scenario: Expose does not produce expose-all
- **WHEN** any `expose` operation completes
- **THEN** the resulting exposing list SHALL be an explicit list, not `(..)`
