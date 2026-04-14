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

const TYPED_FN: &str = "module Main exposing (..)

update : Msg -> Model -> Model
update msg model =
    model + 1
";

const UNTYPED_FN: &str = "module Main exposing (..)

logImpl level msg =
    msg
";

const FOUR_ARG_FN: &str = "module Main exposing (..)

fn : Int -> Int -> Int -> Int -> Int
fn a b c d =
    a + b + c + d
";

// --------- add arg ----------------------------------------------------

#[test]
fn add_arg_typed_fn_requires_type() {
    let f = with_temp_elm(TYPED_FN);
    let path = f.path().to_str().unwrap();
    let before = read(&f);

    let output = elmq()
        .args(["add", "arg", path, "update", "--at", "2", "--name", "flag"])
        .output()
        .unwrap();
    assert!(!output.status.success(), "typed fn requires --type");
    assert_eq!(read(&f), before);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("type signature") || stderr.contains("--type"),
        "stderr: {stderr}"
    );
}

#[test]
fn add_arg_typed_fn_with_type_updates_sig_and_def() {
    let f = with_temp_elm(TYPED_FN);
    let path = f.path().to_str().unwrap();

    let status = elmq()
        .args([
            "add", "arg", path, "update", "--at", "2", "--name", "flag", "--type", "Bool",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let c = read(&f);
    assert!(c.contains("Msg -> Bool -> Model -> Model"), "sig: {c}");
    assert!(c.contains("update msg flag model"), "def: {c}");
}

#[test]
fn add_arg_untyped_fn_validates_type_even_when_ignored() {
    // On an untyped fn, --type is silently ignored (no sig to splice into),
    // but we still syntax-check the value so obviously-broken types don't
    // pass. Regression guard for the review finding.
    let f = with_temp_elm(UNTYPED_FN);
    let path = f.path().to_str().unwrap();
    let before = read(&f);

    let output = elmq()
        .args([
            "add", "arg", path, "logImpl", "--at", "1", "--name", "tag", "--type", "<<<<",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success(), "broken --type must error");
    assert_eq!(read(&f), before);
}

#[test]
fn add_arg_untyped_fn_accepts_at_alone() {
    let f = with_temp_elm(UNTYPED_FN);
    let path = f.path().to_str().unwrap();

    let status = elmq()
        .args(["add", "arg", path, "logImpl", "--at", "1", "--name", "tag"])
        .status()
        .unwrap();
    assert!(status.success());

    let c = read(&f);
    assert!(c.contains("logImpl tag level msg"), "def: {c}");
}

#[test]
fn add_arg_at_1_prepends() {
    let f = with_temp_elm(TYPED_FN);
    let path = f.path().to_str().unwrap();

    let status = elmq()
        .args([
            "add", "arg", path, "update", "--at", "1", "--name", "first", "--type", "Int",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let c = read(&f);
    assert!(
        c.contains("Int -> Msg -> Model -> Model"),
        "sig prepended: {c}"
    );
    assert!(c.contains("update first msg model"), "def prepended: {c}");
}

#[test]
fn add_arg_at_n_plus_1_appends() {
    let f = with_temp_elm(TYPED_FN);
    let path = f.path().to_str().unwrap();

    let status = elmq()
        .args([
            "add", "arg", path, "update", "--at", "3", "--name", "last", "--type", "Int",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let c = read(&f);
    assert!(
        c.contains("Msg -> Model -> Int -> Model"),
        "sig appended: {c}"
    );
    assert!(c.contains("update msg model last"), "def appended: {c}");
}

#[test]
fn add_arg_out_of_range_errors() {
    let f = with_temp_elm(TYPED_FN);
    let path = f.path().to_str().unwrap();
    let before = read(&f);

    let output = elmq()
        .args([
            "add", "arg", path, "update", "--at", "5", "--name", "x", "--type", "Int",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(read(&f), before);
}

// --------- rm arg ----------------------------------------------------

#[test]
fn rm_arg_single_position() {
    let f = with_temp_elm(FOUR_ARG_FN);
    let path = f.path().to_str().unwrap();

    let status = elmq()
        .args(["rm", "arg", path, "fn", "--at", "2"])
        .status()
        .unwrap();
    assert!(status.success());

    let c = read(&f);
    assert!(c.contains("Int -> Int -> Int -> Int\n"), "sig: {c}");
    assert!(c.contains("fn a c d"), "def: {c}");
}

#[test]
fn rm_arg_multi_position_rear_to_front() {
    let f = with_temp_elm(FOUR_ARG_FN);
    let path = f.path().to_str().unwrap();

    // Remove positions 2 and 4 of original (b and d).
    let status = elmq()
        .args(["rm", "arg", path, "fn", "--at", "2", "--at", "4"])
        .status()
        .unwrap();
    assert!(status.success());

    let c = read(&f);
    assert!(c.contains("fn a c ="), "a and c remain: {c}");
    assert!(c.contains("Int -> Int -> Int\n"), "sig has 3 types: {c}");
}

#[test]
fn rm_arg_by_name() {
    let f = with_temp_elm(FOUR_ARG_FN);
    let path = f.path().to_str().unwrap();

    let status = elmq()
        .args(["rm", "arg", path, "fn", "--name", "b", "--name", "d"])
        .status()
        .unwrap();
    assert!(status.success());

    let c = read(&f);
    assert!(c.contains("fn a c ="), "only a and c remain: {c}");
}

// --------- rename arg ----------------------------------------------

#[test]
fn rename_arg_updates_body_references() {
    let src = "module Main exposing (..)

update m model =
    m + model
";
    let f = with_temp_elm(src);
    let path = f.path().to_str().unwrap();

    let status = elmq()
        .args([
            "rename", "arg", path, "update", "--from", "m", "--to", "msg",
        ])
        .status()
        .unwrap();
    assert!(status.success());

    let c = read(&f);
    assert!(c.contains("update msg model"), "param renamed: {c}");
    assert!(c.contains("msg + model"), "body ref rewritten: {c}");
}

#[test]
fn rename_arg_collision_errors() {
    let src = "module Main exposing (..)

update msg model =
    let
        existing =
            1
    in
    msg + model + existing
";
    let f = with_temp_elm(src);
    let path = f.path().to_str().unwrap();
    let before = read(&f);

    let output = elmq()
        .args([
            "rename", "arg", path, "update", "--from", "msg", "--to", "existing",
        ])
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "collision with let binding must error"
    );
    assert_eq!(read(&f), before);
}
