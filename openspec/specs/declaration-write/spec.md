## ADDED Requirements

### Requirement: Set (upsert) a declaration
The `set` command SHALL read a full declaration source from stdin, parse the declaration name from it, and either replace the existing declaration with that name or append it to the end of the file. An optional `--name` flag SHALL override the parsed name.

#### Scenario: Replace existing declaration
- **WHEN** `elmq set File.elm` is run with stdin containing a declaration named `update`
- **THEN** the existing `update` declaration (including doc comment and type annotation) SHALL be replaced with the stdin content, and surrounding formatting SHALL be preserved

#### Scenario: Append new declaration
- **WHEN** `elmq set File.elm` is run with stdin containing a declaration named `newHelper` that does not exist in the file
- **THEN** the declaration SHALL be appended after the last declaration in the file, separated by two blank lines

#### Scenario: Name override with --name
- **WHEN** `elmq set File.elm --name update` is run with stdin containing a declaration
- **THEN** the `--name` value SHALL be used instead of parsing the name from stdin

#### Scenario: Unparseable stdin without --name
- **WHEN** `elmq set File.elm` is run with stdin that tree-sitter cannot parse into a declaration
- **THEN** the command SHALL exit with a non-zero status and an error message suggesting `--name`

### Requirement: Patch a declaration
The `patch` command SHALL perform a scoped find-and-replace within a named declaration using `--old` and `--new` flags. The replacement SHALL only affect text within the declaration's line range (including doc comment and type annotation).

#### Scenario: Successful patch
- **WHEN** `elmq patch File.elm update --old "model.count + 1" --new "model.count + 2"` is run
- **THEN** the first and only occurrence of `--old` within the `update` declaration SHALL be replaced with `--new`

#### Scenario: Old string not found
- **WHEN** `elmq patch File.elm update --old "nonexistent text" --new "replacement"` is run
- **THEN** the command SHALL exit with a non-zero status and an error message indicating the old string was not found in the declaration

#### Scenario: Old string matches multiple times
- **WHEN** `--old` matches more than once within the declaration
- **THEN** the command SHALL exit with a non-zero status and an error message indicating ambiguous match

#### Scenario: Declaration not found
- **WHEN** the named declaration does not exist in the file
- **THEN** the command SHALL exit with a non-zero status and an error message

### Requirement: Remove a declaration
The `rm` command SHALL take a file path and one or more positional declaration names, removing each named declaration by name (including its doc comment and type annotation) and writing the file exactly once via atomic write. Excess blank lines at each removal site SHALL be collapsed to at most two consecutive blank lines. Multi-name invocations SHALL frame output per the shared `## <arg>` block rule in `cli-scaffold`; single-name invocations SHALL produce bare output (typically empty, as today).

#### Scenario: Remove declaration with doc comment and type annotation
- **WHEN** `elmq rm File.elm update` is run and `update` has a doc comment and type annotation
- **THEN** the doc comment, type annotation, and value declaration SHALL all be removed and the file SHALL be written once

#### Scenario: Remove declaration without doc comment
- **WHEN** `elmq rm File.elm helper` is run and `helper` has no doc comment or type annotation
- **THEN** only the value declaration SHALL be removed

#### Scenario: Whitespace cleanup after removal
- **WHEN** a declaration is removed leaving more than two consecutive blank lines
- **THEN** the blank lines SHALL be collapsed to at most two

#### Scenario: Declaration not found (single-name)
- **WHEN** `elmq rm File.elm nonExistent` is run and no declaration named `nonExistent` exists
- **THEN** the command SHALL exit with status `2` and print `error: declaration 'nonExistent' not found` (or equivalent), and the file SHALL NOT be written

#### Scenario: Remove multiple declarations
- **WHEN** `elmq rm File.elm update helper view` is run and all three exist
- **THEN** all three declarations SHALL be removed with per-site whitespace cleanup, the file SHALL be parsed once and written once, and the output SHALL contain empty `## update`, `## helper`, and `## view` blocks (or nothing, if success blocks render empty)

#### Scenario: Partial success in a multi-name remove
- **WHEN** `elmq rm File.elm update nonExistent view` is run and only `update` and `view` exist
- **THEN** `update` and `view` SHALL be removed, the file SHALL be written once with both removals applied, the `## nonExistent` block SHALL contain `error: declaration 'nonExistent' not found`, and the process SHALL exit with status `2`

### Requirement: Atomic file writes
All write commands (`set`, `patch`, `rm`) SHALL write to a temporary file and rename over the original to prevent partial writes. Batch-capable write commands SHALL perform exactly one atomic write per invocation, regardless of the number of positional arguments processed.

#### Scenario: Write-back atomicity
- **WHEN** any write command modifies a file
- **THEN** the modification SHALL be performed via write-to-temp-then-rename, not in-place mutation

#### Scenario: Single write per batch
- **WHEN** `elmq rm File.elm a b c` is run
- **THEN** `File.elm` SHALL be written exactly once via the write-to-temp-then-rename path, with all applicable removals included in that single write
