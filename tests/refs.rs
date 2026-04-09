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
        "module Lib.Utils exposing (helper, Model)\n\nhelper = 1\n\ntype alias Model = { name : String }\n",
    );
    write_elm(
        root,
        "src/Page/Home.elm",
        "module Page.Home exposing (..)\n\nimport Lib.Utils exposing (helper)\n\nview = helper\n",
    );
    write_elm(
        root,
        "src/Page/Settings.elm",
        "module Page.Settings exposing (..)\n\nimport Lib.Utils as LU\n\nview = LU.helper\n",
    );
    write_elm(
        root,
        "src/Main.elm",
        "module Main exposing (..)\n\nimport Lib.Utils\n\nmain = Lib.Utils.helper\n\ntype alias AppModel = Lib.Utils.Model\n",
    );
    write_elm(
        root,
        "src/Unused.elm",
        "module Unused exposing (..)\n\nunused = 1\n",
    );
}

// -- Module-level refs --

#[test]
fn refs_module_level() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup_project(root);

    let output = elmq()
        .current_dir(root)
        .args(["refs", "src/Lib/Utils.elm"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");

    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert_eq!(lines.len(), 3);
    assert!(lines.iter().any(|l| l.starts_with("src/Main.elm:")));
    assert!(lines.iter().any(|l| l.starts_with("src/Page/Home.elm:")));
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("src/Page/Settings.elm:"))
    );
    // No text in module-level mode.
    assert!(lines.iter().all(|l| l.matches(':').count() == 1));
}

#[test]
fn refs_module_level_no_refs() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup_project(root);

    let output = elmq()
        .current_dir(root)
        .args(["refs", "src/Unused.elm"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());
    assert!(stdout.trim().is_empty());
}

// -- Declaration-level refs --

#[test]
fn refs_qualified_declaration() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup_project(root);

    let output = elmq()
        .current_dir(root)
        .args(["refs", "src/Lib/Utils.elm", "helper"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");

    let lines: Vec<&str> = stdout.trim().lines().collect();
    // Page/Home: import line (exposed) + bare usage
    // Page/Settings: LU.helper (alias qualified)
    // Main: Lib.Utils.helper (fully qualified)
    assert_eq!(lines.len(), 4, "got: {stdout}");
    assert!(
        lines
            .iter()
            .any(|l| l.contains("src/Main.elm") && l.contains("Lib.Utils.helper"))
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("src/Page/Settings.elm") && l.contains("LU.helper"))
    );
    assert!(lines.iter().any(
        |l| l.contains("src/Page/Home.elm") && l.contains("import Lib.Utils exposing (helper)")
    ));
    assert!(
        lines
            .iter()
            .any(|l| l.contains("src/Page/Home.elm") && l.contains("view = helper"))
    );
}

#[test]
fn refs_type_declaration() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup_project(root);

    let output = elmq()
        .current_dir(root)
        .args(["refs", "src/Lib/Utils.elm", "Model"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");

    let lines: Vec<&str> = stdout.trim().lines().collect();
    assert!(
        lines
            .iter()
            .any(|l| l.contains("src/Main.elm") && l.contains("Lib.Utils.Model")),
        "got: {stdout}"
    );
}

#[test]
fn refs_declaration_nonexistent_is_per_name_error() {
    // Under batch-positional-args: asking refs about a declaration that
    // does not exist in the target file is a per-name error (exit 2,
    // error on stdout). Previously it returned success with empty output.
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup_project(root);

    let output = elmq()
        .current_dir(root)
        .args(["refs", "src/Lib/Utils.elm", "nonexistent"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(2));
    assert!(stdout.contains("error:"));
    assert!(stdout.contains("nonexistent"));
}

// -- Output formats --

#[test]
fn refs_json_output() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup_project(root);

    let output = elmq()
        .current_dir(root)
        .args(["refs", "src/Lib/Utils.elm", "--format", "json"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());

    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let arr = parsed.as_array().expect("array");
    assert_eq!(arr.len(), 3);
    for item in arr {
        assert!(item.get("file").is_some());
        assert!(item.get("line").is_some());
    }
}

#[test]
fn refs_declaration_json_output() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup_project(root);

    let output = elmq()
        .current_dir(root)
        .args(["refs", "src/Lib/Utils.elm", "helper", "--format", "json"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success());

    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
    let arr = parsed.as_array().expect("array");
    assert_eq!(arr.len(), 4);
    for item in arr {
        assert!(item.get("file").is_some());
        assert!(item.get("line").is_some());
        assert!(item.get("text").is_some());
    }
}

// -- Error cases --

#[test]
fn refs_file_not_found() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    let output = elmq()
        .current_dir(root)
        .args(["refs", "src/Nonexistent.elm"])
        .output()
        .unwrap();

    assert!(!output.status.success());
}

// -- Multi-name refs --

fn setup_multi_helper_project(root: &Path) {
    create_project(root, &["src"]);
    write_elm(
        root,
        "src/Lib/Utils.elm",
        "module Lib.Utils exposing (helperA, helperB, helperC)\n\n\
         helperA = 1\n\n\
         helperB = 2\n\n\
         helperC = 3\n",
    );
    write_elm(
        root,
        "src/Main.elm",
        "module Main exposing (..)\n\n\
         import Lib.Utils\n\n\
         a = Lib.Utils.helperA\n\n\
         b = Lib.Utils.helperB\n\n\
         c = Lib.Utils.helperC\n",
    );
}

#[test]
fn refs_multi_name_success() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup_multi_helper_project(root);

    let output = elmq()
        .current_dir(root)
        .args(["refs", "src/Lib/Utils.elm", "helperA", "helperB", "helperC"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stderr: {stderr}\nstdout: {stdout}"
    );

    let idx_a = stdout.find("## helperA").expect("## helperA present");
    let idx_b = stdout.find("## helperB").expect("## helperB present");
    let idx_c = stdout.find("## helperC").expect("## helperC present");
    assert!(idx_a < idx_b, "helperA before helperB");
    assert!(idx_b < idx_c, "helperB before helperC");

    // Each block body should reference src/Main.elm. Split on the next `##`
    // header to scope our search to the block.
    for name in &["helperA", "helperB", "helperC"] {
        let header = format!("## {name}");
        let start = stdout.find(&header).unwrap() + header.len();
        let rest = &stdout[start..];
        let end = rest.find("\n## ").unwrap_or(rest.len());
        let body = &rest[..end];
        assert!(
            body.contains("src/Main.elm:"),
            "block for {name} missing src/Main.elm:\n{body}"
        );
    }
}

#[test]
fn refs_multi_name_input_order_preserved() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup_multi_helper_project(root);

    let output = elmq()
        .current_dir(root)
        .args(["refs", "src/Lib/Utils.elm", "helperC", "helperA", "helperB"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");

    let idx_c = stdout.find("## helperC").expect("## helperC present");
    let idx_a = stdout.find("## helperA").expect("## helperA present");
    let idx_b = stdout.find("## helperB").expect("## helperB present");
    assert!(idx_c < idx_a, "helperC before helperA");
    assert!(idx_a < idx_b, "helperA before helperB");
}

#[test]
fn refs_multi_name_partial_not_found() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup_multi_helper_project(root);

    let output = elmq()
        .current_dir(root)
        .args([
            "refs",
            "src/Lib/Utils.elm",
            "helperA",
            "nonExistent",
            "helperB",
        ])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(2), "stdout: {stdout}");

    let idx_a = stdout.find("## helperA").expect("## helperA present");
    let idx_n = stdout
        .find("## nonExistent")
        .expect("## nonExistent present");
    let idx_b = stdout.find("## helperB").expect("## helperB present");
    assert!(idx_a < idx_n && idx_n < idx_b, "input order preserved");

    let block = |name: &str| -> String {
        let header = format!("## {name}");
        let start = stdout.find(&header).unwrap() + header.len();
        let rest = &stdout[start..];
        let end = rest.find("\n## ").unwrap_or(rest.len());
        rest[..end].to_string()
    };

    let non_block = block("nonExistent");
    assert!(
        non_block.contains("error:"),
        "nonExistent block missing 'error:': {non_block}"
    );
    assert!(
        non_block.contains("not found"),
        "nonExistent block missing 'not found': {non_block}"
    );

    let a_block = block("helperA");
    assert!(
        a_block.contains("src/Main.elm:"),
        "helperA block missing src/Main.elm: {a_block}"
    );
    let b_block = block("helperB");
    assert!(
        b_block.contains("src/Main.elm:"),
        "helperB block missing src/Main.elm: {b_block}"
    );
}

#[test]
fn refs_zero_names_module_level_unchanged() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup_multi_helper_project(root);

    let output = elmq()
        .current_dir(root)
        .args(["refs", "src/Lib/Utils.elm"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    assert!(
        !stdout.contains("##"),
        "zero-name output should be bare, got: {stdout}"
    );
    assert!(
        stdout.contains("src/Main.elm:"),
        "expected importer in output, got: {stdout}"
    );
}

#[test]
fn refs_single_name_output_unchanged() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup_multi_helper_project(root);

    let output = elmq()
        .current_dir(root)
        .args(["refs", "src/Lib/Utils.elm", "helperA"])
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "stderr: {stderr}");
    assert!(
        !stdout.contains("##"),
        "single-name output should be bare, got: {stdout}"
    );
}
