use std::env;

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Margin, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Padding, Paragraph},
};

use crate::{
    app::App,
    models::CreateField,
    text_edit::line_col_at_cursor,
};

const BG_MAIN: Color = Color::Rgb(0x22, 0x24, 0x36);
const BG_HEADER: Color = Color::Rgb(0x1B, 0x1D, 0x2B);
const BG_LEFT: Color = Color::Rgb(0x1E, 0x20, 0x30);
const BG_RIGHT: Color = Color::Rgb(0x1F, 0x23, 0x35);
const BG_VALUE: Color = Color::Rgb(0x1B, 0x1D, 0x2B);
const BG_ITEM_SELECTED: Color = Color::Rgb(0x2F, 0x33, 0x4D);
const BG_ITEM: Color = Color::Rgb(0x2A, 0x2D, 0x44);
const BORDER: Color = Color::Rgb(0x3B, 0x42, 0x61);
const CYAN: Color = Color::Rgb(0x86, 0xE1, 0xFC);
const FG_TITLE: Color = Color::Rgb(0xC8, 0xD3, 0xF5);
const FG_ACCENT: Color = Color::Rgb(0xFF, 0xC7, 0x77);
const FG_NORMAL: Color = Color::Rgb(0xA9, 0xB8, 0xE8);
const FG_VALUE: Color = Color::Rgb(0xC3, 0xE8, 0x8D);
const FG_LABEL: Color = Color::Rgb(0xF7, 0x8C, 0x6C);
const HEADER_HEIGHT: u16 = 12;

pub fn draw_loading(frame: &mut ratatui::Frame<'_>, spinner: &str, elapsed_secs: u64) {
    frame.render_widget(
        Block::new().style(Style::default().bg(BG_MAIN)),
        frame.area(),
    );

    let loading_area = centered_rect(60, 24, frame.area());
    frame.render_widget(Clear, loading_area);

    let block = Block::new()
        .title(Span::styled(
            " Loading Parameter Store ",
            Style::default().fg(FG_ACCENT).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(BG_HEADER))
        .padding(Padding::new(1, 1, 1, 1));
    let inner = block.inner(loading_area);
    frame.render_widget(block, loading_area);

    let lines = vec![
        Line::from(Span::styled(
            format!("Fetching parameter names from AWS SSM {spinner}"),
            Style::default().fg(CYAN),
        )),
        Line::from(Span::styled(
            format!("Elapsed: {elapsed_secs}s"),
            Style::default().fg(FG_NORMAL),
        )),
        Line::from(Span::styled(
            "Press Ctrl+C to quit",
            Style::default().fg(FG_NORMAL),
        )),
    ];

    frame.render_widget(Paragraph::new(lines), inner);
}

pub fn draw(frame: &mut ratatui::Frame<'_>, app: &App) {
    frame.render_widget(
        Block::new().style(Style::default().bg(BG_MAIN)),
        frame.area(),
    );

    let root = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(HEADER_HEIGHT), Constraint::Min(0)])
        .split(frame.area());

    draw_header(frame, root[0], app);
    draw_body(frame, root[1], app);
    if app.create_mode {
        draw_create_popup(frame, app);
    }
}

fn draw_header(frame: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let header = Block::new()
        .style(Style::default().bg(BG_HEADER))
        .borders(Borders::BOTTOM)
        .border_style(Style::default().fg(BORDER))
        .padding(Padding::new(1, 1, 0, 0));
    let inner = header.inner(area);
    frame.render_widget(header, area);

    let selected = app.selected_parameter();
    let selected_name = selected.map(|p| p.name.as_str()).unwrap_or("-");
    let meta = selected.map(|p| &p.meta);
    let param_type = meta.and_then(|m| m.param_type.as_deref()).unwrap_or("-");
    let version = meta
        .and_then(|m| m.version)
        .map(|v| v.to_string())
        .unwrap_or_else(|| "-".to_string());
    let tier = meta.and_then(|m| m.tier.as_deref()).unwrap_or("-");
    let data_type = meta.and_then(|m| m.data_type.as_deref()).unwrap_or("-");
    let last_modified = meta
        .and_then(|m| m.last_modified_epoch)
        .map(|v| v.to_string())
        .unwrap_or_else(|| "-".to_string());
    let key_id = meta.and_then(|m| m.key_id.as_deref()).unwrap_or("-");
    let description = meta
        .and_then(|m| m.description.as_deref())
        .unwrap_or("No description");
    let last_modified_user = meta
        .and_then(|m| m.last_modified_user.as_deref())
        .unwrap_or("-");

    let profile = env::var("AWS_PROFILE")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "default".to_string());
    let region = app.aws_region.clone();

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(5),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new("AWS SSM PARAMETER STORE")
            .style(Style::default().fg(FG_ACCENT).add_modifier(Modifier::BOLD)),
        rows[0],
    );

    frame.render_widget(Paragraph::new(""), rows[1]);
    frame.render_widget(Paragraph::new(""), rows[2]);

    frame.render_widget(
        Paragraph::new(format!("SELECTED: {selected_name}"))
            .style(Style::default().fg(FG_TITLE).add_modifier(Modifier::BOLD)),
        rows[3],
    );

    let info_cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(rows[4]);

    let left_lines = vec![
        Line::from(vec![
            Span::styled(
                "Name: ",
                Style::default().fg(FG_LABEL).add_modifier(Modifier::BOLD),
            ),
            Span::styled(selected_name.to_string(), Style::default().fg(FG_TITLE)),
        ]),
        Line::from(vec![
            Span::styled(
                "Type: ",
                Style::default().fg(FG_LABEL).add_modifier(Modifier::BOLD),
            ),
            Span::styled(param_type.to_string(), Style::default().fg(CYAN)),
            Span::raw("   "),
            Span::styled(
                "Version: ",
                Style::default().fg(FG_LABEL).add_modifier(Modifier::BOLD),
            ),
            Span::styled(version, Style::default().fg(CYAN)),
            Span::raw("   "),
            Span::styled(
                "Tier: ",
                Style::default().fg(FG_LABEL).add_modifier(Modifier::BOLD),
            ),
            Span::styled(tier.to_string(), Style::default().fg(CYAN)),
        ]),
        Line::from(vec![
            Span::styled(
                "Data type: ",
                Style::default().fg(FG_LABEL).add_modifier(Modifier::BOLD),
            ),
            Span::styled(data_type.to_string(), Style::default().fg(CYAN)),
        ]),
        Line::from(vec![
            Span::styled(
                "Description: ",
                Style::default().fg(FG_LABEL).add_modifier(Modifier::BOLD),
            ),
            Span::styled(description.to_string(), Style::default().fg(FG_NORMAL)),
        ]),
        Line::from(vec![
            Span::styled(
                "User: ",
                Style::default().fg(FG_LABEL).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                last_modified_user.to_string(),
                Style::default().fg(FG_NORMAL),
            ),
            Span::raw("   "),
            Span::styled(
                "Date: ",
                Style::default().fg(FG_LABEL).add_modifier(Modifier::BOLD),
            ),
            Span::styled(last_modified, Style::default().fg(FG_NORMAL)),
            Span::raw("   "),
            Span::styled(
                "KeyId: ",
                Style::default().fg(FG_LABEL).add_modifier(Modifier::BOLD),
            ),
            Span::styled(key_id.to_string(), Style::default().fg(FG_NORMAL)),
        ]),
    ];
    frame.render_widget(Paragraph::new(left_lines), info_cols[0]);

    let right_lines = vec![
        Line::from(vec![
            Span::styled(
                "<a>",
                Style::default().fg(FG_ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Add new Parameter Store"),
        ]),
        Line::from(vec![
            Span::styled(
                "<R>",
                Style::default().fg(FG_ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Refresh all"),
        ]),
        Line::from(vec![
            Span::styled(
                "</>",
                Style::default().fg(FG_ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Search/Filter"),
        ]),
        Line::from(vec![
            Span::styled(
                "<y>",
                Style::default().fg(FG_ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Yank value"),
            Span::raw("   "),
            Span::styled(
                "<e>",
                Style::default().fg(FG_ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Edit"),
            Span::raw("   "),
            Span::styled(
                "<C-c>",
                Style::default().fg(FG_ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" Quit"),
        ]),
        Line::from(Span::styled(
            format!("Profile: {profile}   Region: {region}"),
            Style::default().fg(FG_ACCENT).add_modifier(Modifier::BOLD),
        )),
    ];
    frame.render_widget(
        Paragraph::new(right_lines)
            .style(Style::default().fg(FG_NORMAL))
            .alignment(Alignment::Right),
        info_cols[1],
    );

    frame.render_widget(
        Paragraph::new(format!("Status: {}", app.status)).style(Style::default().fg(FG_NORMAL)),
        rows[5],
    );

    frame.render_widget(
        Paragraph::new("</> search/filter  <R> refresh all  <a> add new  <y> yank  <e> edit  <C-c> quit")
            .style(Style::default().fg(FG_ACCENT).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Right),
        rows[6],
    );
}

fn draw_body(frame: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    draw_left_panel(frame, columns[0], app);
    draw_right_panel(frame, columns[1], app);
}

fn draw_left_panel(frame: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let left = Block::new()
        .style(Style::default().bg(BG_LEFT))
        .borders(Borders::RIGHT)
        .border_style(Style::default().fg(BORDER))
        .padding(Padding::new(1, 1, 0, 0));
    let inner = left.inner(area);
    frame.render_widget(left, area);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "[ PARAMETERS ]",
            Style::default().fg(CYAN).add_modifier(Modifier::BOLD),
        ))),
        layout[0],
    );

    let search_label = if app.search_mode { "SEARCH" } else { "FILTER" };
    let search_line = format!(
        "{search_label}: /{}    Total: {}",
        app.query,
        app.filtered_indices.len()
    );
    let search_style = if app.search_mode {
        Style::default().fg(FG_ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(FG_NORMAL)
    };
    frame.render_widget(Paragraph::new(search_line).style(search_style), layout[1]);

    let view_height = layout[2].height as usize;
    let total = app.filtered_indices.len();

    let items: Vec<ListItem> = if total == 0 {
        vec![
            ListItem::new(Line::from(Span::styled(
                " No parameters matched ",
                Style::default().fg(FG_NORMAL),
            )))
            .style(Style::default().bg(BG_ITEM)),
        ]
    } else {
        let mut start = app.selected.saturating_sub(view_height.saturating_sub(1));
        if start + view_height > total {
            start = total.saturating_sub(view_height);
        }
        let end = (start + view_height).min(total);

        (start..end)
            .map(|global_idx| {
                let source_idx = app.filtered_indices[global_idx];
                let item = app.all_parameters[source_idx].name.as_str();
                let selected = global_idx == app.selected;
                let text_style = if selected {
                    Style::default().fg(FG_ACCENT).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(FG_NORMAL)
                };
                let bg = if selected { BG_ITEM_SELECTED } else { BG_ITEM };
                ListItem::new(Line::from(Span::styled(format!(" {} ", item), text_style)))
                    .style(Style::default().bg(bg))
            })
            .collect()
    };

    frame.render_widget(List::new(items), layout[2]);
}

fn draw_right_panel(frame: &mut ratatui::Frame<'_>, area: Rect, app: &App) {
    let right_bg = Block::new().style(Style::default().bg(BG_RIGHT));
    frame.render_widget(right_bg, area);

    let value_box_area = area.inner(Margin {
        vertical: 1,
        horizontal: 1,
    });
    let value_box = Block::new()
        .style(Style::default().bg(BG_VALUE))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER))
        .padding(Padding::new(1, 1, 1, 1));
    let inner = value_box.inner(value_box_area);
    frame.render_widget(value_box, value_box_area);

    let body = match app.selected_parameter() {
        Some(p) => match p.value.as_deref() {
            Some(value) => value.to_string(),
            None if app.is_value_pending(&p.name) => {
                "Loading value from SSM in background...".to_string()
            }
            None => "Value not loaded yet...".to_string(),
        },
        None => String::from("No selection"),
    };

    frame.render_widget(
        Paragraph::new(body).style(Style::default().fg(FG_VALUE)),
        inner,
    );
}

fn draw_create_popup(frame: &mut ratatui::Frame<'_>, app: &App) {
    let popup_area = centered_rect(78, 62, frame.area());
    frame.render_widget(Clear, popup_area);

    let popup = Block::new()
        .title(Span::styled(
            " Add Parameter Store Entry ",
            Style::default().fg(FG_ACCENT).add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(BG_HEADER))
        .padding(Padding::new(1, 1, 1, 1));
    let inner = popup.inner(popup_area);
    frame.render_widget(popup, popup_area);

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Length(2),
            Constraint::Length(1),
            Constraint::Length(7),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(inner);

    frame.render_widget(
        Paragraph::new("Create in AWS SSM and sync local state").style(Style::default().fg(CYAN)),
        rows[0],
    );

    let name_label_style = if app.create_field == CreateField::Name {
        Style::default().fg(FG_ACCENT).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(FG_NORMAL)
    };
    let name_value_style = if app.create_name.trim().is_empty() {
        Style::default().fg(BORDER)
    } else {
        Style::default().fg(FG_TITLE)
    };

    let name_value = if app.create_name.trim().is_empty() {
        "/prod/service/component/KEY".to_string()
    } else {
        app.create_name.clone()
    };

    let name_cursor = if app.create_field == CreateField::Name {
        "  ◀"
    } else {
        ""
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled("Name : ", name_label_style),
                Span::styled(name_value, name_value_style),
                Span::styled(name_cursor, Style::default().fg(FG_ACCENT)),
            ]),
            Line::from(Span::styled(
                "example: /alston-staging/my-service/env",
                Style::default().fg(BORDER),
            )),
        ]),
        rows[1],
    );

    frame.render_widget(
        Paragraph::new("Value (vim-like editor):").style(Style::default().fg(
            if app.create_field == CreateField::Value {
                FG_ACCENT
            } else {
                FG_NORMAL
            },
        )),
        rows[2],
    );

    let value_block = Block::new()
        .borders(Borders::ALL)
        .border_style(
            Style::default().fg(if app.create_field == CreateField::Value {
                FG_ACCENT
            } else {
                BORDER
            }),
        )
        .style(Style::default().bg(BG_VALUE))
        .padding(Padding::new(1, 1, 0, 0));
    let value_inner = value_block.inner(rows[3]);
    frame.render_widget(value_block, rows[3]);

    let (cursor_line, cursor_col) = line_col_at_cursor(&app.create_value, app.create_value_cursor);
    let all_lines: Vec<&str> = if app.create_value.is_empty() {
        vec![""]
    } else {
        app.create_value.split('\n').collect()
    };
    let visible_height = value_inner.height as usize;
    let start_line = cursor_line.saturating_sub(visible_height.saturating_sub(1));
    let end_line = (start_line + visible_height).min(all_lines.len());
    let display_lines: Vec<Line> = (start_line..end_line)
        .map(|idx| {
            let content = all_lines.get(idx).copied().unwrap_or("");
            Line::from(Span::styled(
                content.to_string(),
                Style::default().fg(FG_VALUE),
            ))
        })
        .collect();
    frame.render_widget(Paragraph::new(display_lines), value_inner);

    if app.create_field == CreateField::Value {
        let cursor_y = value_inner
            .y
            .saturating_add((cursor_line.saturating_sub(start_line)) as u16);
        let cursor_x = value_inner
            .x
            .saturating_add(cursor_col as u16)
            .min(value_inner.right().saturating_sub(1));
        frame.set_cursor_position((cursor_x, cursor_y));
    } else if app.create_field == CreateField::Name {
        let base_x = rows[1].x + 7;
        let cursor_x = base_x
            .saturating_add(app.create_name.chars().count() as u16)
            .min(rows[1].right().saturating_sub(1));
        frame.set_cursor_position((cursor_x, rows[1].y));
    }

    frame.render_widget(
        Paragraph::new(
            "Insert mode: type, Enter=new line, Esc=Normal | Normal mode: h/j/k/l move, i insert, x delete, Enter save, Esc cancel",
        )
        .style(Style::default().fg(BORDER)),
        rows[4],
    );
    frame.render_widget(
        Paragraph::new("Tab switch field, Ctrl+S save, Esc cancel")
            .style(Style::default().fg(FG_NORMAL)),
        rows[5],
    );
    frame.render_widget(
        Paragraph::new("Press Enter on Value to create in AWS SSM")
            .style(Style::default().fg(FG_ACCENT).add_modifier(Modifier::BOLD)),
        rows[6],
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}
