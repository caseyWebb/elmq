## ADDED Requirements

### Requirement: Regex search over Elm sources

The `elmq grep` command SHALL accept a regex pattern and optional path argument and report every match in discovered `.elm` files. The regex dialect SHALL be the Rust `regex` crate's syntax. Patterns SHALL be interpreted as regexes by default; when `-F` or `--fixed` is passed, the pattern SHALL be treated as a literal string. When `-i` or `--ignore-case` is passed, matching SHALL be case-insensitive.

#### Scenario: Regex match reports enclosing top-level declaration

- **WHEN** the user runs `elmq grep "Http\.get"` in a project containing a function `fetchUsers` whose body contains `Http.get { url = "/users" }` on line 42 of `src/Api.elm`
- **THEN** the compact output SHALL include a line of the form `src/Api.elm:42:fetchUsers:    Http.get { url = "/users" }`

#### Scenario: Literal mode with -F

- **WHEN** the user runs `elmq grep -F "a.b" src/`
- **THEN** the tool SHALL match the literal three-character sequence `a.b` and SHALL NOT treat `.` as a regex metacharacter

#### Scenario: Case-insensitive matching

- **WHEN** the user runs `elmq grep -i "http"` against a file containing `Http.get`
- **THEN** the match SHALL be reported

#### Scenario: Invalid regex pattern

- **WHEN** the user runs `elmq grep "[unclosed"`
- **THEN** the tool SHALL write an error message to stderr describing the regex compilation failure
- **AND** the process SHALL exit with code `2`

### Requirement: Enclosing top-level declaration context

The `elmq grep` command SHALL annotate each match with the name and kind of its enclosing **top-level** declaration in the Elm source file. The tool SHALL NOT report nested let-binding names, signature-vs-body distinctions, or any deeper syntactic context in v1. Matches that fall outside any top-level declaration (e.g., inside imports, module header, or top-level type annotations without a body line mapped into a decl) SHALL be reported with a `null` / `-` declaration placeholder.

#### Scenario: Match inside a function body

- **WHEN** a regex matches text on line 42 inside the body of top-level function `fetchUsers` in `src/Api.elm`
- **THEN** the reported declaration name SHALL be `fetchUsers`

#### Scenario: Match inside a let-binding nested in a function

- **WHEN** a regex matches text on a line that is lexically inside a let-binding named `loop` which is itself inside the body of top-level function `update`
- **THEN** the reported declaration name SHALL be `update`
- **AND** the output SHALL NOT mention `loop`

#### Scenario: Match in an import line

- **WHEN** a regex matches text on an `import` line outside any declaration
- **THEN** the compact output SHALL use `-` in the declaration slot
- **AND** the JSON output SHALL set the `decl` field to `null`

#### Scenario: Match in the module header

- **WHEN** a regex matches text on the `module ... exposing (...)` header
- **THEN** the reported declaration SHALL be `null` / `-`
- **AND** the `module` field in JSON output SHALL still contain the module name

### Requirement: Comment and string literal filtering

By default, the `elmq grep` command SHALL discard matches whose position falls inside an Elm line comment (`--`), block comment (`{- -}`), string literal (`"..."`), or multi-line string literal (`"""..."""`). The tool SHALL expose two independent flags to re-enable each class:

- `--include-comments` SHALL re-enable matches inside comments.
- `--include-strings` SHALL re-enable matches inside string literals.

The flags SHALL be independent; passing one SHALL NOT imply the other.

#### Scenario: Comment match filtered by default

- **WHEN** the user runs `elmq grep "TODO"` against a file where the only match is inside a `-- TODO: fix this` comment
- **THEN** no match SHALL be reported
- **AND** the exit code SHALL be `1`

#### Scenario: Comment match included with flag

- **WHEN** the user runs `elmq grep --include-comments "TODO"` against the same file
- **THEN** the comment match SHALL be reported

#### Scenario: String literal match filtered by default

- **WHEN** the user runs `elmq grep "retry"` against a file where the only match is inside `"retry failed"` as a string literal value
- **THEN** no match SHALL be reported

#### Scenario: String literal match included with flag

- **WHEN** the user runs `elmq grep --include-strings "retry"` against the same file
- **THEN** the string literal match SHALL be reported

#### Scenario: Flags are independent

- **WHEN** the user runs `elmq grep --include-comments "retry"` against a file containing both a comment match and a string literal match
- **THEN** only the comment match SHALL be reported
- **AND** the string literal match SHALL still be filtered

### Requirement: Project discovery and file walking

The `elmq grep` command SHALL discover files to search using a two-phase strategy. It SHALL walk upward from the current working directory looking for an `elm.json`. When found, it SHALL use that file's `source-directories` as the set of roots. When no `elm.json` is found in any ancestor, it SHALL fall back to recursively walking the current working directory for `*.elm` files. In both cases, discovery SHALL honor `.gitignore`, `.ignore`, and standard hidden-directory exclusions. When an optional positional `PATH` argument is passed, discovery SHALL be restricted to files under `PATH` without changing project root resolution.

#### Scenario: Invoked at project root with elm.json

- **WHEN** the user runs `elmq grep "foo"` from a directory containing `elm.json` with `"source-directories": ["src", "tests/src"]`
- **THEN** the tool SHALL search every `.elm` file under `src` and `tests/src`
- **AND** SHALL NOT search `.elm` files in directories not listed in `source-directories`

#### Scenario: Invoked from monorepo subdirectory

- **WHEN** the user runs `elmq grep "foo"` from a subdirectory whose ancestor contains `elm.json`
- **THEN** the tool SHALL resolve the ancestor `elm.json` and search its full `source-directories`

#### Scenario: No elm.json in any ancestor

- **WHEN** the user runs `elmq grep "foo"` from a directory with no ancestor `elm.json`
- **THEN** the tool SHALL walk the current working directory recursively for `*.elm` files
- **AND** SHALL report matches from those files

#### Scenario: .gitignore is honored

- **WHEN** the project contains a `.gitignore` entry for `generated/` and matching text exists in `generated/Thing.elm`
- **THEN** the tool SHALL NOT report matches from files under `generated/`

#### Scenario: elm-stuff is excluded

- **WHEN** matching text exists in `elm-stuff/0.19.1/Foo.elm`
- **THEN** the tool SHALL NOT report matches from that file

#### Scenario: Positional PATH narrows scope

- **WHEN** the user runs `elmq grep "foo" src/ui/` from a project with an ancestor `elm.json`
- **THEN** the tool SHALL resolve project context from the ancestor `elm.json`
- **BUT** SHALL only search files whose paths are under `src/ui/`

### Requirement: Output formats

The `elmq grep` command SHALL support two output formats controlled by a `--format` flag whose values SHALL be `compact` and `json`. The default SHALL be `compact`. Compact output SHALL produce one line per match in the form `<file>:<line>:<decl_name>:<line_text>`, using `-` in the declaration slot when the match is outside any top-level declaration. JSON output SHALL produce one JSON object per match, separated by newlines (NDJSON), with at minimum the fields: `file`, `line`, `column`, `module`, `decl`, `decl_kind`, `match`, `line_text`. The `decl` and `decl_kind` fields SHALL be `null` when the match falls outside any declaration.

#### Scenario: Default compact output

- **WHEN** the user runs `elmq grep "Http.get"` without specifying `--format`
- **THEN** output SHALL be in compact form, one line per match

#### Scenario: JSON output is line-delimited

- **WHEN** the user runs `elmq grep --format json "Http.get"` and there are three matches
- **THEN** the output SHALL contain three lines, each of which is a parseable JSON object
- **AND** the output SHALL NOT be wrapped in a JSON array

#### Scenario: JSON match outside any declaration

- **WHEN** a match is reported from an `import` line
- **THEN** the corresponding JSON object's `decl` field SHALL be `null`
- **AND** the `decl_kind` field SHALL be `null`
- **AND** the `module` field SHALL contain the module name of the containing file

### Requirement: Exit codes

The `elmq grep` command SHALL use exit codes that match ripgrep's conventions: `0` when at least one match is reported, `1` when no matches are found, and `2` on any error (invalid regex, I/O error, missing referenced path, etc.).

#### Scenario: Matches found

- **WHEN** at least one match is reported
- **THEN** the process SHALL exit with code `0`

#### Scenario: No matches

- **WHEN** the regex compiles and every searched file produces zero matches
- **THEN** the process SHALL exit with code `1`

#### Scenario: Error

- **WHEN** the regex fails to compile
- **THEN** the process SHALL exit with code `2`

### Requirement: Parse failure resilience

When the `elmq grep` command encounters a file that fails to parse as Elm (for example, due to a syntax error), it SHALL NOT abort the entire run. It SHALL report regex matches from that file with a `null` declaration context, write a warning to stderr identifying the failing file, and continue processing remaining files.

#### Scenario: One file in the project has a syntax error

- **WHEN** `elmq grep "foo"` is run against a project where `src/Broken.elm` does not parse but other files contain matches
- **THEN** the tool SHALL report matches from the other files normally
- **AND** SHALL write a warning about `src/Broken.elm` to stderr
- **AND** SHALL exit with code `0` if any matches were found
