use std::process::Command;

fn elmq() -> Command {
    Command::new(env!("CARGO_BIN_EXE_elmq"))
}

#[test]
fn get_function_with_type_annotation() {
    let output = elmq()
        .args(["get", "test-fixtures/Sample.elm", "update"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("update : Msg -> Model -> Model\n"));
    assert!(stdout.contains("case msg of"));
}

#[test]
fn get_function_without_type_annotation() {
    let output = elmq()
        .args(["get", "test-fixtures/Sample.elm", "helper"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("helper x ="));
    assert!(!stdout.contains(":"));
}

#[test]
fn get_type_declaration() {
    let output = elmq()
        .args(["get", "test-fixtures/Sample.elm", "Msg"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("{-| Messages for the update function -}"));
    assert!(stdout.contains("type Msg"));
    assert!(stdout.contains("| Reset"));
}

#[test]
fn get_type_alias() {
    let output = elmq()
        .args(["get", "test-fixtures/Sample.elm", "Model"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("{-| The model for our app -}"));
    assert!(stdout.contains("type alias Model ="));
    assert!(stdout.contains(", name : String"));
}

#[test]
fn get_port_declaration() {
    let output = elmq()
        .args(["get", "test-fixtures/Sample.elm", "sendMessage"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("port sendMessage : String -> Cmd msg"));
}

#[test]
fn get_not_found() {
    let output = elmq()
        .args(["get", "test-fixtures/Sample.elm", "nonExistent"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not found"));
}

#[test]
fn get_json_format() {
    let output = elmq()
        .args([
            "get",
            "test-fixtures/Sample.elm",
            "update",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["name"], "update");
    assert_eq!(json["kind"], "function");
    assert!(json["source"].as_str().unwrap().contains("case msg of"));
    assert!(json["start_line"].is_number());
    assert!(json["end_line"].is_number());
}

#[test]
fn get_default_format_is_compact() {
    let compact = elmq()
        .args(["get", "test-fixtures/Sample.elm", "update"])
        .output()
        .unwrap();

    let explicit = elmq()
        .args([
            "get",
            "test-fixtures/Sample.elm",
            "update",
            "--format",
            "compact",
        ])
        .output()
        .unwrap();

    assert_eq!(compact.stdout, explicit.stdout);
}
