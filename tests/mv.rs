use std::fs;
use std::path::Path;
use std::process::Command;

fn elmq() -> Command {
    Command::new(env!("CARGO_BIN_EXE_elmq"))
}

/// Create a minimal Elm project in the given directory.
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
fn mv_renames_file_and_updates_references() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Foo/Bar.elm",
        "module Foo.Bar exposing (baz)\n\nbaz = 1\n",
    );
    write_elm(
        root,
        "src/Main.elm",
        "module Main exposing (..)\n\nimport Foo.Bar exposing (baz)\n\nmain = baz\n",
    );

    let output = elmq()
        .current_dir(root)
        .args(["mv", "src/Foo/Bar.elm", "src/Foo/Baz.elm"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stderr: {stderr}\nstdout: {stdout}"
    );

    // Old file is gone, new file exists.
    assert!(!root.join("src/Foo/Bar.elm").exists());
    assert!(root.join("src/Foo/Baz.elm").exists());

    // New file has updated module declaration.
    let new_content = fs::read_to_string(root.join("src/Foo/Baz.elm")).unwrap();
    assert!(new_content.contains("module Foo.Baz exposing (baz)"));

    // Main.elm has updated import.
    let main_content = fs::read_to_string(root.join("src/Main.elm")).unwrap();
    assert!(main_content.contains("import Foo.Baz exposing (baz)"));
    assert!(!main_content.contains("Foo.Bar"));

    // Output reports the rename.
    assert!(stdout.contains("renamed"));
    assert!(stdout.contains("Foo/Baz.elm"));
}

#[test]
fn mv_updates_qualified_references() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Helpers.elm",
        "module Helpers exposing (add)\n\nadd x y = x + y\n",
    );
    write_elm(
        root,
        "src/Main.elm",
        "module Main exposing (..)\n\nimport Helpers\n\nmain = Helpers.add 1 2\n",
    );

    let output = elmq()
        .current_dir(root)
        .args(["mv", "src/Helpers.elm", "src/Utils/Helpers.elm"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");

    let main_content = fs::read_to_string(root.join("src/Main.elm")).unwrap();
    assert!(main_content.contains("import Utils.Helpers"));
    assert!(main_content.contains("Utils.Helpers.add 1 2"));
    assert!(!main_content.contains("\nimport Helpers\n"));

    let new_content = fs::read_to_string(root.join("src/Utils/Helpers.elm")).unwrap();
    assert!(new_content.contains("module Utils.Helpers exposing (add)"));
}

#[test]
fn mv_preserves_import_alias() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Foo/Bar.elm",
        "module Foo.Bar exposing (baz)\n\nbaz = 1\n",
    );
    write_elm(
        root,
        "src/Main.elm",
        "module Main exposing (..)\n\nimport Foo.Bar as FB exposing (baz)\n\nmain = baz\n",
    );

    let output = elmq()
        .current_dir(root)
        .args(["mv", "src/Foo/Bar.elm", "src/Foo/Baz.elm"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");

    let main_content = fs::read_to_string(root.join("src/Main.elm")).unwrap();
    assert!(main_content.contains("import Foo.Baz as FB exposing (baz)"));
}

#[test]
fn mv_dry_run_does_not_modify_files() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Foo/Bar.elm",
        "module Foo.Bar exposing (baz)\n\nbaz = 1\n",
    );
    write_elm(
        root,
        "src/Main.elm",
        "module Main exposing (..)\n\nimport Foo.Bar\n\nmain = Foo.Bar.baz\n",
    );

    let output = elmq()
        .current_dir(root)
        .args(["mv", "src/Foo/Bar.elm", "src/Foo/Baz.elm", "--dry-run"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    assert!(stdout.contains("(dry run)"));

    // Files are unchanged.
    assert!(root.join("src/Foo/Bar.elm").exists());
    assert!(!root.join("src/Foo/Baz.elm").exists());
    let main_content = fs::read_to_string(root.join("src/Main.elm")).unwrap();
    assert!(main_content.contains("import Foo.Bar"));
}

#[test]
fn mv_json_output() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Foo/Bar.elm",
        "module Foo.Bar exposing (baz)\n\nbaz = 1\n",
    );
    write_elm(
        root,
        "src/Main.elm",
        "module Main exposing (..)\n\nimport Foo.Bar\n\nmain = 1\n",
    );

    let output = elmq()
        .current_dir(root)
        .args([
            "mv",
            "src/Foo/Bar.elm",
            "src/Foo/Baz.elm",
            "--format",
            "json",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");

    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["renamed"]["from"], "src/Foo/Bar.elm");
    assert_eq!(json["renamed"]["to"], "src/Foo/Baz.elm");
}

#[test]
fn mv_errors_on_missing_source() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    let output = elmq()
        .current_dir(root)
        .args(["mv", "src/DoesNotExist.elm", "src/Other.elm"])
        .output()
        .unwrap();

    assert!(!output.status.success());
}

#[test]
fn mv_errors_on_existing_target() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(root, "src/Foo.elm", "module Foo exposing (..)\n\nfoo = 1\n");
    write_elm(root, "src/Bar.elm", "module Bar exposing (..)\n\nbar = 1\n");

    let output = elmq()
        .current_dir(root)
        .args(["mv", "src/Foo.elm", "src/Bar.elm"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already exists"));
}

#[test]
fn mv_errors_on_no_elm_json() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    fs::create_dir_all(root.join("src")).unwrap();
    write_elm(root, "src/Foo.elm", "module Foo exposing (..)\n\nfoo = 1\n");

    let output = elmq()
        .current_dir(root)
        .args(["mv", "src/Foo.elm", "src/Bar.elm"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("elm.json"));
}

#[test]
fn mv_same_module_name_between_source_dirs() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src", "lib"]);

    write_elm(root, "src/Foo.elm", "module Foo exposing (..)\n\nfoo = 1\n");
    write_elm(
        root,
        "src/Main.elm",
        "module Main exposing (..)\n\nimport Foo\n\nmain = Foo.foo\n",
    );

    let output = elmq()
        .current_dir(root)
        .args(["mv", "src/Foo.elm", "lib/Foo.elm"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");

    // File moved.
    assert!(!root.join("src/Foo.elm").exists());
    assert!(root.join("lib/Foo.elm").exists());

    // Imports unchanged (same module name).
    let main_content = fs::read_to_string(root.join("src/Main.elm")).unwrap();
    assert!(main_content.contains("import Foo"));
    assert!(main_content.contains("Foo.foo"));

    // Output shows just the rename, no updates.
    assert!(stdout.contains("renamed"));
    assert!(!stdout.contains("updated"));
}

#[test]
fn mv_cleans_up_empty_directories() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Deep/Nested/Module.elm",
        "module Deep.Nested.Module exposing (..)\n\nfoo = 1\n",
    );

    let output = elmq()
        .current_dir(root)
        .args(["mv", "src/Deep/Nested/Module.elm", "src/Flat.elm"])
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");

    // Empty dirs should be cleaned up.
    assert!(!root.join("src/Deep/Nested").exists());
    assert!(!root.join("src/Deep").exists());
}
