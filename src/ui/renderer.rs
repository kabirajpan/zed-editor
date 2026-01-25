use crate::ui::app::App;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub fn render(app: &App, frame: &mut Frame) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Title bar
            Constraint::Min(0),    // Editor area
            Constraint::Length(1), // Status bar
        ])
        .split(frame.area());

    // Title bar
    render_title_bar(frame, chunks[0]);

    // Editor content
    render_editor(app, frame, chunks[1]);

    // Status bar
    render_status_bar(app, frame, chunks[2]);
}

fn render_title_bar(frame: &mut Frame, area: Rect) {
    let title = Line::from(vec![
        Span::styled(
            " Zed",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("-"),
        Span::styled(
            "Editor",
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" "),
    ]);

    let title_bar = Paragraph::new(title).style(Style::default().bg(Color::DarkGray));

    frame.render_widget(title_bar, area);
}

fn render_editor(app: &App, frame: &mut Frame, area: Rect) {
    let text = app.editor.text();
    let cursor = app.editor.cursor();

    let lines: Vec<Line> = text
        .lines()
        .enumerate()
        .map(|(row_idx, line)| {
            let line_num = format!("{:4} ", row_idx + 1);

            // If this is the cursor line, render with block cursor
            if row_idx == cursor.row {
                let mut spans = vec![Span::styled(line_num, Style::default().fg(Color::DarkGray))];

                // Split line at cursor position
                let chars: Vec<char> = line.chars().collect();

                // Before cursor
                if cursor.column > 0 {
                    let before: String = chars.iter().take(cursor.column).collect();
                    spans.push(Span::raw(before));
                }

                // Cursor character (block cursor)
                if cursor.column < chars.len() {
                    let cursor_char = chars[cursor.column].to_string();
                    spans.push(Span::styled(
                        cursor_char,
                        Style::default()
                            .bg(Color::White)
                            .fg(Color::Black)
                            .add_modifier(Modifier::BOLD),
                    ));
                } else {
                    // Cursor at end of line - show space
                    spans.push(Span::styled(
                        " ",
                        Style::default().bg(Color::White).fg(Color::Black),
                    ));
                }

                // After cursor
                if cursor.column + 1 < chars.len() {
                    let after: String = chars.iter().skip(cursor.column + 1).collect();
                    spans.push(Span::raw(after));
                }

                Line::from(spans)
            } else {
                // Regular line (no cursor)
                Line::from(vec![
                    Span::styled(line_num, Style::default().fg(Color::DarkGray)),
                    Span::raw(line),
                ])
            }
        })
        .collect();

    // If empty document, show cursor on first line
    let lines = if lines.is_empty() {
        vec![Line::from(vec![
            Span::styled("   1 ", Style::default().fg(Color::DarkGray)),
            Span::styled(" ", Style::default().bg(Color::White).fg(Color::Black)),
        ])]
    } else {
        lines
    };

    let editor_widget =
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Document"));

    frame.render_widget(editor_widget, area);
}

fn render_status_bar(app: &App, frame: &mut Frame, area: Rect) {
    let cursor = app.editor.cursor();
    let line_count = app.editor.line_count();

    let status_text = if !app.status_message.is_empty() {
        app.status_message.clone()
    } else {
        format!(
            " Line {}, Col {} | {} lines | {} chars | Ctrl+Z: Undo | Ctrl+Y: Redo | Ctrl+Q: Quit",
            cursor.row + 1,
            cursor.column + 1,
            line_count,
            app.editor.text().len(),
        )
    };

    let status_bar =
        Paragraph::new(status_text).style(Style::default().bg(Color::DarkGray).fg(Color::White));

    frame.render_widget(status_bar, area);
}
