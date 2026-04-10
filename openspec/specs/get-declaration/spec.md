## ADDED Requirements

### Requirement: Extract declaration source by name
The `elmq get <file> <name> [<name>...]` command SHALL extract the full source text of each named top-level declaration from the given Elm file, in input order. For each name, the extracted text SHALL include the doc comment (if present), type annotation (if present), and the complete declaration body, preserving original formatting. Multi-name invocations SHALL frame output per the shared `## <arg>` block rule in `cli-scaffold`; single-name invocations SHALL produce bare output.

#### Scenario: Get a function with type annotation
- **WHEN** `elmq get Main.elm update` is run and `Main.elm` contains a function `update` with a type annotation
- **THEN** the output SHALL contain the type annotation line(s) followed by the full function body, exactly as written in the source file, with no `## update` header

#### Scenario: Get a function without type annotation
- **WHEN** `elmq get Main.elm helper` is run and `helper` has no type annotation
- **THEN** the output SHALL contain only the function body, exactly as written in the source file

#### Scenario: Get a type declaration
- **WHEN** `elmq get Main.elm Msg` is run and `Msg` is a custom type
- **THEN** the output SHALL contain the full type declaration including all constructors

#### Scenario: Get a type alias
- **WHEN** `elmq get Main.elm Model` is run and `Model` is a type alias
- **THEN** the output SHALL contain the full type alias declaration including the record fields

#### Scenario: Get a declaration with doc comment
- **WHEN** `elmq get Main.elm Model` is run and `Model` has a `{-| ... -}` doc comment
- **THEN** the output SHALL include the doc comment preceding the declaration

#### Scenario: Get a port declaration
- **WHEN** `elmq get Main.elm sendMessage` is run and `sendMessage` is a port
- **THEN** the output SHALL contain the port annotation line

#### Scenario: Get multiple declarations
- **WHEN** `elmq get Main.elm update view init` is run and all three declarations exist
- **THEN** the output SHALL contain `## update`, `## view`, and `## init` blocks in that order, each body being the same source text the single-name form would produce

### Requirement: Declaration not found error
The command SHALL treat "declaration not found" as a per-name error. In a single-name invocation, the process SHALL exit with status `2` and print an error message. In a multi-name invocation, the failing name's `## <name>` block SHALL contain `error: declaration '<name>' not found`, the other names SHALL still be processed, and the process SHALL exit with status `2` if any name failed.

#### Scenario: Single-name not found
- **WHEN** `elmq get Main.elm nonExistent` is run and no declaration named `nonExistent` exists
- **THEN** the process SHALL exit with status `2` and print `error: declaration 'nonExistent' not found` (or equivalent) to stdout

#### Scenario: Multi-name partial not found
- **WHEN** `elmq get Main.elm update nonExistent view` is run and only `update` and `view` exist
- **THEN** the output SHALL contain `## update` (with source), `## nonExistent` (with `error: declaration 'nonExistent' not found`), and `## view` (with source) in that order, and the process SHALL exit with status `2`

### Requirement: JSON output format
The command SHALL support `--format json` which outputs a JSON object containing the declaration's name, kind, source text, start line, and end line.

#### Scenario: JSON output for a function
- **WHEN** `elmq get Main.elm update --format json` is run
- **THEN** the output SHALL be a JSON object with fields `name`, `kind`, `source`, `start_line`, and `end_line`

### Requirement: Compact output is default
The command SHALL default to compact format, which outputs the raw source text with no additional metadata or framing.

#### Scenario: Default format is compact
- **WHEN** `elmq get Main.elm update` is run without `--format`
- **THEN** the output SHALL be the raw source text only, identical to `--format compact`
