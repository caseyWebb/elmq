### Requirement: elm_summary tool
The system SHALL provide an `elm_summary` tool that returns a summary of an Elm file's structure including module declaration, imports, and all declarations grouped by kind.

Parameters:
- `file` (string, required): Path to the Elm file
- `format` (string, optional): Output format — `compact` (default) or `json`
- `docs` (boolean, optional): Include doc comments — default `false`

#### Scenario: Compact summary
- **WHEN** `elm_summary` is called with a valid Elm file path
- **THEN** the tool SHALL return the file summary in compact text format matching `elmq list` output

#### Scenario: JSON summary
- **WHEN** `elm_summary` is called with `format` set to `json`
- **THEN** the tool SHALL return the file summary as a JSON object matching `elmq list --format json` output

### Requirement: elm_get tool
The system SHALL provide an `elm_get` tool that extracts the full source text of a declaration by name.

Parameters:
- `file` (string, required): Path to the Elm file
- `name` (string, required): Name of the declaration to extract
- `format` (string, optional): Output format — `compact` (default) or `json`

#### Scenario: Extract declaration source
- **WHEN** `elm_get` is called with a valid file and declaration name
- **THEN** the tool SHALL return the full source text of that declaration

#### Scenario: Declaration not found
- **WHEN** `elm_get` is called with a name that does not exist in the file
- **THEN** the tool SHALL return an error indicating the declaration was not found

### Requirement: elm_edit tool
The system SHALL provide an `elm_edit` tool that performs all file mutations. The `action` parameter determines the operation.

Parameters:
- `file` (string, required): Path to the Elm file
- `action` (string, required): One of `set`, `patch`, `rm`, `mv`, `rename`, `move_decl`, `add_import`, `remove_import`, `expose`, `unexpose`, `add_variant`, `rm_variant`
- `source` (string, required for `set`): Full source text of the declaration to upsert
- `name` (string, optional for `set`, required for `patch`/`rm`/`rename`): Declaration name
- `old` (string, required for `patch`): Text to find within the declaration
- `new` (string, required for `patch`/`rename`): Replacement text or new name
- `new_path` (string, required for `mv`): New file path for the module
- `names` (array of strings, required for `move_decl`): Declaration names to move
- `target` (string, required for `move_decl`): Path to target Elm file
- `import` (string, required for `add_import`): Import clause (e.g., `Html exposing (Html, div)`)
- `module_name` (string, required for `remove_import`): Module name to remove (e.g., `Html`)
- `item` (string, required for `expose`/`unexpose`): Item to expose or unexpose (e.g., `update` or `Msg(..)`)
- `type_name` (string, required for `add_variant`/`rm_variant`): Name of the custom type (e.g., `Msg`)
- `definition` (string, required for `add_variant`): Variant definition (e.g., `SetName String`)
- `constructor` (string, required for `rm_variant`): Constructor name to remove (e.g., `Decrement`)
- `copy_shared_helpers` (boolean, optional for `move_decl`): Copy shared helpers instead of erroring
- `dry_run` (boolean, optional for `mv`/`rename`/`move_decl`/`add_variant`/`rm_variant`): Preview without writing

#### Scenario: Set (upsert) a declaration
- **WHEN** `elm_edit` is called with `action: "set"` and `source` containing a declaration
- **THEN** the tool SHALL upsert the declaration in the file (insert if new, replace if existing) and write the file atomically

#### Scenario: Patch a declaration
- **WHEN** `elm_edit` is called with `action: "patch"`, `name`, `old`, and `new`
- **THEN** the tool SHALL find `old` within the named declaration and replace it with `new`, writing atomically

#### Scenario: Remove a declaration
- **WHEN** `elm_edit` is called with `action: "rm"` and `name`
- **THEN** the tool SHALL remove the named declaration (including doc comment and type annotation) and write atomically

#### Scenario: Add an import
- **WHEN** `elm_edit` is called with `action: "add_import"` and an `import` clause
- **THEN** the tool SHALL add or replace the import in the file and write atomically

#### Scenario: Remove an import
- **WHEN** `elm_edit` is called with `action: "remove_import"` and a `module_name`
- **THEN** the tool SHALL remove the import for that module and write atomically

#### Scenario: Expose an item
- **WHEN** `elm_edit` is called with `action: "expose"` and an `item`
- **THEN** the tool SHALL add the item to the module's exposing list and write atomically

#### Scenario: Unexpose an item
- **WHEN** `elm_edit` is called with `action: "unexpose"` and an `item`
- **THEN** the tool SHALL remove the item from the module's exposing list and write atomically

#### Scenario: Add a variant constructor
- **WHEN** `elm_edit` is called with `action: "add_variant"`, `type_name`, and `definition`
- **THEN** the tool SHALL add the constructor to the type and insert `Debug.todo` branches in all matching case expressions project-wide, returning a JSON `VariantResult`

#### Scenario: Remove a variant constructor
- **WHEN** `elm_edit` is called with `action: "rm_variant"`, `type_name`, and `constructor`
- **THEN** the tool SHALL remove the constructor from the type and remove matching branches from all case expressions project-wide, returning a JSON `VariantResult`

#### Scenario: Variant dry run
- **WHEN** `add_variant` or `rm_variant` is called with `dry_run: true`
- **THEN** no files SHALL be written, and the result SHALL include `"dry_run": true`

#### Scenario: Write confirmation
- **WHEN** any `elm_edit` action completes successfully
- **THEN** the tool SHALL return a brief confirmation message
