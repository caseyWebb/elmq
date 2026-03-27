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
The `rm` command SHALL remove a declaration by name, including its doc comment and type annotation. Excess blank lines at the removal site SHALL be collapsed to at most two consecutive blank lines.

#### Scenario: Remove declaration with doc comment and type annotation
- **WHEN** `elmq rm File.elm update` is run and `update` has a doc comment and type annotation
- **THEN** the doc comment, type annotation, and value declaration SHALL all be removed

#### Scenario: Remove declaration without doc comment
- **WHEN** `elmq rm File.elm helper` is run and `helper` has no doc comment or type annotation
- **THEN** only the value declaration SHALL be removed

#### Scenario: Whitespace cleanup after removal
- **WHEN** a declaration is removed leaving more than two consecutive blank lines
- **THEN** the blank lines SHALL be collapsed to at most two

#### Scenario: Declaration not found
- **WHEN** the named declaration does not exist in the file
- **THEN** the command SHALL exit with a non-zero status and an error message

### Requirement: Atomic file writes
All write commands (`set`, `patch`, `rm`) SHALL write to a temporary file and rename over the original to prevent partial writes.

#### Scenario: Write-back atomicity
- **WHEN** any write command modifies a file
- **THEN** the modification SHALL be performed via write-to-temp-then-rename, not in-place mutation
