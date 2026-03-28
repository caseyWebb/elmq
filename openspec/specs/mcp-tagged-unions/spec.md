### Requirement: Tagged union parameters for elm_edit
The `elm_edit` MCP tool SHALL use an internally-tagged enum (`#[serde(tag = "action")]`) for its parameters, where each action variant carries exactly the fields it requires.

#### Scenario: Set action
- **WHEN** `elm_edit` is called with `{"action": "set", "file": "Foo.elm", "source": "foo = 1"}`
- **THEN** it SHALL upsert the declaration (unchanged behavior)

#### Scenario: Patch action
- **WHEN** `elm_edit` is called with `{"action": "patch", "file": "Foo.elm", "name": "view", "old": "div", "new": "span"}`
- **THEN** it SHALL patch the declaration (unchanged behavior)

#### Scenario: Missing required field
- **WHEN** `elm_edit` is called with `{"action": "patch", "file": "Foo.elm", "name": "view"}` (missing `old` and `new`)
- **THEN** it SHALL return a deserialization error (enforced by the type system, not runtime validation)

#### Scenario: Wire format backwards compatibility
- **GIVEN** an existing MCP client sending the current flat JSON format
- **WHEN** any `elm_edit` action is called
- **THEN** the JSON SHALL deserialize correctly because the internally-tagged format matches the existing wire shape

### Requirement: Tagged union parameters for elm_module
The `elm_module` MCP tool SHALL use the same internally-tagged enum pattern for its parameters.

#### Scenario: Add import action
- **WHEN** `elm_module` is called with `{"action": "add_import", "file": "Foo.elm", "import": "Html exposing (div)"}`
- **THEN** it SHALL add the import (unchanged behavior)

#### Scenario: Expose action
- **WHEN** `elm_module` is called with `{"action": "expose", "file": "Foo.elm", "item": "update"}`
- **THEN** it SHALL expose the item (unchanged behavior)
