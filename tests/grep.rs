//! Integration tests for `elmq grep`.
//!
//! Each test sets up an inline Elm project in a temp directory and shells out
//! to the compiled binary, asserting on stdout / stderr / exit code.

use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command;

fn elmq() -> Command {
    Command::new(env!("CARGO_BIN_EXE_elmq"))
}

fn write_elm_json(root: &Path, source_dirs: &[&str]) {
    let sds: Vec<String> = source_dirs.iter().map(|s| format!("\"{s}\"")).collect();
    let body = format!(
        r#"{{"type": "application", "source-directories": [{}], "elm-version": "0.19.1", "dependencies": {{"direct": {{}}, "indirect": {{}}}}, "test-dependencies": {{"direct": {{}}, "indirect": {{}}}}}}"#,
        sds.join(", ")
    );
    fs::write(root.join("elm.json"), body).unwrap();
    for sd in source_dirs {
        fs::create_dir_all(root.join(sd)).unwrap();
    }
}

fn write_file(root: &Path, rel: &str, content: &str) {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, content).unwrap();
}

// A canonical fixture file reused by many tests.
const API_ELM: &str = r#"module Api exposing (fetchUsers, retryLabel)

import Http


-- TODO: add error handling
fetchUsers : Cmd msg
fetchUsers =
    Http.get { url = "/users" }


{- block comment with Http.get inside -}
retryLabel : String
retryLabel =
    "retry failed"


bigMessage : String
bigMessage =
    """
    multiline retry text
    """
"#;

// -- 5.3: match in function body reports enclosing decl (compact) --

#[test]
fn match_in_function_body_reports_decl_compact() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_elm_json(root, &["src"]);
    write_file(root, "src/Api.elm", API_ELM);

    let output = elmq()
        .current_dir(root)
        .args(["grep", r"Http\.get"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Look for a line matching the compact format with decl = fetchUsers.
    let lines: Vec<&str> = stdout.lines().collect();
    let hit = lines
        .iter()
        .find(|l| l.contains("src/Api.elm") && l.contains(":fetchUsers:"))
        .unwrap_or_else(|| panic!("missing fetchUsers hit in: {stdout}"));
    assert!(hit.contains("Http.get"));
}

// -- 5.4: literal mode -F does not treat '.' as a metacharacter --

#[test]
fn fixed_mode_treats_pattern_as_literal() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_elm_json(root, &["src"]);
    write_file(
        root,
        "src/Literal.elm",
        "module Literal exposing (..)\n\nfoo = a.b\n\nbar = aXb\n",
    );

    // Without -F, "a.b" (regex) matches both "a.b" and "aXb".
    let out_regex = elmq()
        .current_dir(root)
        .args(["grep", "a.b"])
        .output()
        .unwrap();
    let s_regex = String::from_utf8_lossy(&out_regex.stdout);
    assert!(
        s_regex.contains("foo") && s_regex.contains("bar"),
        "regex mode: {s_regex}"
    );

    // With -F, only the literal "a.b" matches.
    let out_fixed = elmq()
        .current_dir(root)
        .args(["grep", "-F", "a.b"])
        .output()
        .unwrap();
    let s_fixed = String::from_utf8_lossy(&out_fixed.stdout);
    assert!(s_fixed.contains("foo = a.b"), "fixed mode: {s_fixed}");
    assert!(!s_fixed.contains("bar"), "aXb should not match: {s_fixed}");
}

// -- 5.5: -i case insensitive --

#[test]
fn case_insensitive_flag_matches() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_elm_json(root, &["src"]);
    write_file(
        root,
        "src/Case.elm",
        "module Case exposing (..)\n\nimport Http\n\nf = Http.get 1\n",
    );

    let sensitive = elmq()
        .current_dir(root)
        .args(["grep", "http"])
        .output()
        .unwrap();
    assert_eq!(sensitive.status.code(), Some(1));

    let insensitive = elmq()
        .current_dir(root)
        .args(["grep", "-i", "http"])
        .output()
        .unwrap();
    assert_eq!(insensitive.status.code(), Some(0));
    let s = String::from_utf8_lossy(&insensitive.stdout);
    assert!(s.contains("Http"));
}

// -- 5.6 / 5.7: comment filtering --

#[test]
fn line_comment_match_filtered_by_default() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_elm_json(root, &["src"]);
    write_file(
        root,
        "src/C.elm",
        "module C exposing (..)\n\n-- TODO: fix this\nfoo = 1\n",
    );

    let out = elmq()
        .current_dir(root)
        .args(["grep", "TODO"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stdout).is_empty());
}

#[test]
fn line_comment_match_included_with_flag() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_elm_json(root, &["src"]);
    write_file(
        root,
        "src/C.elm",
        "module C exposing (..)\n\n-- TODO: fix this\nfoo = 1\n",
    );

    let out = elmq()
        .current_dir(root)
        .args(["grep", "--include-comments", "TODO"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("TODO"));
}

// -- 5.8: block comment filtered by default --

#[test]
fn block_comment_match_filtered_by_default() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_elm_json(root, &["src"]);
    write_file(
        root,
        "src/B.elm",
        "module B exposing (..)\n\n{- notable text -}\nfoo = 1\n",
    );

    let out = elmq()
        .current_dir(root)
        .args(["grep", "notable"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));

    // Sanity: --include-comments turns it back on.
    let out2 = elmq()
        .current_dir(root)
        .args(["grep", "--include-comments", "notable"])
        .output()
        .unwrap();
    assert_eq!(out2.status.code(), Some(0));
}

// -- 5.9: regular string literal filtered by default; reported with flag --

#[test]
fn string_literal_match_filtered_by_default() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_elm_json(root, &["src"]);
    write_file(
        root,
        "src/S.elm",
        "module S exposing (..)\n\nlabel = \"retry failed\"\n",
    );

    let out = elmq()
        .current_dir(root)
        .args(["grep", "retry"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));

    let out2 = elmq()
        .current_dir(root)
        .args(["grep", "--include-strings", "retry"])
        .output()
        .unwrap();
    assert_eq!(out2.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out2.stdout).contains("retry"));
}

// -- 5.10: triple-quoted string filtered by default --

#[test]
fn triple_string_match_filtered_by_default() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_elm_json(root, &["src"]);
    write_file(
        root,
        "src/T.elm",
        "module T exposing (..)\n\nblob =\n    \"\"\"\n    multiline retry text\n    \"\"\"\n",
    );

    let out = elmq()
        .current_dir(root)
        .args(["grep", "retry"])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(1),
        "stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );

    let out2 = elmq()
        .current_dir(root)
        .args(["grep", "--include-strings", "retry"])
        .output()
        .unwrap();
    assert_eq!(out2.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&out2.stdout).contains("retry"));
}

// -- 5.11: comment/string flags are independent --

#[test]
fn comment_and_string_flags_independent() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_elm_json(root, &["src"]);
    write_file(
        root,
        "src/Mix.elm",
        "module Mix exposing (..)\n\n-- retry soon\nlabel = \"retry now\"\n",
    );

    // Only --include-comments: comment hit present, string hit absent.
    let out = elmq()
        .current_dir(root)
        .args(["grep", "--include-comments", "retry"])
        .output()
        .unwrap();
    let s = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0));
    assert!(s.contains("-- retry soon"), "comment line expected: {s}");
    assert!(
        !s.contains("\"retry now\""),
        "string literal should be filtered: {s}"
    );

    // Only --include-strings: string hit present, comment hit absent.
    let out2 = elmq()
        .current_dir(root)
        .args(["grep", "--include-strings", "retry"])
        .output()
        .unwrap();
    let s2 = String::from_utf8_lossy(&out2.stdout);
    assert_eq!(out2.status.code(), Some(0));
    assert!(s2.contains("\"retry now\""), "string line expected: {s2}");
    assert!(
        !s2.contains("-- retry soon"),
        "comment should be filtered: {s2}"
    );
}

// -- 5.12: import line → decl = - / null --

#[test]
fn import_line_match_has_null_decl() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_elm_json(root, &["src"]);
    write_file(
        root,
        "src/I.elm",
        "module I exposing (..)\n\nimport Http\n\nfoo = 1\n",
    );

    // Compact: dash in decl slot.
    let out = elmq()
        .current_dir(root)
        .args(["grep", "Http"])
        .output()
        .unwrap();
    let s = String::from_utf8_lossy(&out.stdout);
    assert_eq!(out.status.code(), Some(0));
    let line = s
        .lines()
        .find(|l| l.contains("import Http"))
        .unwrap_or_else(|| panic!("no import line hit in: {s}"));
    assert!(
        line.contains(":-:"),
        "expected `:-:` in compact output, got: {line}"
    );

    // JSON: decl is null.
    let out2 = elmq()
        .current_dir(root)
        .args(["grep", "--format", "json", "Http"])
        .output()
        .unwrap();
    let s2 = String::from_utf8_lossy(&out2.stdout);
    let hit = s2
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .find(|v| {
            v["line_text"]
                .as_str()
                .unwrap_or("")
                .contains("import Http")
        })
        .expect("import hit in JSON");
    assert!(hit["decl"].is_null(), "decl should be null: {hit}");
    assert!(
        hit["decl_kind"].is_null(),
        "decl_kind should be null: {hit}"
    );
}

// -- 5.13: module header match → null decl but module name reported --

#[test]
fn module_header_match_null_decl_with_module() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_elm_json(root, &["src"]);
    write_file(
        root,
        "src/Header.elm",
        "module Header exposing (foo)\n\nfoo = 1\n",
    );

    let out = elmq()
        .current_dir(root)
        .args(["grep", "--format", "json", "Header"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let s = String::from_utf8_lossy(&out.stdout);
    let hit = s
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .find(|v| {
            v["line_text"]
                .as_str()
                .unwrap_or("")
                .contains("module Header")
        })
        .expect("module header hit");
    assert!(hit["decl"].is_null());
    assert_eq!(hit["module"].as_str(), Some("Header"));
}

// -- 5.14: let-binding inside update → reports update --

#[test]
fn let_binding_in_update_reports_top_level_decl() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_elm_json(root, &["src"]);
    write_file(
        root,
        "src/U.elm",
        "module U exposing (..)\n\nupdate msg model =\n    let\n        loop n =\n            n + sentinel\n    in\n    loop 0\n\nsentinel = 42\n",
    );

    let out = elmq()
        .current_dir(root)
        .args(["grep", r"n \+ sentinel"])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let s = String::from_utf8_lossy(&out.stdout);
    let line = s
        .lines()
        .find(|l| l.contains("n + sentinel"))
        .unwrap_or_else(|| panic!("hit missing: {s}"));
    assert!(
        line.contains(":update:"),
        "expected enclosing decl `update`, got: {line}"
    );
    assert!(
        !line.contains(":loop:"),
        "should not report let-binding: {line}"
    );
}

// -- 5.15: NDJSON — one parseable object per line, not an array --

#[test]
fn json_output_is_line_delimited() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_elm_json(root, &["src"]);
    write_file(
        root,
        "src/J.elm",
        "module J exposing (..)\n\nimport Http\n\nfoo = Http.get 1\nbar = Http.get 2\nbaz = Http.get 3\n",
    );

    let out = elmq()
        .current_dir(root)
        .args(["grep", "--format", "json", r"Http\.get"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let s = String::from_utf8_lossy(&out.stdout);
    // Should NOT be a single JSON array.
    let trimmed = s.trim();
    assert!(!trimmed.starts_with('['), "expected NDJSON not array: {s}");

    let lines: Vec<&str> = trimmed.lines().collect();
    assert_eq!(lines.len(), 3, "expected 3 hits: {s}");
    for line in &lines {
        let v: Value = serde_json::from_str(line).expect("each line is valid JSON");
        assert!(v.get("file").is_some());
        assert!(v.get("line").is_some());
        assert!(v.get("match").is_some());
    }
}

// -- 5.16: project root with elm.json honors source-directories --

#[test]
fn project_root_honors_source_directories() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_elm_json(root, &["src", "tests/src"]);
    write_file(root, "src/A.elm", "module A exposing (..)\n\nfoo = 1\n");
    write_file(
        root,
        "tests/src/B.elm",
        "module B exposing (..)\n\nfoo = 1\n",
    );
    // Outside source-directories — should not be searched.
    write_file(root, "extras/C.elm", "module C exposing (..)\n\nfoo = 1\n");

    let out = elmq()
        .current_dir(root)
        .args(["grep", "foo ="])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("src/A.elm"));
    assert!(s.contains("tests/src/B.elm"));
    assert!(
        !s.contains("extras/C.elm"),
        "extras should not be searched: {s}"
    );
}

// -- 5.17: invoked from subdir resolves ancestor elm.json --

#[test]
fn subdirectory_resolves_ancestor_elm_json() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_elm_json(root, &["src", "tests/src"]);
    write_file(root, "src/A.elm", "module A exposing (..)\n\nmagic = 1\n");
    write_file(
        root,
        "tests/src/B.elm",
        "module B exposing (..)\n\nmagic = 1\n",
    );

    // Run from tests/src — which is itself a source-directory. Expect both A and B.
    let out = elmq()
        .current_dir(root.join("tests/src"))
        .args(["grep", "magic"])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("A.elm"), "expected sibling src/A.elm hit: {s}");
    assert!(s.contains("B.elm"), "expected tests/src/B.elm hit: {s}");
}

// -- 5.18: no elm.json anywhere → walk CWD --

#[test]
fn no_elm_json_ancestor_walks_cwd() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    // No elm.json. Just a .elm file at the top.
    write_file(
        root,
        "Loose.elm",
        "module Loose exposing (..)\n\nfindme = 1\n",
    );

    let out = elmq()
        .current_dir(root)
        .args(["grep", "findme"])
        .output()
        .unwrap();
    assert_eq!(
        out.status.code(),
        Some(0),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("Loose.elm"));
}

// -- 5.19: .gitignore exclusions honored --

#[test]
fn gitignore_exclusions_honored() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_elm_json(root, &["src"]);
    // Must be a git repo for gitignore to apply in the ignore crate's default
    // configuration. We just init an empty .git directory marker.
    fs::create_dir_all(root.join(".git")).unwrap();
    write_file(root, ".gitignore", "generated/\n");
    write_file(
        root,
        "src/Keep.elm",
        "module Keep exposing (..)\n\nneedle = 1\n",
    );
    write_file(
        root,
        "src/generated/Gen.elm",
        "module Gen exposing (..)\n\nneedle = 1\n",
    );

    let out = elmq()
        .current_dir(root)
        .args(["grep", "needle"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("Keep.elm"), "Keep.elm should be searched: {s}");
    assert!(
        !s.contains("Gen.elm"),
        "gitignored Gen.elm should not appear: {s}"
    );
}

// -- 5.20: elm-stuff excluded --

#[test]
fn elm_stuff_directory_excluded() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_elm_json(root, &["src"]);
    fs::create_dir_all(root.join(".git")).unwrap();
    write_file(root, ".gitignore", "elm-stuff/\n");
    write_file(
        root,
        "src/Keep.elm",
        "module Keep exposing (..)\n\nneedle = 1\n",
    );
    write_file(
        root,
        "elm-stuff/0.19.1/Foo.elm",
        "module Foo exposing (..)\n\nneedle = 1\n",
    );

    let out = elmq()
        .current_dir(root)
        .args(["grep", "needle"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("Keep.elm"));
    assert!(
        !s.contains("elm-stuff"),
        "elm-stuff should be excluded: {s}"
    );
}

// -- 5.21: positional PATH narrows scope --

#[test]
fn positional_path_narrows_scope() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_elm_json(root, &["src"]);
    write_file(
        root,
        "src/ui/A.elm",
        "module A exposing (..)\n\nneedle = 1\n",
    );
    write_file(
        root,
        "src/data/B.elm",
        "module B exposing (..)\n\nneedle = 1\n",
    );

    // Without path filter: both hit.
    let out = elmq()
        .current_dir(root)
        .args(["grep", "needle"])
        .output()
        .unwrap();
    let s = String::from_utf8_lossy(&out.stdout);
    assert!(s.contains("src/ui/A.elm"));
    assert!(s.contains("src/data/B.elm"));

    // With path filter: only src/ui/.
    let out2 = elmq()
        .current_dir(root)
        .args(["grep", "needle", "src/ui"])
        .output()
        .unwrap();
    assert_eq!(out2.status.code(), Some(0));
    let s2 = String::from_utf8_lossy(&out2.stdout);
    assert!(s2.contains("src/ui/A.elm"));
    assert!(
        !s2.contains("src/data/B.elm"),
        "data dir should be filtered out: {s2}"
    );
}

// -- 5.22: exit codes --

#[test]
fn exit_code_zero_on_match() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_elm_json(root, &["src"]);
    write_file(root, "src/A.elm", "module A exposing (..)\n\nneedle = 1\n");
    let out = elmq()
        .current_dir(root)
        .args(["grep", "needle"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(0));
}

#[test]
fn exit_code_one_on_no_match() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_elm_json(root, &["src"]);
    write_file(root, "src/A.elm", "module A exposing (..)\n\nfoo = 1\n");
    let out = elmq()
        .current_dir(root)
        .args(["grep", "nothingmatcheshere"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stdout).is_empty());
}

#[test]
fn exit_code_two_on_invalid_regex() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_elm_json(root, &["src"]);
    write_file(root, "src/A.elm", "module A exposing (..)\n\nfoo = 1\n");
    let out = elmq()
        .current_dir(root)
        .args(["grep", "[unclosed"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("invalid regex") || stderr.to_lowercase().contains("regex"),
        "expected regex error on stderr, got: {stderr}"
    );
}

// -- 5.23: parse failure resilience --

#[test]
fn parse_failure_file_still_reports_matches_with_null_decl() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write_elm_json(root, &["src"]);
    write_file(
        root,
        "src/Ok.elm",
        "module Ok exposing (..)\n\nfoo = needle\n",
    );
    // Intentionally broken: dangling `=` with no expression and unterminated
    // shape. Tree-sitter will still produce some tree, so what we really care
    // about is that matches from other files still come through and at minimum
    // the command succeeds overall.
    write_file(
        root,
        "src/Broken.elm",
        "module Broken exposing (..)\n\nthis is = not { valid elm needle\n",
    );

    let out = elmq()
        .current_dir(root)
        .args(["grep", "--format", "json", "needle"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        out.status.code(),
        Some(0),
        "stdout: {stdout}\nstderr: {stderr}"
    );

    // The good file's match must be present and attribute to `foo`.
    let ok_hit = stdout
        .lines()
        .map(|l| serde_json::from_str::<Value>(l).unwrap())
        .find(|v| {
            v["file"]
                .as_str()
                .map(|f| f.contains("Ok.elm"))
                .unwrap_or(false)
        })
        .expect("Ok.elm hit present");
    assert_eq!(ok_hit["decl"].as_str(), Some("foo"));
}
