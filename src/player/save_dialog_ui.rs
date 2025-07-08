use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use super::save_dialog::{SaveDialog, SaveDialogFocus};

pub fn draw_save_dialog(f: &mut Frame, area: Rect, dialog: &SaveDialog) {
    // Create a centered modal
    let modal_width = 60.min(area.width - 4);
    let modal_height = 20.min(area.height - 4);

    let modal_area = Rect {
        x: (area.width - modal_width) / 2,
        y: (area.height - modal_height) / 2,
        width: modal_width,
        height: modal_height,
    };

    // Clear the background
    f.render_widget(Clear, modal_area);

    // Draw the modal border
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::White))
        .title(if dialog.has_selection {
            " Save Selection As "
        } else {
            " Save As "
        })
        .title_style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        );
    f.render_widget(block, modal_area);

    let inner_area = modal_area.inner(ratatui::layout::Margin {
        horizontal: 1,
        vertical: 1,
    });

    // Create layout
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2), // Current path
            Constraint::Min(5),    // Directory list
            Constraint::Length(3), // Filename field
            Constraint::Length(2), // Controls
        ])
        .split(inner_area);

    // Current path
    let path_display = format!("📁 {}", dialog.current_path.display());
    let path_widget = Paragraph::new(path_display)
        .style(Style::default().fg(Color::Blue))
        .block(Block::default().borders(Borders::BOTTOM));
    f.render_widget(path_widget, chunks[0]);

    // Directory list
    let dirs: Vec<ListItem> = dialog
        .directories
        .iter()
        .enumerate()
        .map(|(i, dir)| {
            let is_selected =
                i == dialog.selected_index && dialog.focus == SaveDialogFocus::DirectoryList;
            let style = if is_selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            let prefix = if dir == ".." { "↑ " } else { "📁 " };
            ListItem::new(format!("{}{}", prefix, dir)).style(style)
        })
        .collect();

    let dirs_list = List::new(dirs).block(Block::default().borders(Borders::NONE));
    f.render_widget(dirs_list, chunks[1]);

    // Filename field
    let filename_block = Block::default()
        .borders(Borders::ALL)
        .border_style(if dialog.focus == SaveDialogFocus::FilenameField {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        })
        .title(" Filename ");

    let filename_widget = Paragraph::new(dialog.filename.as_str())
        .style(Style::default().fg(Color::White))
        .block(filename_block);
    f.render_widget(filename_widget, chunks[2]);

    // Show cursor in filename field if focused
    if dialog.focus == SaveDialogFocus::FilenameField {
        let cursor_x = chunks[2].x + 1 + dialog.filename.len() as u16;
        let cursor_y = chunks[2].y + 1;
        if cursor_x < chunks[2].x + chunks[2].width - 1 {
            f.set_cursor_position((cursor_x, cursor_y));
        }
    }

    // Controls
    let controls = vec![
        Span::styled("[Tab]", Style::default().fg(Color::Yellow)),
        Span::raw(" switch  "),
        Span::styled("[Enter]", Style::default().fg(Color::Green)),
        Span::raw(" save  "),
        Span::styled("[Esc]", Style::default().fg(Color::Red)),
        Span::raw(" cancel"),
    ];
    let controls_widget = Paragraph::new(Line::from(controls)).alignment(Alignment::Center);
    f.render_widget(controls_widget, chunks[3]);
}
