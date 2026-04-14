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

const TYPED_LET: &str = "module Main exposing (..)

update msg model =
    let
        helper : Int -> Int
        helper n =
            n + 1
    in
    helper model
";

const VALUE_LET: &str = "module Main exposing (..)

processItem item =
    let
        a =
            1
    in
    a + item
";

const FUNCTION_LET: &str = "module Main exposing (..)

update msg model =
    let
        helper n =
            n + 1
    in
    helper model
";

// --------- set let -----------------------------------------------------

#[test]
fn set_let_body_only_edit_preserves_sig() {
    let f = with_temp_elm(TYPED_LET);
    let path = f.path().to_str().unwrap();

    let status = elmq()
        .args([
            "set", "let", path, "update", "--name", "helper", "--body", "n + 2",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let c = read(&f);
    assert!(
        c.contains("helper : Int -> Int"),
        "sig must be preserved: {c}"
    );
    assert!(c.contains("n + 2"), "body updated: {c}");
    assert!(!c.contains("n + 1"), "old body removed: {c}");
}

#[test]
fn set_let_type_replaces_sig() {
    let f = with_temp_elm(TYPED_LET);
    let path = f.path().to_str().unwrap();

    let status = elmq()
        .args([
            "set",
            "let",
            path,
            "update",
            "--name",
            "helper",
            "--type",
            "Int -> String",
            "--body",
            "String.fromInt n",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let c = read(&f);
    assert!(c.contains("helper : Int -> String"));
    assert!(c.contains("String.fromInt n"));
    assert!(!c.contains("Int -> Int"));
}

#[test]
fn set_let_no_type_removes_sig() {
    let f = with_temp_elm(TYPED_LET);
    let path = f.path().to_str().unwrap();

    let status = elmq()
        .args([
            "set",
            "let",
            path,
            "update",
            "--name",
            "helper",
            "--no-type",
            "--body",
            "n + 1",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let c = read(&f);
    assert!(!c.contains(": Int -> Int"), "sig removed: {c}");
    assert!(c.contains("helper n ="), "definition intact: {c}");
}

#[test]
fn set_let_insert_new_value_binding() {
    let f = with_temp_elm(VALUE_LET);
    let path = f.path().to_str().unwrap();

    let status = elmq()
        .args([
            "set",
            "let",
            path,
            "processItem",
            "--name",
            "b",
            "--body",
            "2",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let c = read(&f);
    assert!(c.contains("a ="), "original binding intact: {c}");
    assert!(c.contains("b ="), "new binding present: {c}");
    let a_pos = c.find("a =").unwrap();
    let b_pos = c.find("b =").unwrap();
    assert!(a_pos < b_pos, "b is appended after a: {c}");
}

#[test]
fn set_let_insert_new_function_binding_with_params_and_type() {
    let f = with_temp_elm(FUNCTION_LET);
    let path = f.path().to_str().unwrap();

    let status = elmq()
        .args([
            "set",
            "let",
            path,
            "update",
            "--name",
            "double",
            "--type",
            "Int -> Int",
            "--params",
            "x",
            "--body",
            "x * 2",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let c = read(&f);
    assert!(c.contains("double : Int -> Int"), "sig present: {c}");
    assert!(c.contains("double x ="), "function form: {c}");
    assert!(c.contains("x * 2"), "body: {c}");
}

// set let --name / content mismatch isn't testable at the CLI layer today —
// --body is a raw expression (not a full binding source) and --name is the
// only name source, so they can't disagree. The spec's mismatch guard only
// applies when a binding-with-name is parseable from content (e.g. stdin
// heredoc form). Re-enable this test if that form is added later.

// --------- rm let ------------------------------------------------------

#[test]
fn rm_let_removes_single_binding() {
    // Use a let block with multiple bindings so we don't hit the "removing
    // the last binding leaves an empty let" edge case, which is a separate
    // writer concern.
    let src = "module Main exposing (..)

update msg model =
    let
        helper : Int -> Int
        helper n =
            n + 1

        other =
            model
    in
    helper other
";
    let f = with_temp_elm(src);
    let path = f.path().to_str().unwrap();

    let status = elmq()
        .args(["rm", "let", path, "update", "helper"])
        .status()
        .unwrap();
    assert!(status.success());

    let c = read(&f);
    assert!(!c.contains("helper :"), "sig removed: {c}");
    assert!(!c.contains("helper n ="), "def removed: {c}");
    assert!(c.contains("other ="), "other binding intact: {c}");
}

#[test]
fn rm_let_removing_sole_binding_errors_cleanly() {
    // Pre-check: removing the only binding in a let block would leave
    // `let \n in body` (empty let) which fails the re-parse gate. The
    // writer now detects this up front and emits an actionable error
    // pointing at `set decl` as the alternative.
    let f = with_temp_elm(TYPED_LET);
    let path = f.path().to_str().unwrap();
    let before = read(&f);

    let output = elmq()
        .args(["rm", "let", path, "update", "helper"])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "should refuse sole-binding removal"
    );
    assert_eq!(read(&f), before, "file unchanged");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("only binding") || stderr.contains("set decl"),
        "stderr should name the failure mode: {stderr}"
    );
}

#[test]
fn rm_let_batch_all_or_nothing() {
    let src = "module Main exposing (..)

update msg model =
    let
        a =
            1

        b =
            2

        c =
            3
    in
    a + b + c
";
    let f = with_temp_elm(src);
    let path = f.path().to_str().unwrap();
    let before = read(&f);

    // "missing" doesn't exist; whole operation must fail without mutating.
    let output = elmq()
        .args(["rm", "let", path, "update", "a", "missing", "c"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(read(&f), before, "file unchanged on partial failure");
}

// --------- rename let --------------------------------------------------

#[test]
fn rename_let_preserves_top_level_type_annotation() {
    // Regression for the review-flagged blocker: the rewrite used to splice
    // the new decl over line-based slices, silently dropping the top-level
    // `update : Msg -> Model -> Model` annotation. This test exercises the
    // annotated case to guard against re-regression.
    let src = "module Main exposing (..)

update : Int -> Int
update model =
    let
        h =
            model + 1
    in
    h + h
";
    let f = with_temp_elm(src);
    let path = f.path().to_str().unwrap();

    let status = elmq()
        .args([
            "rename", "let", path, "update", "--from", "h", "--to", "counter",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let c = read(&f);
    assert!(
        c.contains("update : Int -> Int"),
        "top-level sig must be preserved: {c}"
    );
    assert!(c.contains("counter ="), "binding renamed: {c}");
    assert!(c.contains("counter + counter"), "refs rewritten: {c}");
}

#[test]
fn rename_let_updates_references() {
    let src = "module Main exposing (..)

update msg model =
    let
        h =
            model.count
    in
    h + h
";
    let f = with_temp_elm(src);
    let path = f.path().to_str().unwrap();

    let status = elmq()
        .args([
            "rename", "let", path, "update", "--from", "h", "--to", "counter",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let c = read(&f);
    assert!(c.contains("counter ="), "binding renamed: {c}");
    assert!(c.contains("counter + counter"), "refs rewritten: {c}");
    assert!(!c.contains(" h "), "no stray h references: {c}");
}

#[test]
fn rename_let_collision_errors() {
    let src = "module Main exposing (..)

update msg model =
    let
        helper =
            1
    in
    helper + model
";
    let f = with_temp_elm(src);
    let path = f.path().to_str().unwrap();
    let before = read(&f);

    let output = elmq()
        .args([
            "rename", "let", path, "update", "--from", "helper", "--to", "model",
        ])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "collision with function arg must error"
    );
    assert_eq!(read(&f), before, "file unchanged on collision");
}
