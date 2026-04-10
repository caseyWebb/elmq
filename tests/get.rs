use std::fs;
use std::path::Path;
use std::process::Command;

fn elmq() -> Command {
    Command::new(env!("CARGO_BIN_EXE_elmq"))
}

fn create_project(root: &Path, source_dirs: &[&str]) {
    let sd_json: Vec<String> = source_dirs.iter().map(|s| format!("\"{s}\"")).collect();
    let elm_json = format!(
        r#"{{"type": "application", "source-directories": [{}], "elm-version": "0.19.1", "dependencies": {{}}}}"#,
        sd_json.join(", ")
    );
    fs::write(root.join("elm.json"), elm_json).unwrap();
    for sd in source_dirs {
        fs::create_dir_all(root.join(sd)).unwrap();
    }
}

fn write_elm(root: &Path, rel_path: &str, content: &str) {
    let path = root.join(rel_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

#[test]
fn get_function_with_type_annotation() {
    let output = elmq()
        .args(["get", "test-fixtures/Sample.elm", "update"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("update : Msg -> Model -> Model\n"));
    assert!(stdout.contains("case msg of"));
}

#[test]
fn get_function_without_type_annotation() {
    let output = elmq()
        .args(["get", "test-fixtures/Sample.elm", "helper"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("helper x ="));
    assert!(!stdout.contains(":"));
}

#[test]
fn get_type_declaration() {
    let output = elmq()
        .args(["get", "test-fixtures/Sample.elm", "Msg"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("{-| Messages for the update function -}"));
    assert!(stdout.contains("type Msg"));
    assert!(stdout.contains("| Reset"));
}

#[test]
fn get_type_alias() {
    let output = elmq()
        .args(["get", "test-fixtures/Sample.elm", "Model"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.starts_with("{-| The model for our app -}"));
    assert!(stdout.contains("type alias Model ="));
    assert!(stdout.contains(", name : String"));
}

#[test]
fn get_port_declaration() {
    let output = elmq()
        .args(["get", "test-fixtures/Sample.elm", "sendMessage"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("port sendMessage : String -> Cmd msg"));
}

#[test]
fn get_not_found() {
    // Under batch-positional-args: per-argument processing errors go to
    // stdout (not stderr) and the process exits with status 2.
    let output = elmq()
        .args(["get", "test-fixtures/Sample.elm", "nonExistent"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("error:"));
    assert!(stdout.contains("not found"));
}

#[test]
fn get_json_format() {
    let output = elmq()
        .args([
            "get",
            "test-fixtures/Sample.elm",
            "update",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(json["name"], "update");
    assert_eq!(json["kind"], "function");
    assert!(json["source"].as_str().unwrap().contains("case msg of"));
    assert!(json["start_line"].is_number());
    assert!(json["end_line"].is_number());
}

#[test]
fn get_default_format_is_compact() {
    let compact = elmq()
        .args(["get", "test-fixtures/Sample.elm", "update"])
        .output()
        .unwrap();

    let explicit = elmq()
        .args([
            "get",
            "test-fixtures/Sample.elm",
            "update",
            "--format",
            "compact",
        ])
        .output()
        .unwrap();

    assert_eq!(compact.stdout, explicit.stdout);
}

// ---------------------------------------------------------------------------
// Multi-name (batch) tests
// ---------------------------------------------------------------------------

const MULTI_DECL_SOURCE: &str = "module Main exposing (..)

aaa : Int
aaa =
    1


bbb : String
bbb =
    \"hi\"


ccc : Bool
ccc =
    True
";

fn write_temp_elm(name: &str, contents: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    let unique = format!(
        "elmq-get-multi-{}-{}-{}.elm",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        name,
    );
    path.push(unique);
    std::fs::write(&path, contents).unwrap();
    path
}

#[test]
fn get_multi_name_success() {
    let output = elmq()
        .args(["get", "test-fixtures/Sample.elm", "update", "Msg", "Model"])
        .output()
        .unwrap();

    assert!(output.status.success(), "expected success exit");
    let stdout = String::from_utf8(output.stdout).unwrap();

    let p_update = stdout.find("## update").expect("missing ## update header");
    let p_msg = stdout.find("## Msg").expect("missing ## Msg header");
    let p_model = stdout.find("## Model").expect("missing ## Model header");
    assert!(
        p_update < p_msg && p_msg < p_model,
        "headers not in input order: stdout was:\n{stdout}"
    );

    // Each block should contain the actual declaration source.
    let update_block = &stdout[p_update..p_msg];
    assert!(
        update_block.contains("update : Msg -> Model -> Model"),
        "update block missing source:\n{update_block}"
    );
    assert!(update_block.contains("case msg of"));

    let msg_block = &stdout[p_msg..p_model];
    assert!(
        msg_block.contains("type Msg"),
        "Msg block missing source:\n{msg_block}"
    );

    let model_block = &stdout[p_model..];
    assert!(
        model_block.contains("type alias Model ="),
        "Model block missing source:\n{model_block}"
    );
}

#[test]
fn get_multi_name_input_order_preserved() {
    let path = write_temp_elm("order", MULTI_DECL_SOURCE);
    let output = elmq()
        .args(["get", path.to_str().unwrap(), "ccc", "aaa", "bbb"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    let p_ccc = stdout.find("## ccc").expect("missing ## ccc");
    let p_aaa = stdout.find("## aaa").expect("missing ## aaa");
    let p_bbb = stdout.find("## bbb").expect("missing ## bbb");
    assert!(
        p_ccc < p_aaa && p_aaa < p_bbb,
        "headers not in requested order ccc,aaa,bbb:\n{stdout}"
    );

    let _ = std::fs::remove_file(&path);
}

#[test]
fn get_multi_name_partial_not_found() {
    let source = "module Main exposing (..)

aaa : Int
aaa =
    1


ccc : Bool
ccc =
    True
";
    let path = write_temp_elm("partial", source);
    let output = elmq()
        .args(["get", path.to_str().unwrap(), "aaa", "nonExistent", "ccc"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8(output.stdout).unwrap();

    let p_aaa = stdout.find("## aaa").expect("missing ## aaa");
    let p_missing = stdout
        .find("## nonExistent")
        .expect("missing ## nonExistent");
    let p_ccc = stdout.find("## ccc").expect("missing ## ccc");
    assert!(p_aaa < p_missing && p_missing < p_ccc);

    let aaa_block = &stdout[p_aaa..p_missing];
    let missing_block = &stdout[p_missing..p_ccc];
    let ccc_block = &stdout[p_ccc..];

    assert!(
        aaa_block.contains("aaa : Int") && aaa_block.contains("aaa =\n    1"),
        "aaa block missing source:\n{aaa_block}"
    );
    assert!(
        missing_block.contains("error:"),
        "missing block should contain error:\n{missing_block}"
    );
    assert!(
        ccc_block.contains("ccc : Bool") && ccc_block.contains("ccc =\n    True"),
        "ccc block missing source:\n{ccc_block}"
    );

    let _ = std::fs::remove_file(&path);
}

#[test]
fn get_multi_name_all_missing_exit_2() {
    let path = write_temp_elm("all-missing", MULTI_DECL_SOURCE);
    let output = elmq()
        .args(["get", path.to_str().unwrap(), "nope1", "nope2"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8(output.stdout).unwrap();

    let p1 = stdout.find("## nope1").expect("missing ## nope1");
    let p2 = stdout.find("## nope2").expect("missing ## nope2");
    assert!(p1 < p2);

    let block1 = &stdout[p1..p2];
    let block2 = &stdout[p2..];
    assert!(block1.contains("error:"), "block1 missing error:\n{block1}");
    assert!(block2.contains("error:"), "block2 missing error:\n{block2}");

    let _ = std::fs::remove_file(&path);
}

#[test]
fn get_single_name_output_unchanged() {
    let output = elmq()
        .args(["get", "test-fixtures/Sample.elm", "update"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        !stdout.contains("##"),
        "single-name output should have no ## header:\n{stdout}"
    );
    assert!(stdout.contains("update : Msg -> Model -> Model"));
    assert!(stdout.contains("case msg of"));
}

// ---------------------------------------------------------------------------
// Multi-file (-f) tests
// ---------------------------------------------------------------------------

/// Task 6.2: single -f group is byte-identical to bare form.
#[test]
fn get_f_single_group_matches_bare() {
    let bare = elmq()
        .args(["get", "test-fixtures/Sample.elm", "update"])
        .output()
        .unwrap();
    let grouped = elmq()
        .args(["get", "-f", "test-fixtures/Sample.elm", "update"])
        .output()
        .unwrap();

    assert_eq!(bare.status.code(), grouped.status.code());
    assert_eq!(bare.stdout, grouped.stdout);
}

/// Task 6.3: two groups across different files, verify Module.decl framing and
/// input order (within an elm.json project).
#[test]
fn get_f_two_groups_module_framing() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Foo.elm",
        "module Foo exposing (..)\n\nfoo : Int\nfoo =\n    42\n",
    );
    write_elm(
        root,
        "src/Bar.elm",
        "module Bar exposing (..)\n\nbar : String\nbar =\n    \"hi\"\n",
    );

    let output = elmq()
        .current_dir(root)
        .args([
            "get",
            "-f",
            "src/Foo.elm",
            "foo",
            "-f",
            "src/Bar.elm",
            "bar",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    let p_foo = stdout.find("## Foo.foo").expect("missing ## Foo.foo");
    let p_bar = stdout.find("## Bar.bar").expect("missing ## Bar.bar");
    assert!(p_foo < p_bar, "Foo.foo should precede Bar.bar:\n{stdout}");

    let foo_block = &stdout[p_foo..p_bar];
    assert!(
        foo_block.contains("foo : Int"),
        "foo block missing source:\n{foo_block}"
    );

    let bar_block = &stdout[p_bar..];
    assert!(
        bar_block.contains("bar : String"),
        "bar block missing source:\n{bar_block}"
    );
}

/// Task 6.5: multi-file fallback framing when no elm.json is present.
#[test]
fn get_f_fallback_framing_no_elm_json() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    // No elm.json — just two .elm files.
    write_elm(root, "A.elm", "module A exposing (..)\n\na = 1\n");
    write_elm(root, "B.elm", "module B exposing (..)\n\nb = 2\n");

    let output = elmq()
        .current_dir(root)
        .args(["get", "-f", "A.elm", "a", "-f", "B.elm", "b"])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Fallback form: ## file:decl
    assert!(
        stdout.contains("## A.elm:a"),
        "missing fallback header A.elm:a:\n{stdout}"
    );
    assert!(
        stdout.contains("## B.elm:b"),
        "missing fallback header B.elm:b:\n{stdout}"
    );
}

/// Task 6.6: mixing bare positionals and -f exits 1 with usage error.
#[test]
fn get_f_mixing_bare_and_grouped_errors() {
    let output = elmq()
        .args([
            "get",
            "test-fixtures/Sample.elm",
            "update",
            "-f",
            "test-fixtures/Sample.elm",
            "view",
        ])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
}

/// Task 6.7: -f FILE with no names exits 1 with usage error.
#[test]
fn get_f_no_names_errors() {
    let output = elmq()
        .args(["get", "-f", "test-fixtures/Sample.elm"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
}

/// Task 6.8: file-not-found in one group, others succeed, exit 2.
#[test]
fn get_f_file_not_found_partial() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    write_elm(root, "Good.elm", "module Good exposing (..)\n\nx = 1\n");

    let output = elmq()
        .current_dir(root)
        .args(["get", "-f", "Good.elm", "x", "-f", "Missing.elm", "y"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8(output.stdout).unwrap();

    // Good file should have source.
    assert!(
        stdout.contains("x = 1"),
        "good block missing source:\n{stdout}"
    );
    // Missing file should have error.
    assert!(
        stdout.contains("error:"),
        "missing file should have error:\n{stdout}"
    );
}

/// Task 6.9: parse failure in one group, others succeed, exit 2.
#[test]
fn get_f_parse_failure_partial() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    write_elm(root, "Good.elm", "module Good exposing (..)\n\nx = 1\n");
    write_elm(root, "Bad.elm", "this is not valid elm at all !!!");

    let output = elmq()
        .current_dir(root)
        .args(["get", "-f", "Good.elm", "x", "-f", "Bad.elm", "y"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("x = 1"));
    assert!(
        stdout.contains("error:"),
        "bad file should have error:\n{stdout}"
    );
}

/// Task 6.10: declaration-not-found in one name, others in the same group succeed.
#[test]
fn get_f_decl_not_found_in_group() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();

    write_elm(
        root,
        "M.elm",
        "module M exposing (..)\n\nfoo = 1\n\nbar = 2\n",
    );

    let output = elmq()
        .current_dir(root)
        .args(["get", "-f", "M.elm", "foo", "nonExistent", "bar"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(
        stdout.contains("foo = 1"),
        "foo block missing source:\n{stdout}"
    );
    assert!(
        stdout.contains("bar = 2"),
        "bar block missing source:\n{stdout}"
    );
    assert!(
        stdout.contains("error:") && stdout.contains("nonExistent"),
        "nonExistent should have error:\n{stdout}"
    );
}

/// Task 6.11: --format json multi-result emits an array with module and file
/// fields, input order preserved.
#[test]
fn get_f_json_multi_result_array() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Foo.elm",
        "module Foo exposing (..)\n\nfoo : Int\nfoo =\n    42\n",
    );
    write_elm(
        root,
        "src/Bar.elm",
        "module Bar exposing (..)\n\nbar : String\nbar =\n    \"hi\"\n",
    );

    let output = elmq()
        .current_dir(root)
        .args([
            "get",
            "--format",
            "json",
            "-f",
            "src/Foo.elm",
            "foo",
            "-f",
            "src/Bar.elm",
            "bar",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let arr: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert!(arr.is_array(), "expected JSON array:\n{stdout}");
    let arr = arr.as_array().unwrap();
    assert_eq!(arr.len(), 2);

    assert_eq!(arr[0]["name"], "foo");
    assert_eq!(arr[0]["module"], "Foo");
    assert!(arr[0]["file"].as_str().unwrap().contains("Foo.elm"));

    assert_eq!(arr[1]["name"], "bar");
    assert_eq!(arr[1]["module"], "Bar");
    assert!(arr[1]["file"].as_str().unwrap().contains("Bar.elm"));
}

/// Task 6.12: --format json single-result emits a scalar object (unchanged).
#[test]
fn get_f_json_single_result_scalar() {
    let output = elmq()
        .args([
            "get",
            "--format",
            "json",
            "-f",
            "test-fixtures/Sample.elm",
            "update",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();

    assert!(
        json.is_object(),
        "single-result should be an object:\n{stdout}"
    );
    assert_eq!(json["name"], "update");
}
