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
fn unexpose_not_in_list() {
    let f = with_temp_elm("module Main exposing (view)\n\nview = 1\n");
    let path = f.path().to_str().unwrap();

    let output = elmq().args(["unexpose", path, "helper"]).output().unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("not in the exposing list"));
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
