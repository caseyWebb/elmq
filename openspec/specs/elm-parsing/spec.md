### Requirement: Parse Elm source files
The system SHALL parse valid Elm source files using tree-sitter-elm and produce a list of top-level declarations.

#### Scenario: Parse a file with mixed declarations
- **WHEN** given an Elm file containing functions, type aliases, custom types, and port declarations
- **THEN** the parser SHALL return a declaration for each top-level definition

#### Scenario: Parse an empty module
- **WHEN** given an Elm file with only a module declaration and no other content
- **THEN** the parser SHALL return an empty list of declarations

### Requirement: Extract declaration metadata
Each declaration SHALL include: name, kind, type annotation (if present), doc comment (if present), and source line range (start and end line).

#### Scenario: Function with type annotation
- **WHEN** a top-level function has a type annotation
- **THEN** the declaration SHALL include the full type annotation text

#### Scenario: Function without type annotation
- **WHEN** a top-level function has no type annotation
- **THEN** the type annotation field SHALL be empty/none

#### Scenario: Declaration with doc comment
- **WHEN** a declaration is preceded by a doc comment (`{-| ... -}`)
- **THEN** the doc comment SHALL be included as part of the declaration and the line range SHALL start at the doc comment

### Requirement: Determine declaration kind
The parser SHALL categorize declarations into exactly four kinds: `function`, `type`, `type_alias`, and `port`.

#### Scenario: Custom type
- **WHEN** the source contains `type Msg = Increment | Decrement`
- **THEN** the declaration kind SHALL be `type`

#### Scenario: Type alias
- **WHEN** the source contains `type alias Model = { count : Int }`
- **THEN** the declaration kind SHALL be `type_alias`

#### Scenario: Port declaration
- **WHEN** the source contains `port sendMessage : String -> Cmd msg`
- **THEN** the declaration kind SHALL be `port`

### Requirement: Extract imports from CST
The parser SHALL extract import clauses from the tree-sitter CST as raw source text.

#### Scenario: File with imports
- **WHEN** a file contains `import Html exposing (div, text)`
- **THEN** the parser SHALL return the import text `Html exposing (div, text)` (without the `import` keyword)

#### Scenario: File with aliased import
- **WHEN** a file contains `import Html.Attributes as Attr`
- **THEN** the parser SHALL return the import text `Html.Attributes as Attr`

#### Scenario: File with no imports
- **WHEN** a file contains no import statements
- **THEN** the parser SHALL return an empty list of imports

### Requirement: Extract module declaration line
The parser SHALL extract the module declaration as raw source text.

#### Scenario: Standard module
- **WHEN** a file contains `module Main exposing (view, update)`
- **THEN** the parser SHALL return the text `module Main exposing (view, update)`

#### Scenario: Port module
- **WHEN** a file contains `port module Main exposing (..)`
- **THEN** the parser SHALL return the text `port module Main exposing (..)`
