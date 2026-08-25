//! End-to-end compiler tests: compile an .smr source, run the binary, check output.
//!
//! Each test writes a temp file, invokes smrc, runs the produced binary and
//! compares the process exit code (the language's primary output channel).

use std::process::Command;

fn smrc() -> &'static str {
    env!("CARGO_BIN_EXE_smrc")
}

/// Compile `source` to a binary in a unique tmp path, returning (bin_path, stderr).
fn compile(name: &str, source: &str) -> (std::path::PathBuf, String) {
    let dir = std::env::temp_dir().join(format!("smrc_test_{}_{}", name, std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let src = dir.join(format!("{}.smr", name));
    let bin = dir.join(name);
    std::fs::write(&src, source).unwrap();

    let out = Command::new(smrc())
        .arg(&src)
        .arg("-o")
        .arg(&bin)
        .output()
        .expect("failed to spawn smrc");
    let err = String::from_utf8_lossy(&out.stderr).to_string();
    (bin, err)
}

fn run_exit_code(bin: &std::path::Path) -> i32 {
    let out = Command::new(bin).output().expect("failed to run compiled binary");
    out.status.code().unwrap_or(-1)
}

fn compile_ok(name: &str, source: &str) -> std::path::PathBuf {
    let (bin, err) = compile(name, source);
    assert!(bin.exists(), "binary not produced for {}: {}", name, err);
    bin
}

fn compile_err(name: &str, source: &str) -> String {
    let (_, err) = compile(name, source);
    assert!(!err.is_empty(), "expected rejection for {}, got none", name);
    err
}

#[test]
fn returns_literal_exit_code() {
    let bin = compile_ok("ret42", "pub fn main() -> i32 {\n    return 42;\n}\n");
    assert_eq!(run_exit_code(&bin), 42);
}

#[test]
fn arithmetic_and_calls() {
    let src = r#"
fn add(a: i32, b: i32) -> i32 {
    return a + b;
}

pub fn main() -> i32 {
    return add(20, 22);
}
"#;
    let bin = compile_ok("add_call", src);
    assert_eq!(run_exit_code(&bin), 42);
}

#[test]
fn if_else_branching() {
    let src = r#"
pub fn main() -> i32 {
    let x = 10;
    if (x > 5) {
        return 1;
    }
    return 0;
}
"#;
    let bin = compile_ok("branch", src);
    assert_eq!(run_exit_code(&bin), 1);
}

#[test]
fn while_loop_sums() {
    // 0+1+..+9 = 45
    let src = r#"
pub fn main() -> i32 {
    let mut i = 0;
    let mut sum = 0;
    while (i < 10) {
        sum = sum + i;
        i = i + 1;
    }
    return sum;
}
"#;
    let bin = compile_ok("while_sum", src);
    assert_eq!(run_exit_code(&bin), 45);
}

#[test]
fn use_after_move_rejected() {
    let src = r#"
struct Box {
    value: i32,
}

fn take(b: Box) -> i32 {
    return b.value;
}

pub fn main() -> i32 {
    let bx = Box { value: 1 };
    let a = take(bx);
    let c = take(bx);
    return a + c;
}
"#;
    let err = compile_err("use_after_move", src);
    assert!(err.contains("moved"), "unexpected error: {}", err);
}

#[test]
fn double_mut_borrow_rejected() {
    let src = r#"
pub fn main() -> i32 {
    let mut x = 42;
    let r1 = &mut x;
    let r2 = &mut x;
    *r1 = 1;
    *r2 = 2;
    return x;
}
"#;
    let err = compile_err("double_mut", src);
    assert!(err.contains("borrow"), "unexpected error: {}", err);
}

#[test]
fn type_mismatch_rejected() {
    let src = r#"
pub fn main() -> i32 {
    let x: bool = 42;
    return 0;
}
"#;
    let err = compile_err("type_mismatch", src);
    assert!(err.contains("bool") || err.contains("i32"), "unexpected error: {}", err);
}

#[test]
fn undefined_variable_rejected() {
    let src = r#"
pub fn main() -> i32 {
    return nope + 1;
}
"#;
    let err = compile_err("undefined_var", src);
    assert!(err.to_lowercase().contains("undefined") || err.to_lowercase().contains("not found"),
            "unexpected error: {}", err);
}

#[test]
fn struct_field_access() {
    let src = r#"
struct Point {
    x: i32,
    y: i32,
}

pub fn main() -> i32 {
    let p = Point { x: 40, y: 2 };
    return p.x + p.y;
}
"#;
    let bin = compile_ok("field_access", src);
    assert_eq!(run_exit_code(&bin), 42);
}

#[test]
fn for_loop_lowering() {
    // 5+6+7+8+9 = 35
    let src = r#"
pub fn main() -> i32 {
    let mut sum = 0;
    for i in 5..10 {
        sum = sum + i;
    }
    return sum;
}
"#;
    let bin = compile_ok("for_range", src);
    assert_eq!(run_exit_code(&bin), 35);
}

#[test]
fn multiple_errors_reported_together() {
    // Two functions, each with its own error: both must appear in stderr.
    let src = r#"
fn bad_one() -> i32 {
    return nonexistent + 1;
}

pub fn main() -> i32 {
    return 0;
}

fn bad_two() -> i32 {
    let x: bool = 42;
    return 0;
}
"#;
    let dir = std::env::temp_dir().join(format!("smrc_multi_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let src_path = dir.join("multi.smr");
    let bin = dir.join("multi");
    std::fs::write(&src_path, src).unwrap();

    let out = Command::new(smrc())
        .arg(&src_path)
        .arg("-o")
        .arg(&bin)
        .output()
        .expect("failed to spawn smrc");
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.code() == Some(1), "expected failure");
    assert!(err.contains("nonexistent"), "missing undefined-var error: {}", err);
    assert!(err.contains("bool"), "missing type-mismatch error: {}", err);
    assert!(err.contains("2 errors"), "missing error count summary: {}", err);
}
