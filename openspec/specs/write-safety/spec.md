# write-safety Specification

## Purpose
TBD - created by archiving change reject-invalid-write-output. Update Purpose after archive.
## Requirements
### Requirement: Write commands SHALL refuse to operate on files with pre-existing parse errors

Every elmq command that writes to an Elm source file (`set`, `patch`, `rm`, `import add`, `import remove`, `expose`, `unexpose`, `mv`, `rename`, `move-decl`, `variant add`, `variant rm`) MUST parse its input file with tree-sitter-elm and MUST abort with a non-zero exit code before performing any modification if `tree.root_node().has_error()` is true for that file. The command MUST NOT write any bytes to disk in this case. The error message MUST name the offending file and the location (line and column) of the first ERROR node so the user can locate and repair it.

Read-only commands (`list`, `get`, `grep`, `refs`, `variant cases`, `guide`) are explicitly out of scope and retain their current behavior of warning on parse errors but continuing.

#### Scenario: set on a file with unclosed bracket

- **WHEN** the user runs `elmq set Foo.elm bar` against a file containing an unclosed `let ... in` expression that tree-sitter flags with an ERROR node
- **THEN** elmq exits with a non-zero status, writes no bytes to `Foo.elm`, and prints an error to stderr that includes the path `Foo.elm` and a `line:col` pointing at or near the unclosed construct

#### Scenario: patch on a clean file

- **WHEN** the user runs `elmq patch Foo.elm bar` against a file with no ERROR nodes
- **THEN** the input-side check passes and the command proceeds to its normal editing logic

#### Scenario: list on a broken file

- **WHEN** the user runs `elmq list Foo.elm` against a file with an ERROR node
- **THEN** elmq emits the existing parse-error warning on stderr and still prints the `FileSummary` for the well-formed portions (pre-existing read-side behavior is preserved)

### Requirement: Write commands SHALL reject any output buffer whose re-parse produces ERROR nodes

Every elmq write command MUST, after constructing the modified source buffer and before committing it to disk via `atomic_write`, re-parse that buffer with tree-sitter-elm and MUST abort with a non-zero exit code if the re-parsed tree satisfies `root_node().has_error()`. The on-disk file MUST be left unchanged. The error message MUST identify the file being written, describe which operation was attempted, and include the line and column of the first ERROR node in the would-be output so the user can see what elmq (or their input) produced.

This requirement catches two distinct failure modes: (a) user-supplied source fragments such as the `set`/`patch` body or a `variant add --fill` branch that are syntactically invalid, and (b) bugs in elmq's own splicing logic that produce invalid output from valid input.

#### Scenario: set body is syntactically invalid

- **WHEN** the user runs `elmq set Foo.elm bar` and supplies a replacement body that is missing its right-hand side (e.g., `bar =`)
- **THEN** elmq exits non-zero, `Foo.elm` on disk is byte-for-byte unchanged, and stderr names `Foo.elm`, the `set` operation, and a location inside the attempted output that corresponds to the malformed body

#### Scenario: variant add --fill with malformed branch text

- **WHEN** the user runs `elmq variant add Types.elm Msg Reset --fill 'foo#0=model |'` supplying a branch body that does not parse as an Elm expression
- **THEN** elmq exits non-zero, no file in the project is modified, and the error identifies which file's case expression would have produced invalid output along with the offending location

#### Scenario: internal splicing bug would corrupt output

- **WHEN** a write helper's range math produces a buffer whose re-parse yields an ERROR node even though the user's input was well-formed
- **THEN** elmq exits non-zero, the file is left unchanged, and stderr reports that the operation failed post-edit validation so the user can file a bug

### Requirement: Multi-file write commands SHALL validate each file independently and MAY produce partial writes

Commands that rewrite multiple files in a single invocation (`mv`, `rename`, `move-decl`, `variant add`, `variant rm`) MUST apply the input-side and output-side validation rules to every file they touch, in the order the command processes them. If validation fails for file N, files 1..N-1 that have already been written MUST remain on disk (no rollback), file N MUST be left unchanged, and files N+1.. MUST NOT be processed. The error message MUST name the failing file so the user can diagnose and re-run after fixing it. elmq MUST NOT introduce a cross-file transactional staging layer for this change.

#### Scenario: rename succeeds on early files, fails on a later one

- **WHEN** `elmq rename Foo oldName newName` is invoked on a project where the rename in one downstream file would produce a buffer with an ERROR node (e.g., a pre-existing parse error in that file's tail)
- **THEN** earlier files in the iteration order are rewritten normally, the failing file is left unchanged with its original contents, remaining files are not touched, elmq exits non-zero, and stderr identifies the failing file and the location of the ERROR node

#### Scenario: variant add on a clean project

- **WHEN** `elmq variant add Types.elm Msg Reset` is invoked on a project where every case site and the type declaration itself re-parse cleanly after edit
- **THEN** every affected file is written and elmq exits zero

