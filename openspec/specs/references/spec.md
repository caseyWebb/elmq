## ADDED Requirements

### Requirement: Find references command
The CLI SHALL provide a `refs` subcommand that takes a file path and zero or more positional declaration names, and reports where those declarations (or the file's module when no names are given) are referenced across the project.

#### Scenario: Module-level refs with no names
- **WHEN** `elmq refs src/Main.elm` is run with no positional names
- **THEN** the output SHALL report every project file that imports or qualified-references the module declared in `src/Main.elm`

#### Scenario: Single-name refs
- **WHEN** `elmq refs src/Main.elm view` is run
- **THEN** the output SHALL report every project file that references `Main.view` (via qualified reference, aliased reference, or explicit exposing)
- **AND** the output SHALL be bare (no `## view` header), matching the single-argument output contract defined in `cli-scaffold`

#### Scenario: Multi-name refs
- **WHEN** `elmq refs src/Main.elm view update init` is run
- **THEN** the output SHALL contain one `## <name>` header block per positional name, in input order, each block's body listing the references for that name in the same shape as the single-name output

#### Scenario: Name not found is a per-name error
- **WHEN** `elmq refs src/Main.elm view nonexistent update` is run and no declaration named `nonexistent` exists in `src/Main.elm`
- **THEN** the `## nonexistent` block SHALL contain `error: declaration 'nonexistent' not found` and the `## view` and `## update` blocks SHALL still report their refs
- **AND** the process SHALL exit with status `2`

### Requirement: Refs batch single-walk performance
A `refs` invocation SHALL walk each project file at most once, regardless of how many positional names are passed. The per-file parse/analysis work SHALL NOT scale linearly with the number of names in the batch.

#### Scenario: N names parses project once
- **WHEN** `elmq refs src/Main.elm a b c d e` is run on a project with K `.elm` files
- **THEN** the implementation SHALL open and parse each of the K project files no more than once during the single invocation, and collect per-name results from that single pass

#### Scenario: Regression guard
- **WHEN** a unit test instruments file-open or parse calls during a batched `refs` invocation
- **THEN** the count for each project file SHALL be exactly one
