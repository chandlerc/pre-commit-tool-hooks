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
use regex::Regex;
use std::fs;

const IGNORE_START: &str = "<!-- google-doc-style-ignore -->";
const IGNORE_STOP: &str = "<!-- google-doc-style-resume -->";

const REPLACERS: &[(&str, &str)] = &[
  ("blacklist", "blocklist"),
  ("cons", "disadvantages"),
  ("e.g.", "for example"),
  ("file name", "filename"),
  ("filesystem", "file system"),
  ("for instance", "for example"),
  ("i.e.", "that is"),
  ("pros", "advantages"),
  ("repo", "repository"),
  ("repos", "repositories"),
  ("tl;dr", "to summarize"),
  ("via", "by way of"),
  ("vice versa", "the other way around"),
  ("vs.", "versus"),
  ("whitelist", "allowlist"),
];

#[derive(Parser, Debug)]
#[command(
  author,
  version,
  about = "Checks doc style against https://developers.google.com/style.",
  long_about = None
)]
struct Args {
  #[arg(required = true, help = "One or more paths of files to check.")]
  paths: Vec<String>,
}

struct Replacer {
  re: Regex,
  suggestion: String,
}

fn build_replacers() -> Vec<Replacer> {
  let mut replacers = Vec::new();

  // Case sensitive first
  for (before, after) in REPLACERS {
    let re_str = format!(r"(^|\W)({})(\W|$)", regex::escape(before));
    let re = Regex::new(&re_str).unwrap();
    replacers.push(Replacer {
      re,
      suggestion: format!("${{1}}{}${{3}}", after),
    });
  }

  // Case insensitive second
  for (before, after) in REPLACERS {
    let re_str = format!(r"(?i)(^|\W)({})(\W|$)", regex::escape(before));
    let re = Regex::new(&re_str).unwrap();
    let capitalized_after =
      after.chars().next().unwrap().to_uppercase().to_string() + &after[1..];
    replacers.push(Replacer {
      re,
      suggestion: format!("${{1}}{}${{3}}", capitalized_after),
    });
  }

  replacers
}

fn check_style(
  replacers: &[Replacer],
  path: &str,
) -> Result<Option<String>, String> {
  let contents = fs::read_to_string(path)
    .map_err(|e| format!("Failed to read file {}: {}", path, e))?;

  let mut lines: Vec<String> =
    contents.split('\n').map(|s| s.to_string()).collect();
  let mut ignoring = false;
  let mut modified = false;

  for (index, line) in lines.iter_mut().enumerate() {
    if *line == IGNORE_START {
      if ignoring {
        return Err(format!(
          "Found a repeated '{}' without a '{}' on line {}",
          IGNORE_START,
          IGNORE_STOP,
          index + 1
        ));
      }
      ignoring = true;
    } else if *line == IGNORE_STOP {
      if !ignoring {
        return Err(format!(
          "Found a '{}' without a preceding '{}' on line {}",
          IGNORE_STOP,
          IGNORE_START,
          index + 1
        ));
      }
      ignoring = false;
    } else if !ignoring {
      let mut new_line = line.clone();
      for replacer in replacers {
        new_line = replacer
          .re
          .replace_all(&new_line, &replacer.suggestion)
          .to_string();
      }
      if new_line != *line {
        *line = new_line;
        modified = true;
      }
    }
  }

  if ignoring {
    return Err(format!(
      "Found a '{}' without a stopping '{}'",
      IGNORE_START, IGNORE_STOP
    ));
  }

  if modified {
    let new_contents = lines.join("\n");
    fs::write(path, new_contents)
      .map_err(|e| format!("Failed to write file {}: {}", path, e))?;
  }

  Ok(None)
}

fn main() {
  let args = Args::parse();
  let replacers = build_replacers();
  let mut exit_code = 0;

  for path in args.paths {
    if !path.ends_with(".md") {
      continue;
    }
    match check_style(&replacers, &path) {
      Ok(None) => {}
      Ok(Some(err)) => {
        eprintln!("Errors in '{}': {}", path, err);
        exit_code = 1;
      }
      Err(err) => {
        eprintln!("Errors in '{}': {}", path, err);
        exit_code = 1;
      }
    }
  }

  std::process::exit(exit_code);
}
