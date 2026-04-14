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

fn read(f: &tempfile::NamedTempFile) -> String {
    std::fs::read_to_string(f.path()).unwrap()
}

const SIMPLE_CASE: &str = "module Main exposing (..)

update msg model =
    case msg of
        Increment ->
            model + 1

        Decrement ->
            model - 1
";

const CASE_WITH_WILDCARD: &str = "module Main exposing (..)

toLabel n =
    case n of
        0 ->
            \"zero\"

        _ ->
            \"other\"
";

const TWO_CASES: &str = "module Main exposing (..)

view model =
    let
        routeLabel =
            case model.route of
                Home ->
                    \"home\"

                About ->
                    \"about\"

        stateLabel =
            case model.state of
                Loading ->
                    \"loading\"

                Loaded ->
                    \"loaded\"
    in
    routeLabel ++ stateLabel
";

// --------- set case: replace body -------------------------------------

#[test]
fn set_case_replaces_existing_branch_body() {
    let f = with_temp_elm(SIMPLE_CASE);
    let path = f.path().to_str().unwrap();

    let status = elmq()
        .args([
            "set",
            "case",
            path,
            "update",
            "--pattern",
            "Increment",
            "--body",
            "model + 2",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let c = read(&f);
    assert!(c.contains("model + 2"));
    assert!(!c.contains("model + 1"));
    assert!(c.contains("model - 1"), "other branch intact");
}

// --------- set case: add new branch -----------------------------------

#[test]
fn set_case_adds_new_branch_at_end() {
    let f = with_temp_elm(SIMPLE_CASE);
    let path = f.path().to_str().unwrap();

    let status = elmq()
        .args([
            "set",
            "case",
            path,
            "update",
            "--pattern",
            "Reset",
            "--body",
            "0",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let c = read(&f);
    assert!(c.contains("Reset ->"));
    let inc_pos = c.find("Increment ->").unwrap();
    let reset_pos = c.find("Reset ->").unwrap();
    assert!(inc_pos < reset_pos, "new branch appended: {c}");
}

// --------- set case: insert before wildcard ---------------------------

#[test]
fn set_case_inserts_before_wildcard() {
    let f = with_temp_elm(CASE_WITH_WILDCARD);
    let path = f.path().to_str().unwrap();

    let status = elmq()
        .args([
            "set",
            "case",
            path,
            "toLabel",
            "--pattern",
            "1",
            "--body",
            "\"one\"",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let c = read(&f);
    let one_pos = c.find("1 ->").unwrap();
    let wild_pos = c.find("_ ->").unwrap();
    assert!(one_pos < wild_pos, "1 branch is before wildcard: {c}");
}

// --------- set case: scrutinee ambiguity -----------------------------

#[test]
fn set_case_scrutinee_ambiguity_errors_then_on_resolves() {
    let f = with_temp_elm(TWO_CASES);
    let path = f.path().to_str().unwrap();
    let before = read(&f);

    let output = elmq()
        .args([
            "set",
            "case",
            path,
            "view",
            "--pattern",
            "Home",
            "--body",
            "\"home!\"",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success(), "ambiguity must error");
    assert_eq!(read(&f), before, "file unchanged");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("ambiguous") || stderr.contains("--on"),
        "stderr should hint --on: {stderr}"
    );

    // Retry with --on.
    let status = elmq()
        .args([
            "set",
            "case",
            path,
            "view",
            "--on",
            "model.route",
            "--pattern",
            "Home",
            "--body",
            "\"home!\"",
        ])
        .status()
        .unwrap();
    assert!(status.success());
    let c = read(&f);
    assert!(c.contains("\"home!\""));
}

// --------- set case: no case expression --------------------------------

#[test]
fn set_case_errors_when_no_case_expression() {
    let src = "module Main exposing (..)

simple n =
    n + 1
";
    let f = with_temp_elm(src);
    let path = f.path().to_str().unwrap();
    let before = read(&f);

    let output = elmq()
        .args([
            "set",
            "case",
            path,
            "simple",
            "--pattern",
            "X",
            "--body",
            "0",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(read(&f), before);
}

// --------- rm case: single pattern ------------------------------------

#[test]
fn rm_case_removes_single_branch() {
    let f = with_temp_elm(SIMPLE_CASE);
    let path = f.path().to_str().unwrap();

    let status = elmq()
        .args(["rm", "case", path, "update", "--pattern", "Decrement"])
        .status()
        .unwrap();
    assert!(status.success());

    let c = read(&f);
    assert!(!c.contains("Decrement ->"));
    assert!(c.contains("Increment ->"), "other branch intact");
}

// --------- rm case: multi-target --------------------------------------

#[test]
fn rm_case_multi_target_removes_both() {
    let src = "module Main exposing (..)

update msg model =
    case msg of
        A ->
            1

        B ->
            2

        C ->
            3
";
    let f = with_temp_elm(src);
    let path = f.path().to_str().unwrap();

    let status = elmq()
        .args([
            "rm",
            "case",
            path,
            "update",
            "--pattern",
            "A",
            "--pattern",
            "C",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let c = read(&f);
    assert!(!c.contains("A ->"));
    assert!(!c.contains("C ->"));
    assert!(c.contains("B ->"), "B remains");
}

// --------- rm case: would-empty --------------------------------------

#[test]
fn rm_case_refuses_to_empty_case() {
    let f = with_temp_elm(SIMPLE_CASE);
    let path = f.path().to_str().unwrap();
    let before = read(&f);

    let output = elmq()
        .args([
            "rm",
            "case",
            path,
            "update",
            "--pattern",
            "Increment",
            "--pattern",
            "Decrement",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success(), "removing all branches must error");
    assert_eq!(read(&f), before);
}
