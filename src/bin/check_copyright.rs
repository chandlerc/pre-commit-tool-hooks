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

use chrono::Datelike;
use clap::Parser;
use regex::Regex;
use std::fs;
use std::io;
use std::path::Path;

const DEFAULT_COPYRIGHT: &str = "Copyright YYYY Google LLC

Licensed under the Apache License, Version 2.0 (the \"License\");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    https://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an \"AS IS\" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.";

const DEFAULT_SKIP_PATTERN: &str = "(?:^LICENSE|Cargo\\.lock|\\.(?:ico|json))$";

const BUILTIN_FORMATS: &[(&str, &str, &str, &str)] = &[
  ("\\.(cpp|h|rs)$", "", "// ", ""),
  ("\\.(html|md)$", "<!--", "", "-->"),
  ("\\.js$", "/*", " * ", " */"),
  ("\\.py$", "__copyright__ = \"\"\"", "", "\"\"\""),
  ("", "", "# ", ""),
];

#[derive(Parser, Debug)]
#[command(
  author,
  version,
  about = "Checks that files contain copyrights.",
  long_about = None
)]
struct Args {
  #[arg(
    long,
    default_value = DEFAULT_COPYRIGHT,
    help = "The copyright to check for. Use `YYYY` to insert the current year."
  )]
  copyright: String,

  #[arg(
    long,
    num_args = 4,
    action = clap::ArgAction::Append,
    help = "Overrides copyright formatting for a given path pattern. \
            Takes 4 arguments: PATH_PATTERN PREFIX PER_LINE_PREFIX SUFFIX. \
            Use `\\-` to escape leading dashes."
  )]
  custom_format: Vec<String>,

  #[arg(
    long,
    default_value = DEFAULT_SKIP_PATTERN,
    help = "A path pattern for paths to skip."
  )]
  skip_pattern: String,

  #[arg(required = true, help = "One or more paths of files to check.")]
  paths: Vec<String>,
}

struct CopyrightValidator {
  skip_regex: Regex,
  formats: Vec<(Regex, Regex, String)>,
}

impl CopyrightValidator {
  fn new(
    copyright: &str,
    skip_pattern: &str,
    custom_formats: Vec<String>,
  ) -> Result<Self, String> {
    let skip_regex = Regex::new(skip_pattern).map_err(|e| {
      format!("Invalid --skip_pattern `{}`: {}", skip_pattern, e)
    })?;
    let mut formats = Vec::new();

    // Dashes are treated as flags by clap, so we support escaping leading
    // dashes with `\-` to allow them to be passed as values. We strip the
    // leading backslash here.
    let custom_formats: Vec<String> = custom_formats
      .into_iter()
      .map(|s| {
        if let Some(stripped) = s.strip_prefix("\\-") {
          format!("-{}", stripped)
        } else {
          s
        }
      })
      .collect();

    let copyright = copyright.trim_matches('\n');

    for chunk in custom_formats.chunks(4) {
      if chunk.len() != 4 {
        return Err(
          "Custom format must have exactly 4 arguments per instance"
            .to_string(),
        );
      }
      let path_pattern = &chunk[0];
      let prefix = &chunk[1];
      let per_line_prefix = &chunk[2];
      let suffix = &chunk[3];
      Self::add_format(
        &mut formats,
        copyright,
        path_pattern,
        prefix,
        per_line_prefix,
        suffix,
      )?;
    }

    for (path_pattern, prefix, per_line_prefix, suffix) in BUILTIN_FORMATS {
      Self::add_format(
        &mut formats,
        copyright,
        path_pattern,
        prefix,
        per_line_prefix,
        suffix,
      )?;
    }

    Ok(CopyrightValidator {
      skip_regex,
      formats,
    })
  }

  fn add_format(
    formats: &mut Vec<(Regex, Regex, String)>,
    copyright: &str,
    path_pattern: &str,
    prefix: &str,
    per_line_prefix: &str,
    suffix: &str,
  ) -> Result<(), String> {
    let path_re = Regex::new(path_pattern)
      .map_err(|e| format!("Invalid pattern `{}`: {}", path_pattern, e))?;

    // Apply the per-line prefix to each line of the copyright template.
    // If a line is empty or contains only whitespace, we strip the trailing
    // whitespace from the prefix (e.g., "# " becomes "#") to avoid trailing
    // spaces on empty lines.
    //
    // Example:
    // copyright: "Line 1\n\nLine 2"
    // per_line_prefix: "# "
    // Result:
    // # Line 1
    // #
    // # Line 2
    let mut formatted = String::new();
    for line in copyright.lines() {
      if line.trim().is_empty() {
        formatted.push_str(per_line_prefix.trim_end());
      } else {
        formatted.push_str(per_line_prefix);
        formatted.push_str(line);
      }
      formatted.push('\n');
    }

    // Assemble the final formatted copyright block by adding optional
    // prefix and suffix blocks.
    //
    // Example with prefix "/*", per_line_prefix " * ", and suffix " */":
    // /*
    //  * Line 1
    //  *
    //  * Line 2
    //  */
    let mut final_formatted = String::new();
    if !prefix.is_empty() {
      final_formatted.push_str(prefix);
      final_formatted.push('\n');
    }
    final_formatted.push_str(&formatted);
    if !suffix.is_empty() {
      final_formatted.push_str(suffix);
      final_formatted.push('\n');
    }

    let copyright_re_str =
      regex::escape(&final_formatted).replace("YYYY", r"\d+");
    let copyright_re = Regex::new(&copyright_re_str).unwrap();

    let current_year = chrono::Utc::now().year().to_string();
    let suggest = final_formatted.replace("YYYY", &current_year);

    formats.push((path_re, copyright_re, suggest));
    Ok(())
  }

  fn get_copyright(&self, path: &str) -> Option<(&Regex, &String)> {
    if self.skip_regex.is_match(path) {
      return None;
    }

    for (path_re, copyright_re, suggest) in &self.formats {
      if path_re.is_match(path) {
        return Some((copyright_re, suggest));
      }
    }
    None
  }

  fn validate(&self, path: &str) -> Result<bool, io::Error> {
    let path_obj = Path::new(path);
    if path_obj.is_dir() {
      return Ok(true);
    }

    let (copyright_re, suggest) = match self.get_copyright(path) {
      Some(c) => c,
      None => return Ok(true),
    };

    let contents = match fs::read_to_string(path) {
      Ok(c) => c,
      Err(e) => {
        eprintln!("Skipping {}: {}", path, e);
        return Ok(true);
      }
    };

    if contents.len() <= 1 {
      return Ok(true);
    }

    if copyright_re.is_match(&contents) {
      return Ok(true);
    }

    eprintln!("Missing copyright in {}:\n{}", path, suggest);
    Ok(false)
  }
}

fn main() {
  let args = Args::parse();

  let validator = match CopyrightValidator::new(
    &args.copyright,
    &args.skip_pattern,
    args.custom_format,
  ) {
    Ok(v) => v,
    Err(e) => {
      eprintln!("Error: {}", e);
      std::process::exit(1);
    }
  };

  let mut exit_code = 0;
  for path in args.paths {
    // Ignore empty paths. These can happen in standard ways of building up
    // parameters when run (e.g., from shell expansions or empty variables).
    if path.is_empty() {
      continue;
    }
    match validator.validate(&path) {
      Ok(true) => {}
      Ok(false) => exit_code = 1,
      Err(e) => {
        eprintln!("Error processing {}: {}", path, e);
        exit_code = 1;
      }
    }
  }

  std::process::exit(exit_code);
}
