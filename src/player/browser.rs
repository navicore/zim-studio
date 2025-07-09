//! Telescope-style file browser for audio file discovery.
//!
//! This module implements a searchable file browser that uses markdown sidecar files
//! for metadata-based searching. It allows users to quickly find audio files by
//! searching through associated metadata (tags, descriptions, notes) while displaying
//! the actual audio files for selection. The search uses substring matching to find
//! relevant content within the sidecar files.

use log::{debug, info, warn};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};
use std::fs;
use std::path::{Path, PathBuf};

const SUPPORTED_AUDIO_EXTENSIONS: &[&str] = &["wav", "flac"];
const DEFAULT_CONTEXT_SIZE: usize = 80;
#[allow(dead_code)]
const DEFAULT_PREVIEW_LENGTH: usize = 200;

#[derive(Clone)]
pub struct AudioFile {
    pub audio_path: PathBuf,
    pub sidecar_path: Option<PathBuf>,
    pub metadata: FileMetadata,
}

#[derive(Clone, Default)]
pub struct FileMetadata {
    pub title: String,
    pub tags: Vec<String>,
    pub content: String, // Full markdown content for searching
}

pub struct Browser {
    pub items: Vec<AudioFile>,
    pub filtered_items: Vec<(AudioFile, Option<String>)>, // (file, matched_context)
    pub selected: usize,
    pub search_query: String,
    pub is_active: bool,
}

impl Browser {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            filtered_items: Vec::new(),
            selected: 0,
            search_query: String::new(),
            is_active: false,
        }
    }

    pub fn scan_directory(&mut self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        self.items.clear();

        // Recursively find all audio files
        self.scan_directory_recursive(path)?;

        // Sort by filename
        self.items
            .sort_by(|a, b| a.audio_path.file_name().cmp(&b.audio_path.file_name()));

        // Initially show all items with no context
        self.filtered_items = self.items.iter().map(|item| (item.clone(), None)).collect();

        info!("Found {} audio files", self.items.len());
        let with_sidecar = self
            .items
            .iter()
            .filter(|i| !i.metadata.content.is_empty())
            .count();
        info!("  {with_sidecar} have sidecar metadata");

        Ok(())
    }

    fn scan_directory_recursive(&mut self, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
        // Skip hidden directories (starting with .)
        if let Some(name) = path.file_name() {
            if let Some(name_str) = name.to_str() {
                if name_str.starts_with('.') && path != Path::new(".") {
                    return Ok(());
                }
            }
        }

        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                // Recursively scan subdirectories
                if let Err(e) = self.scan_directory_recursive(&path) {
                    warn!("Could not scan directory {path:?}: {e}");
                }
            } else if path.is_file() && is_supported_audio_file(&path) {
                match self.create_audio_file(path.clone()) {
                    Ok(audio_file) => self.items.push(audio_file),
                    Err(e) => warn!("Could not create audio file for {path:?}: {e}"),
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
            debug!("Found sidecar: {sidecar:?}");
            audio_file.sidecar_path = Some(sidecar.clone());

            // Read and parse sidecar content
            if let Ok(content) = fs::read_to_string(&sidecar) {
                let mut metadata = parse_sidecar_content(&content);
                metadata.content = content; // Store full content for searching
                audio_file.metadata = metadata;
                debug!(
                    "Loaded sidecar for {}: {} chars, title: '{}', tags: {:?}",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    audio_file.metadata.content.len(),
                    audio_file.metadata.title,
                    audio_file.metadata.tags
                );
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

    pub fn toggle(&mut self) {
        self.is_active = !self.is_active;
        if self.is_active {
            self.search_query.clear();
            self.filter_items();
        }
    }

    pub fn push_char(&mut self, c: char) {
        self.search_query.push(c);
        self.filter_items();
    }

    pub fn pop_char(&mut self) {
        self.search_query.pop();
        self.filter_items();
    }

    #[allow(dead_code)]
    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.filter_items();
    }

    fn filter_items(&mut self) {
        if self.search_query.is_empty() {
            self.filtered_items = self.items.iter().map(|item| (item.clone(), None)).collect();
        } else {
            let query = self.search_query.to_lowercase();
            debug!("Filtering with query: '{query}'");

            // Score each item and find matching context
            let mut scored_items: Vec<(AudioFile, i64, Option<String>)> = self
                .items
                .iter()
                .filter_map(|item| score_item(item, &query))
                .collect();

            // Sort by score (highest first)
            scored_items.sort_by(|a, b| b.1.cmp(&a.1));

            self.filtered_items = scored_items
                .into_iter()
                .map(|(item, _, context)| (item, context))
                .collect();

            info!(
                "Search '{}' returned {} results",
                query,
                self.filtered_items.len()
            );
        }

        // Reset selection if out of bounds
        if self.selected >= self.filtered_items.len() {
            self.selected = 0;
        }
    }

    pub fn select_next(&mut self) {
        if !self.filtered_items.is_empty() {
            self.selected = (self.selected + 1) % self.filtered_items.len();
        }
    }

    pub fn select_previous(&mut self) {
        if !self.filtered_items.is_empty() {
            if self.selected == 0 {
                self.selected = self.filtered_items.len() - 1;
            } else {
                self.selected -= 1;
            }
        }
    }

    pub fn get_selected_path(&self) -> Option<&Path> {
        self.filtered_items
            .get(self.selected)
            .map(|(item, _)| item.audio_path.as_path())
    }
}

fn is_supported_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| SUPPORTED_AUDIO_EXTENSIONS.contains(&e.to_lowercase().as_str()))
        .unwrap_or(false)
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

            debug!(
                "Content match for {}: substring found at position {}",
                item.audio_path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy(),
                pos
            );
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
        debug!("Filename match for {filename}");
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
    let mut in_tags = false;

    for line in content.lines() {
        let line = line.trim();

        // Extract title from H1
        if let Some(title) = line.strip_prefix("# ") {
            metadata.title = title.trim().to_string();
        }
        // Extract tags
        else if let Some(tags_str) = line
            .strip_prefix("- Tags:")
            .or_else(|| line.strip_prefix("- tags:"))
        {
            let tags_str = tags_str.trim();
            metadata.tags = tags_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }
        // Look for tag lists
        else if line == "Tags:" || line == "tags:" {
            in_tags = true;
        } else if in_tags && line.starts_with("- ") {
            metadata.tags.push(line[2..].trim().to_string());
        } else if in_tags && !line.starts_with('-') && !line.is_empty() {
            in_tags = false;
        }
    }

    // Note: content field will be filled by the caller with the full file content
    metadata
}

#[allow(dead_code)]
pub fn draw_browser(f: &mut Frame, area: Rect, browser: &Browser) {
    // Create a floating window effect
    let popup_area = centered_rect(90, 85, area);

    // Clear the background
    f.render_widget(Clear, popup_area);

    // Main layout - split horizontally for list and preview
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(50), // File list
            Constraint::Percentage(50), // Preview
        ])
        .split(popup_area);

    // Left side - search and results
    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Search input
            Constraint::Min(5),    // Results list
            Constraint::Length(2), // Help
        ])
        .split(main_chunks[0]);

    // Draw components
    draw_search_input(f, left_chunks[0], browser);
    draw_results_list(f, left_chunks[1], browser);
    draw_help_bar(f, left_chunks[2]);
    draw_preview(f, main_chunks[1], browser);
}

#[allow(dead_code)]
fn draw_search_input(f: &mut Frame, area: Rect, browser: &Browser) {
    let search_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Search (ESC to close) ");

    let search_text = Paragraph::new(format!("> {}", browser.search_query))
        .style(Style::default().fg(Color::Yellow))
        .block(search_block);

    f.render_widget(search_text, area);
}

#[allow(dead_code)]
fn draw_results_list(f: &mut Frame, area: Rect, browser: &Browser) {
    let items: Vec<ListItem> = browser
        .filtered_items
        .iter()
        .enumerate()
        .map(|(idx, (item, _))| create_list_item(idx, item, browser.selected))
        .collect();

    let results_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(format!(" Results ({}) ", browser.filtered_items.len()));

    let results_list = List::new(items).block(results_block);

    f.render_widget(results_list, area);
}

#[allow(dead_code)]
fn create_list_item(idx: usize, item: &AudioFile, selected_idx: usize) -> ListItem<'static> {
    let is_selected = idx == selected_idx;

    // Always show the audio filename
    let filename = item
        .audio_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("Unknown");

    let tags = if !item.metadata.tags.is_empty() {
        format!(" [{}]", item.metadata.tags.join(", "))
    } else {
        String::new()
    };

    let content = format!("{filename}{tags}");

    let style = if is_selected {
        Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    ListItem::new(content).style(style)
}

#[allow(dead_code)]
fn draw_help_bar(f: &mut Frame, area: Rect) {
    let help_text = Line::from(vec![
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::raw(" play  "),
        Span::styled("↑↓", Style::default().fg(Color::Yellow)),
        Span::raw(" navigate"),
    ]);

    let help_widget = Paragraph::new(help_text)
        .alignment(ratatui::layout::Alignment::Center)
        .block(Block::default().borders(Borders::TOP));

    f.render_widget(help_widget, area);
}

#[allow(dead_code)]
fn draw_preview(f: &mut Frame, area: Rect, browser: &Browser) {
    let preview_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray))
        .title(" Preview ");

    if let Some((item, context)) = browser.filtered_items.get(browser.selected) {
        let preview_text = if let Some(ctx) = context {
            // Show the matched context
            ctx.clone()
        } else if !item.metadata.content.is_empty() {
            // Show beginning of file if no specific match
            if item.metadata.content.len() > DEFAULT_PREVIEW_LENGTH {
                format!("{}...", &item.metadata.content[..DEFAULT_PREVIEW_LENGTH])
            } else {
                item.metadata.content.clone()
            }
        } else {
            "No metadata available".to_string()
        };

        let preview_widget = Paragraph::new(preview_text)
            .block(preview_block)
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(Color::White));

        f.render_widget(preview_widget, area);
    } else {
        let preview_widget = Paragraph::new("No file selected")
            .block(preview_block)
            .style(Style::default().fg(Color::DarkGray));

        f.render_widget(preview_widget, area);
    }
}

#[allow(dead_code)]
fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
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
                tags: vec!["tag1".to_string(), "tag2".to_string()],
                content: "Test content for searching".to_string(),
            },
        }
    }

    #[test]
    fn test_new_browser() {
        let browser = create_test_browser();
        assert!(browser.items.is_empty());
        assert!(browser.filtered_items.is_empty());
        assert_eq!(browser.selected, 0);
        assert!(browser.search_query.is_empty());
        assert!(!browser.is_active);
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
    fn test_toggle_browser() {
        let mut browser = create_test_browser();
        assert!(!browser.is_active);

        browser.toggle();
        assert!(browser.is_active);

        browser.toggle();
        assert!(!browser.is_active);
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

        browser.clear_search();
        assert!(browser.search_query.is_empty());
    }

    #[test]
    fn test_navigation() {
        let mut browser = create_test_browser();
        browser.filtered_items = vec![
            (create_test_audio_file("1.wav"), None),
            (create_test_audio_file("2.wav"), None),
            (create_test_audio_file("3.wav"), None),
        ];

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
        browser.filtered_items = vec![
            (create_test_audio_file("/path/to/1.wav"), None),
            (create_test_audio_file("/path/to/2.wav"), None),
        ];

        assert_eq!(
            browser.get_selected_path(),
            Some(Path::new("/path/to/1.wav"))
        );

        browser.selected = 1;
        assert_eq!(
            browser.get_selected_path(),
            Some(Path::new("/path/to/2.wav"))
        );

        browser.filtered_items.clear();
        assert!(browser.get_selected_path().is_none());
    }

    #[test]
    fn test_parse_sidecar_content() {
        let content = r#"# My Audio File
- Tags: ambient, drone, experimental

Some description here.
More content.

Tags:
- field-recording
- nature
"#;

        let metadata = parse_sidecar_content(content);

        assert_eq!(metadata.title, "My Audio File");
        assert_eq!(metadata.tags.len(), 5);
        assert!(metadata.tags.contains(&"ambient".to_string()));
        assert!(metadata.tags.contains(&"field-recording".to_string()));
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

        assert_eq!(browser.filtered_items.len(), 2);
        assert!(browser.filtered_items[0].1.is_none()); // No context for empty query
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

        assert_eq!(browser.filtered_items.len(), 1);
        assert_eq!(
            browser.filtered_items[0].0.audio_path.to_str().unwrap(),
            "ambient.wav"
        );
    }

    #[test]
    fn test_create_audio_file() {
        let temp_dir = TempDir::new().unwrap();
        let audio_path = temp_dir.path().join("test.wav");
        let sidecar_path = temp_dir.path().join("test.wav.md");

        fs::write(&audio_path, b"fake wav").unwrap();
        fs::write(&sidecar_path, "# Test Audio\n- Tags: test, sample").unwrap();

        let browser = create_test_browser();
        let audio_file = browser.create_audio_file(audio_path).unwrap();

        assert_eq!(audio_file.metadata.title, "Test Audio");
        assert_eq!(audio_file.metadata.tags.len(), 2);
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
