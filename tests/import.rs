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
        .args(["add", "import", path, "Browser exposing (element)"])
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
            "add",
            "import",
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
        .args(["add", "import", path, "Html exposing (Html)"])
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
        .args(["rm", "import", path, "Html"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(!content.contains("import Html exposing"));
    assert!(content.contains("import Html.Attributes as Attr"));
}

#[test]
fn import_remove_not_found_is_idempotent_noop() {
    // Breaking change vs. prior behavior: removing an import that does not
    // exist is a successful no-op, not an error. See batch-positional-args
    // change for rationale.
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();
    let before = std::fs::read_to_string(f.path()).unwrap();

    let output = elmq()
        .args(["rm", "import", path, "NonExistent"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let after = std::fs::read_to_string(f.path()).unwrap();
    assert_eq!(after, before, "file should be unchanged");
}

#[test]
fn import_add_with_import_prefix() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let output = elmq()
        .args(["add", "import", path, "import Browser exposing (element)"])
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
        .args(["rm", "import", path, "Html"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(!content.contains("import"));
    assert!(content.contains("view = 1"));
    assert!(!content.contains("\n\n\n\n"));
}

#[test]
fn import_add_multi_clause_success() {
    let f = with_temp_elm(NO_IMPORTS);
    let path = f.path().to_str().unwrap();

    let output = elmq()
        .args([
            "add",
            "import",
            path,
            "Http",
            "Json.Decode exposing (field)",
            "Html exposing (div)",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(content.contains("import Http"));
    assert!(content.contains("import Json.Decode exposing (field)"));
    assert!(content.contains("import Html exposing (div)"));

    // Each clause is placed in alphabetical position independently, so the
    // final file order should be Html, Http, Json.Decode.
    let html_pos = content.find("import Html").unwrap();
    let http_pos = content.find("import Http").unwrap();
    let json_pos = content.find("import Json.Decode").unwrap();
    assert!(html_pos < http_pos);
    assert!(http_pos < json_pos);
}

#[test]
fn import_add_multi_clause_last_wins() {
    let f = with_temp_elm(NO_IMPORTS);
    let path = f.path().to_str().unwrap();

    let output = elmq()
        .args([
            "add",
            "import",
            path,
            "Html exposing (div)",
            "Html exposing (text)",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(content.contains("import Html exposing (text)"));
    assert!(!content.contains("import Html exposing (div)"));
}

#[test]
fn import_add_multi_clause_input_order_headers() {
    let f = with_temp_elm(NO_IMPORTS);
    let path = f.path().to_str().unwrap();

    let output = elmq()
        .args([
            "add",
            "import",
            path,
            "Json.Decode exposing (field)",
            "Http",
            "Html exposing (div)",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let p1 = stdout
        .find("## Json.Decode exposing (field)")
        .expect("missing header for clause 1");
    let p2 = stdout.find("## Http").expect("missing header for clause 2");
    let p3 = stdout
        .find("## Html exposing (div)")
        .expect("missing header for clause 3");
    assert!(p1 < p2, "clause 1 header should precede clause 2 header");
    assert!(p2 < p3, "clause 2 header should precede clause 3 header");
}

#[test]
fn import_remove_multi_module_success() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let output = elmq()
        .args(["rm", "import", path, "Html", "Html.Attributes"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(!content.contains("import Html exposing"));
    assert!(!content.contains("import Html.Attributes"));
}

#[test]
fn import_remove_multi_module_partial_idempotent() {
    let f = with_temp_elm(SAMPLE);
    let path = f.path().to_str().unwrap();

    let output = elmq()
        .args([
            "rm",
            "import",
            path,
            "Html",
            "NonExistent",
            "Html.Attributes",
        ])
        .output()
        .unwrap();
    // Missing-module removal is idempotent, not an error — overall exit 0.
    assert_eq!(output.status.code(), Some(0));

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(!content.contains("import Html exposing"));
    assert!(!content.contains("import Html.Attributes"));
}

#[test]
fn import_single_clause_output_unchanged() {
    let f = with_temp_elm(NO_IMPORTS);
    let path = f.path().to_str().unwrap();

    let output = elmq()
        .args(["add", "import", path, "Http"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        !stdout.contains("##"),
        "single-arg output should not contain header markers, got: {stdout:?}"
    );
}

const BROKEN: &str = "module Broken exposing (bar)\n\nbar =\n    let\n        x = 1\n";

#[test]
fn import_add_rejects_input_with_parse_errors() {
    let f = with_temp_elm(BROKEN);
    let path = f.path().to_str().unwrap();
    let before = std::fs::read(f.path()).unwrap();

    let output = elmq()
        .args(["add", "import", path, "Html"])
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
fn import_remove_rejects_input_with_parse_errors() {
    let f = with_temp_elm(BROKEN);
    let path = f.path().to_str().unwrap();
    let before = std::fs::read(f.path()).unwrap();

    let output = elmq()
        .args(["rm", "import", path, "Html"])
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
