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

const SAMPLE_FOO: &str = r#"module Foo exposing (..)

view : String
view =
    "hello"


helper : Int -> Int
helper x =
    x + 1
"#;

fn sample_with_module(name: &str) -> String {
    format!(
        "module {name} exposing (..)\n\nview : String\nview =\n    \"hello\"\n\n\nhelper : Int -> Int\nhelper x =\n    x + 1\n"
    )
}

#[test]
fn list_single_file_bare_output_unchanged() {
    let output = elmq()
        .args(["list", "test-fixtures/Sample.elm"])
        .output()
        .expect("failed to run elmq");
    assert!(output.status.success(), "expected success, got {output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        !stdout.contains("##"),
        "single-file output should not contain '##' header, got:\n{stdout}"
    );
    assert!(
        stdout.contains("module Sample"),
        "stdout should contain module line, got:\n{stdout}"
    );
    assert!(
        stdout.contains("functions:"),
        "stdout should contain 'functions:' header, got:\n{stdout}"
    );
}

#[test]
fn list_two_files_framed_output() {
    let f1 = with_temp_elm(&sample_with_module("Foo"));
    let f2 = with_temp_elm(&sample_with_module("Bar"));
    let p1 = f1.path().to_str().unwrap();
    let p2 = f2.path().to_str().unwrap();

    let output = elmq()
        .args(["list", p1, p2])
        .output()
        .expect("failed to run elmq");
    assert!(output.status.success(), "expected success, got {output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();

    let header1 = format!("## {p1}");
    let header2 = format!("## {p2}");
    let pos1 = stdout
        .find(&header1)
        .unwrap_or_else(|| panic!("missing header for first file in:\n{stdout}"));
    let pos2 = stdout
        .find(&header2)
        .unwrap_or_else(|| panic!("missing header for second file in:\n{stdout}"));
    assert!(pos1 < pos2, "header order wrong:\n{stdout}");

    let foo_pos = stdout.find("module Foo").expect("missing module Foo");
    let bar_pos = stdout.find("module Bar").expect("missing module Bar");
    assert!(
        pos1 < foo_pos && foo_pos < pos2,
        "module Foo should appear after first header and before second header"
    );
    assert!(
        pos2 < bar_pos,
        "module Bar should appear after second header"
    );
}

#[test]
fn list_three_files_input_order_preserved() {
    let fa = with_temp_elm(&sample_with_module("AAA"));
    let fb = with_temp_elm(&sample_with_module("BBB"));
    let fc = with_temp_elm(&sample_with_module("CCC"));
    let pa = fa.path().to_str().unwrap();
    let pb = fb.path().to_str().unwrap();
    let pc = fc.path().to_str().unwrap();

    // Pass in non-alphabetical order: B, C, A
    let output = elmq()
        .args(["list", pb, pc, pa])
        .output()
        .expect("failed to run elmq");
    assert!(output.status.success(), "expected success, got {output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();

    let h_b = stdout.find(&format!("## {pb}")).expect("missing B header");
    let h_c = stdout.find(&format!("## {pc}")).expect("missing C header");
    let h_a = stdout.find(&format!("## {pa}")).expect("missing A header");
    assert!(
        h_b < h_c && h_c < h_a,
        "headers should appear in input order B, C, A; got positions B={h_b} C={h_c} A={h_a}\n{stdout}"
    );
}

#[test]
fn list_missing_file_in_middle_is_per_arg_error() {
    let fa = with_temp_elm(&sample_with_module("Aaa"));
    let fc = with_temp_elm(&sample_with_module("Ccc"));
    let pa = fa.path().to_str().unwrap();
    let pc = fc.path().to_str().unwrap();
    let missing = "/tmp/elmq-test-does-not-exist-xyz.elm";

    let output = elmq()
        .args(["list", pa, missing, pc])
        .output()
        .expect("failed to run elmq");
    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit 2, got {:?}\nstdout: {}\nstderr: {}",
        output.status.code(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let h_a = stdout.find(&format!("## {pa}")).expect("missing A header");
    let h_m = stdout
        .find(&format!("## {missing}"))
        .expect("missing middle header");
    let h_c = stdout.find(&format!("## {pc}")).expect("missing C header");
    assert!(
        h_a < h_m && h_m < h_c,
        "headers should appear in input order; positions A={h_a} M={h_m} C={h_c}\n{stdout}"
    );

    // Slice middle block: from after the missing-header line to the next header.
    let middle_start = h_m;
    let middle_end = h_c;
    let middle_block = &stdout[middle_start..middle_end];
    assert!(
        middle_block.to_lowercase().contains("error"),
        "middle block should contain 'error', got:\n{middle_block}"
    );

    // First and third blocks should still contain real summary content.
    let first_block = &stdout[h_a..h_m];
    let third_block = &stdout[h_c..];
    assert!(
        first_block.contains("module Aaa"),
        "first block should contain its module line, got:\n{first_block}"
    );
    assert!(
        third_block.contains("module Ccc"),
        "third block should contain its module line, got:\n{third_block}"
    );
}

#[test]
fn list_all_files_missing_exit_2() {
    let p1 = "/does/not/exist/A.elm";
    let p2 = "/does/not/exist/B.elm";
    let output = elmq()
        .args(["list", p1, p2])
        .output()
        .expect("failed to run elmq");
    assert_eq!(
        output.status.code(),
        Some(2),
        "expected exit 2, got {:?}",
        output.status.code()
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let h1 = stdout
        .find(&format!("## {p1}"))
        .expect("missing first header");
    let h2 = stdout
        .find(&format!("## {p2}"))
        .expect("missing second header");
    assert!(h1 < h2);

    let block1 = &stdout[h1..h2];
    let block2 = &stdout[h2..];
    assert!(
        block1.to_lowercase().contains("error"),
        "first block missing 'error':\n{block1}"
    );
    assert!(
        block2.to_lowercase().contains("error"),
        "second block missing 'error':\n{block2}"
    );
}

#[test]
fn list_docs_flag_still_works_with_multi_file() {
    let with_doc = r#"module Doc exposing (..)


{-| This is a documented helper -}
helper : Int -> Int
helper x =
    x + 1
"#;
    let f1 = with_temp_elm(with_doc);
    let f2 = with_temp_elm(SAMPLE_FOO);
    let p1 = f1.path().to_str().unwrap();
    let p2 = f2.path().to_str().unwrap();

    let output = elmq()
        .args(["list", p1, p2, "--docs"])
        .output()
        .expect("failed to run elmq");
    assert!(output.status.success(), "expected success, got {output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();

    let h1 = stdout
        .find(&format!("## {p1}"))
        .expect("missing first header");
    let h2 = stdout
        .find(&format!("## {p2}"))
        .expect("missing second header");
    let first_block = &stdout[h1..h2];
    assert!(
        first_block.contains("This is a documented helper"),
        "first block should contain doc comment text, got:\n{first_block}"
    );
}
