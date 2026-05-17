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
use pre_commit_tool_hooks::{Link, get_links};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use url::Url;

#[derive(Parser, Debug)]
#[command(
  author,
  version,
  about = "Validates links in markdown files.",
  long_about = None
)]
struct Args {
  #[arg(required = true, help = "One or more paths of files to check.")]
  paths: Vec<String>,

  #[arg(
    long,
    help = "Only validate intra-document anchors, \
            ignoring cross-document links."
  )]
  anchors_only: bool,
}

fn print_error(path: &str, link: &Link, message: &str) {
  eprintln!(
    "{}:{}: [{}]({}): {}",
    path, link.line_number, link.label, link.destination, message
  );
}

struct LinkCache {
  cache: HashMap<PathBuf, (Vec<String>, Vec<Link>)>,
}

impl LinkCache {
  fn new() -> Self {
    LinkCache {
      cache: HashMap::new(),
    }
  }

  fn get(&mut self, path: &Path) -> Result<&(Vec<String>, Vec<Link>), String> {
    let abs_path = path.canonicalize().map_err(|e| {
      format!("Failed to canonicalize {}: {}", path.display(), e)
    })?;

    if !self.cache.contains_key(&abs_path) {
      let contents = fs::read_to_string(&abs_path)
        .map_err(|e| format!("Failed to read {}: {}", abs_path.display(), e))?;
      let (headers, links) = get_links(&contents)?;
      let anchors = headers.into_iter().map(|h| h.anchor).collect();
      self.cache.insert(abs_path.clone(), (anchors, links));
    }

    Ok(self.cache.get(&abs_path).unwrap())
  }
}

fn check_links(
  link_cache: &mut LinkCache,
  repo_root: &Path,
  path: &str,
  anchors_only: bool,
) -> Result<bool, String> {
  let path_obj = Path::new(path);
  let abs_path = path_obj
    .canonicalize()
    .map_err(|e| format!("Failed to canonicalize {}: {}", path, e))?;

  let (anchors_ref, links) = link_cache.get(&abs_path)?;
  let anchors = anchors_ref.clone();
  let links = links.clone();

  let mut has_errors = false;

  for link in links {
    if link.destination.starts_with('#') {
      // Internal anchor
      let fragment = &link.destination[1..];
      if !anchors.contains(&fragment.to_string()) {
        print_error(path, &link, "Link points at a non-existent anchor.");
        has_errors = true;
      }
      continue;
    }

    if link.destination.starts_with("//") {
      print_error(path, &link, "Link is missing a scheme, such as https://.");
      has_errors = true;
      continue;
    }

    match Url::parse(&link.destination) {
      Ok(_) => {
        // Has scheme, skip
        continue;
      }
      Err(url::ParseError::RelativeUrlWithoutBase) => {
        // It's a path
        if anchors_only {
          continue;
        }

        let mut parts = link.destination.split('#');
        let path_part = parts.next().unwrap_or("");
        let fragment_part = parts.next();

        if path_part.is_empty() {
          // Should have been handled by starts_with('#')
          continue;
        }

        let mut dest_path = if path_part.starts_with('/') {
          repo_root.join(path_part.trim_start_matches('/'))
        } else {
          abs_path
            .parent()
            .expect("File should have a parent directory")
            .join(path_part)
        };

        if dest_path.is_dir() {
          if fragment_part.is_none() {
            continue;
          }
          dest_path = dest_path.join("README.md");
        }

        if !dest_path.exists() {
          print_error(path, &link, "Link points at a non-existent file.");
          has_errors = true;
          continue;
        }

        if let Some(fragment) = fragment_part {
          let (dest_anchors, _) = link_cache.get(&dest_path)?;
          if !dest_anchors.contains(&fragment.to_string()) {
            print_error(path, &link, "Link points at a non-existent anchor.");
            has_errors = true;
          }
        }
      }
      Err(_) => {
        print_error(path, &link, "Invalid URL.");
        has_errors = true;
      }
    }
  }

  Ok(has_errors)
}

fn main() {
  let args = Args::parse();

  // Get repo root using jj
  let output = Command::new("jj")
    .args(["workspace", "root"])
    .output()
    .expect("Failed to execute jj");

  if !output.status.success() {
    eprintln!("Error: Failed to get workspace root from jj");
    std::process::exit(1);
  }

  let repo_root_str = String::from_utf8(output.stdout)
    .expect("jj output should be valid UTF-8")
    .trim()
    .to_string();
  let repo_root = Path::new(&repo_root_str);

  let mut link_cache = LinkCache::new();
  let mut exit_code = 0;

  for path in args.paths {
    if !path.ends_with(".md") {
      continue;
    }
    match check_links(&mut link_cache, repo_root, &path, args.anchors_only) {
      Ok(true) => exit_code = 1,
      Ok(false) => {}
      Err(e) => {
        eprintln!("Error processing {}: {}", path, e);
        exit_code = 1;
      }
    }
  }

  std::process::exit(exit_code);
}
