//! Terminal user interface rendering for the audio player.
//!
//! This module handles all visual rendering for the player, including the oscilloscope
//! waveform display, LED-style level meters, progress bars with mark indicators,
//! and control hints. It adapts the display based on terminal size, showing more
//! detailed visualizations when space permits.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Gauge, Paragraph,
        canvas::{Canvas, Context},
    },
};

use super::app::App;
use super::browser::draw_browser;
use super::save_dialog_ui::draw_save_dialog;

// UI Constants
const MIN_HEIGHT_FOR_OSCILLOSCOPE: u16 = 20;
const LED_LEVEL_THRESHOLDS: [(f32, &str); 3] = [
    (0.3, "●"),  // Full circle
    (0.05, "◐"), // Half filled
    (0.0, "○"),  // Empty circle
];

// LED color definitions for different levels
const LED_CLIPPING_LEVEL: f32 = 0.9;
const LED_HIGH_LEVEL: f32 = 0.3;
const LED_LOW_LEVEL: f32 = 0.05;

// Grid constants for oscilloscope
const GRID_COLOR: Color = Color::Rgb(0, 60, 30);
const GRID_VERTICAL_STEP: usize = 10;
const GRID_HORIZONTAL_LINES: [f64; 7] = [-0.75, -0.5, -0.25, 0.0, 0.25, 0.5, 0.75];

// Helper functions
fn format_time(seconds: u64) -> String {
    let mins = seconds / 60;
    let secs = seconds % 60;
    format!("{mins:02}:{secs:02}")
}

fn format_duration(duration: std::time::Duration) -> String {
    format_time(duration.as_secs())
}

fn create_control_button(key: &str, style: Style) -> Span<'static> {
    Span::styled(format!("[{key}]"), style)
}

pub fn draw(f: &mut Frame, app: &App) {
    let size = f.area();

    // Draw main UI
    draw_main_ui(f, app);

    // Draw browser overlay if active
    if app.browser.is_active {
        draw_browser(f, size, &app.browser);
    }

    // Draw save dialog if active
    if let Some(ref save_dialog) = app.save_dialog {
        draw_save_dialog(f, size, save_dialog);
    }
}

fn draw_main_ui(f: &mut Frame, app: &App) {
    let size = f.area();
    let show_oscilloscope = size.height > MIN_HEIGHT_FOR_OSCILLOSCOPE;

    let constraints = if show_oscilloscope {
        vec![
            Constraint::Length(2), // Title (reduced from 3)
            Constraint::Length(3), // File info + LEDs
            Constraint::Length(3), // Progress bar
            Constraint::Min(7),    // Waveform area
            Constraint::Length(4), // Controls (increased for 2 rows)
        ]
    } else {
        vec![
            Constraint::Length(2), // Title (reduced from 3)
            Constraint::Length(3), // File info + LEDs
            Constraint::Length(3), // Progress bar
            Constraint::Length(4), // Controls (increased for 2 rows)
        ]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(constraints)
        .split(size);

    // Title (more compact)
    let title = Paragraph::new("🎵 ZIM Player")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center);
    f.render_widget(title, chunks[0]);

    // File info with LED indicators
    draw_file_info_with_leds(f, chunks[1], app);

    // Progress bar
    draw_progress_bar(f, chunks[2], app);

    // Oscilloscope visualization (only if window is tall enough)
    if show_oscilloscope {
        draw_oscilloscope(f, chunks[3], app);
    }

    // Controls (two rows)
    let controls_idx = if show_oscilloscope { 4 } else { 3 };

    // Split controls area into two rows
    let control_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(chunks[controls_idx]);

    // First row of controls
    let play_color = if app.is_playing {
        Color::Yellow
    } else {
        Color::Green
    };
    let controls_row1 = vec![
        create_control_button("space", Style::default().fg(play_color)),
        Span::raw(if app.is_playing {
            " pause  "
        } else {
            " play  "
        }),
        create_control_button("←→", Style::default().fg(Color::Magenta)),
        Span::raw(" seek  "),
        create_control_button("/", Style::default().fg(Color::Blue)),
        Span::raw(" browse  "),
        create_control_button("q", Style::default().fg(Color::Red)),
        Span::raw(" quit"),
    ];

    // Second row of controls
    let loop_style = if app.is_looping {
        Style::default().fg(Color::Magenta).bg(Color::DarkGray)
    } else {
        Style::default().fg(Color::Magenta)
    };

    let controls_row2 = vec![
        create_control_button("i", Style::default().fg(Color::Green)),
        Span::raw(" in  "),
        create_control_button("o", Style::default().fg(Color::Green)),
        Span::raw(" out  "),
        create_control_button("x", Style::default().fg(Color::Yellow)),
        Span::raw(" clear  "),
        create_control_button("l", loop_style),
        Span::raw(if app.is_looping {
            " loop ●  "
        } else {
            " loop  "
        }),
        create_control_button("s", Style::default().fg(Color::Cyan)),
        Span::raw(" save"),
    ];

    let controls_widget1 = Paragraph::new(Line::from(controls_row1)).alignment(Alignment::Center);
    let controls_widget2 = Paragraph::new(Line::from(controls_row2)).alignment(Alignment::Center);

    // Add top border only on first row
    let border_widget = Block::default().borders(Borders::TOP);
    f.render_widget(border_widget, chunks[controls_idx]);

    f.render_widget(controls_widget1, control_chunks[0]);
    f.render_widget(controls_widget2, control_chunks[1]);
}

fn draw_file_info_with_leds(f: &mut Frame, area: Rect, app: &App) {
    // Split area horizontally for file info and LEDs
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(20),    // File info
            Constraint::Length(12), // LED indicators
        ])
        .split(area);

    // File info
    let file_info = if let Some(file) = &app.current_file {
        let filename = std::path::Path::new(file)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(file);
        format!("Loaded: {filename}")
    } else {
        "No file selected - Pass a file path to play".to_string()
    };

    let file_widget = Paragraph::new(file_info).style(Style::default().fg(Color::White));
    f.render_widget(file_widget, chunks[0]);

    // LED indicators
    draw_leds(f, chunks[1], app);

    // Bottom border
    let border = Block::default().borders(Borders::BOTTOM);
    f.render_widget(border, area);
}

fn draw_leds(f: &mut Frame, area: Rect, app: &App) {
    let led_text = if app.current_file.is_some() {
        let l_char = get_led_char(app.left_level);
        let r_char = get_led_char(app.right_level);
        let l_color = get_led_color(app.left_level, true);
        let r_color = get_led_color(app.right_level, false);

        vec![
            Span::raw("L"),
            Span::styled(l_char, Style::default().fg(l_color)),
            Span::raw(" R"),
            Span::styled(r_char, Style::default().fg(r_color)),
        ]
    } else {
        vec![
            Span::raw("L"),
            Span::styled("○", Style::default().fg(Color::DarkGray)),
            Span::raw(" R"),
            Span::styled("○", Style::default().fg(Color::DarkGray)),
        ]
    };

    let led_widget = Paragraph::new(Line::from(led_text)).alignment(Alignment::Right);
    f.render_widget(led_widget, area);
}

fn get_led_char(level: f32) -> &'static str {
    for (threshold, symbol) in LED_LEVEL_THRESHOLDS.iter() {
        if level >= *threshold {
            return symbol;
        }
    }
    "○" // Default to empty circle
}

fn get_led_color(level: f32, is_left: bool) -> Color {
    match (is_left, level) {
        // Clipping (both channels show red)
        (_, l) if l > LED_CLIPPING_LEVEL => Color::Rgb(255, 100, 100),
        // Left channel (green)
        (true, l) if l > LED_HIGH_LEVEL => Color::Rgb(100, 255, 100),
        (true, l) if l > LED_LOW_LEVEL => Color::Rgb(50, 200, 50),
        (true, _) => Color::Rgb(20, 100, 20),
        // Right channel (orange/red)
        (false, l) if l > LED_HIGH_LEVEL => Color::Rgb(255, 150, 0),
        (false, l) if l > LED_LOW_LEVEL => Color::Rgb(200, 100, 0),
        (false, _) => Color::Rgb(100, 50, 0),
    }
}

fn draw_oscilloscope(f: &mut Frame, area: Rect, app: &App) {
    // Create oscilloscope-style canvas
    let canvas = Canvas::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .paint(|ctx| {
            // Draw grid
            draw_grid(ctx, area);

            // Draw waveform (real data or demo)
            draw_waveform(ctx, area, app);

            // Draw center reference line
            ctx.draw(&ratatui::widgets::canvas::Line {
                x1: 0.0,
                y1: 0.0,
                x2: area.width as f64,
                y2: 0.0,
                color: Color::Rgb(0, 100, 50), // Darker green for reference
            });
        })
        .x_bounds([0.0, area.width as f64])
        .y_bounds([-1.0, 1.0]);

    f.render_widget(canvas, area);
}

fn draw_grid(ctx: &mut Context, area: Rect) {
    // Vertical grid lines
    for x in (0..area.width).step_by(GRID_VERTICAL_STEP) {
        ctx.draw(&ratatui::widgets::canvas::Line {
            x1: x as f64,
            y1: -1.0,
            x2: x as f64,
            y2: 1.0,
            color: GRID_COLOR,
        });
    }

    // Horizontal grid lines
    for y in GRID_HORIZONTAL_LINES {
        ctx.draw(&ratatui::widgets::canvas::Line {
            x1: 0.0,
            y1: y,
            x2: area.width as f64,
            y2: y,
            color: GRID_COLOR,
        });
    }
}

fn draw_waveform(ctx: &mut Context, area: Rect, app: &App) {
    // Get samples from the waveform buffer
    let samples = app.waveform_buffer.get_display_samples(area.width as usize);

    let points: Vec<(f64, f64)> = if samples.iter().any(|&s| s != 0.0) {
        // Use real audio data - amplify for better visibility
        samples
            .iter()
            .enumerate()
            .map(|(i, &sample)| {
                let amplified = (sample * 1.5).clamp(-0.95, 0.95);
                (i as f64, amplified as f64)
            })
            .collect()
    } else {
        // Demo sine wave when no audio loaded
        let time_offset = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as f64
            / 1000.0;

        (0..area.width)
            .map(|x| {
                let t = x as f64 / area.width as f64 * 4.0 * std::f64::consts::PI;
                // Mix two sine waves for more interesting visualization
                let y1 = (t + time_offset * 0.5).sin() * 0.8;
                let y2 = ((t * 2.0) + time_offset).sin() * 0.4;
                let y = (y1 + y2).clamp(-0.95, 0.95);
                (x as f64, y)
            })
            .collect()
    };

    // Draw the waveform with brighter green
    for window in points.windows(2) {
        ctx.draw(&ratatui::widgets::canvas::Line {
            x1: window[0].0,
            y1: window[0].1,
            x2: window[1].0,
            y2: window[1].1,
            color: Color::Rgb(0, 255, 100), // Bright green like old oscilloscopes
        });
    }
}

fn draw_progress_bar(f: &mut Frame, area: Rect, app: &App) {
    let progress = app.playback_position;

    // Format time display
    let time_info = if let Some(duration) = app.duration {
        let current_secs = (duration.as_secs() as f32 * progress) as u64;
        let current_time = format_time(current_secs);
        let total_time = format_duration(duration);

        let mut time_str = format!("{current_time} / {total_time}");

        // Add selection duration if marks are set
        if let Some(selection_duration) = app.get_selection_duration() {
            let sel_secs = selection_duration.as_secs_f32();
            time_str.push_str(&format!(" [{sel_secs:.1}s]"));
        }

        time_str
    } else {
        "00:00 / 00:00".to_string()
    };

    // Create layout for time and progress
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(10),    // Progress bar
            Constraint::Length(20), // Time display (increased for selection info)
        ])
        .split(area);

    // Draw custom progress bar with markers
    draw_progress_with_marks(f, chunks[0], app);

    // Time display
    let time_widget = Paragraph::new(time_info)
        .style(Style::default().fg(Color::White))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::ALL));

    f.render_widget(time_widget, chunks[1]);
}

fn draw_progress_with_marks(f: &mut Frame, area: Rect, app: &App) {
    let progress = app.playback_position;
    let progress_percent = (progress * 100.0) as u16;

    // First draw the gauge
    let label_style = if progress_percent >= 50 {
        Style::default()
            .fg(Color::Black)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let progress_widget = Gauge::default()
        .block(Block::default().borders(Borders::ALL))
        .gauge_style(Style::default().fg(Color::Cyan))
        .percent(progress_percent)
        .label(Span::styled(format!("{progress_percent}%"), label_style));

    f.render_widget(progress_widget, area);

    // Now overlay the markers
    let inner_area = area.inner(ratatui::layout::Margin {
        horizontal: 1,
        vertical: 1,
    });
    let bar_width = inner_area.width;

    // Draw mark in
    if let Some(mark_in) = app.mark_in {
        let mark_x = inner_area.x + (mark_in * bar_width as f32) as u16;
        if mark_x < inner_area.x + bar_width {
            let marker = Paragraph::new("┃").style(
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            );
            let marker_area = Rect {
                x: mark_x,
                y: inner_area.y,
                width: 1,
                height: 1,
            };
            f.render_widget(marker, marker_area);
        }
    }

    // Draw mark out
    if let Some(mark_out) = app.mark_out {
        let mark_x = inner_area.x + (mark_out * bar_width as f32) as u16;
        if mark_x < inner_area.x + bar_width {
            let marker = Paragraph::new("┃")
                .style(Style::default().fg(Color::Red).add_modifier(Modifier::BOLD));
            let marker_area = Rect {
                x: mark_x,
                y: inner_area.y,
                width: 1,
                height: 1,
            };
            f.render_widget(marker, marker_area);
        }
    }

    // Highlight selection region if both marks are set
    if let (Some(mark_in), Some(mark_out)) = (app.mark_in, app.mark_out) {
        let start = mark_in.min(mark_out);
        let end = mark_in.max(mark_out);

        let start_x = (start * bar_width as f32) as u16;
        let end_x = (end * bar_width as f32) as u16;
        let selection_width = end_x.saturating_sub(start_x).max(1);

        if start_x < bar_width {
            let selection_area = Rect {
                x: inner_area.x + start_x,
                y: inner_area.y,
                width: selection_width.min(bar_width - start_x),
                height: 1,
            };

            // Draw selection highlight
            let selection = Block::default().style(Style::default().bg(Color::DarkGray));
            f.render_widget(selection, selection_area);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_time() {
        assert_eq!(format_time(0), "00:00");
        assert_eq!(format_time(59), "00:59");
        assert_eq!(format_time(60), "01:00");
        assert_eq!(format_time(3661), "61:01");
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(std::time::Duration::from_secs(0)), "00:00");
        assert_eq!(
            format_duration(std::time::Duration::from_secs(125)),
            "02:05"
        );
    }

    #[test]
    fn test_get_led_char() {
        assert_eq!(get_led_char(0.0), "○");
        assert_eq!(get_led_char(0.04), "○");
        assert_eq!(get_led_char(0.05), "◐");
        assert_eq!(get_led_char(0.2), "◐");
        assert_eq!(get_led_char(0.3), "●");
        assert_eq!(get_led_char(1.0), "●");
    }

    #[test]
    fn test_get_led_color_left_channel() {
        // Test left channel colors
        assert_eq!(get_led_color(0.01, true), Color::Rgb(20, 100, 20));
        assert_eq!(get_led_color(0.1, true), Color::Rgb(50, 200, 50));
        assert_eq!(get_led_color(0.5, true), Color::Rgb(100, 255, 100));
        assert_eq!(get_led_color(0.95, true), Color::Rgb(255, 100, 100));
    }

    #[test]
    fn test_get_led_color_right_channel() {
        // Test right channel colors
        assert_eq!(get_led_color(0.01, false), Color::Rgb(100, 50, 0));
        assert_eq!(get_led_color(0.1, false), Color::Rgb(200, 100, 0));
        assert_eq!(get_led_color(0.5, false), Color::Rgb(255, 150, 0));
        assert_eq!(get_led_color(0.95, false), Color::Rgb(255, 100, 100));
    }

    #[test]
    fn test_led_clipping_color() {
        // Both channels should show red when clipping
        assert_eq!(
            get_led_color(LED_CLIPPING_LEVEL + 0.01, true),
            Color::Rgb(255, 100, 100)
        );
        assert_eq!(
            get_led_color(LED_CLIPPING_LEVEL + 0.01, false),
            Color::Rgb(255, 100, 100)
        );
    }

    #[test]
    fn test_create_control_button() {
        let button = create_control_button("q", Style::default().fg(Color::Red));
        assert_eq!(button.content, "[q]");
        assert_eq!(button.style.fg, Some(Color::Red));
    }
}
