use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Paragraph,
        canvas::{Canvas, Context},
    },
};

use super::app::App;
use super::browser::draw_browser;

pub fn draw(f: &mut Frame, app: &App) {
    let size = f.area();

    // Draw main UI
    draw_main_ui(f, app);

    // Draw browser overlay if active
    if app.browser.is_active {
        draw_browser(f, size, &app.browser);
    }
}

fn draw_main_ui(f: &mut Frame, app: &App) {
    let size = f.area();
    let show_oscilloscope = size.height > 20; // Only show oscilloscope if window is tall enough

    let constraints = if show_oscilloscope {
        vec![
            Constraint::Length(3), // Title
            Constraint::Length(3), // File info + LEDs
            Constraint::Min(10),   // Waveform area
            Constraint::Length(3), // Controls
        ]
    } else {
        vec![
            Constraint::Length(3), // Title
            Constraint::Length(3), // File info + LEDs
            Constraint::Length(3), // Controls
        ]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(constraints)
        .split(size);

    // Title
    let title = Paragraph::new("🎵 ZIM Audio Player")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(title, chunks[0]);

    // File info with LED indicators
    draw_file_info_with_leds(f, chunks[1], app);

    // Oscilloscope visualization (only if window is tall enough)
    if show_oscilloscope {
        draw_oscilloscope(f, chunks[2], app);
    }

    // Controls
    let controls_idx = if show_oscilloscope { 3 } else { 2 };
    let controls = vec![
        if app.is_playing {
            Span::styled("[space]", Style::default().fg(Color::Yellow))
        } else {
            Span::styled("[space]", Style::default().fg(Color::Green))
        },
        Span::raw(if app.is_playing {
            " pause  "
        } else {
            " play  "
        }),
        Span::styled("[/]", Style::default().fg(Color::Blue)),
        Span::raw(" browse  "),
        Span::styled("[q]", Style::default().fg(Color::Red)),
        Span::raw(" quit"),
    ];
    let controls_widget = Paragraph::new(Line::from(controls))
        .alignment(Alignment::Center)
        .block(Block::default().borders(Borders::TOP));
    f.render_widget(controls_widget, chunks[controls_idx]);
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
        format!("Loaded: {}", filename)
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
    if level < 0.05 {
        "○" // Empty circle
    } else if level < 0.3 {
        "◐" // Half filled
    } else {
        "●" // Full circle
    }
}

fn get_led_color(level: f32, is_left: bool) -> Color {
    let base_color = if is_left {
        // Green for left channel
        if level > 0.9 {
            Color::Rgb(255, 100, 100) // Red when clipping
        } else if level > 0.3 {
            Color::Rgb(100, 255, 100) // Bright green
        } else if level > 0.05 {
            Color::Rgb(50, 200, 50) // Medium green
        } else {
            Color::Rgb(20, 100, 20) // Dim green
        }
    } else {
        // Red/orange for right channel
        if level > 0.9 {
            Color::Rgb(255, 50, 50) // Bright red when clipping
        } else if level > 0.3 {
            Color::Rgb(255, 150, 0) // Orange
        } else if level > 0.05 {
            Color::Rgb(200, 100, 0) // Dim orange
        } else {
            Color::Rgb(100, 50, 0) // Very dim
        }
    };
    base_color
}

fn draw_oscilloscope(f: &mut Frame, area: Rect, app: &App) {
    // Create oscilloscope-style canvas
    let canvas = Canvas::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan))
                .title(" Oscilloscope ")
                .title_style(
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
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
    let grid_color = Color::Rgb(0, 60, 30); // Dark green

    // Vertical grid lines
    for x in (0..area.width).step_by(10) {
        ctx.draw(&ratatui::widgets::canvas::Line {
            x1: x as f64,
            y1: -1.0,
            x2: x as f64,
            y2: 1.0,
            color: grid_color,
        });
    }

    // Horizontal grid lines
    for y in [-0.75, -0.5, -0.25, 0.0, 0.25, 0.5, 0.75] {
        ctx.draw(&ratatui::widgets::canvas::Line {
            x1: 0.0,
            y1: y,
            x2: area.width as f64,
            y2: y,
            color: grid_color,
        });
    }
}

fn draw_waveform(ctx: &mut Context, area: Rect, app: &App) {
    // Get samples from the waveform buffer
    let samples = app.waveform_buffer.get_display_samples(area.width as usize);

    let points: Vec<(f64, f64)> = if samples.iter().any(|&s| s != 0.0) {
        // Use real audio data
        samples
            .iter()
            .enumerate()
            .map(|(i, &sample)| (i as f64, sample.clamp(-0.9, 0.9) as f64))
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
                let y1 = (t + time_offset * 0.5).sin() * 0.6;
                let y2 = ((t * 2.0) + time_offset).sin() * 0.3;
                let y = (y1 + y2).clamp(-0.9, 0.9);
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
