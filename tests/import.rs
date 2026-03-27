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

import Html exposing (Html, div, text)
import Html.Attributes as Attr
"#;

const NO_IMPORTS: &str = r#"module Main exposing (..)


view =
    42
"#;

#[test]
fn import_add_new() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let output = elmq()
        .args(["import", "add", path, "Browser exposing (element)"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(content.contains("import Browser exposing (element)"));
    // Should be alphabetically before Html
    let browser_pos = content.find("import Browser").unwrap();
    let html_pos = content.find("import Html").unwrap();
    assert!(browser_pos < html_pos);
}

#[test]
fn import_add_replace_existing() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let output = elmq()
        .args([
            "import",
            "add",
            path,
            "Html exposing (Html, div, text, span)",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(content.contains("import Html exposing (Html, div, text, span)"));
    assert!(!content.contains("import Html exposing (Html, div, text)\n"));
}

#[test]
fn import_add_to_file_with_no_imports() {
    let f = with_temp_elm(NO_IMPORTS);
    let path = f.path().to_str().unwrap();

    let output = elmq()
        .args(["import", "add", path, "Html exposing (Html)"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(content.contains("import Html exposing (Html)"));
}

#[test]
fn import_remove_existing() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let output = elmq()
        .args(["import", "remove", path, "Html"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(!content.contains("import Html exposing"));
    assert!(content.contains("import Html.Attributes as Attr"));
}

#[test]
fn import_remove_not_found() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let output = elmq()
        .args(["import", "remove", path, "NonExistent"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not found"));
}

#[test]
fn import_add_with_import_prefix() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let output = elmq()
        .args(["import", "add", path, "import Browser exposing (element)"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    // Should not double the "import" prefix
    assert!(content.contains("import Browser exposing (element)"));
    assert!(!content.contains("import import"));
}

#[test]
fn import_remove_last_import() {
    let f = with_temp_elm("module Main exposing (..)\n\nimport Html\n\nview = 1\n");
    let path = f.path().to_str().unwrap();

    let output = elmq()
        .args(["import", "remove", path, "Html"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(!content.contains("import"));
    assert!(content.contains("view = 1"));
    assert!(!content.contains("\n\n\n\n"));
}
