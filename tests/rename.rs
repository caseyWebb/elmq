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

fn setup_project(root: &Path) {
    create_project(root, &["src"]);
    write_elm(
        root,
        "src/Lib/Utils.elm",
        "module Lib.Utils exposing (helper, Model)\n\ntype alias Model = { name : String }\n\nhelper : String -> String\nhelper name =\n    name\n",
    );
    write_elm(
        root,
        "src/Page/Home.elm",
        "module Page.Home exposing (..)\n\nimport Lib.Utils exposing (helper)\n\nview = helper \"x\"\n",
    );
    write_elm(
        root,
        "src/Page/Settings.elm",
        "module Page.Settings exposing (..)\n\nimport Lib.Utils as LU\n\nview = LU.helper \"y\"\n",
    );
    write_elm(
        root,
        "src/Main.elm",
        "module Main exposing (..)\n\nimport Lib.Utils\n\nmain = Lib.Utils.helper \"z\"\n\ntype alias AppModel = Lib.Utils.Model\n",
    );
    write_elm(
        root,
        "src/Wildcard.elm",
        "module Wildcard exposing (..)\n\nimport Lib.Utils exposing (..)\n\nfoo = helper \"w\"\n",
    );
}

// -- Single-file rename (non-exposed) --

#[test]
fn rename_single_file() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_elm(
        root,
        "src/Foo.elm",
        "module Foo exposing (view)\n\ninternalHelper = 1\n\nview = internalHelper\n",
    );

    let output = elmq()
        .current_dir(root)
        .args([
            "rename",
            "decl",
            "src/Foo.elm",
            "internalHelper",
            "formatName",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    assert!(
        stdout.contains("renamed internalHelper -> formatName"),
        "got: {stdout}"
    );

    let content = fs::read_to_string(root.join("src/Foo.elm")).unwrap();
    assert!(content.contains("formatName = 1"));
    assert!(content.contains("view = formatName"));
    assert!(!content.contains("internalHelper"));
}

// -- Project-wide rename --

#[test]
fn rename_project_wide() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup_project(root);

    let output = elmq()
        .current_dir(root)
        .args([
            "rename",
            "decl",
            "src/Lib/Utils.elm",
            "helper",
            "formatName",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    assert!(
        stdout.contains("renamed helper -> formatName"),
        "got: {stdout}"
    );

    // Defining file updated.
    let utils = fs::read_to_string(root.join("src/Lib/Utils.elm")).unwrap();
    assert!(utils.contains("formatName : String -> String"));
    assert!(utils.contains("formatName name ="));
    assert!(utils.contains("exposing (formatName, Model)"));

    // Exposed bare refs updated.
    let home = fs::read_to_string(root.join("src/Page/Home.elm")).unwrap();
    assert!(home.contains("exposing (formatName)"));
    assert!(home.contains("view = formatName \"x\""));

    // Aliased refs updated.
    let settings = fs::read_to_string(root.join("src/Page/Settings.elm")).unwrap();
    assert!(settings.contains("LU.formatName \"y\""));

    // Qualified refs updated.
    let main = fs::read_to_string(root.join("src/Main.elm")).unwrap();
    assert!(main.contains("Lib.Utils.formatName \"z\""));

    // exposing(..) file NOT modified.
    let wildcard = fs::read_to_string(root.join("src/Wildcard.elm")).unwrap();
    assert!(wildcard.contains("helper \"w\""));
}

// -- Dry run --

#[test]
fn rename_dry_run() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup_project(root);

    let output = elmq()
        .current_dir(root)
        .args([
            "rename",
            "decl",
            "src/Lib/Utils.elm",
            "helper",
            "formatName",
            "--dry-run",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.contains("(dry run)"), "got: {stdout}");

    // Files should be unchanged.
    let utils = fs::read_to_string(root.join("src/Lib/Utils.elm")).unwrap();
    assert!(utils.contains("helper : String -> String"));
}

// -- JSON output --

#[test]
fn rename_json_output() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup_project(root);

    let output = elmq()
        .current_dir(root)
        .args([
            "rename",
            "decl",
            "src/Lib/Utils.elm",
            "helper",
            "formatName",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());

    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["renamed"]["from"], "helper");
    assert_eq!(parsed["renamed"]["to"], "formatName");
    assert!(parsed["updated"].as_array().unwrap().len() >= 2);
}

// -- Variant rename --

#[test]
fn rename_variant() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_elm(
        root,
        "src/Foo.elm",
        "module Foo exposing (Msg(..))\n\ntype Msg = GotResponse String | Other\n\nupdate msg =\n    case msg of\n        GotResponse s -> s\n        Other -> \"\"\n",
    );

    let output = elmq()
        .current_dir(root)
        .args([
            "rename",
            "decl",
            "src/Foo.elm",
            "GotResponse",
            "ReceivedResponse",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");

    let content = fs::read_to_string(root.join("src/Foo.elm")).unwrap();
    assert!(content.contains("ReceivedResponse String | Other"));
    assert!(content.contains("ReceivedResponse s -> s"));
    assert!(!content.contains("GotResponse"));
}

// -- Variant rename project-wide via Type(..) --

#[test]
fn rename_variant_project_wide() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_elm(
        root,
        "src/Lib/Types.elm",
        "module Lib.Types exposing (Msg(..))\n\ntype Msg = GotResponse String | Other\n",
    );
    write_elm(
        root,
        "src/Main.elm",
        "module Main exposing (..)\n\nimport Lib.Types exposing (Msg(..))\n\nupdate msg =\n    case msg of\n        GotResponse s -> s\n        Other -> \"\"\n",
    );
    write_elm(
        root,
        "src/Qualified.elm",
        "module Qualified exposing (..)\n\nimport Lib.Types\n\nfoo = Lib.Types.GotResponse \"x\"\n",
    );

    let output = elmq()
        .current_dir(root)
        .args([
            "rename",
            "decl",
            "src/Lib/Types.elm",
            "GotResponse",
            "ReceivedResponse",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    assert!(
        stdout.contains("renamed GotResponse -> ReceivedResponse"),
        "got: {stdout}"
    );

    // Defining file updated.
    let types = fs::read_to_string(root.join("src/Lib/Types.elm")).unwrap();
    assert!(types.contains("ReceivedResponse String | Other"));
    assert!(!types.contains("GotResponse"));

    // Bare refs via Msg(..) updated.
    let main = fs::read_to_string(root.join("src/Main.elm")).unwrap();
    assert!(main.contains("ReceivedResponse s -> s"));
    assert!(!main.contains("GotResponse"));
    // Msg(..) import unchanged.
    assert!(main.contains("exposing (Msg(..))"));

    // Qualified refs updated.
    let qualified = fs::read_to_string(root.join("src/Qualified.elm")).unwrap();
    assert!(qualified.contains("Lib.Types.ReceivedResponse \"x\""));
    assert!(!qualified.contains("GotResponse"));
}

// -- Error: declaration not found --

#[test]
fn rename_not_found() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_elm(root, "src/Foo.elm", "module Foo exposing (..)\n\nfoo = 1\n");

    let output = elmq()
        .current_dir(root)
        .args(["rename", "decl", "src/Foo.elm", "nonexistent", "newName"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not found"), "stderr: {stderr}");
}

// -- Error: name conflict --

#[test]
fn rename_conflict() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_elm(
        root,
        "src/Foo.elm",
        "module Foo exposing (..)\n\nfoo = 1\n\nbar = 2\n",
    );

    let output = elmq()
        .current_dir(root)
        .args(["rename", "decl", "src/Foo.elm", "foo", "bar"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already exists"), "stderr: {stderr}");
}

// -- Type rename project-wide --

#[test]
fn rename_type_project_wide() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup_project(root);

    let output = elmq()
        .current_dir(root)
        .args(["rename", "decl", "src/Lib/Utils.elm", "Model", "AppModel"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    assert!(
        stdout.contains("renamed Model -> AppModel"),
        "got: {stdout}"
    );

    let utils = fs::read_to_string(root.join("src/Lib/Utils.elm")).unwrap();
    assert!(utils.contains("type alias AppModel"));
    assert!(utils.contains("exposing (helper, AppModel)"));

    let main = fs::read_to_string(root.join("src/Main.elm")).unwrap();
    assert!(main.contains("Lib.Utils.AppModel"));
}

#[test]
fn rename_rejects_broken_source_file() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_elm(
        root,
        "src/Broken.elm",
        "module Broken exposing (bar)\n\nbar =\n    let\n        x = 1\n",
    );
    let before = fs::read(root.join("src/Broken.elm")).unwrap();

    let output = elmq()
        .current_dir(root)
        .args(["rename", "decl", "src/Broken.elm", "bar", "baz"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("refusing to edit"), "stderr: {stderr}");
    assert_eq!(fs::read(root.join("src/Broken.elm")).unwrap(), before);
}

#[test]
fn rename_rejects_when_downstream_file_is_broken() {
    // Clean defining file, but a downstream file that imports the renamed
    // declaration has a pre-existing parse error. Input-side validation
    // must catch this before any file is mutated on disk.
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_elm(
        root,
        "src/Lib.elm",
        "module Lib exposing (helper)\n\nhelper : String -> String\nhelper s = s\n",
    );
    write_elm(
        root,
        "src/Downstream.elm",
        "module Downstream exposing (..)\n\nimport Lib exposing (helper)\n\nuse =\n    let\n        x = helper \"hi\"\n",
    );
    let lib_before = fs::read(root.join("src/Lib.elm")).unwrap();
    let downstream_before = fs::read(root.join("src/Downstream.elm")).unwrap();

    let output = elmq()
        .current_dir(root)
        .args(["rename", "decl", "src/Lib.elm", "helper", "helper2"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("refusing to edit") && stderr.contains("Downstream.elm"),
        "stderr: {stderr}"
    );
    // The defining file may or may not have been rewritten before the
    // downstream read failed — partial-write semantics allow both. But
    // the broken downstream file MUST be unchanged.
    assert_eq!(
        fs::read(root.join("src/Downstream.elm")).unwrap(),
        downstream_before
    );
    // The defining file should not have been written either, because
    // the rename implementation scans downstream files before writing
    // the defining file last.
    assert_eq!(fs::read(root.join("src/Lib.elm")).unwrap(), lib_before);
}
