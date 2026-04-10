### Requirement: List declarations command
The CLI SHALL provide a `list` subcommand that takes one or more file path arguments and outputs a grouped file summary for each.

#### Scenario: List file summary in compact format
- **WHEN** the user runs `elmq list src/Main.elm`
- **THEN** the output SHALL show the module declaration, imports, and declarations grouped by kind (types, functions, ports) with empty sections omitted, and SHALL contain no `## <path>` header line

#### Scenario: List multiple files in compact format
- **WHEN** the user runs `elmq list src/Main.elm src/Page/Home.elm`
- **THEN** the output SHALL contain a `## src/Main.elm` block followed by a `## src/Page/Home.elm` block, each body being the same grouped summary the single-file form would produce, in input order

#### Scenario: List file summary in JSON format
- **WHEN** the user runs `elmq list src/Main.elm --format json`
- **THEN** the output SHALL be a JSON object with `module`, `imports`, and `declarations` fields

#### Scenario: List with doc comments
- **WHEN** the user runs `elmq list src/Main.elm --docs`
- **THEN** the output SHALL include doc comment text indented under declarations that have them

#### Scenario: List best-effort across multiple files
- **WHEN** the user runs `elmq list src/Main.elm src/Missing.elm src/Page/Home.elm` and `src/Missing.elm` does not exist
- **THEN** the `## src/Missing.elm` block SHALL contain `error: file not found`, the other blocks SHALL still show their summaries, and the process SHALL exit with status `2`

### Requirement: Error handling for invalid input
The CLI SHALL provide clear error messages for invalid input. Usage errors SHALL exit with status `1`. Per-argument processing errors in batch-capable subcommands SHALL be surfaced inline in the output and SHALL exit with status `2`. In a single-argument (N=1) invocation of a batch-capable subcommand, processing errors SHALL print an error message to stdout (matching the batch contract) and exit with status `2`.

#### Scenario: File not found in single-argument list
- **WHEN** the user runs `elmq list nonexistent.elm`
- **THEN** the CLI SHALL print `error: file not found` (or equivalent) and exit with status `2`

#### Scenario: File is not valid Elm
- **WHEN** the user runs `elmq list` on a file that is not valid Elm
- **THEN** the CLI SHALL report parse errors with the best information available (tree-sitter produces partial parses, so partial results with error indicators are acceptable)

#### Scenario: Malformed command line
- **WHEN** the user passes an unknown flag or omits a required positional argument
- **THEN** clap SHALL emit its usage message to stderr and the process SHALL exit with status `1`

### Requirement: Multi-argument output framing
Subcommands that accept a variable-cardinality positional argument list (today: `list` files; `get`, `rm`, `refs` names; `import add` clauses; `import remove` modules; `expose`, `unexpose` items; `move-decl` names) SHALL frame their output according to a shared rule: when exactly one argument is provided the output SHALL be bare (identical to the pre-batching single-argument output for that command); when two or more arguments are provided the output SHALL consist of one block per argument, in input order, each block introduced by a line of the form `## <arg>` where `<arg>` is the literal argument as passed on the command line.

#### Scenario: Single-argument call stays bare
- **WHEN** a batch-capable subcommand is invoked with exactly one positional argument
- **THEN** the output SHALL contain no `##` header line and SHALL be byte-identical to the pre-batching single-argument form of that command (modulo any output changes specified elsewhere)

#### Scenario: Multi-argument blocks in input order
- **WHEN** a batch-capable subcommand is invoked with two or more positional arguments
- **THEN** the output SHALL contain one `## <arg>` header block per argument, in the same order the arguments were passed on the command line

#### Scenario: Header body is per-argument result
- **WHEN** an argument in a multi-argument call would have produced output X as a single-argument call
- **THEN** the body of that argument's `## <arg>` block SHALL be X (without adding extra indentation or framing)

### Requirement: Multi-argument error semantics and exit codes
Batch-capable subcommands SHALL process each positional argument independently. A failure on any one argument SHALL NOT abort processing of the remaining arguments. Failures SHALL surface as `error: <message>` lines in that argument's output block (for multi-argument calls) or on the standard output stream (for single-argument calls, where there is no block). Exit codes SHALL be: `0` if every argument succeeded, `2` if any argument failed for a non-usage reason, and `1` reserved for usage errors (argparse failures, missing required arguments, invalid flag values).

#### Scenario: All arguments succeed
- **WHEN** a batch-capable subcommand is invoked and every positional argument is processed successfully
- **THEN** the process SHALL exit with status `0`

#### Scenario: One argument fails in a batch
- **WHEN** a batch-capable subcommand is invoked with multiple positional arguments and one of them fails (e.g., file not found, declaration not found)
- **THEN** the failing argument's `## <arg>` block SHALL contain `error: <message>` as its body, the other arguments SHALL still be processed, and the process SHALL exit with status `2`

#### Scenario: Usage error is distinct from per-argument error
- **WHEN** the command line is malformed (unknown flag, missing required value, invalid subcommand)
- **THEN** clap SHALL emit its usage message to stderr and the process SHALL exit with status `1`, not `2`

#### Scenario: Errors are inline, not on stderr
- **WHEN** a batch-capable subcommand reports a per-argument failure
- **THEN** the `error:` line SHALL appear in the normal output stream (stdout) inside the failing argument's block, not on stderr

### Requirement: Write-command batches are atomic per file
When a batch-capable write subcommand (today: `rm`, `import add`, `import remove`, `expose`, `unexpose`) is invoked with multiple positional arguments against a single file, the implementation SHALL parse the file once, apply every argument's effect in input order against the accumulating source, and perform a single atomic write at the end of the batch. Per-argument failures SHALL NOT abort the accumulation; they SHALL be reported in the output and the successful arguments' effects SHALL still land in the final write.

#### Scenario: Single write per batch
- **WHEN** `elmq rm Foo.elm a b c` is run
- **THEN** `Foo.elm` SHALL be opened for reading at most once and written (via atomic write-to-temp-then-rename) at most once, regardless of the number of positional names

#### Scenario: Partial success still writes
- **WHEN** `elmq rm Foo.elm a b c` is run and `b` does not exist
- **THEN** `a` and `c` SHALL be removed, the `## b` block SHALL contain `error: declaration 'b' not found`, the file SHALL be written once with `a` and `c` removed, and the process SHALL exit with status `2`

### Requirement: Mise-managed toolchain
The project SHALL include a `.mise.toml` that pins the Rust toolchain version.

#### Scenario: Fresh clone builds successfully
- **WHEN** a developer clones the repo and runs `mise install && cargo build`
- **THEN** the project SHALL compile successfully with the pinned Rust version
