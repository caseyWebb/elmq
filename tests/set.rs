use std::io::Write;
use std::process::{Command, Stdio};

fn elmq() -> Command {
    Command::new(env!("CARGO_BIN_EXE_elmq"))
}

fn with_temp_elm(content: &str) -> tempfile::NamedTempFile {
    let mut f = tempfile::Builder::new().suffix(".elm").tempfile().unwrap();
    f.write_all(content.as_bytes()).unwrap();
    f.flush().unwrap();
    f
}

const SAMPLE: &str = r#"module Main exposing (..)

import Html exposing (Html)


view : Html msg
view =
    Html.text "hello"


helper x =
    x + 1
"#;

#[test]
fn set_replace_existing() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let mut child = elmq()
        .args(["set", path])
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();

    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"helper x =\n    x + 42\n")
        .unwrap();

    let status = child.wait().unwrap();
    assert!(status.success());

    let output = elmq().args(["get", path, "helper"]).output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("x + 42"));
    assert!(!stdout.contains("x + 1"));
}

#[test]
fn set_append_new() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let mut child = elmq()
        .args(["set", path])
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();

    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"newFunc y =\n    y * 2\n")
        .unwrap();

    let status = child.wait().unwrap();
    assert!(status.success());

    let output = elmq().args(["get", path, "newFunc"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("y * 2"));
}

#[test]
fn set_with_name_override() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let mut child = elmq()
        .args(["set", path, "--name", "helper"])
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();

    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"renamed x =\n    x + 99\n")
        .unwrap();

    let status = child.wait().unwrap();
    assert!(status.success());

    // The old "helper" location should now have the new content
    // (replaced by --name targeting "helper")
    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(content.contains("renamed x ="));
    assert!(!content.contains("x + 1"));
}

#[test]
fn set_parse_error_without_name() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let mut child = elmq()
        .args(["set", path])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"not valid elm {{{")
        .unwrap();

    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("--name"));
}

#[test]
fn set_upsert_type_declaration() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let mut child = elmq()
        .args(["set", path])
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();

    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"type Msg\n    = Increment\n    | Decrement\n")
        .unwrap();

    let status = child.wait().unwrap();
    assert!(status.success());

    let output = elmq().args(["get", path, "Msg"]).output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Decrement"));
}

#[test]
fn set_append_to_file_with_no_declarations() {
    let f = with_temp_elm("module Main exposing (..)\n\nimport Html\n");
    let path = f.path().to_str().unwrap();

    let mut child = elmq()
        .args(["set", path])
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();

    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"view = 1\n")
        .unwrap();

    let status = child.wait().unwrap();
    assert!(status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(content.contains("module Main exposing (..)"));
    assert!(content.contains("import Html"));
    assert!(content.contains("view = 1"));
}
