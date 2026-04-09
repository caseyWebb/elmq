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

#[test]
fn expose_add_item() {
    let f = with_temp_elm("module Main exposing (view)\n\nview = 1\n\nupdate = 2\n");
    let path = f.path().to_str().unwrap();

    let output = elmq().args(["expose", path, "update"]).output().unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(content.starts_with("module Main exposing (view, update)"));
}

#[test]
fn expose_type_with_constructors() {
    let f = with_temp_elm("module Main exposing (view)\n\ntype Msg = Inc\n\nview = 1\n");
    let path = f.path().to_str().unwrap();

    let output = elmq().args(["expose", path, "Msg(..)"]).output().unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(content.starts_with("module Main exposing (view, Msg(..))"));
}

#[test]
fn expose_already_exposed_noop() {
    let f = with_temp_elm("module Main exposing (view)\n\nview = 1\n");
    let path = f.path().to_str().unwrap();

    let output = elmq().args(["expose", path, "view"]).output().unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(content.starts_with("module Main exposing (view)"));
}

#[test]
fn expose_when_expose_all_noop() {
    let f = with_temp_elm("module Main exposing (..)\n\nview = 1\n");
    let path = f.path().to_str().unwrap();

    let output = elmq().args(["expose", path, "update"]).output().unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(content.starts_with("module Main exposing (..)"));
}

#[test]
fn unexpose_item() {
    let f = with_temp_elm("module Main exposing (view, helper)\n\nview = 1\n\nhelper = 2\n");
    let path = f.path().to_str().unwrap();

    let output = elmq().args(["unexpose", path, "helper"]).output().unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(content.starts_with("module Main exposing (view)"));
}

#[test]
fn unexpose_auto_expand_expose_all() {
    let f =
        with_temp_elm("module Main exposing (..)\n\ntype Msg = Inc\n\nview = 1\n\nhelper = 2\n");
    let path = f.path().to_str().unwrap();

    let output = elmq().args(["unexpose", path, "helper"]).output().unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    // Should have expanded (..) and removed helper from exposing
    let first_line = content.lines().next().unwrap();
    assert!(!first_line.contains("exposing (..)"));
    assert!(!first_line.contains("helper"));
    assert!(first_line.contains("Msg(..)"));
    assert!(first_line.contains("view"));
}

#[test]
fn unexpose_not_in_list_is_idempotent_noop() {
    // Breaking change vs. prior behavior: unexposing an item that is not in
    // the exposing list is a successful no-op, not an error. See
    // batch-positional-args change for rationale (the batching contract
    // depends on the idempotent write path).
    let source = "module Main exposing (view)\n\nview = 1\n";
    let f = with_temp_elm(source);
    let path = f.path().to_str().unwrap();

    let output = elmq().args(["unexpose", path, "helper"]).output().unwrap();
    assert!(output.status.success());
    let content = std::fs::read_to_string(f.path()).unwrap();
    assert_eq!(content, source, "file should be unchanged");
}

#[test]
fn expose_port_module() {
    let f = with_temp_elm(
        "port module Ports exposing (sendMessage)\n\nport sendMessage : String -> Cmd msg\n\nview = 1\n",
    );
    let path = f.path().to_str().unwrap();

    let output = elmq().args(["expose", path, "view"]).output().unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(content.starts_with("port module Ports exposing (sendMessage, view)"));
}

#[test]
fn unexpose_type_with_constructors() {
    let f = with_temp_elm("module Main exposing (view, Msg(..))\n\ntype Msg = Inc\n\nview = 1\n");
    let path = f.path().to_str().unwrap();

    let output = elmq().args(["unexpose", path, "Msg"]).output().unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    let first_line = content.lines().next().unwrap();
    assert_eq!(first_line, "module Main exposing (view)");
}

#[test]
fn expose_with_mixed_items() {
    let f = with_temp_elm(
        "module Main exposing (Msg(..), view)\n\ntype Msg = Inc\n\nview = 1\n\nupdate = 2\n",
    );
    let path = f.path().to_str().unwrap();

    let output = elmq().args(["expose", path, "update"]).output().unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(content.starts_with("module Main exposing (Msg(..), view, update)"));
}

#[test]
fn expose_multiline_module_declaration() {
    let f = with_temp_elm(
        "module Main\n    exposing\n        ( view\n        )\n\nview = 1\n\nupdate = 2\n",
    );
    let path = f.path().to_str().unwrap();

    let output = elmq().args(["expose", path, "update"]).output().unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(content.contains("exposing (view, update)"));
}

#[test]
fn unexpose_multiline_module_declaration() {
    let f = with_temp_elm(
        "module Main\n    exposing\n        ( view\n        , helper\n        )\n\nview = 1\n\nhelper = 2\n",
    );
    let path = f.path().to_str().unwrap();

    let output = elmq().args(["unexpose", path, "helper"]).output().unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(content.contains("exposing (view)"));
    assert!(!content.lines().next().unwrap_or("").contains("helper"));
}

#[test]
fn expose_preserves_rest_of_file() {
    let f = with_temp_elm("module Main exposing (view)\n\nimport Html\n\nview = 1\n\nhelper = 2\n");
    let path = f.path().to_str().unwrap();

    let output = elmq().args(["expose", path, "helper"]).output().unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    assert!(content.contains("import Html"));
    assert!(content.contains("view = 1"));
    assert!(content.contains("helper = 2"));
}

#[test]
fn expose_multi_item_success() {
    let f = with_temp_elm(
        "module Main exposing (view)\n\ntype Msg = A | B\n\nview = 1\nupdate = 2\ninit = 3\n",
    );
    let path = f.path().to_str().unwrap();

    let output = elmq()
        .args(["expose", path, "update", "init", "Msg(..)"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    let first_line = content.lines().next().unwrap();
    assert!(first_line.contains("view"), "missing view: {first_line}");
    assert!(
        first_line.contains("update"),
        "missing update: {first_line}"
    );
    assert!(first_line.contains("init"), "missing init: {first_line}");
    assert!(
        first_line.contains("Msg(..)"),
        "missing Msg(..): {first_line}"
    );
}

#[test]
fn expose_multi_item_input_order_headers() {
    let f = with_temp_elm("module Main exposing (view)\n\nview = 0\na = 1\nb = 2\nc = 3\n");
    let path = f.path().to_str().unwrap();

    let output = elmq()
        .args(["expose", path, "a", "b", "c"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    let pos_a = stdout.find("## a").expect("missing ## a header");
    let pos_b = stdout.find("## b").expect("missing ## b header");
    let pos_c = stdout.find("## c").expect("missing ## c header");
    assert!(pos_a < pos_b, "expected ## a before ## b");
    assert!(pos_b < pos_c, "expected ## b before ## c");
}

#[test]
fn unexpose_multi_item_success() {
    let f = with_temp_elm(
        "module Main exposing (view, helper, internal, debug)\n\nview = 1\nhelper = 2\ninternal = 3\ndebug = 4\n",
    );
    let path = f.path().to_str().unwrap();

    let output = elmq()
        .args(["unexpose", path, "helper", "internal", "debug"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let content = std::fs::read_to_string(f.path()).unwrap();
    let first_line = content.lines().next().unwrap();
    assert_eq!(first_line, "module Main exposing (view)");
}

#[test]
fn unexpose_multi_item_partial_idempotent() {
    let f = with_temp_elm(
        "module Main exposing (view, helper, debug)\n\nview = 1\nhelper = 2\ndebug = 3\n",
    );
    let path = f.path().to_str().unwrap();

    let output = elmq()
        .args(["unexpose", path, "helper", "nonExistent", "debug"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0));

    let content = std::fs::read_to_string(f.path()).unwrap();
    let first_line = content.lines().next().unwrap();
    assert_eq!(first_line, "module Main exposing (view)");
}

#[test]
fn expose_single_item_output_unchanged() {
    let f = with_temp_elm("module Main exposing (view)\n\nview = 1\nupdate = 2\n");
    let path = f.path().to_str().unwrap();

    let output = elmq().args(["expose", path, "update"]).output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        !stdout.contains("##"),
        "single-item output should be bare, got: {stdout:?}"
    );
}
