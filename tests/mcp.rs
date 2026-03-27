use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use serde_json::Value;

fn call_tool(name: &str, args: Value) -> Value {
    let mut child = Command::new(env!("CARGO_BIN_EXE_elmq"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to start elmq mcp");

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Send initialize and read response
    let init = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 0,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "test", "version": "1.0"}
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&init).unwrap()).unwrap();
    let mut init_resp = String::new();
    reader.read_line(&mut init_resp).unwrap();

    // Send initialized notification
    let initialized = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    });
    writeln!(stdin, "{}", serde_json::to_string(&initialized).unwrap()).unwrap();

    // Send tool call
    let tool_call = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": name,
            "arguments": args
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&tool_call).unwrap()).unwrap();

    // Read tool response
    let mut resp_line = String::new();
    reader.read_line(&mut resp_line).unwrap();

    // Close stdin to let server exit
    drop(stdin);
    let _ = child.wait();

    serde_json::from_str(&resp_line).expect("invalid JSON response")
}

fn result_text(response: &Value) -> &str {
    response["result"]["content"][0]["text"].as_str().unwrap()
}

fn is_error(response: &Value) -> bool {
    response["result"]["isError"].as_bool().unwrap_or(false)
}

// -- Server tests --

#[test]
fn server_initialize() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_elmq"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    let init = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 0,
        "method": "initialize",
        "params": {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "test", "version": "1.0"}
        }
    });
    writeln!(stdin, "{}", serde_json::to_string(&init).unwrap()).unwrap();

    let mut resp_line = String::new();
    reader.read_line(&mut resp_line).unwrap();
    let resp: Value = serde_json::from_str(&resp_line).unwrap();

    assert_eq!(resp["result"]["serverInfo"]["name"], "elmq");
    assert_eq!(
        resp["result"]["serverInfo"]["version"],
        env!("CARGO_PKG_VERSION")
    );
    assert!(resp["result"]["capabilities"]["tools"].is_object());

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn tools_list() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_elmq"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout);

    // Initialize
    writeln!(
        stdin,
        "{}",
        serde_json::to_string(&serde_json::json!({
            "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": {"protocolVersion": "2024-11-05", "capabilities": {},
                       "clientInfo": {"name": "test", "version": "1.0"}}
        }))
        .unwrap()
    )
    .unwrap();
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();

    // Initialized notification
    writeln!(
        stdin,
        "{}",
        serde_json::to_string(
            &serde_json::json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
        )
        .unwrap()
    )
    .unwrap();

    // List tools
    writeln!(
        stdin,
        "{}",
        serde_json::to_string(
            &serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"})
        )
        .unwrap()
    )
    .unwrap();

    let mut resp_line = String::new();
    reader.read_line(&mut resp_line).unwrap();
    let resp: Value = serde_json::from_str(&resp_line).unwrap();

    let tools = resp["result"]["tools"].as_array().unwrap();
    let names: Vec<&str> = tools.iter().map(|t| t["name"].as_str().unwrap()).collect();
    assert!(names.contains(&"elm_summary"));
    assert!(names.contains(&"elm_get"));
    assert!(names.contains(&"elm_edit"));
    assert!(names.contains(&"elm_module"));
    assert_eq!(names.len(), 4);

    drop(stdin);
    let _ = child.wait();
}

// -- elm_summary tests --

#[test]
fn summary_compact() {
    let resp = call_tool(
        "elm_summary",
        serde_json::json!({"file": "test-fixtures/Sample.elm"}),
    );
    assert!(!is_error(&resp));
    let text = result_text(&resp);
    assert!(text.contains("port module Sample exposing"));
    assert!(text.contains("imports:"));
    assert!(text.contains("functions:"));
    assert!(text.contains("update"));
}

#[test]
fn summary_json() {
    let resp = call_tool(
        "elm_summary",
        serde_json::json!({"file": "test-fixtures/Sample.elm", "format": "json"}),
    );
    assert!(!is_error(&resp));
    let text = result_text(&resp);
    let parsed: Value = serde_json::from_str(text).unwrap();
    assert!(parsed["module_line"].is_string());
    assert!(parsed["declarations"].is_array());
}

#[test]
fn summary_file_not_found() {
    let resp = call_tool(
        "elm_summary",
        serde_json::json!({"file": "nonexistent.elm"}),
    );
    assert!(is_error(&resp));
    assert!(result_text(&resp).contains("invalid path"));
}

// -- elm_get tests --

#[test]
fn get_declaration() {
    let resp = call_tool(
        "elm_get",
        serde_json::json!({"file": "test-fixtures/Sample.elm", "name": "update"}),
    );
    assert!(!is_error(&resp));
    let text = result_text(&resp);
    assert!(text.starts_with("update : Msg -> Model -> Model"));
    assert!(text.contains("case msg of"));
}

#[test]
fn get_declaration_json() {
    let resp = call_tool(
        "elm_get",
        serde_json::json!({"file": "test-fixtures/Sample.elm", "name": "update", "format": "json"}),
    );
    assert!(!is_error(&resp));
    let text = result_text(&resp);
    let parsed: Value = serde_json::from_str(text).unwrap();
    assert_eq!(parsed["name"], "update");
    assert_eq!(parsed["kind"], "function");
    assert!(parsed["source"].as_str().unwrap().contains("case msg of"));
}

#[test]
fn get_declaration_not_found() {
    let resp = call_tool(
        "elm_get",
        serde_json::json!({"file": "test-fixtures/Sample.elm", "name": "nonexistent"}),
    );
    assert!(is_error(&resp));
    assert!(result_text(&resp).contains("not found"));
}

// -- elm_edit tests --

#[test]
fn edit_set_declaration() {
    let dir = tempfile::tempdir_in(".").unwrap();
    let file = dir.path().join("Test.elm");
    std::fs::write(&file, "module Test exposing (foo)\n\n\nfoo =\n    42\n").unwrap();

    let resp = call_tool(
        "elm_edit",
        serde_json::json!({
            "file": file.to_str().unwrap(),
            "action": "set",
            "source": "bar =\n    99\n"
        }),
    );
    assert!(!is_error(&resp));
    assert!(result_text(&resp).contains("set bar"));

    let content = std::fs::read_to_string(&file).unwrap();
    assert!(content.contains("bar =\n    99"));
}

#[test]
fn edit_patch_declaration() {
    let dir = tempfile::tempdir_in(".").unwrap();
    let file = dir.path().join("Test.elm");
    std::fs::write(&file, "module Test exposing (foo)\n\n\nfoo =\n    42\n").unwrap();

    let resp = call_tool(
        "elm_edit",
        serde_json::json!({
            "file": file.to_str().unwrap(),
            "action": "patch",
            "name": "foo",
            "old": "42",
            "new": "99"
        }),
    );
    assert!(!is_error(&resp));
    assert!(result_text(&resp).contains("patched foo"));

    let content = std::fs::read_to_string(&file).unwrap();
    assert!(content.contains("99"));
    assert!(!content.contains("42"));
}

#[test]
fn edit_rm_declaration() {
    let dir = tempfile::tempdir_in(".").unwrap();
    let file = dir.path().join("Test.elm");
    std::fs::write(
        &file,
        "module Test exposing (foo, bar)\n\n\nfoo =\n    42\n\n\nbar =\n    99\n",
    )
    .unwrap();

    let resp = call_tool(
        "elm_edit",
        serde_json::json!({
            "file": file.to_str().unwrap(),
            "action": "rm",
            "name": "foo"
        }),
    );
    assert!(!is_error(&resp));
    assert!(result_text(&resp).contains("removed foo"));

    let content = std::fs::read_to_string(&file).unwrap();
    assert!(!content.contains("foo ="));
    assert!(content.contains("bar"));
}

#[test]
fn edit_invalid_action() {
    let resp = call_tool(
        "elm_edit",
        serde_json::json!({
            "file": "test-fixtures/Sample.elm",
            "action": "invalid"
        }),
    );
    // serde rejects unknown enum variants at the protocol level
    assert!(resp.get("error").is_some() || is_error(&resp));
}

#[test]
fn edit_missing_required_params() {
    let resp = call_tool(
        "elm_edit",
        serde_json::json!({
            "file": "test-fixtures/Sample.elm",
            "action": "patch",
            "name": "update"
        }),
    );
    assert!(is_error(&resp));
    assert!(result_text(&resp).contains("\"old\" is required"));
}

// -- elm_module tests --

#[test]
fn module_add_import() {
    let dir = tempfile::tempdir_in(".").unwrap();
    let file = dir.path().join("Test.elm");
    std::fs::write(
        &file,
        "module Test exposing (foo)\n\nimport Html\n\n\nfoo =\n    42\n",
    )
    .unwrap();

    let resp = call_tool(
        "elm_module",
        serde_json::json!({
            "file": file.to_str().unwrap(),
            "action": "add_import",
            "import": "Json.Decode exposing (Decoder)"
        }),
    );
    assert!(!is_error(&resp));

    let content = std::fs::read_to_string(&file).unwrap();
    assert!(content.contains("import Json.Decode exposing (Decoder)"));
}

#[test]
fn module_remove_import() {
    let dir = tempfile::tempdir_in(".").unwrap();
    let file = dir.path().join("Test.elm");
    std::fs::write(
        &file,
        "module Test exposing (foo)\n\nimport Html\nimport Json.Decode\n\n\nfoo =\n    42\n",
    )
    .unwrap();

    let resp = call_tool(
        "elm_module",
        serde_json::json!({
            "file": file.to_str().unwrap(),
            "action": "remove_import",
            "module_name": "Html"
        }),
    );
    assert!(!is_error(&resp));

    let content = std::fs::read_to_string(&file).unwrap();
    assert!(!content.contains("import Html"));
    assert!(content.contains("import Json.Decode"));
}

#[test]
fn module_expose() {
    let dir = tempfile::tempdir_in(".").unwrap();
    let file = dir.path().join("Test.elm");
    std::fs::write(
        &file,
        "module Test exposing (foo)\n\n\nfoo =\n    42\n\n\nbar =\n    99\n",
    )
    .unwrap();

    let resp = call_tool(
        "elm_module",
        serde_json::json!({
            "file": file.to_str().unwrap(),
            "action": "expose",
            "item": "bar"
        }),
    );
    assert!(!is_error(&resp));

    let content = std::fs::read_to_string(&file).unwrap();
    assert!(content.contains("bar"));
    assert!(content.contains("foo"));
}

#[test]
fn module_unexpose() {
    let dir = tempfile::tempdir_in(".").unwrap();
    let file = dir.path().join("Test.elm");
    std::fs::write(
        &file,
        "module Test exposing (foo, bar)\n\n\nfoo =\n    42\n\n\nbar =\n    99\n",
    )
    .unwrap();

    let resp = call_tool(
        "elm_module",
        serde_json::json!({
            "file": file.to_str().unwrap(),
            "action": "unexpose",
            "item": "bar"
        }),
    );
    assert!(!is_error(&resp));

    let content = std::fs::read_to_string(&file).unwrap();
    assert!(content.contains("exposing (foo)"));
}

#[test]
fn module_invalid_action() {
    let resp = call_tool(
        "elm_module",
        serde_json::json!({
            "file": "test-fixtures/Sample.elm",
            "action": "invalid"
        }),
    );
    // serde rejects unknown enum variants at the protocol level
    assert!(resp.get("error").is_some() || is_error(&resp));
}

// -- Path traversal tests --

#[test]
fn rejects_absolute_path_outside_cwd() {
    let path = if cfg!(windows) {
        r"C:\Windows\System32\drivers\etc\hosts"
    } else {
        "/etc/hosts"
    };
    let resp = call_tool("elm_summary", serde_json::json!({"file": path}));
    assert!(is_error(&resp));
    assert!(result_text(&resp).contains("outside the working directory"));
}

#[test]
fn rejects_relative_path_traversal() {
    let resp = call_tool(
        "elm_summary",
        serde_json::json!({"file": "../../etc/passwd"}),
    );
    assert!(is_error(&resp));
    // Either "outside the working directory" or "invalid path" (if file doesn't exist)
    let text = result_text(&resp);
    assert!(text.contains("outside the working directory") || text.contains("invalid path"),);
}
