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

fn read_elm(root: &Path, rel_path: &str) -> String {
    fs::read_to_string(root.join(rel_path)).unwrap()
}

// -- add variant: simple case --

#[test]
fn add_variant_simple() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "\
module Types exposing (Msg(..))

type Msg
    = Increment
    | Decrement
",
    );

    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types exposing (Msg(..))

update : Msg -> Int -> Int
update msg count =
    case msg of
        Increment ->
            count + 1

        Decrement ->
            count - 1
",
    );

    let output = elmq()
        .current_dir(root)
        .args(["variant", "add", "src/Types.elm", "--type", "Msg", "Reset"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stderr: {stderr}\nstdout: {stdout}"
    );

    // Type should have the new variant.
    let types_content = read_elm(root, "src/Types.elm");
    assert!(
        types_content.contains("| Reset"),
        "type should have Reset variant: {types_content}"
    );

    // Case expression should have a new branch.
    let main_content = read_elm(root, "src/Main.elm");
    assert!(
        main_content.contains("Reset ->"),
        "case should have Reset branch: {main_content}"
    );
    assert!(
        main_content.contains("Debug.todo \"Reset\""),
        "branch should use Debug.todo: {main_content}"
    );
}

// -- add variant: with arguments --

#[test]
fn add_variant_with_args() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "\
module Types exposing (Msg(..))

type Msg
    = Increment
    | Decrement
",
    );

    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types exposing (Msg(..))

update : Msg -> Int -> Int
update msg count =
    case msg of
        Increment ->
            count + 1

        Decrement ->
            count - 1
",
    );

    let output = elmq()
        .current_dir(root)
        .args([
            "variant",
            "add",
            "src/Types.elm",
            "--type",
            "Msg",
            "SetCount Int",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");

    let main_content = read_elm(root, "src/Main.elm");
    assert!(
        main_content.contains("SetCount _ ->"),
        "branch should have wildcard arg: {main_content}"
    );
}

// -- add variant: wildcard branch skipped --

#[test]
fn add_variant_wildcard_skipped() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "\
module Types exposing (Msg(..))

type Msg
    = Increment
    | Decrement
",
    );

    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types exposing (Msg(..))

update : Msg -> Int -> Int
update msg count =
    case msg of
        Increment ->
            count + 1

        _ ->
            count
",
    );

    let output = elmq()
        .current_dir(root)
        .args(["variant", "add", "src/Types.elm", "--type", "Msg", "Reset"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    assert!(
        stdout.contains("skipped"),
        "should report skipped case: {stdout}"
    );

    // The case expression should NOT have a new branch.
    let main_content = read_elm(root, "src/Main.elm");
    assert!(
        !main_content.contains("Reset ->"),
        "wildcard case should not get new branch: {main_content}"
    );
}

// -- add variant: multi-file project --

#[test]
fn add_variant_multi_file() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "\
module Types exposing (Msg(..))

type Msg
    = Increment
    | Decrement
",
    );

    write_elm(
        root,
        "src/Update.elm",
        "\
module Update exposing (..)

import Types exposing (Msg(..))

update : Msg -> Int -> Int
update msg count =
    case msg of
        Increment ->
            count + 1

        Decrement ->
            count - 1
",
    );

    write_elm(
        root,
        "src/View.elm",
        "\
module View exposing (..)

import Types exposing (Msg(..))

label : Msg -> String
label msg =
    case msg of
        Increment ->
            \"inc\"

        Decrement ->
            \"dec\"
",
    );

    let output = elmq()
        .current_dir(root)
        .args(["variant", "add", "src/Types.elm", "--type", "Msg", "Reset"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");

    // Both files should have the new branch.
    let update_content = read_elm(root, "src/Update.elm");
    assert!(
        update_content.contains("Reset ->"),
        "Update.elm should have Reset branch: {update_content}"
    );

    let view_content = read_elm(root, "src/View.elm");
    assert!(
        view_content.contains("Reset ->"),
        "View.elm should have Reset branch: {view_content}"
    );
}

// -- rm variant: simple case --

#[test]
fn rm_variant_simple() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "\
module Types exposing (Msg(..))

type Msg
    = Increment
    | Decrement
    | Reset
",
    );

    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types exposing (Msg(..))

update : Msg -> Int -> Int
update msg count =
    case msg of
        Increment ->
            count + 1

        Decrement ->
            count - 1

        Reset ->
            0
",
    );

    let output = elmq()
        .current_dir(root)
        .args(["variant", "rm", "src/Types.elm", "--type", "Msg", "Reset"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stderr: {stderr}\nstdout: {stdout}"
    );

    // Type should not have the variant.
    let types_content = read_elm(root, "src/Types.elm");
    assert!(
        !types_content.contains("Reset"),
        "type should not have Reset: {types_content}"
    );

    // Case should not have the branch.
    let main_content = read_elm(root, "src/Main.elm");
    assert!(
        !main_content.contains("Reset ->"),
        "case should not have Reset branch: {main_content}"
    );
}

// -- rm variant: last variant error --

#[test]
fn rm_variant_last_variant_error() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "\
module Types exposing (Msg(..))

type Msg
    = Only
",
    );

    let output = elmq()
        .current_dir(root)
        .args(["variant", "rm", "src/Types.elm", "--type", "Msg", "Only"])
        .output()
        .unwrap();

    assert!(!output.status.success(), "should fail for last variant");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("cannot remove the last variant") || stderr.contains("elmq rm"),
        "should suggest using elmq rm: {stderr}"
    );
}

// -- dry-run: no files modified --

#[test]
fn add_variant_dry_run() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "\
module Types exposing (Msg(..))

type Msg
    = Increment
    | Decrement
",
    );

    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types exposing (Msg(..))

update msg count =
    case msg of
        Increment ->
            count + 1

        Decrement ->
            count - 1
",
    );

    let original_types = read_elm(root, "src/Types.elm");
    let original_main = read_elm(root, "src/Main.elm");

    let output = elmq()
        .current_dir(root)
        .args([
            "variant",
            "add",
            "src/Types.elm",
            "--type",
            "Msg",
            "--dry-run",
            "Reset",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    assert!(
        stdout.contains("(dry run)"),
        "should show dry run prefix: {stdout}"
    );

    // Files should be unchanged.
    assert_eq!(read_elm(root, "src/Types.elm"), original_types);
    assert_eq!(read_elm(root, "src/Main.elm"), original_main);
}

// -- complex variant definition --

#[test]
fn add_variant_complex_definition() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "\
module Types exposing (Msg(..))

type Msg
    = Increment
    | Decrement
",
    );

    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types exposing (Msg(..))

update msg count =
    case msg of
        Increment ->
            count + 1

        Decrement ->
            count - 1
",
    );

    let output = elmq()
        .current_dir(root)
        .args([
            "variant",
            "add",
            "src/Types.elm",
            "--type",
            "Msg",
            "GotResponse (Result String Int)",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");

    let types_content = read_elm(root, "src/Types.elm");
    assert!(
        types_content.contains("| GotResponse (Result String Int)"),
        "type should have complex variant: {types_content}"
    );

    let main_content = read_elm(root, "src/Main.elm");
    assert!(
        main_content.contains("GotResponse _ ->"),
        "branch should have wildcard for complex arg: {main_content}"
    );
}

// -- rm variant: multi-file --

#[test]
fn rm_variant_multi_file() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "\
module Types exposing (Msg(..))

type Msg
    = Increment
    | Decrement
    | Reset
",
    );

    write_elm(
        root,
        "src/Update.elm",
        "\
module Update exposing (..)

import Types exposing (Msg(..))

update msg count =
    case msg of
        Increment ->
            count + 1

        Decrement ->
            count - 1

        Reset ->
            0
",
    );

    write_elm(
        root,
        "src/View.elm",
        "\
module View exposing (..)

import Types exposing (Msg(..))

label msg =
    case msg of
        Increment ->
            \"inc\"

        Decrement ->
            \"dec\"

        Reset ->
            \"reset\"
",
    );

    let output = elmq()
        .current_dir(root)
        .args(["variant", "rm", "src/Types.elm", "--type", "Msg", "Reset"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");

    let update_content = read_elm(root, "src/Update.elm");
    assert!(
        !update_content.contains("Reset"),
        "Update.elm should not have Reset: {update_content}"
    );

    let view_content = read_elm(root, "src/View.elm");
    assert!(
        !view_content.contains("Reset"),
        "View.elm should not have Reset: {view_content}"
    );
}

// -- add variant: same-file case expressions --

#[test]
fn add_variant_same_file() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "\
module Types exposing (Msg(..))

type Msg
    = Increment
    | Decrement

update msg count =
    case msg of
        Increment ->
            count + 1

        Decrement ->
            count - 1

label msg =
    case msg of
        Increment ->
            \"inc\"

        Decrement ->
            \"dec\"
",
    );

    let output = elmq()
        .current_dir(root)
        .args(["variant", "add", "src/Types.elm", "--type", "Msg", "Reset"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");

    let content = read_elm(root, "src/Types.elm");
    // Should have the variant in the type.
    assert!(
        content.contains("| Reset"),
        "should have Reset variant: {content}"
    );

    // Count occurrences of "Reset ->" — should be 2 (one per case expression).
    let branch_count = content.matches("Reset ->").count();
    assert_eq!(
        branch_count, 2,
        "should have 2 Reset branches (one per case): {content}"
    );
}

// -- add variant: qualified constructor references --

#[test]
fn add_variant_qualified_refs() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "\
module Types exposing (Msg(..))

type Msg
    = Increment
    | Decrement
",
    );

    // Uses qualified constructor references (no exposing (Msg(..)))
    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types

update : Types.Msg -> Int -> Int
update msg count =
    case msg of
        Types.Increment ->
            count + 1

        Types.Decrement ->
            count - 1
",
    );

    let output = elmq()
        .current_dir(root)
        .args(["variant", "add", "src/Types.elm", "--type", "Msg", "Reset"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");

    let main_content = read_elm(root, "src/Main.elm");
    assert!(
        main_content.contains("Reset ->"),
        "should insert branch for qualified refs: {main_content}"
    );
}

// -- add variant: aliased import --

#[test]
fn add_variant_aliased_import() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "\
module Types exposing (Msg(..))

type Msg
    = Increment
    | Decrement
",
    );

    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types as T

update msg count =
    case msg of
        T.Increment ->
            count + 1

        T.Decrement ->
            count - 1
",
    );

    let output = elmq()
        .current_dir(root)
        .args(["variant", "add", "src/Types.elm", "--type", "Msg", "Reset"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");

    let main_content = read_elm(root, "src/Main.elm");
    assert!(
        main_content.contains("Reset ->"),
        "should insert branch for aliased refs: {main_content}"
    );
}

// -- multiple types: doesn't affect unrelated type --

#[test]
fn add_variant_multiple_types() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "\
module Types exposing (Msg(..), Color(..))

type Msg
    = Increment
    | Decrement

type Color
    = Red
    | Blue
",
    );

    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types exposing (Msg(..), Color(..))

update msg count =
    case msg of
        Increment ->
            count + 1

        Decrement ->
            count - 1

label color =
    case color of
        Red ->
            \"red\"

        Blue ->
            \"blue\"
",
    );

    let output = elmq()
        .current_dir(root)
        .args(["variant", "add", "src/Types.elm", "--type", "Msg", "Reset"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");

    let main_content = read_elm(root, "src/Main.elm");
    // Msg case should have Reset.
    assert!(
        main_content.contains("Reset ->"),
        "Msg case should have Reset: {main_content}"
    );
    // Color case should NOT have Reset.
    let color_case_start = main_content.find("case color of").unwrap();
    let color_case = &main_content[color_case_start..];
    assert!(
        !color_case.contains("Reset"),
        "Color case should not have Reset: {color_case}"
    );
}

// -- rm variant: first variant removal --

#[test]
fn rm_variant_first() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "\
module Types exposing (Msg(..))

type Msg
    = Increment
    | Decrement
    | Reset
",
    );

    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types exposing (Msg(..))

update msg count =
    case msg of
        Increment ->
            count + 1

        Decrement ->
            count - 1

        Reset ->
            0
",
    );

    let output = elmq()
        .current_dir(root)
        .args([
            "variant",
            "rm",
            "src/Types.elm",
            "--type",
            "Msg",
            "Increment",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stderr: {stderr}\nstdout: {stdout}"
    );

    let types_content = read_elm(root, "src/Types.elm");
    assert!(
        !types_content.contains("Increment"),
        "type should not have Increment: {types_content}"
    );
    // Should still have the other variants.
    assert!(
        types_content.contains("Decrement"),
        "type should still have Decrement: {types_content}"
    );

    let main_content = read_elm(root, "src/Main.elm");
    assert!(
        !main_content.contains("Increment"),
        "case should not have Increment: {main_content}"
    );
}

// -- rm variant: dry-run --

#[test]
fn rm_variant_dry_run() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "\
module Types exposing (Msg(..))

type Msg
    = Increment
    | Decrement
",
    );

    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types exposing (Msg(..))

update msg count =
    case msg of
        Increment ->
            count + 1

        Decrement ->
            count - 1
",
    );

    let original_types = read_elm(root, "src/Types.elm");
    let original_main = read_elm(root, "src/Main.elm");

    let output = elmq()
        .current_dir(root)
        .args([
            "variant",
            "rm",
            "src/Types.elm",
            "--type",
            "Msg",
            "--dry-run",
            "Decrement",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    assert!(
        stdout.contains("(dry run)"),
        "should show dry run prefix: {stdout}"
    );

    // Files should be unchanged.
    assert_eq!(read_elm(root, "src/Types.elm"), original_types);
    assert_eq!(read_elm(root, "src/Main.elm"), original_main);
}

// -- error: constructor already exists --

#[test]
fn add_variant_already_exists() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "\
module Types exposing (Msg(..))

type Msg
    = Increment
    | Decrement
",
    );

    let output = elmq()
        .current_dir(root)
        .args([
            "variant",
            "add",
            "src/Types.elm",
            "--type",
            "Msg",
            "Increment",
        ])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "should fail for existing constructor"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("already exists"),
        "should say already exists: {stderr}"
    );
}

// -- error: constructor not found for rm --

#[test]
fn rm_variant_not_found() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "\
module Types exposing (Msg(..))

type Msg
    = Increment
    | Decrement
",
    );

    let output = elmq()
        .current_dir(root)
        .args([
            "variant",
            "rm",
            "src/Types.elm",
            "--type",
            "Msg",
            "NonExistent",
        ])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "should fail for missing constructor"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("not found"),
        "should say not found: {stderr}"
    );
}
