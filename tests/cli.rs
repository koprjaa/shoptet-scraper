// Project: shoptet-scraper
// File: cli.rs
// Description: Executable argument, exit status and offline input tests.
// Author: Jan Alexandr Kopřiva
// License: MIT
mod common;
use common::Temp;
use std::{
    io::Write,
    process::{Command, Stdio},
};
fn cmd(tmp: &Temp) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_shoptet-scraper"));
    cmd.current_dir(&tmp.0);
    cmd
}
#[test]
fn help_succeeds_without_creating_output() {
    let t = Temp::default();
    let o = cmd(&t).arg("--help").output().unwrap();
    assert!(o.status.success());
    let s = String::from_utf8(o.stdout).unwrap();
    for f in ["--store", "--output", "--verbose", "--all-images", "eshop"] {
        assert!(s.contains(f));
    }
    assert_eq!(std::fs::read_dir(&t.0).unwrap().count(), 0);
}
#[test]
fn argument_errors_exit_two() {
    let t = Temp::default();
    for args in [
        vec![],
        vec!["wrong"],
        vec!["eshop", "--unknown"],
        vec!["eshop", "--store"],
    ] {
        assert_eq!(cmd(&t).args(args).output().unwrap().status.code(), Some(2));
    }
}
#[test]
fn unsafe_direct_store_returns_zero_and_logs() {
    let t = Temp::default();
    let o = cmd(&t)
        .args([
            "eshop",
            "--store",
            "http://127.0.0.1",
            "-va",
            "--output",
            "custom",
        ])
        .output()
        .unwrap();
    assert!(o.status.success());
    let log = std::fs::read_to_string(t.0.join("shoptet_scraper.log")).unwrap();
    assert!(log.contains("Refused unsafe shop URL"));
    assert!(log.contains("No Shoptet shop at URL"));
    assert_eq!(std::fs::read_dir(t.0.join("custom")).unwrap().count(), 1);
}
#[test]
fn blank_interactive_input_returns_zero() {
    let t = Temp::default();
    let mut c = cmd(&t)
        .arg("eshop")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    c.stdin.take().unwrap().write_all(b"\n").unwrap();
    let o = c.wait_with_output().unwrap();
    assert!(o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains("No URLs given"));
}
#[test]
fn eof_exits_cleanly() {
    let t = Temp::default();
    assert!(cmd(&t)
        .arg("eshop")
        .stdin(Stdio::null())
        .output()
        .unwrap()
        .status
        .success());
}
#[test]
fn fatal_output_error_exits_one() {
    let t = Temp::default();
    std::fs::write(t.0.join("file"), "x").unwrap();
    assert_eq!(
        cmd(&t)
            .args(["eshop", "--output", "file"])
            .output()
            .unwrap()
            .status
            .code(),
        Some(1)
    );
}

#[test]
fn abbreviated_and_repeated_options_remain_compatible() {
    let t = Temp::default();
    let output = cmd(&t)
        .args([
            "eshop",
            "--sto",
            "file:///unsafe",
            "-v",
            "-v",
            "--out",
            "first",
            "--output",
            "second",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(t.0.join("second").is_dir());
    assert!(!t.0.join("first").exists());
}
