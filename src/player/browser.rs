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
        info!("  {} have sidecar metadata", with_sidecar);

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
                    warn!("Could not scan directory {:?}: {}", path, e);
                }
            } else if path.is_file() {
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    match ext.to_lowercase().as_str() {
                        "wav" | "flac" => {
                            let mut audio_file = AudioFile {
                                audio_path: path.clone(),
                                sidecar_path: None,
                                metadata: FileMetadata::default(),
                            };

                            // Look for sidecar .md file (append .md to full filename)
                            let mut sidecar = PathBuf::from(path.as_os_str());
                            sidecar.as_mut_os_string().push(".md");

                            if sidecar.exists() {
                                debug!("Found sidecar: {:?}", sidecar);
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

                            self.items.push(audio_file);
                        }
                        _ => {}
                    }
                }
            }
        }

        Ok(())
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
            debug!("Filtering with query: '{}'", query);

            // Score each item and find matching context
            let mut scored_items: Vec<(AudioFile, i64, Option<String>)> = self
                .items
                .iter()
                .filter_map(|item| {
                    let mut best_score = None;
                    let mut context = None;

                    // Search in metadata content using substring matching
                    if !item.metadata.content.is_empty() {
                        let content_lower = item.metadata.content.to_lowercase();

                        if let Some(pos) = content_lower.find(&query) {
                            // Found exact substring match
                            context = Some(extract_context(&item.metadata.content, pos, 80));
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

                    if best_score.is_none() && filename.to_lowercase().contains(&query) {
                        best_score = Some(50); // Lower score for filename matches
                        debug!("Filename match for {}", filename);
                    }

                    best_score.map(|score| (item.clone(), score, context))
                })
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
        if line.starts_with("# ") {
            metadata.title = line[2..].trim().to_string();
        }
        // Extract tags
        else if line.starts_with("- Tags:") || line.starts_with("- tags:") {
            let tags_str = line[7..].trim();
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

    // Search input
    let search_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Search (ESC to close) ");

    let search_text = Paragraph::new(format!("> {}", browser.search_query))
        .style(Style::default().fg(Color::Yellow))
        .block(search_block);

    f.render_widget(search_text, left_chunks[0]);

    // Results list
    let items: Vec<ListItem> = browser
        .filtered_items
        .iter()
        .enumerate()
        .map(|(idx, (item, _))| {
            let is_selected = idx == browser.selected;

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

            let content = format!("{}{}", filename, tags);

            let style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(content).style(style)
        })
        .collect();

    let results_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(format!(" Results ({}) ", browser.filtered_items.len()));

    let results_list = List::new(items).block(results_block);

    f.render_widget(results_list, left_chunks[1]);

    // Help text
    let help_text = Line::from(vec![
        Span::styled("Enter", Style::default().fg(Color::Yellow)),
        Span::raw(" play  "),
        Span::styled("↑↓", Style::default().fg(Color::Yellow)),
        Span::raw(" navigate"),
    ]);

    let help_widget = Paragraph::new(help_text)
        .alignment(ratatui::layout::Alignment::Center)
        .block(Block::default().borders(Borders::TOP));

    f.render_widget(help_widget, left_chunks[2]);

    // Right side - Preview
    draw_preview(f, main_chunks[1], browser);
}

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
            let preview_len = 200;
            let preview = if item.metadata.content.len() > preview_len {
                format!("{}...", &item.metadata.content[..preview_len])
            } else {
                item.metadata.content.clone()
            };
            preview
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
