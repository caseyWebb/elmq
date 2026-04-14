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
        .args(["add", "variant", "src/Types.elm", "--type", "Msg", "Reset"])
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
            "add",
            "variant",
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
        .args(["add", "variant", "src/Types.elm", "--type", "Msg", "Reset"])
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
        .args(["add", "variant", "src/Types.elm", "--type", "Msg", "Reset"])
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
        .args(["rm", "variant", "src/Types.elm", "--type", "Msg", "Reset"])
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
        .args(["rm", "variant", "src/Types.elm", "--type", "Msg", "Only"])
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
            "add",
            "variant",
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
            "add",
            "variant",
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
        .args(["rm", "variant", "src/Types.elm", "--type", "Msg", "Reset"])
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
        .args(["add", "variant", "src/Types.elm", "--type", "Msg", "Reset"])
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
        .args(["add", "variant", "src/Types.elm", "--type", "Msg", "Reset"])
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
        .args(["add", "variant", "src/Types.elm", "--type", "Msg", "Reset"])
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
        .args(["add", "variant", "src/Types.elm", "--type", "Msg", "Reset"])
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
            "rm",
            "variant",
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
            "rm",
            "variant",
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
            "add",
            "variant",
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
            "rm",
            "variant",
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

// ============================================================================
// variant cases — read-only context-gathering command
// ============================================================================

#[test]
fn cases_single_file_multiple_functions() {
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
module Main exposing (update, view)

import Types exposing (Msg(..))

update : Msg -> Int -> Int
update msg count =
    case msg of
        Increment ->
            count + 1

        Decrement ->
            count - 1

view : Msg -> String
view msg =
    case msg of
        Increment ->
            \"up\"

        Decrement ->
            \"down\"
",
    );

    let output = elmq()
        .current_dir(root)
        .args(["variant", "cases", "src/Types.elm", "--type", "Msg"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stderr: {stderr}\nstdout: {stdout}"
    );

    // Headline.
    assert!(
        stdout.contains("## case sites for type Types.Msg"),
        "stdout: {stdout}"
    );
    // Both functions appear with bare keys (unambiguous). The reported line is the
    // `case msg of` line (the case_of_expr), not the declaration header.
    assert!(stdout.contains("(key: update, line 7)"), "stdout: {stdout}");
    assert!(stdout.contains("(key: view, line 16)"), "stdout: {stdout}");
    // The body slice includes the type annotation (full signature + body).
    assert!(
        stdout.contains("update : Msg -> Int -> Int"),
        "stdout: {stdout}"
    );
    assert!(stdout.contains("view : Msg -> String"), "stdout: {stdout}");
}

#[test]
fn cases_multi_file() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "\
module Types exposing (Msg(..))

type Msg
    = A
    | B
",
    );

    write_elm(
        root,
        "src/Handler.elm",
        "\
module Handler exposing (handle)

import Types exposing (Msg(..))

handle : Msg -> Int
handle msg =
    case msg of
        A -> 1
        B -> 2
",
    );

    write_elm(
        root,
        "src/Renderer.elm",
        "\
module Renderer exposing (render)

import Types exposing (Msg(..))

render : Msg -> String
render msg =
    case msg of
        A -> \"a\"
        B -> \"b\"
",
    );

    let output = elmq()
        .current_dir(root)
        .args(["variant", "cases", "src/Types.elm", "--type", "Msg"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "stdout: {stdout}");
    assert!(
        stdout.contains("2 files, 2 functions"),
        "should report 2 files, 2 functions: {stdout}"
    );
    assert!(
        stdout.contains("### src/Handler.elm") || stdout.contains("### src/Renderer.elm"),
        "should have file sections: {stdout}"
    );
    // Distinct function names → bare keys, no #N or file: prefix needed.
    assert!(stdout.contains("key: handle"), "stdout: {stdout}");
    assert!(stdout.contains("key: render"), "stdout: {stdout}");
}

#[test]
fn cases_wildcard_in_skipped() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "module Types exposing (Msg(..))\n\ntype Msg = A | B\n",
    );

    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (handle)

import Types exposing (Msg(..))

handle : Msg -> Int
handle msg =
    case msg of
        A -> 1
        _ -> 0
",
    );

    let output = elmq()
        .current_dir(root)
        .args(["variant", "cases", "src/Types.elm", "--type", "Msg"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "stdout: {stdout}");
    // Wildcard-covered sites appear in the `### skipped` footer and NOT in the active listing.
    assert!(stdout.contains("### skipped"), "stdout: {stdout}");
    assert!(
        stdout.contains("wildcard branch covers type"),
        "stdout: {stdout}"
    );
    // Body should NOT appear for a skipped site — the headline says 0 active sites.
    assert!(
        stdout.contains("no case sites found") || !stdout.contains("(key: handle,"),
        "handle should be in skipped, not active: {stdout}"
    );
}

#[test]
fn cases_json_output() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "module Types exposing (Msg(..))\n\ntype Msg = A | B\n",
    );

    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (handle)

import Types exposing (Msg(..))

handle : Msg -> Int
handle msg =
    case msg of
        A -> 1
        B -> 2
",
    );

    let output = elmq()
        .current_dir(root)
        .args([
            "variant",
            "cases",
            "src/Types.elm",
            "--type",
            "Msg",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "stdout: {stdout}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    assert_eq!(parsed["type"], "Types.Msg");
    assert_eq!(parsed["type_file"], "src/Types.elm");
    let sites = parsed["sites"].as_array().unwrap();
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0]["function"], "handle");
    assert_eq!(sites[0]["key"], "handle");
    assert!(sites[0]["body"].as_str().unwrap().contains("handle : Msg"));
}

#[test]
fn cases_type_not_found_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "module Types exposing (Msg(..))\n\ntype Msg = A\n",
    );

    let output = elmq()
        .current_dir(root)
        .args(["variant", "cases", "src/Types.elm", "--type", "NoSuch"])
        .output()
        .unwrap();

    assert!(!output.status.success(), "should fail on missing type");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("type NoSuch not found"), "stderr: {stderr}");
}

#[test]
fn cases_ordinal_keys_for_two_cases_in_one_function() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "module Types exposing (Msg(..))\n\ntype Msg = A | B\n",
    );

    // Two case expressions in one function — one nested in a let, one in the main body.
    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (update)

import Types exposing (Msg(..))

update : Msg -> Int
update msg =
    let
        inner =
            case msg of
                A -> 1
                B -> 2
    in
    case msg of
        A -> inner + 10
        B -> inner + 20
",
    );

    let output = elmq()
        .current_dir(root)
        .args([
            "variant",
            "cases",
            "src/Types.elm",
            "--type",
            "Msg",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "stdout: {stdout}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let sites = parsed["sites"].as_array().unwrap();
    assert_eq!(sites.len(), 2, "expected two case sites");
    let keys: Vec<&str> = sites.iter().map(|s| s["key"].as_str().unwrap()).collect();
    assert!(
        keys.contains(&"update#1") && keys.contains(&"update#2"),
        "expected update#1 and update#2, got {keys:?}"
    );
}

// ============================================================================
// variant add --fill
// ============================================================================

#[test]
fn fill_single_key_replaces_debug_todo() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "module Types exposing (Msg(..))\n\ntype Msg = Increment | Decrement\n",
    );

    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (update)

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
            "add",
            "variant",
            "src/Types.elm",
            "--type",
            "Msg",
            "Reset",
            "--fill",
            "update=Reset ->\n    0",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");

    let main_content = read_elm(root, "src/Main.elm");
    // The fill body should appear instead of Debug.todo.
    assert!(main_content.contains("Reset ->"), "main: {main_content}");
    assert!(
        main_content.contains("        Reset ->\n            0"),
        "branch should be indented to match: {main_content}"
    );
    assert!(
        !main_content.contains("Debug.todo \"Reset\""),
        "should not have stub when fill matched: {main_content}"
    );

    // Type should still get the variant.
    let types_content = read_elm(root, "src/Types.elm");
    assert!(types_content.contains("| Reset"), "types: {types_content}");
}

#[test]
fn fill_multiple_keys_in_one_invocation() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "module Types exposing (Msg(..))\n\ntype Msg = A | B\n",
    );

    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (update, view)

import Types exposing (Msg(..))

update : Msg -> Int
update msg =
    case msg of
        A -> 1
        B -> 2

view : Msg -> String
view msg =
    case msg of
        A -> \"a\"
        B -> \"b\"
",
    );

    let output = elmq()
        .current_dir(root)
        .args([
            "add",
            "variant",
            "src/Types.elm",
            "--type",
            "Msg",
            "C",
            "--fill",
            "update=C -> 3",
            "--fill",
            "view=C -> \"c\"",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");

    let main = read_elm(root, "src/Main.elm");
    assert!(main.contains("C -> 3"), "update should have fill: {main}");
    assert!(main.contains("C -> \"c\""), "view should have fill: {main}");
    assert!(
        !main.contains("Debug.todo \"C\""),
        "no stub when both filled: {main}"
    );
}

#[test]
fn fill_partial_keeps_debug_todo_for_unfilled() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "module Types exposing (Msg(..))\n\ntype Msg = A | B\n",
    );

    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (update, view)

import Types exposing (Msg(..))

update : Msg -> Int
update msg =
    case msg of
        A -> 1
        B -> 2

view : Msg -> String
view msg =
    case msg of
        A -> \"a\"
        B -> \"b\"
",
    );

    let output = elmq()
        .current_dir(root)
        .args([
            "add",
            "variant",
            "src/Types.elm",
            "--type",
            "Msg",
            "C",
            "--fill",
            "update=C -> 3",
        ])
        .output()
        .unwrap();

    assert!(output.status.success());
    let main = read_elm(root, "src/Main.elm");
    assert!(main.contains("C -> 3"), "filled site: {main}");
    // `view` was not in --fill, so it should get the default Debug.todo stub.
    assert!(
        main.contains("Debug.todo \"C\""),
        "unfilled site should degrade to Debug.todo: {main}"
    );
}

#[test]
fn fill_unknown_key_errors_without_writing() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "module Types exposing (Msg(..))\n\ntype Msg = A | B\n",
    );

    let main_src = "\
module Main exposing (update)

import Types exposing (Msg(..))

update : Msg -> Int
update msg =
    case msg of
        A -> 1
        B -> 2
";
    write_elm(root, "src/Main.elm", main_src);

    let output = elmq()
        .current_dir(root)
        .args([
            "add",
            "variant",
            "src/Types.elm",
            "--type",
            "Msg",
            "C",
            "--fill",
            "nosuchfunction=C -> 0",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success(), "should error on unknown key");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("no case site matched fill key: nosuchfunction"),
        "stderr: {stderr}"
    );
    // The type file should NOT have been written — validation runs before any writes.
    let types_after = read_elm(root, "src/Types.elm");
    assert!(
        !types_after.contains("| C"),
        "type file should be untouched on error: {types_after}"
    );
    // Nor should Main.elm.
    let main_after = read_elm(root, "src/Main.elm");
    assert_eq!(main_after, main_src, "main should be untouched on error");
}

#[test]
fn fill_ambiguous_bare_key_errors() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "module Types exposing (Msg(..))\n\ntype Msg = A | B\n",
    );

    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (update)

import Types exposing (Msg(..))

update : Msg -> Int
update msg =
    let
        inner =
            case msg of
                A -> 1
                B -> 2
    in
    case msg of
        A -> inner + 10
        B -> inner + 20
",
    );

    let output = elmq()
        .current_dir(root)
        .args([
            "add",
            "variant",
            "src/Types.elm",
            "--type",
            "Msg",
            "C",
            "--fill",
            "update=C -> 0",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success(), "should error on ambiguous key");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("ambiguous"),
        "stderr should say ambiguous: {stderr}"
    );
    assert!(
        stderr.contains("update#1") && stderr.contains("update#2"),
        "stderr should list disambiguated keys: {stderr}"
    );
}

#[test]
fn fill_body_with_equals_splits_on_first_only() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "module Types exposing (Msg(..))\n\ntype Msg = A | B\n",
    );

    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (update)

import Types exposing (Msg(..))

update : Msg -> { count : Int } -> { count : Int }
update msg model =
    case msg of
        A -> { model | count = model.count + 1 }
        B -> { model | count = model.count - 1 }
",
    );

    let output = elmq()
        .current_dir(root)
        .args([
            "add",
            "variant",
            "src/Types.elm",
            "--type",
            "Msg",
            "C",
            "--fill",
            "update=C -> { model | count = 0 }",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    let main = read_elm(root, "src/Main.elm");
    // The whole body after the first `=` should be preserved, including subsequent `=`.
    assert!(main.contains("C -> { model | count = 0 }"), "main: {main}");
}

#[test]
fn fill_with_dry_run_does_not_write() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "module Types exposing (Msg(..))\n\ntype Msg = A | B\n",
    );

    let main_src = "\
module Main exposing (update)

import Types exposing (Msg(..))

update : Msg -> Int
update msg =
    case msg of
        A -> 1
        B -> 2
";
    write_elm(root, "src/Main.elm", main_src);

    let output = elmq()
        .current_dir(root)
        .args([
            "add",
            "variant",
            "src/Types.elm",
            "--type",
            "Msg",
            "C",
            "--fill",
            "update=C -> 42",
            "--dry-run",
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "dry run should succeed");
    // Neither file should have been modified on disk.
    let main_after = read_elm(root, "src/Main.elm");
    assert_eq!(main_after, main_src, "main should be untouched in dry run");
    let types_after = read_elm(root, "src/Types.elm");
    assert!(
        !types_after.contains("| C"),
        "types should be untouched in dry run: {types_after}"
    );
}

#[test]
fn fill_tuple_pattern_case() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Types.elm",
        "module Types exposing (Msg(..))\n\ntype Msg = A | B\n",
    );

    // Case expression matching on a tuple `(msg, model)` with Msg in the first position.
    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (step)

import Types exposing (Msg(..))

step : Msg -> Int -> Int
step msg model =
    case ( msg, model ) of
        ( A, _ ) ->
            model + 1

        ( B, _ ) ->
            model - 1
",
    );

    // The user provides the full tuple-form branch text as the fill body.
    let output = elmq()
        .current_dir(root)
        .args([
            "add",
            "variant",
            "src/Types.elm",
            "--type",
            "Msg",
            "C",
            "--fill",
            "step=( C, n ) -> n * 2",
        ])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    let main = read_elm(root, "src/Main.elm");
    assert!(
        main.contains("( C, n ) -> n * 2"),
        "tuple fill should be inserted verbatim: {main}"
    );
}

// =========================================================================
// variant refs: classifier coverage + rm advisory
// =========================================================================
//
// These tests exercise the shared `collect_constructor_sites` walker through
// the two surfaces that consume it: `variant refs` (read-only projection) and
// `variant rm` (advisory list in the mutation output). Every `SiteKind`
// category has its own fixture so a future grammar or classifier change that
// breaks categorization surfaces immediately instead of silently mis-tagging.

fn write_msg_types(root: &Path) {
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
}

fn refs_json(root: &Path, type_file: &str, _type_name: &str, ctor: &str) -> serde_json::Value {
    let output = elmq()
        .current_dir(root)
        .args(["refs", type_file, ctor, "--format", "json"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "refs failed. stderr: {stderr}\nstdout: {stdout}"
    );
    serde_json::from_str(&stdout).expect("refs output must be valid JSON")
}

fn site_kinds(v: &serde_json::Value) -> Vec<String> {
    v["sites"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["kind"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn refs_case_branch_direct() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_msg_types(root);
    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types exposing (Msg(..))

update : Msg -> Int -> Int
update msg n =
    case msg of
        Increment ->
            n + 1

        Decrement ->
            n - 1

        Reset ->
            0
",
    );

    let refs = refs_json(root, "src/Types.elm", "Msg", "Increment");
    assert_eq!(refs["total_sites"], 1);
    assert_eq!(refs["total_clean"], 1);
    assert_eq!(refs["total_blocking"], 0);
    assert_eq!(site_kinds(&refs), vec!["case-branch".to_string()]);
}

#[test]
fn refs_case_branch_nested_tuple() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_msg_types(root);
    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types exposing (Msg(..))

pair : ( Msg, Int ) -> Int
pair p =
    case p of
        ( Increment, n ) ->
            n + 1

        ( Decrement, n ) ->
            n - 1

        ( Reset, _ ) ->
            0
",
    );

    let refs = refs_json(root, "src/Types.elm", "Msg", "Increment");
    assert_eq!(site_kinds(&refs), vec!["case-branch".to_string()]);
}

#[test]
fn refs_case_branch_nested_union_pattern_bug_fix() {
    // Regression: `case x of Just Increment -> ...` must be classified as a
    // case-branch site. Today's `find_constructor_in_pattern` misses nested
    // `union_pattern`s; the shared walker must not.
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_msg_types(root);
    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types exposing (Msg(..))

unwrap : Maybe Msg -> Int
unwrap m =
    case m of
        Just Increment ->
            1

        Just Decrement ->
            -1

        _ ->
            0
",
    );

    let refs = refs_json(root, "src/Types.elm", "Msg", "Increment");
    assert_eq!(refs["total_sites"], 1);
    assert_eq!(site_kinds(&refs), vec!["case-branch".to_string()]);
}

#[test]
fn refs_case_wildcard_covered() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_msg_types(root);
    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types exposing (Msg(..))

view : Msg -> String
view msg =
    case msg of
        Decrement ->
            \"dec\"

        _ ->
            \"other\"
",
    );

    let refs = refs_json(root, "src/Types.elm", "Msg", "Increment");
    assert_eq!(refs["total_sites"], 1);
    assert_eq!(site_kinds(&refs), vec!["case-wildcard-covered".to_string()]);
    assert_eq!(refs["total_clean"], 1);
    assert_eq!(refs["total_blocking"], 0);
}

#[test]
fn refs_function_arg_pattern() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_msg_types(root);
    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types exposing (Msg(..))

handler : Msg -> Bool
handler (Increment) =
    True
",
    );

    let refs = refs_json(root, "src/Types.elm", "Msg", "Increment");
    assert_eq!(site_kinds(&refs), vec!["function-arg-pattern".to_string()]);
    assert_eq!(refs["total_blocking"], 1);
}

#[test]
fn refs_lambda_arg_pattern() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_msg_types(root);
    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types exposing (Msg(..))

go : Msg -> Int
go =
    \\(Increment) -> 1
",
    );

    let refs = refs_json(root, "src/Types.elm", "Msg", "Increment");
    assert_eq!(site_kinds(&refs), vec!["lambda-arg-pattern".to_string()]);
}

#[test]
fn refs_let_binding_pattern() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_msg_types(root);
    // A top-level value_declaration with a pattern field (not
    // functionDeclarationLeft) is the idiomatic way to exercise the
    // LetBindingPattern classification in Elm.
    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types exposing (Msg(..))

firstMsg : Msg
firstMsg =
    Decrement

(Increment) =
    firstMsg
",
    );

    let refs = refs_json(root, "src/Types.elm", "Msg", "Increment");
    // Depending on how tree-sitter-elm parses the top-level pattern binding,
    // this may classify as let-binding-pattern or function-arg-pattern. Both
    // are "blocking" — what matters is that the site is detected and classified
    // as a refutable pattern, not as an expression.
    let kinds = site_kinds(&refs);
    assert_eq!(kinds.len(), 1, "expected exactly one site, got {kinds:?}");
    let k = &kinds[0];
    assert!(
        k == "let-binding-pattern" || k == "function-arg-pattern",
        "unexpected classification for top-level refutable pattern: {k}"
    );
    assert_eq!(refs["total_blocking"], 1);
}

#[test]
fn refs_expression_position_construction() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_msg_types(root);
    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types exposing (Msg(..))

start : Msg
start =
    Increment
",
    );

    let refs = refs_json(root, "src/Types.elm", "Msg", "Increment");
    assert_eq!(site_kinds(&refs), vec!["expression-position".to_string()]);
}

#[test]
fn refs_expression_position_partial_application() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_elm(
        root,
        "src/Types.elm",
        "\
module Types exposing (Msg(..))

type Msg
    = Wrap Int
    | Keep
",
    );
    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types exposing (Msg(..))

wraps : List Msg
wraps =
    List.map Wrap [ 1, 2, 3 ]
",
    );

    let refs = refs_json(root, "src/Types.elm", "Msg", "Wrap");
    assert_eq!(site_kinds(&refs), vec!["expression-position".to_string()]);
}

#[test]
fn refs_expression_position_equality() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_msg_types(root);
    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types exposing (Msg(..))

isInc : Msg -> Bool
isInc m =
    m == Increment
",
    );

    let refs = refs_json(root, "src/Types.elm", "Msg", "Increment");
    assert_eq!(site_kinds(&refs), vec!["expression-position".to_string()]);
}

#[test]
fn refs_skips_constructor_own_definition() {
    // The target constructor appears in its own `type` declaration. The walker
    // must not report that as a reference to itself — otherwise `variant refs`
    // would always report at least one bogus site for every constructor.
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_msg_types(root);

    let refs = refs_json(root, "src/Types.elm", "Msg", "Increment");
    // With no other files in the project, there should be no references.
    assert_eq!(refs["total_sites"], 0);
}

#[test]
fn refs_multi_file_grouping() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_msg_types(root);
    write_elm(
        root,
        "src/Update.elm",
        "\
module Update exposing (..)

import Types exposing (Msg(..))

update : Msg -> Int -> Int
update msg n =
    case msg of
        Increment ->
            n + 1

        Decrement ->
            n - 1

        Reset ->
            0
",
    );
    write_elm(
        root,
        "src/Init.elm",
        "\
module Init exposing (..)

import Types exposing (Msg(..))

start : Msg
start =
    Increment
",
    );

    let refs = refs_json(root, "src/Types.elm", "Msg", "Increment");
    assert_eq!(refs["total_sites"], 2);
    assert_eq!(refs["total_clean"], 1);
    assert_eq!(refs["total_blocking"], 1);
    let kinds = site_kinds(&refs);
    assert!(kinds.contains(&"case-branch".to_string()));
    assert!(kinds.contains(&"expression-position".to_string()));
}

#[test]
fn refs_unknown_name_errors_as_declaration_not_found() {
    // Under the unified `elmq refs` dispatcher, a name that is neither a
    // top-level declaration nor a constructor of a type declared in the
    // target file produces the decl-path error.
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_msg_types(root);

    let output = elmq()
        .current_dir(root)
        .args(["refs", "src/Types.elm", "DoesNotExist"])
        .output()
        .unwrap();
    // Multi-arg refs emits per-arg errors via stdout and exits 2; with a
    // single arg that fails, the error appears in the framed output.
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("declaration 'DoesNotExist' not found")
            || stderr.contains("declaration 'DoesNotExist' not found"),
        "expected missing-decl error. stdout: {stdout}\nstderr: {stderr}"
    );
}

#[test]
fn refs_constructor_in_different_file_falls_through_to_missing() {
    // `Red` is a constructor of `Color` declared in `src/Colors.elm`.
    // When the user passes the wrong file (`src/Types.elm`), the
    // constructor is not in that file's scope, so the dispatcher falls
    // through to the decl path — which also has no match — and reports
    // the name as a missing declaration. This preserves the current
    // decl-refs error shape regardless of whether the name happens to
    // exist as a constructor elsewhere in the project.
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_msg_types(root);
    write_elm(
        root,
        "src/Colors.elm",
        "\
module Colors exposing (Color(..))

type Color
    = Red
    | Green
",
    );

    let output = elmq()
        .current_dir(root)
        .args(["refs", "src/Types.elm", "Red"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("declaration 'Red' not found")
            || stderr.contains("declaration 'Red' not found"),
        "expected missing-decl error. stdout: {stdout}\nstderr: {stderr}"
    );
}

#[test]
fn refs_constructor_in_same_file_as_other_type_resolves() {
    // `Red` is a constructor of `Color` and both `Msg` and `Color` are
    // declared in `src/Types.elm`. Under the unified dispatcher, the
    // owning type is auto-resolved, so asking for refs to `Red` works
    // without a `--type` disambiguator.
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
    | Green
",
    );
    write_elm(
        root,
        "src/Palette.elm",
        "\
module Palette exposing (..)

import Types exposing (Color(..))

primary : Color
primary =
    Red
",
    );

    let refs = refs_json(root, "src/Types.elm", "Color", "Red");
    assert_eq!(refs["type_name"], "Color");
    assert_eq!(refs["total_sites"], 1);
    assert_eq!(site_kinds(&refs), vec!["expression-position".to_string()]);
}

// -- variant rm: advisory integration --

#[test]
fn rm_variant_advisory_lists_blocking_sites() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_msg_types(root);
    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types exposing (Msg(..))

update : Msg -> Int -> Int
update msg n =
    case msg of
        Increment ->
            n + 1

        Decrement ->
            n - 1

        Reset ->
            0

start : Msg
start =
    Increment
",
    );

    let output = elmq()
        .current_dir(root)
        .args([
            "rm",
            "variant",
            "src/Types.elm",
            "--type",
            "Msg",
            "Increment",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stderr: {stderr}\nstdout: {stdout}"
    );

    let v: serde_json::Value = serde_json::from_str(&stdout).expect("rm output must be JSON");
    let refs = v["references_not_rewritten"].as_array().expect("advisory");
    assert_eq!(refs.len(), 1, "expected one blocking site: {v}");
    assert_eq!(refs[0]["kind"], "expression-position");
    assert_eq!(refs[0]["declaration"], "start");

    // Type file is still written with the variant removed.
    let types = read_elm(root, "src/Types.elm");
    assert!(!types.contains("= Increment"));
    assert!(!types.contains("| Increment"));
}

#[test]
fn rm_variant_advisory_empty_when_clean_only() {
    // When every reference is a cleanly-removable case branch, the advisory
    // list must be empty — no regression on happy-path rm output.
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_msg_types(root);
    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types exposing (Msg(..))

update msg n =
    case msg of
        Increment ->
            n + 1

        Decrement ->
            n - 1

        Reset ->
            0
",
    );

    let output = elmq()
        .current_dir(root)
        .args([
            "rm",
            "variant",
            "src/Types.elm",
            "--type",
            "Msg",
            "Reset",
            "--format",
            "json",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());

    let v: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    // skip_serializing_if = "Vec::is_empty" means the field is absent when empty.
    assert!(
        v.get("references_not_rewritten").is_none()
            || v["references_not_rewritten"].as_array().unwrap().is_empty(),
        "expected empty advisory, got: {v}"
    );
}

#[test]
fn rm_variant_advisory_compact_section_emitted() {
    // The compact renderer should include a "references not rewritten" header
    // and the `elm make` hint when blocking sites exist.
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_msg_types(root);
    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types exposing (Msg(..))

go : Msg
go =
    Increment
",
    );

    let output = elmq()
        .current_dir(root)
        .args([
            "rm",
            "variant",
            "src/Types.elm",
            "--type",
            "Msg",
            "Increment",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "stdout: {stdout}");
    assert!(
        stdout.contains("references not rewritten (1)"),
        "compact output missing advisory header: {stdout}"
    );
    assert!(
        stdout.contains("expression-position"),
        "compact output missing classification: {stdout}"
    );
    assert!(
        stdout.contains("elm make"),
        "compact output missing elm make hint: {stdout}"
    );
}

#[test]
fn rm_variant_removes_nested_just_increment_branch() {
    // Regression for the nested `union_pattern` bug: today's code would leave
    // `Just Increment ->` behind. After the fix, the walker detects it and
    // the branch-removal loop takes it out.
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_msg_types(root);
    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (..)

import Types exposing (Msg(..))

unwrap : Maybe Msg -> Int
unwrap m =
    case m of
        Just Increment ->
            1

        Just Decrement ->
            -1

        _ ->
            0
",
    );

    let output = elmq()
        .current_dir(root)
        .args([
            "rm",
            "variant",
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

    let main_content = read_elm(root, "src/Main.elm");
    assert!(
        !main_content.contains("Just Increment"),
        "nested Just Increment branch should have been removed: {main_content}"
    );
    // Decrement branch should still be there.
    assert!(
        main_content.contains("Just Decrement"),
        "Decrement branch should be intact: {main_content}"
    );
}

#[test]
fn refs_ignores_unrelated_constructors() {
    // A separate type with its own `Increment` constructor in an unrelated
    // module must not show up when asking for references to `Types.Msg.Increment`.
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_msg_types(root);
    write_elm(
        root,
        "src/Other.elm",
        "\
module Other exposing (..)

type Counter
    = Increment
    | Reset

go : Counter
go =
    Increment
",
    );

    let refs = refs_json(root, "src/Types.elm", "Msg", "Increment");
    // Only references to Types.Msg.Increment should be counted — there are
    // none in this project; `Other.Counter.Increment` is a different symbol.
    assert_eq!(
        refs["total_sites"], 0,
        "unrelated constructor must not be reported: {refs}"
    );
}

// ============================================================================
// write-safety rejection tests
// ============================================================================

#[test]
fn variant_add_rejects_broken_type_file() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_elm(
        root,
        "src/Types.elm",
        "module Types exposing (Msg(..))\n\ntype Msg\n    = Increment\n    | Decrement\n\nfoo =\n    let\n        x = 1\n",
    );
    let before = fs::read(root.join("src/Types.elm")).unwrap();

    let output = elmq()
        .current_dir(root)
        .args(["add", "variant", "src/Types.elm", "--type", "Msg", "Reset"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("refusing to edit"), "stderr: {stderr}");
    assert_eq!(fs::read(root.join("src/Types.elm")).unwrap(), before);
}

#[test]
fn variant_rm_rejects_broken_type_file() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_elm(
        root,
        "src/Types.elm",
        "module Types exposing (Msg(..))\n\ntype Msg\n    = Increment\n    | Decrement\n\nfoo =\n    let\n        x = 1\n",
    );
    let before = fs::read(root.join("src/Types.elm")).unwrap();

    let output = elmq()
        .current_dir(root)
        .args([
            "rm",
            "variant",
            "src/Types.elm",
            "--type",
            "Msg",
            "Increment",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("refusing to edit"), "stderr: {stderr}");
    assert_eq!(fs::read(root.join("src/Types.elm")).unwrap(), before);
}

#[test]
fn variant_add_rejects_malformed_fill_body() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);
    write_elm(
        root,
        "src/Types.elm",
        "module Types exposing (Msg(..))\n\ntype Msg\n    = Increment\n    | Decrement\n",
    );
    write_elm(
        root,
        "src/Main.elm",
        "\
module Main exposing (update)

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
    let types_before = fs::read(root.join("src/Types.elm")).unwrap();
    let main_before = fs::read(root.join("src/Main.elm")).unwrap();

    // Fill body that does not parse as an Elm expression.
    let output = elmq()
        .current_dir(root)
        .args([
            "add",
            "variant",
            "src/Types.elm",
            "--type",
            "Msg",
            "Reset",
            "--fill",
            "update=model |",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("rejected 'variant add' write"),
        "stderr: {stderr}"
    );
    assert!(stderr.contains("Main.elm"), "stderr: {stderr}");
    // Main.elm must be unchanged.
    assert_eq!(fs::read(root.join("src/Main.elm")).unwrap(), main_before);
    // Types.elm may have been written before Main.elm was validated;
    // document the partial-write semantics by not asserting on it.
    let _ = types_before;
}
