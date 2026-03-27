### Requirement: MCP stdio server startup
The system SHALL start an MCP server using stdio transport when the `elmq mcp` subcommand is invoked. The server SHALL register all elmq tools and handle JSON-RPC requests until the input stream closes.

#### Scenario: Server starts and responds to initialize
- **WHEN** `elmq mcp` is invoked
- **THEN** the server SHALL perform MCP capability negotiation over stdio and report available tools

#### Scenario: Server exits on input close
- **WHEN** the stdin stream is closed by the MCP client
- **THEN** the server SHALL exit cleanly with status code 0

### Requirement: Server metadata
The server SHALL identify itself with name `elmq` and the current crate version during MCP initialization.

#### Scenario: Server reports identity
- **WHEN** an MCP client connects and sends an initialize request
- **THEN** the server SHALL respond with server name `elmq` and version matching `Cargo.toml`

### Requirement: Compact output default
All MCP tool responses SHALL use compact text format by default. Read tools (`elm_summary`, `elm_get`) SHALL accept an optional `format` parameter with values `compact` (default) or `json`.

#### Scenario: Default format is compact
- **WHEN** a read tool is called without a `format` parameter
- **THEN** the response SHALL use compact text format

#### Scenario: JSON format requested
- **WHEN** a read tool is called with `format` set to `json`
- **THEN** the response SHALL use JSON format

### Requirement: Error handling
Tool errors SHALL be returned as MCP tool error responses with a human-readable message. The server SHALL NOT crash on invalid tool input.

#### Scenario: File not found
- **WHEN** a tool is called with a file path that does not exist
- **THEN** the tool SHALL return an error response with a message indicating the file was not found

#### Scenario: Declaration not found
- **WHEN** `elm_get` or `elm_edit` (patch/rm) is called with a name that does not exist in the file
- **THEN** the tool SHALL return an error response indicating the declaration was not found
