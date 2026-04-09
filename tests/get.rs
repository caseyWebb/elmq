use std::process::Command;

fn elmq() -> Command {
    Command::new(env!("CARGO_BIN_EXE_elmq"))
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
