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

use clap::Parser;
use pre_commit_tool_hooks::get_links;
use regex::RegexBuilder;
use std::fs;

#[derive(Parser, Debug)]
#[command(
  author,
  version,
  about = "Updates the markdown table of contents.",
  long_about = None
)]
struct Args {
  #[arg(required = true, help = "One or more paths of files to check.")]
  paths: Vec<String>,
}

fn update_toc(path: &str) -> Result<Option<String>, String> {
  let contents = fs::read_to_string(path)
    .map_err(|e| format!("Failed to read file {}: {}", path, e))?;

  if !contents.contains("<!-- toc -->") {
    return Ok(None);
  }
  if !contents.contains("<!-- tocstop -->") {
    return Err("Missing tocstop".to_string());
  }

  let (headers, _) = get_links(&contents)?;

  let mut toc = vec!["<!-- toc -->\n\n## Table of contents\n".to_string()];
  for header in headers {
    if header.label.to_lowercase() == "table of contents" {
      continue;
    }
    if header.level == 1 {
      // This is the doc title; exclude it.
      continue;
    }

    let indent = " ".repeat(4 * (header.level as usize - 2));
    toc.push(format!(
      "{}-   [{}](#{})",
      indent, header.label, header.anchor
    ));
  }

  if toc.len() > 1 {
    toc.push("".to_string());
  }
  toc.push("<!-- tocstop -->".to_string());

  let toc_string = toc.join("\n");

  let re = RegexBuilder::new("<!-- toc -->.*?<!-- tocstop -->")
    .dot_matches_new_line(true)
    .build()
    .unwrap();

  let new_contents = re.replace(&contents, &toc_string).to_string();

  if new_contents != contents {
    fs::write(path, new_contents)
      .map_err(|e| format!("Failed to write file {}: {}", path, e))?;
  }

  Ok(None)
}

fn main() {
  let args = Args::parse();
  let mut exit_code = 0;

  for path in args.paths {
    if !path.ends_with(".md") {
      continue;
    }
    match update_toc(&path) {
      Ok(None) => {}
      Ok(Some(err)) => {
        eprintln!("Error in {}: {}", path, err);
        exit_code = 1;
      }
      Err(err) => {
        eprintln!("Error in {}: {}", path, err);
        exit_code = 1;
      }
    }
  }

  std::process::exit(exit_code);
}
