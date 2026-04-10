use std::process::Command;

fn elmq() -> Command {
    Command::new(env!("CARGO_BIN_EXE_elmq"))
}

#[test]
fn guide_outputs_content() {
    let output = elmq().arg("guide").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("elmq"));
    assert!(stdout.len() > 100);
}
