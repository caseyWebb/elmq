use std::fs;
use std::path::Path;
use std::process::Command;

fn create_project(root: &Path, source_dirs: &[&str]) {
    let elm_json = format!(
        r#"{{
  "type": "application",
  "source-directories": [{}]
}}"#,
        source_dirs
            .iter()
            .map(|s| format!("\"{s}\""))
            .collect::<Vec<_>>()
            .join(", ")
    );
    fs::write(root.join("elm.json"), elm_json).unwrap();
    for sd in source_dirs {
        fs::create_dir_all(root.join(sd)).unwrap();
    }
}

fn write_elm(root: &Path, relative: &str, content: &str) {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

fn read_elm(root: &Path, relative: &str) -> String {
    fs::read_to_string(root.join(relative)).unwrap()
}

fn run_move(
    root: &Path,
    source: &str,
    names: &[&str],
    target: &str,
    copy_shared: bool,
) -> Result<elmq::move_decl::MoveResult, anyhow::Error> {
    let source_path = root.join(source).canonicalize().unwrap();
    let target_path = root.join(target);
    let name_strings: Vec<String> = names.iter().map(|s| s.to_string()).collect();
    elmq::move_decl::execute_move_declaration(
        &source_path,
        &name_strings,
        &target_path,
        copy_shared,
        false,
    )
}

fn run_move_dry(
    root: &Path,
    source: &str,
    names: &[&str],
    target: &str,
) -> Result<elmq::move_decl::MoveResult, anyhow::Error> {
    let source_path = root.join(source).canonicalize().unwrap();
    let target_path = root.join(target);
    let name_strings: Vec<String> = names.iter().map(|s| s.to_string()).collect();
    elmq::move_decl::execute_move_declaration(
        &source_path,
        &name_strings,
        &target_path,
        false,
        true,
    )
}

// -- Basic move --

#[test]
fn basic_move_function_with_annotation() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Source.elm",
        "module Source exposing (helper, main)\n\n\nmain =\n    helper 1\n\n\nhelper : Int -> Int\nhelper x =\n    x + 1\n",
    );
    write_elm(
        root,
        "src/Target.elm",
        "module Target exposing (..)\n\n\nother =\n    42\n",
    );

    let result = run_move(root, "src/Source.elm", &["helper"], "src/Target.elm", false).unwrap();

    assert!(result.moved.contains(&"helper".to_string()));

    let source = read_elm(root, "src/Source.elm");
    assert!(!source.contains("helper : Int -> Int"));
    assert!(!source.contains("helper x ="));

    let target = read_elm(root, "src/Target.elm");
    assert!(target.contains("helper : Int -> Int"));
    assert!(target.contains("helper x ="));
}

// -- Import style rewriting --

#[test]
fn move_rewrites_qualified_to_alias() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Source.elm",
        "module Source exposing (view)\n\nimport Html\n\n\nview =\n    Html.div [] []\n",
    );
    write_elm(
        root,
        "src/Target.elm",
        "module Target exposing (..)\n\nimport Html as H\n\n\nother =\n    H.span [] []\n",
    );

    run_move(root, "src/Source.elm", &["view"], "src/Target.elm", false).unwrap();

    let target = read_elm(root, "src/Target.elm");
    assert!(
        target.contains("H.div"),
        "expected H.div in target, got:\n{target}"
    );
}

#[test]
fn move_rewrites_bare_to_qualified() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Source.elm",
        "module Source exposing (view)\n\nimport Html exposing (div)\n\n\nview =\n    div [] []\n",
    );
    write_elm(
        root,
        "src/Target.elm",
        "module Target exposing (..)\n\nimport Html\n\n\nother =\n    Html.span [] []\n",
    );

    run_move(root, "src/Source.elm", &["view"], "src/Target.elm", false).unwrap();

    let target = read_elm(root, "src/Target.elm");
    assert!(
        target.contains("Html.div"),
        "expected Html.div in target, got:\n{target}"
    );
}

#[test]
fn move_rewrites_alias_to_bare() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Source.elm",
        "module Source exposing (view)\n\nimport Html as H\n\n\nview =\n    H.div [] []\n",
    );
    write_elm(
        root,
        "src/Target.elm",
        "module Target exposing (..)\n\nimport Html exposing (div)\n\n\nother =\n    div [] []\n",
    );

    run_move(root, "src/Source.elm", &["view"], "src/Target.elm", false).unwrap();

    let target = read_elm(root, "src/Target.elm");
    assert!(
        target.contains("\n    div [] []\n"),
        "expected bare div in target, got:\n{target}"
    );
}

// -- Batch move --

#[test]
fn batch_move_multiple_declarations() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Source.elm",
        "module Source exposing (funcA, funcB, stay)\n\n\nfuncA =\n    1\n\n\nfuncB =\n    2\n\n\nstay =\n    3\n",
    );
    write_elm(
        root,
        "src/Target.elm",
        "module Target exposing (..)\n\n\nother =\n    0\n",
    );

    let result = run_move(
        root,
        "src/Source.elm",
        &["funcA", "funcB"],
        "src/Target.elm",
        false,
    )
    .unwrap();
    assert!(result.moved.contains(&"funcA".to_string()));
    assert!(result.moved.contains(&"funcB".to_string()));

    let source = read_elm(root, "src/Source.elm");
    assert!(!source.contains("funcA"));
    assert!(!source.contains("funcB"));
    assert!(source.contains("stay"));

    let target = read_elm(root, "src/Target.elm");
    assert!(target.contains("funcA"));
    assert!(target.contains("funcB"));
}

// -- Auto-include helpers --

#[test]
fn auto_include_private_helper() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Source.elm",
        "module Source exposing (funcA)\n\n\nfuncA =\n    helperX 1\n\n\nhelperX x =\n    x + 1\n",
    );
    write_elm(root, "src/Target.elm", "module Target exposing (..)\n");

    let result = run_move(root, "src/Source.elm", &["funcA"], "src/Target.elm", false).unwrap();
    assert!(result.auto_included.contains(&"helperX".to_string()));

    let target = read_elm(root, "src/Target.elm");
    assert!(target.contains("helperX"));
}

// -- Shared helper error --

#[test]
fn error_on_shared_helper() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Source.elm",
        "module Source exposing (funcA, funcC)\n\n\nfuncA =\n    shared 1\n\n\nfuncC =\n    shared 2\n\n\nshared x =\n    x\n",
    );
    write_elm(root, "src/Target.elm", "module Target exposing (..)\n");

    let err = run_move(root, "src/Source.elm", &["funcA"], "src/Target.elm", false).unwrap_err();
    assert!(
        err.to_string().contains("shared"),
        "expected error about shared helper, got: {err}"
    );
    assert!(err.to_string().contains("copy-shared-helpers"));
}

#[test]
fn copy_shared_helpers_flag() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Source.elm",
        "module Source exposing (funcA, funcC)\n\n\nfuncA =\n    shared 1\n\n\nfuncC =\n    shared 2\n\n\nshared x =\n    x\n",
    );
    write_elm(root, "src/Target.elm", "module Target exposing (..)\n");

    let result = run_move(root, "src/Source.elm", &["funcA"], "src/Target.elm", true).unwrap();
    assert!(result.copied.contains(&"shared".to_string()));

    // shared should be in both files
    let source = read_elm(root, "src/Source.elm");
    let target = read_elm(root, "src/Target.elm");
    assert!(source.contains("shared x ="));
    assert!(target.contains("shared x ="));
}

// -- Constructor error --

#[test]
fn error_on_constructor_move() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Source.elm",
        "module Source exposing (Msg(..))\n\ntype Msg\n    = Increment\n    | Decrement\n",
    );
    write_elm(root, "src/Target.elm", "module Target exposing (..)\n");

    let err = run_move(
        root,
        "src/Source.elm",
        &["Increment"],
        "src/Target.elm",
        false,
    )
    .unwrap_err();
    assert!(err.to_string().contains("constructor of Msg"), "got: {err}");
    assert!(err.to_string().contains("move Msg instead"));
}

// -- Target file creation --

#[test]
fn create_target_file() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Source.elm",
        "module Source exposing (helper)\n\n\nhelper =\n    42\n",
    );

    run_move(
        root,
        "src/Source.elm",
        &["helper"],
        "src/Utils/Helpers.elm",
        false,
    )
    .unwrap();

    let target = read_elm(root, "src/Utils/Helpers.elm");
    assert!(target.contains("module Utils.Helpers exposing"));
    assert!(target.contains("helper"));
}

// -- Port move --

#[test]
fn port_move_upgrades_target() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Source.elm",
        "port module Source exposing (sendMessage)\n\nport sendMessage : String -> Cmd msg\n",
    );
    write_elm(
        root,
        "src/Target.elm",
        "module Target exposing (..)\n\n\nother =\n    42\n",
    );

    run_move(
        root,
        "src/Source.elm",
        &["sendMessage"],
        "src/Target.elm",
        false,
    )
    .unwrap();

    let target = read_elm(root, "src/Target.elm");
    assert!(
        target.starts_with("port module Target"),
        "expected port module, got:\n{target}"
    );
    assert!(target.contains("port sendMessage : String -> Cmd msg"));
}

#[test]
fn port_move_creates_port_module() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Source.elm",
        "port module Source exposing (sendMessage)\n\nport sendMessage : String -> Cmd msg\n",
    );

    run_move(
        root,
        "src/Source.elm",
        &["sendMessage"],
        "src/Ports.elm",
        false,
    )
    .unwrap();

    let target = read_elm(root, "src/Ports.elm");
    assert!(
        target.starts_with("port module Ports"),
        "expected port module, got:\n{target}"
    );
}

// -- Project-wide reference rewriting --

#[test]
fn rewrites_qualified_refs_in_other_files() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Source.elm",
        "module Source exposing (helper)\n\n\nhelper =\n    42\n",
    );
    write_elm(root, "src/Target.elm", "module Target exposing (..)\n");
    write_elm(
        root,
        "src/Consumer.elm",
        "module Consumer exposing (..)\n\nimport Source\n\n\nfoo =\n    Source.helper\n",
    );

    run_move(root, "src/Source.elm", &["helper"], "src/Target.elm", false).unwrap();

    let consumer = read_elm(root, "src/Consumer.elm");
    assert!(
        consumer.contains("Target.helper"),
        "expected Target.helper in consumer, got:\n{consumer}"
    );
}

#[test]
fn rewrites_exposed_refs_in_other_files() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Source.elm",
        "module Source exposing (helper)\n\n\nhelper =\n    42\n",
    );
    write_elm(root, "src/Target.elm", "module Target exposing (..)\n");
    write_elm(
        root,
        "src/Consumer.elm",
        "module Consumer exposing (..)\n\nimport Source exposing (helper)\n\n\nfoo =\n    helper\n",
    );

    run_move(root, "src/Source.elm", &["helper"], "src/Target.elm", false).unwrap();

    let consumer = read_elm(root, "src/Consumer.elm");
    assert!(
        consumer.contains("import Target exposing (helper)"),
        "expected Target import in consumer, got:\n{consumer}"
    );
}

// -- Dry run --

#[test]
fn dry_run_does_not_write() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    write_elm(
        root,
        "src/Source.elm",
        "module Source exposing (helper)\n\n\nhelper =\n    42\n",
    );
    write_elm(root, "src/Target.elm", "module Target exposing (..)\n");

    let original_source = read_elm(root, "src/Source.elm");
    let original_target = read_elm(root, "src/Target.elm");

    let result = run_move_dry(root, "src/Source.elm", &["helper"], "src/Target.elm").unwrap();
    assert!(result.dry_run);
    assert!(result.moved.contains(&"helper".to_string()));

    // Files should be unchanged.
    assert_eq!(read_elm(root, "src/Source.elm"), original_source);
    assert_eq!(read_elm(root, "src/Target.elm"), original_target);
}

// -- CLI positional-name tests --

fn elmq_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_elmq"))
}

fn setup_cli_project(root: &Path) {
    create_project(root, &["src"]);
    write_elm(
        root,
        "src/Source.elm",
        "module Source exposing (funcA, funcB)\n\n\nfuncA : Int -> Int\nfuncA x =\n    x + 1\n\n\nfuncB : Int -> Int\nfuncB x =\n    x + 2\n",
    );
    write_elm(root, "src/Target.elm", "module Target exposing (..)\n");
}

#[test]
fn cli_move_decl_positional_names_success() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup_cli_project(root);

    let output = elmq_bin()
        .current_dir(root)
        .args([
            "move-decl",
            "src/Source.elm",
            "--to",
            "src/Target.elm",
            "funcA",
            "funcB",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "expected success, got status {:?}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let target = read_elm(root, "src/Target.elm");
    assert!(target.contains("funcA"), "target missing funcA:\n{target}");
    assert!(target.contains("funcB"), "target missing funcB:\n{target}");

    let source = read_elm(root, "src/Source.elm");
    assert!(
        !source.contains("funcA x ="),
        "source still contains funcA body:\n{source}"
    );
    assert!(
        !source.contains("funcB x ="),
        "source still contains funcB body:\n{source}"
    );
}

#[test]
fn cli_move_decl_positional_names_before_flag() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup_cli_project(root);

    let output = elmq_bin()
        .current_dir(root)
        .args([
            "move-decl",
            "src/Source.elm",
            "funcA",
            "funcB",
            "--to",
            "src/Target.elm",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "expected success with names before --to, got status {:?}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let target = read_elm(root, "src/Target.elm");
    assert!(target.contains("funcA"), "target missing funcA:\n{target}");
    assert!(target.contains("funcB"), "target missing funcB:\n{target}");

    let source = read_elm(root, "src/Source.elm");
    assert!(!source.contains("funcA x ="));
    assert!(!source.contains("funcB x ="));
}

#[test]
fn cli_move_decl_rejects_old_name_flag() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup_cli_project(root);

    let original_source = read_elm(root, "src/Source.elm");

    let output = elmq_bin()
        .current_dir(root)
        .args([
            "move-decl",
            "src/Source.elm",
            "--name",
            "funcA",
            "--to",
            "src/Target.elm",
        ])
        .output()
        .unwrap();

    // clap's default exit code for argparse/usage errors is 2.
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected usage-error exit code 1 (spec reserves 1 for usage errors; clap's default 2 is overridden in main.rs)\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unexpected argument") || stderr.contains("--name"),
        "expected clap usage error mentioning --name, got stderr:\n{stderr}"
    );

    // Source file must be untouched.
    assert_eq!(read_elm(root, "src/Source.elm"), original_source);
}

#[test]
fn cli_move_decl_single_name_positional() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    setup_cli_project(root);

    let output = elmq_bin()
        .current_dir(root)
        .args([
            "move-decl",
            "src/Source.elm",
            "--to",
            "src/Target.elm",
            "funcA",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "expected success, got status {:?}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let target = read_elm(root, "src/Target.elm");
    assert!(target.contains("funcA"), "target missing funcA:\n{target}");

    let source = read_elm(root, "src/Source.elm");
    assert!(
        !source.contains("funcA x ="),
        "source still contains funcA body:\n{source}"
    );
    // funcB should remain in source.
    assert!(
        source.contains("funcB x ="),
        "funcB should remain in source:\n{source}"
    );
}

// -- Bug fix: moved items exposed in target only when referenced externally --

#[test]
fn unexposed_decl_is_exposed_in_target() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    // helper is NOT in the exposing list of Source
    write_elm(
        root,
        "src/Source.elm",
        "module Source exposing (main)\n\n\nmain =\n    helper 1\n\n\nhelper x =\n    x + 1\n",
    );
    write_elm(
        root,
        "src/Target.elm",
        "module Target exposing (other)\n\n\nother =\n    42\n",
    );

    run_move(root, "src/Source.elm", &["helper"], "src/Target.elm", false).unwrap();

    let target = read_elm(root, "src/Target.elm");
    // helper must be exposed in Target so other modules can import it
    assert!(
        target.contains("helper") && {
            // Check the module declaration exposes helper
            let first_line = target.lines().next().unwrap_or("");
            first_line.contains("helper")
        },
        "expected helper to be exposed in target module declaration, got:\n{target}"
    );
}

#[test]
fn unexposed_decl_exposed_in_new_target() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    // helper is NOT exposed
    write_elm(
        root,
        "src/Source.elm",
        "module Source exposing (main)\n\n\nmain =\n    helper 1\n\n\nhelper x =\n    x + 1\n",
    );

    run_move(
        root,
        "src/Source.elm",
        &["helper"],
        "src/NewTarget.elm",
        false,
    )
    .unwrap();

    let target = read_elm(root, "src/NewTarget.elm");
    let first_line = target.lines().next().unwrap_or("");
    assert!(
        first_line.contains("helper"),
        "expected helper in new target exposing list, got:\n{target}"
    );
}

// -- Bug fix: source imports target when it still references moved decls --

#[test]
fn source_imports_target_for_moved_references() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    // main references helper, but we move helper out. main stays, so source needs
    // an import for Target to resolve the bare `helper` reference.
    write_elm(
        root,
        "src/Source.elm",
        "module Source exposing (main)\n\n\nmain =\n    helper 1\n\n\nhelper x =\n    x + 1\n",
    );
    write_elm(
        root,
        "src/Target.elm",
        "module Target exposing (..)\n\n\nother =\n    42\n",
    );

    run_move(root, "src/Source.elm", &["helper"], "src/Target.elm", false).unwrap();

    let source = read_elm(root, "src/Source.elm");
    assert!(
        source.contains("import Target"),
        "expected source to import Target after moving helper out, got:\n{source}"
    );
    assert!(
        source.contains("helper"),
        "expected source to still reference helper, got:\n{source}"
    );
}

#[test]
fn source_no_import_when_no_remaining_refs() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    // main does NOT reference helper, so after moving helper there should be
    // no import for Target added to Source.
    write_elm(
        root,
        "src/Source.elm",
        "module Source exposing (main, helper)\n\n\nmain =\n    42\n\n\nhelper x =\n    x + 1\n",
    );
    write_elm(
        root,
        "src/Target.elm",
        "module Target exposing (..)\n\n\nother =\n    0\n",
    );

    run_move(root, "src/Source.elm", &["helper"], "src/Target.elm", false).unwrap();

    let source = read_elm(root, "src/Source.elm");
    assert!(
        !source.contains("import Target"),
        "expected no Target import when source doesn't reference moved decl, got:\n{source}"
    );
}

#[test]
fn unreferenced_decl_not_exposed_in_new_target() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    create_project(root, &["src"]);

    // helper is NOT referenced by main (or any other file), so it should
    // stay behind exposing (..) in the new target — not explicitly exposed.
    write_elm(
        root,
        "src/Source.elm",
        "module Source exposing (main, helper)\n\n\nmain =\n    42\n\n\nhelper x =\n    x + 1\n",
    );

    run_move(
        root,
        "src/Source.elm",
        &["helper"],
        "src/NewTarget.elm",
        false,
    )
    .unwrap();

    let target = read_elm(root, "src/NewTarget.elm");
    let first_line = target.lines().next().unwrap_or("");
    assert!(
        first_line.contains("(..)"),
        "expected exposing (..) for unreferenced decl, got:\n{target}"
    );
}
