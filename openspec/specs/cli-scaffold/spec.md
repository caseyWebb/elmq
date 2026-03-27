### Requirement: List declarations command
The CLI SHALL provide a `list` subcommand that takes a file path argument and outputs a grouped file summary.

#### Scenario: List file summary in compact format
- **WHEN** the user runs `elmq list src/Main.elm`
- **THEN** the output SHALL show the module declaration, imports, and declarations grouped by kind (types, functions, ports) with empty sections omitted

#### Scenario: List file summary in JSON format
- **WHEN** the user runs `elmq list src/Main.elm --format json`
- **THEN** the output SHALL be a JSON object with `module`, `imports`, and `declarations` fields

#### Scenario: List with doc comments
- **WHEN** the user runs `elmq list src/Main.elm --docs`
- **THEN** the output SHALL include doc comment text indented under declarations that have them

### Requirement: Error handling for invalid input
The CLI SHALL provide clear error messages for invalid input.

#### Scenario: File not found
- **WHEN** the user runs `elmq list nonexistent.elm`
- **THEN** the CLI SHALL exit with a non-zero status code and print an error message indicating the file was not found

#### Scenario: File is not valid Elm
- **WHEN** the user runs `elmq list` on a file that is not valid Elm
- **THEN** the CLI SHALL report parse errors with the best information available (tree-sitter produces partial parses, so partial results with error indicators are acceptable)

### Requirement: Mise-managed toolchain
The project SHALL include a `.mise.toml` that pins the Rust toolchain version.

#### Scenario: Fresh clone builds successfully
- **WHEN** a developer clones the repo and runs `mise install && cargo build`
- **THEN** the project SHALL compile successfully with the pinned Rust version
