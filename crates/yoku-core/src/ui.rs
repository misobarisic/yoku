pub mod app;

use crate::todo::NoteEnum;
use crate::ui::app::{App, EditorMode, SaveConflictKind, EMPTY_LIST, EMPTY_NOTE_VEC};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Tabs, Wrap},
    Frame, Terminal,
};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

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

        if app.show_help {
            if matches!(key.code, KeyCode::Esc | KeyCode::F(1) | KeyCode::Char('?')) {
                app.show_help = false;
            }
            continue;
        }

        if app.confirm_discard {
            match key.code {
                KeyCode::Char('y') | KeyCode::Enter => return Ok(()),
                KeyCode::Esc | KeyCode::Char('n') => app.cancel_discard(),
                _ => {}
            }
            continue;
        }

        if app.pending_file_delete.is_some() {
            match key.code {
                KeyCode::Char('y') | KeyCode::Enter => app.confirm_file_delete(),
                KeyCode::Esc | KeyCode::Char('n') => app.cancel_file_delete(),
                _ => {}
            }
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

        if app.mode == EditorMode::Nothing
            && key.code == KeyCode::Char('z')
            && key.modifiers == KeyModifiers::CONTROL
        {
            app.undo_last_delete();
            continue;
        }

        match app.mode {
            EditorMode::Nothing => match key.code {
                KeyCode::Char('/') => app.begin_search(),
                KeyCode::Char('?') | KeyCode::F(1) => app.show_help = true,
                KeyCode::Char('n') => app.search_next(true),
                KeyCode::Char('N') => app.search_next(false),
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
                KeyCode::Char('q')
                    if key.modifiers == KeyModifiers::CONTROL
                        && !app.begin_discard_confirmation() =>
                {
                    return Ok(())
                }
                KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => {}
                KeyCode::Char('q') => match app.save() {
                    Ok(true) => return Ok(()),
                    Ok(false) => {}
                    Err(error) => app.status_message = Some(format!("Save failed: {error}")),
                },
                KeyCode::Char('c')
                    if key.modifiers == KeyModifiers::CONTROL
                        && !app.begin_discard_confirmation() =>
                {
                    return Ok(())
                }
                KeyCode::Char('c') if key.modifiers == KeyModifiers::CONTROL => {}
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
                KeyCode::Char('q')
                    if key.modifiers == KeyModifiers::CONTROL
                        && !app.begin_discard_confirmation() =>
                {
                    return Ok(())
                }
                KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => {}
                KeyCode::Char('c')
                    if key.modifiers == KeyModifiers::CONTROL
                        && !app.begin_discard_confirmation() =>
                {
                    return Ok(())
                }
                KeyCode::Char('c') if key.modifiers == KeyModifiers::CONTROL => {}
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
    frame.render_widget(
        Block::default().style(Style::default().bg(Color::Rgb(31, 41, 55)).fg(Color::White)),
        area,
    );
    if area.width == 0 || area.height == 0 {
        return;
    }
    if app.show_help {
        render_help(frame, area);
        return;
    }

    let editing = app.mode != EditorMode::Nothing;
    let minimum_height = if editing { 13 } else { 10 };
    if area.width < 20 || area.height < minimum_height {
        render_compact(frame, area, app);
        render_status(frame, area, app);
        return;
    }

    let content_area = Rect::new(0, 0, area.width, area.height.saturating_sub(1));
    let chunks = if editing {
        Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(
                [
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(1),
                    Constraint::Length(3),
                ]
                .as_ref(),
            )
            .split(content_area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(
                [
                    Constraint::Length(3),
                    Constraint::Length(3),
                    Constraint::Min(1),
                ]
                .as_ref(),
            )
            .split(content_area)
    };

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
                        EditorMode::Search => "Search",
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
            if cursor_x < chunks[3].x.saturating_add(chunks[3].width)
                && cursor_y < chunks[3].y.saturating_add(chunks[3].height)
            {
                frame.set_cursor_position((cursor_x, cursor_y));
            }
        }
    }

    render_status(frame, area, app);
}

fn render_status(frame: &mut Frame<'_>, area: Rect, app: &App<'_>) {
    if area.height == 0 {
        return;
    }
    let message = app
        .confirm_discard
        .then(|| "Unsaved changes. Press y to discard or Esc to keep editing".to_owned())
        .or_else(|| {
            app.pending_file_delete.map(|index| {
                format!(
                    "Delete file '{}'? Press y/Enter to confirm or Esc to cancel",
                    app.files
                        .get(index)
                        .map(String::as_str)
                        .unwrap_or("unknown")
                )
            })
        })
        .or_else(|| {
            app.save_conflict.as_ref().map(|conflict| {
                let action = if conflict.kind == SaveConflictKind::DestinationExists {
                    "o overwrite, Esc cancel"
                } else {
                    "r reload, o overwrite, Esc cancel"
                };
                format!("Save conflict at {}. {action}", conflict.path.display())
            })
        })
        .or_else(|| app.status_message.clone())
        .unwrap_or_else(|| "?: help  /: search  Ctrl+Z: undo  q: save and quit".into());
    let status = Paragraph::new(message).style(
        Style::default()
            .fg(Color::Yellow)
            .bg(Color::Rgb(31, 41, 55)),
    );
    frame.render_widget(status, Rect::new(0, area.height - 1, area.width, 1));
}

fn render_compact(frame: &mut Frame<'_>, area: Rect, app: &App<'_>) {
    let list = app.lists.get(app.file_index).unwrap_or(EMPTY_LIST);
    let filename = app
        .files
        .get(app.file_index)
        .map(String::as_str)
        .unwrap_or("(no file)");
    let title = list
        .titles
        .get(app.list_index)
        .map(String::as_str)
        .unwrap_or("(no list)");
    let note = list
        .notes
        .get(app.list_index)
        .and_then(|notes| notes.get(app.note_index))
        .map(|note| note.to_string())
        .unwrap_or_else(|| "(no task)".into());
    let body_height = area.height.saturating_sub(1);
    if app.mode != EditorMode::Nothing {
        let context = format!("{filename} / {title}");
        if body_height > 1 {
            frame.render_widget(
                Paragraph::new(context)
                    .wrap(Wrap { trim: true })
                    .style(Style::default().fg(Color::White)),
                Rect::new(0, 0, area.width, body_height - 1),
            );
        }
        let label = match app.mode {
            EditorMode::CreateFile => "File",
            EditorMode::CreateList => "List",
            EditorMode::CreateNote => "Task",
            EditorMode::ChangeFileName => "File",
            EditorMode::ChangeListName => "List",
            EditorMode::ChangeListDescription => "Description",
            EditorMode::ChangeNoteContent => "Task",
            EditorMode::Search => "Search",
            EditorMode::Nothing => "Input",
        };
        let prefix = format!("{label}: ");
        let line = format!("{prefix}{}", app.input);
        let cursor_cell =
            UnicodeWidthStr::width(prefix.as_str()) + app.input_cursor_display_width();
        let visible_width = area.width as usize;
        let horizontal_scroll = cursor_cell.saturating_sub(visible_width.saturating_sub(1));
        let input_row = body_height.saturating_sub(1);
        frame.render_widget(
            Paragraph::new(line)
                .scroll((horizontal_scroll.min(u16::MAX as usize) as u16, 0))
                .style(Style::default().fg(Color::White).bg(Color::DarkGray)),
            Rect::new(0, input_row, area.width, 1),
        );
        if body_height > 0 && area.width > 0 {
            let cursor_x = cursor_cell
                .saturating_sub(horizontal_scroll)
                .min(visible_width.saturating_sub(1)) as u16;
            frame.set_cursor_position((cursor_x, input_row));
        }
        return;
    }
    let content = vec![
        Line::from(format!("File: {filename}")),
        Line::from(format!("List: {title}")),
        Line::from(format!("Task: {note}")),
        Line::from("Resize terminal for full view"),
    ];
    frame.render_widget(
        Paragraph::new(content)
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::White)),
        Rect::new(0, 0, area.width, body_height),
    );
}

fn render_help(frame: &mut Frame<'_>, area: Rect) {
    let width = area.width.min(72);
    let height = area.height.min(20);
    let rect = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    let help = "Navigation\n  Arrows or h/j/k/l or WASD move between files, lists, and tasks.\n\nEditing\n  e edit the selected item; Ctrl+E edits a list description.\n  u create a file, i create a list, o create a task.\n  Enter/Space toggles a task. r deletes; Ctrl+Z undoes the last deletion.\n\nSearch and save\n  / searches names, list titles, descriptions, and tasks. n/N moves through matches.\n  q saves and quits. Ctrl+Q or Ctrl+C asks before discarding changes.\n  F1 or ? opens this help. Esc closes help or cancels an editor.";
    frame.render_widget(Clear, rect);
    frame.render_widget(
        Paragraph::new(help)
            .wrap(Wrap { trim: true })
            .block(Block::default().borders(Borders::ALL).title("Yoku help"))
            .style(Style::default().fg(Color::White)),
        rect,
    );
}

pub fn make_tab_items(values: &[String]) -> Vec<Line<'static>> {
    values
        .iter()
        .map(|value| {
            let split_at = value
                .grapheme_indices(true)
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

#[cfg(test)]
mod tests {
    use super::{make_tab_items, ui};
    use crate::todo::{FileList, Note, NoteEnum};
    use crate::ui::app::{App, EditorMode};
    use ratatui::{backend::TestBackend, Terminal};
    use std::collections::HashMap;

    #[test]
    fn layout_renders_after_tiny_and_large_resizes() {
        let mut files = Vec::new();
        let mut paths = Vec::new();
        let mut lists = Vec::new();
        let mut hashes = HashMap::new();
        let mut disk_hashes = HashMap::new();
        let mut removed = Vec::new();
        let mut app = App::new(
            &mut files,
            &mut paths,
            &mut lists,
            &mut hashes,
            &mut disk_hashes,
            std::path::Path::new("."),
            &mut removed,
        );

        for (width, height) in [(1, 1), (10, 5), (19, 14), (80, 24)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal.draw(|frame| ui(frame, &mut app)).unwrap();
        }
        app.show_help = true;
        let mut terminal = Terminal::new(TestBackend::new(8, 4)).unwrap();
        terminal.draw(|frame| ui(frame, &mut app)).unwrap();
    }

    #[test]
    fn long_tabs_and_task_lists_render_without_panicking() {
        let mut files = vec!["📚".repeat(200)];
        let mut paths = vec![std::path::PathBuf::from("long.md")];
        let mut lists = vec![FileList::from_parts(
            vec!["🧑‍💻 planning".repeat(100)],
            vec!["details".repeat(100)],
            vec![(0..500)
                .map(|index| Note {
                    content: format!("task {index}"),
                    state: NoteEnum::Open,
                })
                .collect()],
        )];
        let mut hashes = HashMap::new();
        let mut disk_hashes = HashMap::new();
        let mut removed = Vec::new();
        let mut app = App::new(
            &mut files,
            &mut paths,
            &mut lists,
            &mut hashes,
            &mut disk_hashes,
            std::path::Path::new("."),
            &mut removed,
        );
        let mut terminal = Terminal::new(TestBackend::new(30, 12)).unwrap();
        terminal.draw(|frame| ui(frame, &mut app)).unwrap();
    }

    #[test]
    fn tab_split_keeps_a_full_emoji_grapheme_together() {
        let tabs = make_tab_items(&["👩‍💻 tools".to_owned()]);
        assert_eq!(tabs[0].spans[0].content, "👩‍💻");
        assert_eq!(tabs[0].spans[1].content, " tools");
    }

    #[test]
    fn compact_editor_layout_keeps_input_visible() {
        let mut files = vec!["work".into()];
        let mut paths = Vec::new();
        let mut lists = vec![FileList::default()];
        let mut hashes = HashMap::new();
        let mut disk_hashes = HashMap::new();
        let mut removed = Vec::new();
        let mut app = App::new(
            &mut files,
            &mut paths,
            &mut lists,
            &mut hashes,
            &mut disk_hashes,
            std::path::Path::new("."),
            &mut removed,
        );
        app.mode = EditorMode::CreateNote;
        app.input = "draft note".into();
        app.input_cursor = 5;
        let mut terminal = Terminal::new(TestBackend::new(15, 7)).unwrap();
        terminal.draw(|frame| ui(frame, &mut app)).unwrap();
    }
}
