pub mod app;

use crate::todo::NoteEnum;
use crate::ui::app::{App, EditorMode, EMPTY_LIST, EMPTY_NOTE_VEC};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Tabs},
    Frame, Terminal,
};

pub fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App<'_>,
) -> Result<(), Box<dyn std::error::Error>>
where
    B::Error: 'static,
{
    loop {
        terminal.draw(|frame| ui(frame, &mut app))?;

        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind == KeyEventKind::Release {
            continue;
        }

        if let Some(conflict) = app.save_conflict.clone() {
            match key.code {
                KeyCode::Char('o') => {
                    app.overwrite_conflict();
                    match app.save() {
                        Ok(true) => return Ok(()),
                        Ok(false) => {}
                        Err(error) => {
                            app.status_message = Some(format!("Save failed: {error}"));
                        }
                    }
                }
                KeyCode::Char('r')
                    if conflict.kind != crate::ui::app::SaveConflictKind::DestinationExists =>
                {
                    if let Err(error) = app.reload_conflict() {
                        app.save_conflict = Some(conflict);
                        app.status_message = Some(format!("Could not reload file: {error}"));
                    }
                }
                KeyCode::Esc => app.cancel_conflict(),
                _ => {}
            }
            continue;
        }

        match app.mode {
            EditorMode::Nothing => match key.code {
                KeyCode::Char('o') => app.create_note(),
                KeyCode::Char('u') => app.create_file(),
                KeyCode::Char('i') => app.create_list(),
                KeyCode::Char('r') => app.remove(),
                KeyCode::Char('e') => {
                    if key.modifiers == KeyModifiers::CONTROL {
                        app.change_description();
                    } else {
                        app.change();
                    }
                }
                KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => return Ok(()),
                KeyCode::Char('q') => match app.save() {
                    Ok(true) => return Ok(()),
                    Ok(false) => {}
                    Err(error) => app.status_message = Some(format!("Save failed: {error}")),
                },
                KeyCode::Char('c') if key.modifiers == KeyModifiers::CONTROL => return Ok(()),
                KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('l') => app.next(),
                KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('h') => app.previous(),
                KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('k') => app.navigate_up(),
                KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('j') => app.navigate_down(),
                KeyCode::Esc if app.cursor_vertical == 2 => {
                    app.note_index = 0;
                    app.navigate_up();
                }
                KeyCode::Enter | KeyCode::Char(' ') if app.cursor_vertical == 2 => {
                    app.cycle_note_state();
                }
                KeyCode::Char('x' | '+') if app.cursor_vertical == 2 => {
                    app.set_note_state(NoteEnum::Done);
                }
                KeyCode::Char('-') if app.cursor_vertical == 2 => {
                    app.set_note_state(NoteEnum::Rejected);
                }
                _ => {}
            },
            _ => match key.code {
                KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => return Ok(()),
                KeyCode::Char('c') if key.modifiers == KeyModifiers::CONTROL => return Ok(()),
                KeyCode::Char('a') if key.modifiers == KeyModifiers::CONTROL => {
                    app.move_input_home()
                }
                KeyCode::Char('e') if key.modifiers == KeyModifiers::CONTROL => {
                    app.move_input_end()
                }
                KeyCode::Left => app.move_input_left(),
                KeyCode::Right => app.move_input_right(),
                KeyCode::Home => app.move_input_home(),
                KeyCode::End => app.move_input_end(),
                KeyCode::Backspace => app.backspace_input(),
                KeyCode::Delete => app.delete_input(),
                KeyCode::Enter => app.handle_enter(),
                KeyCode::Esc => {
                    app.mode = EditorMode::Nothing;
                    app.input.clear();
                    app.input_cursor = 0;
                }
                KeyCode::Char(character)
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT) =>
                {
                    app.insert_input_char(character)
                }
                _ => {}
            },
        }
    }
}

pub fn ui(frame: &mut Frame<'_>, app: &mut App<'_>) {
    let area = frame.area();
    let chunks = if app.mode != EditorMode::Nothing {
        Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints(
                [
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(0),
                    Constraint::Length(3),
                ]
                .as_ref(),
            )
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints(
                [
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(0),
                ]
                .as_ref(),
            )
            .split(area)
    };

    frame.render_widget(
        Block::default().style(Style::default().bg(Color::Rgb(31, 41, 55)).fg(Color::White)),
        area,
    );

    let current_list = app.lists.get(app.file_index).unwrap_or(EMPTY_LIST);
    let mut list_tabs = Tabs::new(make_tab_items(&current_list.titles))
        .block(Block::default().borders(Borders::ALL).title("Lists"))
        .select(
            app.list_index
                .min(current_list.titles.len().saturating_sub(1)),
        )
        .style(Style::default().fg(Color::Cyan));
    if app.cursor_vertical == 1 {
        list_tabs = list_tabs.highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(Color::DarkGray),
        );
    }
    frame.render_widget(list_tabs, chunks[1]);

    let mut file_tabs = Tabs::new(make_tab_items(app.files))
        .block(Block::default().borders(Borders::ALL).title("Files"))
        .select(app.file_index.min(app.files.len().saturating_sub(1)))
        .style(Style::default().fg(Color::Cyan));
    if app.cursor_vertical == 0 {
        file_tabs = file_tabs.highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(Color::DarkGray),
        );
    }
    frame.render_widget(file_tabs, chunks[0]);

    let notes = current_list
        .notes
        .get(app.list_index)
        .unwrap_or(EMPTY_NOTE_VEC);
    let items = notes
        .iter()
        .enumerate()
        .map(|(index, note)| {
            let content = if index == app.note_index && app.cursor_vertical == 2 {
                note.to_string_custom(">")
            } else {
                note.to_string()
            };
            ListItem::new(Line::from(content)).style(Style::default().fg(Color::White))
        })
        .collect::<Vec<_>>();
    let description = current_list
        .descriptions
        .get(app.list_index)
        .map(String::as_str)
        .unwrap_or("");
    let note_list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(description))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );
    frame.render_stateful_widget(note_list, chunks[2], &mut app.notes_state);

    if app.mode != EditorMode::Nothing {
        let input_width = chunks[3].width.saturating_sub(2) as usize;
        let cursor_cell = app.input_cursor_display_width();
        let horizontal_scroll = cursor_cell.saturating_sub(input_width.saturating_sub(1));
        let input = Paragraph::new(app.input.as_str())
            .scroll((horizontal_scroll.min(u16::MAX as usize) as u16, 0))
            .style(Style::default().fg(Color::White))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(match app.mode {
                        EditorMode::CreateFile => "Create New File",
                        EditorMode::CreateList => "Create New List",
                        EditorMode::CreateNote => "Create New Note",
                        EditorMode::ChangeFileName => "Change File Name",
                        EditorMode::ChangeListName => "Change List Name",
                        EditorMode::ChangeListDescription => "Change List Description",
                        EditorMode::ChangeNoteContent => "Change Note Content",
                        EditorMode::Nothing => "",
                    })
                    .style(Style::default().fg(Color::LightCyan)),
            );
        frame.render_widget(input, chunks[3]);
        if chunks[3].width >= 2 && chunks[3].height >= 2 {
            let cursor_x = chunks[3].x.saturating_add(1).saturating_add(
                cursor_cell
                    .saturating_sub(horizontal_scroll)
                    .min(input_width) as u16,
            );
            let cursor_y = chunks[3].y.saturating_add(1);
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }

    if area.height > 0 {
        let message = app
            .save_conflict
            .as_ref()
            .map(|conflict| {
                let action = if conflict.kind == crate::ui::app::SaveConflictKind::DestinationExists
                {
                    "o overwrite, Esc cancel"
                } else {
                    "r reload, o overwrite, Esc cancel"
                };
                format!("Save conflict at {}. {action}", conflict.path.display())
            })
            .or_else(|| app.status_message.clone());
        if let Some(message) = message {
            let status = Paragraph::new(message).style(
                Style::default()
                    .fg(Color::Yellow)
                    .bg(Color::Rgb(31, 41, 55)),
            );
            frame.render_widget(
                status,
                ratatui::layout::Rect::new(0, area.height - 1, area.width, 1),
            );
        }
    }
}

pub fn make_tab_items(values: &[String]) -> Vec<Line<'static>> {
    values
        .iter()
        .map(|value| {
            let split_at = value
                .char_indices()
                .nth(1)
                .map_or(value.len(), |(index, _)| index);
            let (first, rest) = value.split_at(split_at);
            Line::from(vec![
                Span::styled(first.to_owned(), Style::default().fg(Color::Yellow)),
                Span::styled(rest.to_owned(), Style::default().fg(Color::Green)),
            ])
        })
        .collect()
}
