// Copyright 2026 Google LLC
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn get_temp_dir() -> PathBuf {
  let nanos = SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .unwrap()
    .as_nanos();
  let path = std::env::temp_dir().join(format!("prek_test_{}", nanos));
  fs::create_dir(&path).unwrap();
  path
}

fn verify_golden(
  test_path: &std::path::Path,
  file_name: &str,
  golden_name: &str,
) {
  let actual = fs::read_to_string(test_path.join(file_name)).unwrap();
  let data_dir = std::env::current_dir().unwrap().join("tests/data");
  let expected = fs::read_to_string(data_dir.join(golden_name)).unwrap();
  assert_eq!(
    actual, expected,
    "File {} does not match {}",
    file_name, golden_name
  );
}

#[test]
fn test_integration() {
  let test_path = get_temp_dir();
  println!("Using temp dir: {}", test_path.display());

  // 1. Install prek via cargo
  println!("Installing prek...");
  let status = Command::new("cargo")
    .args([
      "binstall",
      "--no-confirm",
      "prek",
      "--root",
      test_path.to_str().unwrap(),
    ])
    .status()
    .expect("Failed to run cargo binstall");
  assert!(status.success(), "cargo binstall prek failed");

  // 2. Install pre-commit via venv
  println!("Installing pre-commit...");
  let status = Command::new("python3")
    .args(["-m", "venv", test_path.join("venv").to_str().unwrap()])
    .status()
    .expect("Failed to create venv");
  assert!(status.success(), "Failed to create venv");

  let pip_path = test_path.join("venv/bin/pip");
  let status = Command::new(pip_path)
    .args(["install", "pre-commit"])
    .status()
    .expect("Failed to run pip install");
  assert!(status.success(), "Failed to install pre-commit");

  // 3. Setup dummy git repo
  println!("Setting up dummy git repo...");
  let status = Command::new("git")
    .arg("init")
    .current_dir(&test_path)
    .status()
    .expect("Failed to run git init");
  assert!(status.success());

  // Configure git user for commits in test
  Command::new("git")
    .args(["config", "user.name", "Test User"])
    .current_dir(&test_path)
    .status()
    .unwrap();
  Command::new("git")
    .args(["config", "user.email", "test@example.com"])
    .current_dir(&test_path)
    .status()
    .unwrap();

  // 4. Create .pre-commit-config.yaml
  let config_content = format!(
    r#"
repos:
-   repo: {}
    rev: HEAD
    hooks:
    -   id: check-copyright
    -   id: check-google-doc-style
    -   id: check-links
    -   id: markdown-toc
"#,
    std::env::current_dir().unwrap().to_str().unwrap()
  );
  fs::write(test_path.join(".pre-commit-config.yaml"), config_content).unwrap();

  // Copy test data files
  let data_dir = std::env::current_dir().unwrap().join("tests/data");
  let files = [
    "fail.py",
    "pass.py",
    "style_fail.md",
    "links_fail.md",
    "toc.md",
  ];
  for file in &files {
    fs::copy(data_dir.join(file), test_path.join(file)).unwrap();
  }

  // Git add files
  Command::new("git")
    .arg("add")
    .args(files)
    .current_dir(&test_path)
    .status()
    .unwrap();

  // 5. Run pre-commit
  println!("Running pre-commit...");
  let pre_commit_path = test_path.join("venv/bin/pre-commit");
  let output = Command::new(pre_commit_path)
    .args(["run", "--all-files"])
    .current_dir(&test_path)
    .output()
    .expect("Failed to run pre-commit");

  println!(
    "pre-commit stdout: {}",
    String::from_utf8_lossy(&output.stdout)
  );
  println!(
    "pre-commit stderr: {}",
    String::from_utf8_lossy(&output.stderr)
  );

  // It should fail because files have issues
  assert!(!output.status.success(), "pre-commit should have failed");

  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains("Missing copyright"));
  assert!(stdout.contains("style_fail.md"));
  assert!(stdout.contains("Link points at a non-existent anchor"));

  verify_golden(&test_path, "style_fail.md", "style_fail.golden.md");
  verify_golden(&test_path, "toc.md", "toc.golden.md");

  // Revert files to original state for prek
  Command::new("git")
    .args(["checkout", "--", "."])
    .current_dir(&test_path)
    .status()
    .unwrap();

  // 6. Run prek
  println!("Running prek...");
  let prek_path = test_path.join("bin/prek");
  let output = Command::new(prek_path)
    .args(["run", "--all-files"])
    .current_dir(&test_path)
    .output()
    .expect("Failed to run prek");

  println!("prek stdout: {}", String::from_utf8_lossy(&output.stdout));
  println!("prek stderr: {}", String::from_utf8_lossy(&output.stderr));

  // It should also fail
  assert!(!output.status.success(), "prek should have failed");

  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains("Missing copyright"));
  assert!(stdout.contains("style_fail.md"));
  assert!(stdout.contains("Link points at a non-existent anchor"));

  verify_golden(&test_path, "style_fail.md", "style_fail.golden.md");
  verify_golden(&test_path, "toc.md", "toc.golden.md");

  // Clean up
  fs::remove_dir_all(&test_path).unwrap();
}

#[test]
fn test_check_copyright_empty_path() {
  let output = std::process::Command::new("cargo")
    .args([
      "run",
      "--bin",
      "check_copyright",
      "--",
      "",
      "src/bin/check_copyright.rs",
    ])
    .output()
    .expect("Failed to run cargo");

  assert!(output.status.success());
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(!stderr.contains("Skipping :"));
}

#[test]
fn test_check_copyright_escaped_dash() {
  let output = std::process::Command::new("cargo")
    .args([
      "run",
      "--bin",
      "check_copyright",
      "--",
      "--custom-format",
      ".rs",
      "\\-",
      " * ",
      "",
      "src/bin/check_copyright.rs",
    ])
    .output()
    .expect("Failed to run cargo");

  assert!(!output.status.success());
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(stderr.contains("Missing copyright in src/bin/check_copyright.rs:"));
  assert!(stderr.contains("-"));
}
