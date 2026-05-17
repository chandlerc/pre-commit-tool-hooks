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

use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use regex::Regex;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Header {
  pub label: String,
  pub anchor: String,
  pub level: u32,
}

#[derive(Debug, Clone)]
pub struct Link {
  pub label: String,
  pub destination: String,
  pub line_number: usize,
}

pub fn make_anchor(
  label: &str,
  used_anchors: &mut HashMap<String, usize>,
) -> String {
  let mut anchor = label.to_lowercase().trim().to_string();
  anchor = anchor.replace(' ', "-");

  // Remove punctuation as per GFM (approximate)
  // Python: re.sub("[!\"#$%&'()*+,./:;<=>?@[\\\\\\]^`{|}~]", "", anchor)
  let re = Regex::new("[!\"#$%&'()*+,./:;<=>?@\\[\\\\\\]^`{|}~]").unwrap();
  anchor = re.replace_all(&anchor, "").to_string();

  if let Some(count) = used_anchors.get_mut(&anchor) {
    *count += 1;
    format!("{}-{}", anchor, count)
  } else {
    used_anchors.insert(anchor.clone(), 0);
    anchor
  }
}

pub fn get_links(contents: &str) -> Result<(Vec<Header>, Vec<Link>), String> {
  let mut headers = Vec::new();
  let mut links = Vec::new();
  let mut used_anchors = HashMap::new();

  let parser = Parser::new(contents);
  let iter = parser.into_offset_iter();

  let mut current_header: Option<(String, u32, usize)> = None;
  let mut current_link: Option<(String, String, usize)> = None;

  let mut prev_level = 1;
  let mut prev_header = "(first header)".to_string();

  // Helper to get line number from byte offset
  let get_line_number =
    |offset: usize| -> usize { contents[..offset].lines().count() };

  for (event, range) in iter {
    match event {
      Event::Start(Tag::Heading { level, .. }) => {
        current_header =
          Some((String::new(), level as u32, get_line_number(range.start)));
      }
      Event::End(TagEnd::Heading(_)) => {
        if let Some((label, level, _)) = current_header.take() {
          if level - 1 > prev_level {
            return Err(format!(
              "Header '{}' has level {}, which is too deep versus \
               previous header '{}' with level {}.",
              label, level, prev_header, prev_level
            ));
          }
          prev_level = level;
          prev_header = label.clone();

          let anchor = make_anchor(&label, &mut used_anchors);
          headers.push(Header {
            label,
            anchor,
            level,
          });
        }
      }
      Event::Start(Tag::Link { dest_url, .. }) => {
        current_link = Some((
          String::new(),
          dest_url.to_string(),
          get_line_number(range.start),
        ));
      }
      Event::End(TagEnd::Link) => {
        if let Some((label, destination, line)) = current_link.take() {
          links.push(Link {
            label,
            destination,
            line_number: line,
          });
        }
      }
      Event::Text(text) | Event::Code(text) => {
        if let Some((ref mut label, _, _)) = current_header {
          label.push_str(&text);
        }
        if let Some((ref mut label, _, _)) = current_link {
          label.push_str(&text);
        }
      }
      _ => {}
    }
  }

  Ok((headers, links))
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::HashMap;

  #[test]
  fn test_make_anchor() {
    let mut used = HashMap::new();
    assert_eq!(make_anchor("Hello World", &mut used), "hello-world");
    assert_eq!(make_anchor("Hello! World?", &mut used), "hello-world-1");
    assert_eq!(make_anchor("Hello World", &mut used), "hello-world-2");
  }

  #[test]
  fn test_get_links() {
    let contents = "# Title\n\n[Link](url)\n## Header\n";
    let (headers, links) = get_links(contents).unwrap();
    assert_eq!(headers.len(), 2);
    assert_eq!(headers[0].label, "Title");
    assert_eq!(headers[1].label, "Header");
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].label, "Link");
    assert_eq!(links[0].destination, "url");
  }
}
