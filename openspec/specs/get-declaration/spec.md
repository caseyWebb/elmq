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

### Requirement: Multi-file extraction via `-f`/`--file` flag
The `elmq get` command SHALL accept a repeatable `-f`/`--file` flag that introduces a group of the form `-f <FILE> <NAME> [<NAME>...]`. Each occurrence of `-f` starts a new group: its first value is the path to an Elm file and its subsequent values (at least one) are declaration names to extract from that file. Multiple `-f` groups in a single invocation SHALL read declarations across several files in one call. Bare positional form (`elmq get FILE NAME...`) and `-f` grouped form are mutually exclusive within a single invocation; mixing them SHALL be a usage error exiting with status `1`.

#### Scenario: Single `-f` group is equivalent to the bare form
- **WHEN** `elmq get -f Main.elm update` is run
- **THEN** the output SHALL be byte-identical to `elmq get Main.elm update` modulo the framing rules defined in "Multi-file output framing"

#### Scenario: Multiple `-f` groups read across files
- **WHEN** `elmq get -f src/Foo.elm a b -f src/Bar.elm c` is run against a project where `src/Foo.elm` exposes `a`, `b` and `src/Bar.elm` exposes `c`
- **THEN** the output SHALL contain three blocks in input order — one for `a`, one for `b`, one for `c` — each body being the same source text the single-file form would produce for that name

#### Scenario: Same file across multiple `-f` groups is parsed once
- **WHEN** `elmq get -f Foo.elm a -f Bar.elm b -f Foo.elm c` is run
- **THEN** `Foo.elm` SHALL be opened and parsed exactly once for the whole invocation, the output SHALL contain blocks for `a`, `b`, and `c` in input order, and no block SHALL reflect a re-read of `Foo.elm`

#### Scenario: Mixing bare positionals with `-f` is a usage error
- **WHEN** `elmq get Main.elm update -f Other.elm view` is run
- **THEN** the process SHALL exit with status `1` and clap SHALL emit a usage message explaining that `-f` cannot be combined with bare positional files

#### Scenario: `-f` with no names is a usage error
- **WHEN** `elmq get -f Main.elm` is run with no names following the file
- **THEN** the process SHALL exit with status `1` and clap SHALL emit a usage message requiring at least one name per `-f` group

### Requirement: Multi-file output framing with module-qualified headers
When `elmq get` is invoked with `-f` grouping and produces more than one declaration block, each block SHALL be introduced by a `## <Module>.<name>` header line, where `<Module>` is the Elm module name resolved from the group's file path via the project's `elm.json` source-directory rules and `<name>` is the declaration name as passed on the command line. When no `elm.json` can be discovered (walking up from the given file path), each block SHALL instead be introduced by `## <file>:<name>` where `<file>` is the file path verbatim as passed on the command line. When an invocation produces exactly one declaration block (single `-f` group with a single name), the output SHALL be bare (no `##` header), matching the existing single-name behavior.

#### Scenario: Two groups, project discoverable
- **WHEN** `elmq get -f src/Page/Home.elm update view -f src/Update.elm main` is run inside a project whose `elm.json` maps `src/` to `Page.Home` and `Update` modules
- **THEN** the output SHALL contain `## Page.Home.update`, `## Page.Home.view`, and `## Update.main` blocks in that order, each body being the source text the single-name form would produce

#### Scenario: Multi-file fallback when no `elm.json`
- **WHEN** `elmq get -f fixtures/A.elm x -f fixtures/B.elm y` is run in a directory tree with no discoverable `elm.json`
- **THEN** the output SHALL contain `## fixtures/A.elm:x` and `## fixtures/B.elm:y` blocks in that order, each body being the source text the single-name form would produce

#### Scenario: Single-group single-name stays bare
- **WHEN** `elmq get -f Main.elm update` is run
- **THEN** the output SHALL contain no `##` header line and SHALL be byte-identical to the bare `elmq get Main.elm update` output

#### Scenario: Module resolution collision is a per-group error
- **WHEN** `elmq get -f a/Shared.elm x -f b/Shared.elm y` is run in a project where both paths resolve to the same module name `Shared`
- **THEN** each affected `## Shared.*` block SHALL contain `error: ambiguous module resolution for <file>` as its body, the process SHALL exit with status `2`, and any unaffected groups SHALL still produce their source blocks

### Requirement: Per-group file and parse errors
Errors that are scoped to a whole group (file not found, parse failure, module resolution failure) SHALL surface as the body of every `## <Module>.<name>` (or `## <file>:<name>`) block in that group, SHALL NOT abort processing of other groups, and SHALL cause the process to exit with status `2`. Per-name errors within a successfully-read group (declaration not found) SHALL continue to follow the existing per-name error contract.

#### Scenario: Missing file in one group
- **WHEN** `elmq get -f Good.elm a -f Missing.elm b c -f Other.elm d` is run and `Missing.elm` does not exist
- **THEN** the `## Good.a` and `## Other.d` blocks SHALL contain source, the `## Missing.b` and `## Missing.c` blocks SHALL each contain `error: file not found` (or equivalent), and the process SHALL exit with status `2`

#### Scenario: Parse failure in one group
- **WHEN** `elmq get -f Bad.elm a b -f Good.elm c` is run and `Bad.elm` is not valid Elm
- **THEN** the `## Bad.a` and `## Bad.b` blocks SHALL each contain `error: <parse message>`, the `## Good.c` block SHALL contain source, and the process SHALL exit with status `2`

#### Scenario: Missing declaration in a successfully-read group
- **WHEN** `elmq get -f Foo.elm a nonExistent -f Bar.elm b` is run and `nonExistent` is not defined in `Foo.elm`
- **THEN** the `## Foo.a` block SHALL contain source, the `## Foo.nonExistent` block SHALL contain `error: declaration 'nonExistent' not found`, the `## Bar.b` block SHALL contain source, and the process SHALL exit with status `2`

### Requirement: JSON output format for multi-file get
When `elmq get --format json` is invoked with `-f` grouping and produces more than one declaration result, the output SHALL be a JSON array whose elements are objects containing the declaration's `name`, `kind`, `source`, `start_line`, `end_line`, plus the resolved `module` (or `null` when no `elm.json` is discoverable) and `file` (the path as passed on the command line). Order SHALL match the input order of `(file, name)` pairs flattened across `-f` groups. A single-group single-name invocation SHALL continue to emit a single JSON object, matching the existing behavior.

#### Scenario: JSON array for multi-file
- **WHEN** `elmq get --format json -f src/Foo.elm a b -f src/Bar.elm c` is run in a discoverable project
- **THEN** the output SHALL be a JSON array of three objects, each with `name`, `kind`, `source`, `start_line`, `end_line`, `module`, and `file` fields, in input order

#### Scenario: JSON array fallback with null module
- **WHEN** `elmq get --format json -f fixtures/A.elm x` is run with no `elm.json` discoverable and the invocation has multiple names
- **THEN** each element's `module` field SHALL be `null` and `file` SHALL be the literal path passed on the command line

#### Scenario: Single-result JSON stays scalar
- **WHEN** `elmq get --format json -f Main.elm update` is run
- **THEN** the output SHALL be a single JSON object (not an array), matching the existing single-name behavior
