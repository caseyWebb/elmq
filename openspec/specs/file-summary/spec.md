### Requirement: Grouped compact output
The `list` command compact output SHALL group declarations by kind under section headers, with the module declaration shown first and imports listed separately.

#### Scenario: File with all declaration kinds
- **WHEN** the user runs `elmq list` on a file with types, type aliases, functions, and ports
- **THEN** the output SHALL show the module declaration line, then sections for imports, type aliases, types, functions, and ports, each with a header and indented entries

#### Scenario: Type aliases section
- **WHEN** the user runs `elmq list` on a file with type alias declarations
- **THEN** the output SHALL include a `type aliases:` section with each type alias listed by name and line range, without a kind label

#### Scenario: Types section
- **WHEN** the user runs `elmq list` on a file with custom type declarations
- **THEN** the output SHALL include a `types:` section with each type listed by name and line range, without a kind label

#### Scenario: Section ordering
- **WHEN** the user runs `elmq list` on a file with both type aliases and types
- **THEN** the `type aliases:` section SHALL appear before the `types:` section

#### Scenario: Empty sections omitted
- **WHEN** the user runs `elmq list` on a file with no ports
- **THEN** the output SHALL not include a `ports:` section

#### Scenario: Module declaration shown first
- **WHEN** the user runs `elmq list` on any Elm file
- **THEN** the first line of output SHALL be the module declaration as it appears in source

### Requirement: Import listing
The `list` command SHALL include imports in the file summary.

#### Scenario: File with imports
- **WHEN** the user runs `elmq list` on a file with import statements
- **THEN** the output SHALL include an `imports:` section with each import on its own indented line, as it appears in source (without the `import` keyword)

#### Scenario: File with no imports
- **WHEN** the user runs `elmq list` on a file with no import statements
- **THEN** the output SHALL not include an `imports:` section

### Requirement: Doc comments with --docs flag
The `list` command SHALL support a `--docs` flag that includes doc comments inline.

#### Scenario: Declaration with doc comment and --docs
- **WHEN** the user runs `elmq list --docs` on a file where a declaration has a doc comment
- **THEN** the full doc comment text SHALL appear indented under that declaration, with `{-|` and `-}` delimiters stripped

#### Scenario: Declaration without doc comment and --docs
- **WHEN** the user runs `elmq list --docs` on a file where a declaration has no doc comment
- **THEN** no doc comment line SHALL appear under that declaration

#### Scenario: Without --docs flag
- **WHEN** the user runs `elmq list` without `--docs`
- **THEN** no doc comments SHALL appear in the output

### Requirement: No per-declaration exposed status
The compact output SHALL NOT include per-declaration exposed/unexposed markers. Exposed status is implicit from the module declaration line.

#### Scenario: Exposed status omitted
- **WHEN** the user runs `elmq list` on any file
- **THEN** no declaration line SHALL contain the word "exposed" or any exposed/unexposed indicator

### Requirement: JSON output includes module and imports
The JSON output SHALL include module declaration and imports alongside the flat declarations array.

#### Scenario: JSON output structure
- **WHEN** the user runs `elmq list --format json`
- **THEN** the output SHALL be a JSON object with fields: `module` (string), `imports` (array of strings), and `declarations` (array of declaration objects)

#### Scenario: JSON declarations remain flat
- **WHEN** the user runs `elmq list --format json`
- **THEN** the `declarations` array SHALL contain objects with `name`, `kind`, `start_line`, `end_line`, and optionally `type_annotation` and `doc_comment` — NOT grouped by kind

### Requirement: JSON output excludes exposed field
The JSON declaration objects SHALL NOT include an `exposed` field.

#### Scenario: No exposed field in JSON
- **WHEN** the user runs `elmq list --format json`
- **THEN** no declaration object in the `declarations` array SHALL contain an `exposed` key
