//! Telescope-style file browser for audio file discovery.
//!
//! This module implements a searchable file browser that uses markdown sidecar files
//! for metadata-based searching. It allows users to quickly find audio files by
//! searching through associated metadata (tags, descriptions, notes) while displaying
//! the actual audio files for selection. The search uses substring matching to find
//! relevant content within the sidecar files.

use log::warn;
use std::fs;
use std::path::{Path, PathBuf};
use zim_studio::zimignore::ZimIgnore;

const SUPPORTED_AUDIO_EXTENSIONS: &[&str] = &["wav", "flac"];
const DEFAULT_CONTEXT_SIZE: usize = 80;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BrowserFocus {
    Search,
    Files,
}

#[derive(Debug, Clone)]
enum SearchQuery {
    FullText(String),
    FieldQuery { field: String, value: String },
}

#[derive(Clone)]
pub struct AudioFile {
    pub audio_path: PathBuf,
    pub sidecar_path: Option<PathBuf>,
    pub metadata: FileMetadata,
}

#[derive(Clone, Default)]
pub struct FileMetadata {
    pub title: String,
    pub project: Option<String>,
    pub tags: Vec<String>,
    pub content: String, // Full markdown content for searching
}

pub struct Browser {
    pub items: Vec<AudioFile>,
    pub filtered_indices: Vec<(usize, Option<String>)>, // (index into items, matched_context)
    pub selected: usize,
    pub search_query: String,
    pub focus: BrowserFocus,
    pub search_visible: bool, // Whether search box is shown
    zimignore: ZimIgnore,
}

impl Browser {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            filtered_indices: Vec::new(),
            selected: 0,
            search_query: String::new(),
            focus: BrowserFocus::Files, // Start with files focused
            search_visible: false,      // Start with search hidden
            zimignore: ZimIgnore::new(),
        }
    }

    pub fn scan_directory(&mut self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        self.items.clear();

        // Load .zimignore patterns for this directory
        self.zimignore = ZimIgnore::load_for_directory(path);

        // Recursively find all audio files
        self.scan_directory_recursive(path)?;

        // Sort by filename
        self.items
            .sort_by(|a, b| a.audio_path.file_name().cmp(&b.audio_path.file_name()));

        // Apply current search filter (preserves existing search)
        self.filter_items();

        Ok(())
    }

    fn scan_directory_recursive(&mut self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        // Skip hidden directories (starting with .)
        if let Some(name) = path.file_name()
            && let Some(name_str) = name.to_str()
            && name_str.starts_with('.')
            && path != Path::new(".")
        {
            return Ok(());
        }

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();

            // Convert to relative path for zimignore checking
            let relative_path = if let Ok(stripped) = path.strip_prefix(".") {
                stripped
            } else if let Ok(stripped) = path.strip_prefix("./") {
                stripped
            } else {
                &path
            };

            if path.is_dir() {
                // Check if directory should be ignored
                if self.zimignore.is_ignored(relative_path, true) {
                    continue;
                }

                // Skip . and .. to avoid infinite recursion
                if let Some(name) = path.file_name()
                    && let Some(name_str) = name.to_str()
                    && (name_str == "." || name_str == "..")
                {
                    continue;
                }
                // Recursively scan subdirectories
                if let Err(e) = self.scan_directory_recursive(&path) {
                    warn!("Could not scan directory {path:?}: {e}");
                }
            } else if path.is_file() {
                // Check if file should be ignored
                if self.zimignore.is_ignored(relative_path, false) {
                    log::debug!("Ignoring file due to .zimignore: {path:?}");
                    continue;
                }
                let is_audio = is_supported_audio_file(&path);
                log::debug!("Checking file: {path:?}, is_audio: {is_audio}");

                if is_audio {
                    // Check if this file is already in items (shouldn't happen but let's be sure)
                    if self.items.iter().any(|item| item.audio_path == path) {
                        log::warn!("Duplicate file found, skipping: {path:?}");
                        continue;
                    }

                    match self.create_audio_file(path.clone()) {
                        Ok(audio_file) => {
                            self.items.push(audio_file);
                        }
                        Err(e) => warn!("Could not create audio file for {path:?}: {e}"),
                    }
                }
            }
        }

        Ok(())
    }

    fn create_audio_file(&self, path: PathBuf) -> Result<AudioFile, Box<dyn std::error::Error>> {
        let mut audio_file = AudioFile {
            audio_path: path.clone(),
            sidecar_path: None,
            metadata: FileMetadata::default(),
        };

        // Look for sidecar .md file (append .md to full filename)
        let mut sidecar = PathBuf::from(path.as_os_str());
        sidecar.as_mut_os_string().push(".md");

        if sidecar.exists() {
            audio_file.sidecar_path = Some(sidecar.clone());

            // Read and parse sidecar content
            if let Ok(content) = fs::read_to_string(&sidecar) {
                let mut metadata = parse_sidecar_content(&content);
                metadata.content = content; // Store full content for searching
                audio_file.metadata = metadata;
            }
        } else {
            // Use filename as title if no sidecar
            audio_file.metadata.title = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("Unknown")
                .to_string();
        }

        Ok(audio_file)
    }

    pub fn push_char(&mut self, c: char) {
        log::debug!(
            "Before push_char: search_query = {:?}, char = {:?}",
            self.search_query,
            c
        );
        self.search_query.push(c);
        log::debug!("After push_char: search_query = {:?}", self.search_query);
        self.filter_items();
    }

    pub fn pop_char(&mut self) {
        self.search_query.pop();
        self.filter_items();
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            BrowserFocus::Search => BrowserFocus::Files,
            BrowserFocus::Files => BrowserFocus::Search,
        };
    }

    pub fn show_search(&mut self) {
        self.search_visible = true;
        self.focus = BrowserFocus::Search;
    }

    pub fn hide_search(&mut self) {
        self.search_visible = false;
        self.focus = BrowserFocus::Files;
    }

    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.filter_items();
    }

    fn filter_items(&mut self) {
        if self.search_query.is_empty() {
            // No search - show all items by index
            self.filtered_indices = (0..self.items.len()).map(|idx| (idx, None)).collect();
        } else {
            let parsed_query = parse_search_query(&self.search_query);

            // Score each item and find matching context
            let mut scored_items: Vec<(usize, i64, Option<String>)> = self
                .items
                .iter()
                .enumerate()
                .filter_map(|(idx, item)| {
                    score_item_with_query(item, &parsed_query)
                        .map(|(_, score, context)| (idx, score, context))
                })
                .collect();

            // Sort by score (highest first)
            scored_items.sort_by(|a, b| b.1.cmp(&a.1));

            self.filtered_indices = scored_items
                .into_iter()
                .map(|(idx, _, context)| (idx, context))
                .collect();
        }

        // Reset selection if out of bounds
        if self.selected >= self.filtered_indices.len() {
            self.selected = 0;
        }
    }

    pub fn select_next(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.selected = (self.selected + 1) % self.filtered_indices.len();
        }
    }

    pub fn select_previous(&mut self) {
        if !self.filtered_indices.is_empty() {
            if self.selected == 0 {
                self.selected = self.filtered_indices.len() - 1;
            } else {
                self.selected -= 1;
            }
        }
    }

    pub fn get_selected_path(&self) -> Option<&Path> {
        self.filtered_indices
            .get(self.selected)
            .and_then(|(idx, _)| self.items.get(*idx))
            .map(|item| item.audio_path.as_path())
    }

    pub fn get_filtered_items(&self) -> Vec<(&AudioFile, &Option<String>)> {
        self.filtered_indices
            .iter()
            .filter_map(|(idx, context)| self.items.get(*idx).map(|item| (item, context)))
            .collect()
    }
}

fn is_supported_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_AUDIO_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
}

fn parse_search_query(query: &str) -> SearchQuery {
    // Check for field queries like "title: something" or "tags: something"
    if let Some(colon_pos) = query.find(':') {
        let field = query[..colon_pos].trim().to_lowercase();
        let value = query[colon_pos + 1..].trim().to_lowercase();

        // Only recognize specific fields (allow both singular and plural for tags)
        match field.as_str() {
            "title" | "tags" | "tag" => {
                // Normalize "tag" to "tags" internally for consistency
                let normalized_field = if field == "tag" {
                    "tags".to_string()
                } else {
                    field.clone()
                };
                SearchQuery::FieldQuery {
                    field: normalized_field,
                    value,
                }
            }
            _ => SearchQuery::FullText(query.to_lowercase()),
        }
    } else {
        SearchQuery::FullText(query.to_lowercase())
    }
}

fn score_item_with_query(
    item: &AudioFile,
    query: &SearchQuery,
) -> Option<(AudioFile, i64, Option<String>)> {
    match query {
        SearchQuery::FullText(text) => score_item(item, text),
        SearchQuery::FieldQuery { field, value } => score_field_query(item, field, value),
    }
}

fn score_field_query(
    item: &AudioFile,
    field: &str,
    value: &str,
) -> Option<(AudioFile, i64, Option<String>)> {
    match field {
        "title" => {
            // If value is empty, match any item with a non-empty title
            if value.is_empty() {
                if !item.metadata.title.is_empty() {
                    let context = Some(format!("Title: {}", item.metadata.title));
                    Some((item.clone(), 100, context))
                } else {
                    None
                }
            } else if item.metadata.title.to_lowercase().contains(value) {
                let context = Some(format!("Title: {}", item.metadata.title));
                Some((item.clone(), 100, context))
            } else {
                None
            }
        }
        "tags" => {
            // If value is empty, match any item with tags
            if value.is_empty() {
                if !item.metadata.tags.is_empty() {
                    let context = Some(format!("Tags: {}", item.metadata.tags.join(", ")));
                    Some((item.clone(), 100, context))
                } else {
                    None
                }
            } else {
                // Match against any tag
                let matching_tags: Vec<&String> = item
                    .metadata
                    .tags
                    .iter()
                    .filter(|tag| tag.to_lowercase().contains(value))
                    .collect();

                if !matching_tags.is_empty() {
                    let tags_str: Vec<&str> = matching_tags.iter().map(|s| s.as_str()).collect();
                    let context = Some(format!("Tags: {}", tags_str.join(", ")));
                    Some((item.clone(), 100, context))
                } else {
                    None
                }
            }
        }
        _ => None,
    }
}

fn score_item(item: &AudioFile, query: &str) -> Option<(AudioFile, i64, Option<String>)> {
    let mut best_score = None;
    let mut context = None;

    // Search in metadata content using substring matching
    if !item.metadata.content.is_empty() {
        let content_lower = item.metadata.content.to_lowercase();

        if let Some(pos) = content_lower.find(query) {
            // Found exact substring match
            context = Some(extract_context(
                &item.metadata.content,
                pos,
                DEFAULT_CONTEXT_SIZE,
            ));
            best_score = Some(100); // High score for exact matches
        }
    }

    // Also search in filename using substring matching
    let filename = item
        .audio_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if best_score.is_none() && filename.to_lowercase().contains(query) {
        best_score = Some(50); // Lower score for filename matches
    }

    best_score.map(|score| (item.clone(), score, context))
}

fn extract_context(content: &str, pos: usize, context_size: usize) -> String {
    let start = pos.saturating_sub(context_size / 2);
    let end = (pos + context_size / 2).min(content.len());

    // Find word boundaries
    let start = if start > 0 {
        content[..start].rfind(' ').map(|i| i + 1).unwrap_or(start)
    } else {
        0
    };

    let end = if end < content.len() {
        content[end..].find(' ').map(|i| end + i).unwrap_or(end)
    } else {
        content.len()
    };

    let mut context = String::new();
    if start > 0 {
        context.push_str("...");
    }
    context.push_str(content[start..end].trim());
    if end < content.len() {
        context.push_str("...");
    }

    context
}

fn parse_sidecar_content(content: &str) -> FileMetadata {
    let mut metadata = FileMetadata::default();

    // Check if content has YAML frontmatter
    if let Some(content_after_marker) = content.strip_prefix("---\n") {
        // Find the end of frontmatter
        if let Some(end_pos) = content_after_marker.find("\n---\n") {
            let yaml_content = &content_after_marker[..end_pos];

            // Parse YAML line by line (simple parser for our needs)
            for line in yaml_content.lines() {
                let line = line.trim();

                if let Some(colon_pos) = line.find(':') {
                    let key = line[..colon_pos].trim();
                    let value = line[colon_pos + 1..].trim();

                    match key {
                        "title" => {
                            // Remove quotes if present
                            metadata.title = value.trim_matches('"').to_string();
                        }
                        "project" => {
                            // Remove quotes if present
                            let project_value = value.trim_matches('"');
                            if project_value != "unknown" && !project_value.is_empty() {
                                metadata.project = Some(project_value.to_string());
                            }
                        }
                        "tags" => {
                            // Parse array format: ["tag1", "tag2"] or []
                            if value.starts_with('[') && value.ends_with(']') {
                                let tags_str = &value[1..value.len() - 1];
                                if !tags_str.is_empty() {
                                    metadata.tags = tags_str
                                        .split(',')
                                        .map(|s| s.trim().trim_matches('"').to_string())
                                        .filter(|s| !s.is_empty())
                                        .collect();
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Also look for H1 title after frontmatter as fallback
        if metadata.title.is_empty() {
            for line in content.lines() {
                if let Some(title) = line.strip_prefix("# ") {
                    metadata.title = title.trim().to_string();
                    break;
                }
            }
        }
    } else {
        // Fallback to old markdown parsing for files without frontmatter
        for line in content.lines() {
            let line = line.trim();
            if let Some(title) = line.strip_prefix("# ") {
                metadata.title = title.trim().to_string();
                break;
            }
        }
    }

    // Note: content field will be filled by the caller with the full file content
    metadata
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_browser() -> Browser {
        Browser::new()
    }

    fn create_test_audio_file(path: &str) -> AudioFile {
        AudioFile {
            audio_path: PathBuf::from(path),
            sidecar_path: None,
            metadata: FileMetadata {
                title: "Test Title".to_string(),
                project: Some("test-project".to_string()),
                tags: vec!["tag1".to_string(), "tag2".to_string()],
                content: "Test content for searching".to_string(),
            },
        }
    }

    #[test]
    fn test_new_browser() {
        let browser = create_test_browser();
        assert!(browser.items.is_empty());
        assert!(browser.filtered_indices.is_empty());
        assert_eq!(browser.selected, 0);
        assert!(browser.search_query.is_empty());
    }

    #[test]
    fn test_is_supported_audio_file() {
        assert!(is_supported_audio_file(Path::new("test.wav")));
        assert!(is_supported_audio_file(Path::new("test.flac")));
        assert!(is_supported_audio_file(Path::new("test.WAV")));
        assert!(is_supported_audio_file(Path::new("test.FLAC")));
        assert!(!is_supported_audio_file(Path::new("test.mp3")));
        assert!(!is_supported_audio_file(Path::new("test.txt")));
        assert!(!is_supported_audio_file(Path::new("test")));
    }

    #[test]
    fn test_search_input() {
        let mut browser = create_test_browser();
        browser.items.push(create_test_audio_file("test.wav"));

        browser.push_char('t');
        browser.push_char('e');
        browser.push_char('s');
        browser.push_char('t');
        assert_eq!(browser.search_query, "test");

        browser.pop_char();
        assert_eq!(browser.search_query, "tes");
    }

    #[test]
    fn test_navigation() {
        let mut browser = create_test_browser();
        browser.items = vec![
            create_test_audio_file("1.wav"),
            create_test_audio_file("2.wav"),
            create_test_audio_file("3.wav"),
        ];
        browser.filtered_indices = vec![(0, None), (1, None), (2, None)];

        assert_eq!(browser.selected, 0);

        browser.select_next();
        assert_eq!(browser.selected, 1);

        browser.select_next();
        assert_eq!(browser.selected, 2);

        // Test wraparound
        browser.select_next();
        assert_eq!(browser.selected, 0);

        browser.select_previous();
        assert_eq!(browser.selected, 2);

        browser.select_previous();
        assert_eq!(browser.selected, 1);
    }

    #[test]
    fn test_get_selected_path() {
        let mut browser = create_test_browser();
        browser.items = vec![
            create_test_audio_file("/path/to/1.wav"),
            create_test_audio_file("/path/to/2.wav"),
        ];
        browser.filtered_indices = vec![(0, None), (1, None)];

        assert_eq!(
            browser.get_selected_path(),
            Some(Path::new("/path/to/1.wav"))
        );

        browser.selected = 1;
        assert_eq!(
            browser.get_selected_path(),
            Some(Path::new("/path/to/2.wav"))
        );

        browser.filtered_indices.clear();
        assert!(browser.get_selected_path().is_none());
    }

    #[test]
    fn test_parse_sidecar_content_yaml() {
        // Test YAML frontmatter format
        let content = r#"---
title: "My Audio File"
tags: ["ambient", "drone", "experimental"]
---

# My Audio File

Some description here.
"#;

        let metadata = parse_sidecar_content(content);

        assert_eq!(metadata.title, "My Audio File");
        assert_eq!(metadata.tags.len(), 3);
        assert!(metadata.tags.contains(&"ambient".to_string()));
        assert!(metadata.tags.contains(&"drone".to_string()));
        assert!(metadata.tags.contains(&"experimental".to_string()));
    }

    #[test]
    fn test_parse_sidecar_content_markdown() {
        // Test old markdown format (fallback)
        let content = r#"# My Audio File

Some description here.
"#;

        let metadata = parse_sidecar_content(content);
        assert_eq!(metadata.title, "My Audio File");
        assert!(metadata.tags.is_empty());
    }

    #[test]
    fn test_parse_empty_sidecar() {
        let metadata = parse_sidecar_content("");
        assert!(metadata.title.is_empty());
        assert!(metadata.tags.is_empty());
    }

    #[test]
    fn test_extract_context() {
        let content = "This is a test string with some content to extract context from.";
        let pos = content.find("string").unwrap();
        let context = extract_context(content, pos, 20);

        assert!(context.contains("string"));
        assert!(context.contains("..."));
    }

    #[test]
    fn test_score_item() {
        let mut item = create_test_audio_file("test.wav");
        item.metadata.content = "This is some sample content with keywords".to_string();

        // Test content match
        let result = score_item(&item, "sample");
        assert!(result.is_some());
        let (_, score, context) = result.unwrap();
        assert_eq!(score, 100); // Content match gets high score
        assert!(context.is_some());

        // Test filename match
        let result = score_item(&item, "test");
        assert!(result.is_some());
        let (_, score, _) = result.unwrap();
        assert_eq!(score, 50); // Filename match gets lower score

        // Test no match
        let result = score_item(&item, "xyz");
        assert!(result.is_none());
    }

    #[test]
    fn test_filter_items_empty_query() {
        let mut browser = create_test_browser();
        browser.items = vec![
            create_test_audio_file("1.wav"),
            create_test_audio_file("2.wav"),
        ];

        browser.filter_items();

        assert_eq!(browser.filtered_indices.len(), 2);
        assert!(browser.filtered_indices[0].1.is_none()); // No context for empty query
    }

    #[test]
    fn test_filter_items_with_query() {
        let mut browser = create_test_browser();

        let mut item1 = create_test_audio_file("ambient.wav");
        item1.metadata.content = "Ambient soundscape recording".to_string();

        let mut item2 = create_test_audio_file("nature.wav");
        item2.metadata.content = "Nature field recording".to_string();

        browser.items = vec![item1, item2];
        browser.search_query = "ambient".to_string();

        browser.filter_items();

        assert_eq!(browser.filtered_indices.len(), 1);
        let filtered_items = browser.get_filtered_items();
        assert_eq!(
            filtered_items[0].0.audio_path.to_str().unwrap(),
            "ambient.wav"
        );
    }

    #[test]
    fn test_create_audio_file() {
        let temp_dir = TempDir::new().unwrap();
        let audio_path = temp_dir.path().join("test.wav");
        let sidecar_path = temp_dir.path().join("test.wav.md");

        fs::write(&audio_path, b"fake wav").unwrap();
        fs::write(
            &sidecar_path,
            r#"---
title: "Test Audio"
tags: ["test", "sample"]
---

# Test Audio

Some content here.
"#,
        )
        .unwrap();

        let browser = create_test_browser();
        let audio_file = browser.create_audio_file(audio_path).unwrap();

        assert_eq!(audio_file.metadata.title, "Test Audio");
        assert_eq!(audio_file.metadata.tags.len(), 2);
        assert!(audio_file.metadata.tags.contains(&"test".to_string()));
        assert!(audio_file.metadata.tags.contains(&"sample".to_string()));
        assert!(audio_file.sidecar_path.is_some());
    }

    #[test]
    fn test_create_audio_file_no_sidecar() {
        let temp_dir = TempDir::new().unwrap();
        let audio_path = temp_dir.path().join("test.flac");

        fs::write(&audio_path, b"fake flac").unwrap();

        let browser = create_test_browser();
        let audio_file = browser.create_audio_file(audio_path).unwrap();

        assert_eq!(audio_file.metadata.title, "test");
        assert!(audio_file.metadata.tags.is_empty());
        assert!(audio_file.sidecar_path.is_none());
    }
}
