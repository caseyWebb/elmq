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
        .args(["set", "decl", path])
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
        .args(["set", "decl", path])
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
fn set_name_mismatch_with_parsed_name_errors() {
    // New behavior: --name that disagrees with the parsed name in the
    // content errors out instead of silently renaming. To rename a
    // declaration, use `rename decl`.
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let mut child = elmq()
        .args(["set", "decl", path, "--name", "helper"])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"renamed x =\n    x + 99\n")
        .unwrap();

    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("does not match parsed name"),
        "unexpected stderr: {stderr}"
    );

    // File should be unchanged.
    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(content.contains("helper x =\n    x + 1"));
    assert!(!content.contains("renamed x ="));
}

#[test]
fn set_parse_error_without_name() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let mut child = elmq()
        .args(["set", "decl", path])
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
        .args(["set", "decl", path])
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

const BROKEN: &str = "module Broken exposing (bar)\n\nbar =\n    let\n        x = 1\n";

#[test]
fn set_rejects_input_with_parse_errors() {
    let f = with_temp_elm(BROKEN);
    let path = f.path().to_str().unwrap();
    let before = std::fs::read(f.path()).unwrap();

    let mut child = elmq()
        .args(["set", "decl", path, "--name", "bar"])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"bar =\n    42\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("refusing to edit") && stderr.contains(path),
        "stderr: {stderr}"
    );
    assert_eq!(std::fs::read(f.path()).unwrap(), before);
}

#[test]
fn set_rejects_output_that_would_not_parse() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();
    let before = std::fs::read(f.path()).unwrap();

    let mut child = elmq()
        .args(["set", "decl", path, "--name", "helper"])
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"helper =\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("rejected 'set decl' write") && stderr.contains(path),
        "stderr: {stderr}"
    );
    assert!(stderr.contains(" at "), "stderr lacks line:col: {stderr}");
    assert_eq!(std::fs::read(f.path()).unwrap(), before);
}

#[test]
fn set_append_to_file_with_no_declarations() {
    let f = with_temp_elm("module Main exposing (..)\n\nimport Html\n");
    let path = f.path().to_str().unwrap();

    let mut child = elmq()
        .args(["set", "decl", path])
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

#[test]
fn set_decl_content_flag_alternative_to_stdin() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let output = elmq()
        .args(["set", "decl", path, "--content", "newFn = 42"])
        .output()
        .unwrap();

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout, "ok\n");

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(content.contains("newFn = 42"));
}

#[test]
fn set_decl_name_mismatch_errors() {
    let f = with_temp_elm("module Main exposing (..)\n\nfoo = 1\n");
    let path = f.path().to_str().unwrap();
    let before = std::fs::read_to_string(f.path()).unwrap();

    let output = elmq()
        .args(["set", "decl", path, "--name", "foo", "--content", "bar = 2"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("does not match parsed name"),
        "stderr: {stderr}"
    );

    let after = std::fs::read_to_string(f.path()).unwrap();
    assert_eq!(after, before);
}
