### Requirement: Move declarations between modules
The system SHALL provide a `move-decl` command that moves one or more declarations from a source Elm file to a target Elm file, updating all references project-wide so the code compiles after the operation.

#### Scenario: Basic function move
- **GIVEN** `Source.elm` contains `helper : Int -> Int` and `Target.elm` exists
- **WHEN** `elmq move-decl Source.elm --name helper --to Target.elm` is run
- **THEN** `helper` (with type annotation and doc comment) SHALL be removed from `Source.elm`, inserted into `Target.elm`, and all project files referencing `Source.helper` SHALL be updated to reference `Target.helper`

#### Scenario: Batch move
- **GIVEN** `Source.elm` contains `funcA` and `funcB`
- **WHEN** `elmq move-decl Source.elm --name funcA --name funcB --to Target.elm` is run
- **THEN** both declarations SHALL be moved atomically

#### Scenario: Dry run
- **WHEN** `--dry-run` is passed
- **THEN** no files SHALL be written, but the output SHALL report what would change

### Requirement: Import style rewriting
When a moved declaration's body references modules that the target file imports differently than the source file, the declaration body SHALL be rewritten to match the target file's import conventions.

#### Scenario: Source uses full qualifier, target uses alias
- **GIVEN** source has `import Html.Attributes` and declaration uses `Html.Attributes.class`, target has `import Html.Attributes as Attr`
- **WHEN** the declaration is moved
- **THEN** `Html.Attributes.class` in the body SHALL become `Attr.class`

#### Scenario: Source uses alias, target uses exposed name
- **GIVEN** source has `import Json.Decode as D` and declaration uses `D.field`, target has `import Json.Decode exposing (field)`
- **WHEN** the declaration is moved
- **THEN** `D.field` in the body SHALL become `field`

#### Scenario: Source uses bare exposed name, target uses alias
- **GIVEN** source has `import Html exposing (div)` and declaration uses bare `div`, target has `import Html as H`
- **WHEN** the declaration is moved
- **THEN** bare `div` in the body SHALL become `H.div`

### Requirement: Import carrying
When a moved declaration depends on modules not imported by the target file, the system SHALL add those imports to the target file using the source file's import style for that module.

#### Scenario: Target lacks a required import
- **GIVEN** source has `import Http` and the moved declaration uses `Http.get`, target does not import `Http`
- **WHEN** the declaration is moved
- **THEN** `import Http` SHALL be added to the target file

#### Scenario: Auto-imported modules are not carried
- **GIVEN** a declaration uses `String.length`
- **WHEN** the declaration is moved
- **THEN** no explicit `import String` SHALL be added (it is auto-imported)

### Requirement: References to source module
When a moved declaration references exported declarations that remain in the source module, the target file SHALL import those declarations from the source module.

#### Scenario: Moved function uses type from source
- **GIVEN** `Source.elm` exposes `Model` and `view`, `view` references `Model`
- **WHEN** `view` is moved to `Target.elm` (but `Model` stays)
- **THEN** `Target.elm` SHALL contain `import Source exposing (Model)` (or equivalent)

### Requirement: Unexposed helper detection
The system SHALL automatically include unexposed helper declarations that are used exclusively by the declarations being moved.

#### Scenario: Private helper used only by moved function
- **GIVEN** `Source.elm` has exposed `funcA` which calls unexposed `helperX`, and no other declaration calls `helperX`
- **WHEN** `funcA` is moved
- **THEN** `helperX` SHALL also be moved

#### Scenario: Shared private helper — error
- **GIVEN** `Source.elm` has exposed `funcA` and `funcC`, both calling unexposed `helperZ`
- **WHEN** only `funcA` is moved
- **THEN** the system SHALL error with a message indicating `helperZ` is shared

#### Scenario: Shared private helper — copy flag
- **GIVEN** the same setup as above
- **WHEN** `--copy-shared-helpers` is passed
- **THEN** `helperZ` SHALL be copied (not moved) to the target, remaining in the source as well

### Requirement: Constructor move prevention
The system SHALL reject attempts to move individual type constructors.

#### Scenario: Move a constructor
- **GIVEN** `Source.elm` has `type Msg = Increment | Decrement`
- **WHEN** `elmq move-decl Source.elm --name Increment --to Target.elm` is run
- **THEN** the system SHALL error with "Increment is a constructor of Msg; move Msg instead"

### Requirement: Target file creation
When the target file does not exist, the system SHALL create it with the appropriate module declaration (derived from file path and project source-directories) and the necessary imports.

#### Scenario: Move to new file
- **WHEN** `elmq move-decl Source.elm --name helper --to src/Utils/Helpers.elm` is run and `src/Utils/Helpers.elm` does not exist
- **THEN** `src/Utils/Helpers.elm` SHALL be created with `module Utils.Helpers exposing (helper)` and the moved declaration

### Requirement: Port module handling
When moving a port declaration, the target file SHALL be a `port module`. If it is not, the system SHALL upgrade it.

#### Scenario: Move port to regular module
- **GIVEN** `Target.elm` has `module Target exposing (..)`
- **WHEN** a port declaration is moved to `Target.elm`
- **THEN** `Target.elm` SHALL be rewritten to `port module Target exposing (..)`

#### Scenario: Move port to new file
- **WHEN** a port declaration is moved to a file that does not exist
- **THEN** the new file SHALL be created with `port module ...`

### Requirement: Exposing list management
The system SHALL update exposing lists in both source and target files.

#### Scenario: Exposed declaration moved
- **GIVEN** `Source.elm` has `module Source exposing (funcA, funcB)` and `funcA` is moved
- **WHEN** the move completes
- **THEN** `Source.elm` SHALL have `module Source exposing (funcB)` and `Target.elm` SHALL expose `funcA`

#### Scenario: Unexposed declaration moved
- **GIVEN** `funcA` is not in the source's exposing list (it was auto-included as a helper)
- **WHEN** the move completes
- **THEN** `funcA` SHALL not be added to the target's exposing list

### Requirement: Project-wide reference rewriting
The system SHALL update all files in the project that reference moved declarations.

#### Scenario: Qualified reference
- **GIVEN** `Other.elm` has `import Source` and uses `Source.funcA`
- **WHEN** `funcA` is moved to `Target`
- **THEN** `Other.elm` SHALL be updated to import `Target` and use `Target.funcA`

#### Scenario: Aliased reference
- **GIVEN** `Other.elm` has `import Source as S` and uses `S.funcA`
- **WHEN** `funcA` is moved to `Target`
- **THEN** `Other.elm` SHALL update the reference (adding `import Target` and using `Target.funcA`, or preserving alias style if `Target` is already aliased)

#### Scenario: Bare exposed reference
- **GIVEN** `Other.elm` has `import Source exposing (funcA)` and uses bare `funcA`
- **WHEN** `funcA` is moved to `Target`
- **THEN** `Other.elm` SHALL remove `funcA` from the Source exposing list, add `import Target exposing (funcA)`, and bare uses remain as-is

#### Scenario: exposing (..) is skipped
- **GIVEN** `Other.elm` has `import Source exposing (..)`
- **WHEN** `funcA` is moved from `Source`
- **THEN** `Other.elm` SHALL not be modified (consistent with `refs` and `rename` behavior)

### Requirement: MCP integration
The `elm_edit` MCP tool SHALL support a `move_decl` action.

Parameters:
- `names` (array of strings, required): Declaration names to move
- `target` (string, required): Path to the target Elm file
- `copy_shared_helpers` (boolean, optional): Copy shared helpers instead of erroring — default `false`
- `dry_run` (boolean, optional): Preview without writing — default `false`

#### Scenario: MCP move declaration
- **WHEN** `elm_edit` is called with `{"action": "move_decl", "file": "src/Source.elm", "names": ["funcA"], "target": "src/Target.elm"}`
- **THEN** it SHALL perform the move and return a JSON summary of changes
