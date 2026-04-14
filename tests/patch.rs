use std::io::Write;
use std::process::Command;

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


update : Msg -> Model -> Model
update msg model =
    case msg of
        Increment ->
            { model | count = model.count + 1 }

        Decrement ->
            { model | count = model.count - 1 }
"#;

#[test]
fn patch_success() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let output = elmq()
        .args([
            "patch",
            path,
            "update",
            "--old",
            "model.count + 1",
            "--new",
            "model.count + 2",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(content.contains("model.count + 2"));
    assert!(!content.contains("model.count + 1"));
    // Unchanged part preserved
    assert!(content.contains("model.count - 1"));
}

#[test]
fn patch_not_found_declaration() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let output = elmq()
        .args(["patch", path, "nonExistent", "--old", "x", "--new", "y"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not found"));
}

#[test]
fn patch_old_string_not_found() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let output = elmq()
        .args([
            "patch",
            path,
            "update",
            "--old",
            "nonexistent text",
            "--new",
            "replacement",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not found"));
}

#[test]
fn patch_ambiguous_match() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    // "model.count" appears multiple times in update
    let output = elmq()
        .args([
            "patch",
            path,
            "update",
            "--old",
            "model.count",
            "--new",
            "m.count",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("matches"));
}

#[test]
fn patch_multiline_old_string() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let output = elmq()
        .args([
            "patch",
            path,
            "update",
            "--old",
            "Increment ->\n            { model | count = model.count + 1 }",
            "--new",
            "Increment ->\n            { model | count = model.count + 5 }",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(content.contains("model.count + 5"));
    // Rest of file preserved
    assert!(content.contains("module Main exposing (..)"));
    assert!(content.contains("model.count - 1"));
}

const BROKEN: &str = "module Broken exposing (bar)\n\nbar =\n    let\n        x = 1\n";

#[test]
fn patch_rejects_input_with_parse_errors() {
    let f = with_temp_elm(BROKEN);
    let path = f.path().to_str().unwrap();
    let before = std::fs::read(f.path()).unwrap();

    let output = elmq()
        .args(["patch", path, "bar", "--old", "x", "--new", "y"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("refusing to edit") && stderr.contains(path),
        "stderr: {stderr}"
    );
    assert_eq!(std::fs::read(f.path()).unwrap(), before);
}

#[test]
fn patch_rejects_output_that_would_not_parse() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();
    let before = std::fs::read(f.path()).unwrap();

    // Replace a valid expression with an unbalanced brace — splice produces
    // a buffer that tree-sitter rejects.
    let output = elmq()
        .args([
            "patch",
            path,
            "update",
            "--old",
            "{ model | count = model.count + 1 }",
            "--new",
            "{ model | count = model.count + 1",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("rejected 'patch' write") && stderr.contains(path),
        "stderr: {stderr}"
    );
    assert!(stderr.contains(" at "), "stderr lacks line:col: {stderr}");
    assert_eq!(std::fs::read(f.path()).unwrap(), before);
}
