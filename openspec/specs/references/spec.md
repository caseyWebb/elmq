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

### Requirement: Constructor-aware refs dispatch
The `refs` subcommand SHALL dispatch each positional name on what it resolves to in the target file. A top-level declaration (value, type, or type alias) SHALL be handled via the existing decl-refs path (flat list of reference sites). A constructor of a custom type declared in the target file SHALL be handled via the constructor classifier, which walks every `upper_case_qid` resolving to the constructor and classifies each site by its syntactic role. Names that are neither SHALL produce the `declaration '<name>' not found` per-arg error. No separate command surface exists for constructor references — the top-level `refs` command is the single entry point.

#### Scenario: Constructor name routed to classifier
- **GIVEN** `src/Types.elm` declares `type Msg = Increment | Decrement`
- **WHEN** `elmq refs src/Types.elm Increment` is run
- **THEN** the output SHALL be a classified report listing every project-wide reference to `Types.Msg.Increment`, with each site tagged as `case-branch`, `case-wildcard-covered`, `function-arg-pattern`, `lambda-arg-pattern`, `let-binding-pattern`, or `expression-position`, plus a header summarizing total, clean, and blocking counts

#### Scenario: Decl and constructor names in the same call
- **WHEN** `elmq refs src/Types.elm Increment Msg` is run where `Increment` is a constructor and `Msg` is the type declaration
- **THEN** the output SHALL contain one `## Increment` block with the classified report and one `## Msg` block with today's decl-refs output, in input order

#### Scenario: Constructor declared in a different file falls through
- **GIVEN** `Red` is a constructor of `Color` declared in `src/Colors.elm` and NOT in `src/Types.elm`
- **WHEN** `elmq refs src/Types.elm Red` is run
- **THEN** the output SHALL report `declaration 'Red' not found` (the name does not resolve as a decl in `src/Types.elm` *or* as a constructor of a type declared there)

#### Scenario: JSON output for constructor refs
- **WHEN** `elmq refs src/Types.elm Increment --format json` is run (single-arg, so no `## <arg>` framing)
- **THEN** output SHALL be a JSON object with `type_file`, `type_name`, `constructor`, `total_sites`, `total_clean`, `total_blocking`, and a `sites` array of objects each containing `file`, `line`, `column`, `declaration`, `kind`, and `snippet`
- **AND** `kind` SHALL be one of `case-branch`, `case-wildcard-covered`, `function-arg-pattern`, `lambda-arg-pattern`, `let-binding-pattern`, or `expression-position`

#### Scenario: Constructor's own type declaration is excluded
- **GIVEN** the target file declares `type Msg = Increment | Decrement` and nothing else references `Increment`
- **WHEN** `elmq refs <file> Increment` is run
- **THEN** the output SHALL list zero sites (the constructor's own definition inside the `type` declaration is not a reference to itself)

#### Scenario: Nested union patterns are detected
- **GIVEN** some project file contains `case x of Just Increment -> ...`
- **WHEN** `elmq refs <file> Increment` is run
- **THEN** the nested site SHALL be classified as `case-branch` (not silently missed, matching the same guarantee `variant rm` relies on for its branch-removal loop)
