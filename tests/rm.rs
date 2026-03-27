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


{-| A documented function -}
view : Html msg
view =
    Html.text "hello"


helper x =
    x + 1


another y =
    y * 2
"#;

#[test]
fn rm_with_doc_comment_and_annotation() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let output = elmq().args(["rm", path, "view"]).output().unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(!content.contains("view"));
    assert!(!content.contains("A documented function"));
    assert!(!content.contains("Html msg"));
    // Others preserved
    assert!(content.contains("helper x ="));
    assert!(content.contains("another y ="));
}

#[test]
fn rm_without_doc_comment() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let output = elmq().args(["rm", path, "helper"]).output().unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(!content.contains("helper"));
    assert!(!content.contains("x + 1"));
    assert!(content.contains("view"));
    assert!(content.contains("another y ="));
}

#[test]
fn rm_whitespace_cleanup() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let output = elmq().args(["rm", path, "helper"]).output().unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    // Should not have more than 2 consecutive blank lines
    assert!(!content.contains("\n\n\n\n"));
}

#[test]
fn rm_not_found() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let output = elmq().args(["rm", path, "nonExistent"]).output().unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not found"));
}

#[test]
fn rm_first_declaration() {
    // view is the first declaration in SAMPLE
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let output = elmq().args(["rm", path, "view"]).output().unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(content.starts_with("module Main exposing (..)"));
    assert!(content.contains("helper x ="));
    assert!(!content.contains("\n\n\n\n"));
}

#[test]
fn rm_last_declaration() {
    // another is the last declaration in SAMPLE
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let output = elmq().args(["rm", path, "another"]).output().unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(!content.contains("another"));
    assert!(content.contains("helper x ="));
}

#[test]
fn rm_only_declaration() {
    let f = with_temp_elm("module Main exposing (..)\n\n\nview = 1\n");
    let path = f.path().to_str().unwrap();

    let output = elmq().args(["rm", path, "view"]).output().unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(content.contains("module Main exposing (..)"));
    assert!(!content.contains("view"));
}
