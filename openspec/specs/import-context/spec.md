### Requirement: Import context abstraction
The system SHALL provide an `ImportContext` type that models how a given Elm file imports and references other modules.

#### Scenario: Build from parsed file
- **GIVEN** a tree-sitter parse of an Elm file
- **WHEN** `ImportContext::from_tree()` is called
- **THEN** it SHALL produce an `ImportContext` containing one `ModuleImport` entry per import clause, including module name, alias, exposed names (distinguishing values, types, and types-with-constructors), and `exposing (..)` flag

### Requirement: Resolve qualified references
The system SHALL resolve module prefixes (qualified or aliased) to canonical module names.

#### Scenario: Resolve full module name
- **GIVEN** a file with `import Html.Attributes`
- **WHEN** `resolve_prefix("Html.Attributes")` is called
- **THEN** it SHALL return `"Html.Attributes"`

#### Scenario: Resolve alias
- **GIVEN** a file with `import Html.Attributes as Attr`
- **WHEN** `resolve_prefix("Attr")` is called
- **THEN** it SHALL return `"Html.Attributes"`

#### Scenario: Resolve auto-imported module
- **GIVEN** any Elm file (even without explicit imports)
- **WHEN** `resolve_prefix("String")` is called
- **THEN** it SHALL return `"String"` (recognized as auto-imported)

### Requirement: Resolve bare references
The system SHALL resolve bare (unqualified) names to the module that exposes them.

#### Scenario: Bare exposed value
- **GIVEN** a file with `import Html exposing (div, text)`
- **WHEN** `resolve_bare("div")` is called
- **THEN** it SHALL return `"Html"`

#### Scenario: Bare exposed type with constructors
- **GIVEN** a file with `import Maybe exposing (Maybe(..))`
- **WHEN** `resolve_bare("Just")` is called
- **THEN** it SHALL return `"Maybe"` (constructor exposed via `(..)`)

#### Scenario: Unknown bare name
- **GIVEN** a file with no import exposing `foo`
- **WHEN** `resolve_bare("foo")` is called
- **THEN** it SHALL return `None`

### Requirement: Emit references in file context
The system SHALL emit the correct syntax for referencing a module's declaration in the file's import style.

#### Scenario: Module is aliased
- **GIVEN** a file with `import Html.Attributes as Attr`
- **WHEN** `emit_ref("Html.Attributes", "class")` is called
- **THEN** it SHALL return `"Attr.class"`

#### Scenario: Name is explicitly exposed
- **GIVEN** a file with `import Html exposing (div, text)`
- **WHEN** `emit_ref("Html", "div")` is called
- **THEN** it SHALL return `"div"` (bare, since it's exposed)

#### Scenario: Module imported without alias or exposing
- **GIVEN** a file with `import Http`
- **WHEN** `emit_ref("Http", "get")` is called
- **THEN** it SHALL return `"Http.get"` (fully qualified)

#### Scenario: Module not imported
- **GIVEN** a file that does not import `Http`
- **WHEN** `emit_ref("Http", "get")` is called
- **THEN** it SHALL return `"Http.get"` (fully qualified, caller is responsible for adding the import)

### Requirement: Ensure import exists
The system SHALL add imports that are missing, using a provided style hint, without overriding existing imports.

#### Scenario: Import does not exist
- **GIVEN** a file that does not import `Json.Decode`
- **WHEN** `ensure_import("Json.Decode", style_hint)` is called with a style hint of `import Json.Decode as D`
- **THEN** it SHALL add `import Json.Decode as D` to the context

#### Scenario: Import already exists with different style
- **GIVEN** a file with `import Json.Decode as Decode`
- **WHEN** `ensure_import("Json.Decode", style_hint)` is called with a style hint of `import Json.Decode as D`
- **THEN** it SHALL keep the existing `import Json.Decode as Decode` unchanged

#### Scenario: Auto-imported module
- **GIVEN** any file
- **WHEN** `ensure_import("Basics", style_hint)` is called
- **THEN** it SHALL be a no-op (auto-imported modules need no explicit import)

### Requirement: Render imports
The system SHALL render the import block as text, sorted alphabetically by module name.
