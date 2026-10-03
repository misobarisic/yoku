pub mod app;
mod editor;

use crate::query::TaskView;
use crate::todo::NoteEnum;
use crate::ui::app::{App, EditorMode, EMPTY_LIST};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Tabs, Wrap},
    Frame, Terminal,
};
use std::time::{Duration, Instant};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

pub fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    mut app: App<'_>,
) -> Result<(), Box<dyn std::error::Error>>
where
    B::Error: 'static,
{
    let mut refreshed_at = Instant::now();
    loop {
        if app.editor_requested && app.save_conflict.is_none() && !app.has_unsaved_changes() {
            app.editor_requested = false;
            if let Some(path) = app.paths.get(app.file_index).cloned() {
                let editor_result = editor::edit_file(terminal, &path);
                let refresh_result = app.refresh();
                match (editor_result, refresh_result) {
                    (Err(error), _) => {
                        app.status_message = Some(format!("Could not edit file: {error}"))
                    }
                    (_, Err(error)) => {
                        app.status_message =
                            Some(format!("Could not reload editor changes: {error}"))
                    }
                    (Ok(()), Ok(report)) if !report.changed() => {
                        app.status_message = Some("Editor closed; no file changes".into())
                    }
                    _ => {}
                }
            }
        }
        if refreshed_at.elapsed() >= Duration::from_secs(2)
            && app.mode == EditorMode::Nothing
            && app.save_conflict.is_none()
            && app.move_picker.is_none()
            && app.pending_file_delete.is_none()
            && !app.confirm_discard
            && !app.show_help
        {
            if let Err(error) = app.refresh() {
                app.status_message = Some(format!("Refresh failed: {error}"));
            }
            refreshed_at = Instant::now();
        }
        terminal.draw(|frame| ui(frame, &mut app))?;

        if !event::poll(Duration::from_secs(1))? {
            continue;
        }

        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind == KeyEventKind::Release {
            continue;
        }

        if app.show_help {
            match key.code {
                KeyCode::Esc | KeyCode::F(1) | KeyCode::Char('?') => app.show_help = false,
                KeyCode::Up | KeyCode::Char('k') => {
                    app.help_scroll = app.help_scroll.saturating_sub(1)
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    app.help_scroll = app.help_scroll.saturating_add(1)
                }
                KeyCode::PageUp => app.help_scroll = app.help_scroll.saturating_sub(10),
                KeyCode::PageDown => app.help_scroll = app.help_scroll.saturating_add(10),
                KeyCode::Home => app.help_scroll = 0,
                KeyCode::End => app.help_scroll = u16::MAX,
                _ => {}
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

        if app.move_picker.is_some() {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => app.select_move_destination(false),
                KeyCode::Down | KeyCode::Char('j') => app.select_move_destination(true),
                KeyCode::Enter => app.confirm_move(),
                KeyCode::Esc => app.move_picker = None,
                _ => {}
            }
            continue;
        }

        if let Some(conflict) = app.save_conflict.clone() {
            match key.code {
                KeyCode::Char('o') => {
                    app.overwrite_conflict();
                    match app.retry_save() {
                        Ok(true) => return Ok(()),
                        Ok(false) => {}
                        Err(error) => {
                            app.status_message = Some(format!("Save failed: {error}"));
                        }
                    }
                }
                KeyCode::Char('r') => {
                    if let Err(error) = app.reload_conflict() {
                        app.save_conflict = Some(conflict);
                        app.status_message = Some(format!("Could not reload file: {error}"));
                    } else if app.editor_requested {
                        if let Err(error) = app.begin_external_edit() {
                            app.status_message = Some(format!("Save failed: {error}"));
                        }
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
            app.undo();
            continue;
        }

        if app.mode == EditorMode::Nothing
            && (key.code == KeyCode::Char('y') && key.modifiers == KeyModifiers::CONTROL
                || key.code == KeyCode::Char('Z') && key.modifiers.contains(KeyModifiers::CONTROL))
        {
            app.redo();
            continue;
        }

        if app.mode == EditorMode::Nothing
            && key.code == KeyCode::Char('s')
            && key.modifiers == KeyModifiers::CONTROL
        {
            if let Err(error) = app.request_save(false) {
                app.status_message = Some(format!("Save failed: {error}"));
            }
            continue;
        }

        match app.mode {
            EditorMode::Nothing => match key.code {
                KeyCode::Char('/') => app.begin_search(),
                KeyCode::Char('f') => app.cycle_task_filter(),
                KeyCode::Char('g') => app.toggle_global_view(),
                KeyCode::Char('F') => app.begin_filter(),
                KeyCode::Char('S') => app.cycle_task_sort(),
                KeyCode::Char('t') => app.show_agenda(false),
                KeyCode::Char('v') => app.show_agenda(true),
                KeyCode::Char('E') => {
                    if let Err(error) = app.begin_external_edit() {
                        app.status_message = Some(format!("Save failed: {error}"));
                    }
                }
                KeyCode::Char('R') => match app.refresh() {
                    Ok(report) if !report.changed() => {
                        app.status_message = Some(
                            if report.conflicts > 0 {
                                "Edited files changed externally; Ctrl+S resolves conflicts"
                            } else {
                                "No external changes; local edits kept"
                            }
                            .into(),
                        )
                    }
                    Err(error) => app.status_message = Some(format!("Refresh failed: {error}")),
                    _ => {}
                },
                KeyCode::Char('?') | KeyCode::F(1) => {
                    app.show_help = true;
                    app.help_scroll = 0;
                }
                KeyCode::Char('n') => app.search_next(true),
                KeyCode::Char('N') => app.search_next(false),
                KeyCode::Char('o') => app.create_note(),
                KeyCode::Char('u') => app.create_file(),
                KeyCode::Char('i') => app.create_list(),
                KeyCode::Char('r') => app.remove(),
                KeyCode::Char('m') => app.begin_move(),
                KeyCode::Char('J') => app.reorder_selected_task(true),
                KeyCode::Char('K') => app.reorder_selected_task(false),
                KeyCode::Tab => app.indent_selected_task(false),
                KeyCode::BackTab => app.indent_selected_task(true),
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
                KeyCode::Char('q') => match app.request_save(true) {
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
        render_help(frame, area, app);
        return;
    }
    if app.move_picker.is_some() {
        render_move_picker(frame, area, app);
        return;
    }

    let editing = app.mode != EditorMode::Nothing;
    let minimum_height = if editing { 15 } else { 12 };
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
    let list_labels = current_list
        .titles
        .iter()
        .enumerate()
        .map(|(section, title)| {
            let counts = app.list_counts(app.file_index, section);
            format!("{title} ({}/{})", counts.done, counts.total())
        })
        .collect::<Vec<_>>();
    let (list_items, selected_list) = make_visible_tab_items(
        &list_labels,
        app.list_index,
        chunks[1].width.saturating_sub(2) as usize,
    );
    let mut list_tabs = Tabs::new(list_items)
        .block(Block::default().borders(Borders::ALL).title("Lists"))
        .select(selected_list)
        .style(Style::default().fg(Color::Cyan));
    if app.cursor_vertical == 1 {
        list_tabs = list_tabs.highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(Color::DarkGray),
        );
    }
    frame.render_widget(list_tabs, chunks[1]);

    let file_labels = app
        .files
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let counts = app.file_counts(index);
            let dirty = if app.file_is_dirty(index) { "* " } else { "" };
            format!("{dirty}{name} ({}/{})", counts.done, counts.total())
        })
        .collect::<Vec<_>>();
    let (file_items, selected_file) = make_visible_tab_items(
        &file_labels,
        app.file_index,
        chunks[0].width.saturating_sub(2) as usize,
    );
    let mut file_tabs = Tabs::new(file_items)
        .block(Block::default().borders(Borders::ALL).title("Files"))
        .select(selected_file)
        .style(Style::default().fg(Color::Cyan));
    if app.cursor_vertical == 0 {
        file_tabs = file_tabs.highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(Color::DarkGray),
        );
    }
    frame.render_widget(file_tabs, chunks[0]);

    let tasks = app.visible_tasks();
    let selected = tasks.iter().position(|task| *task == app.selected_task());
    let note_capacity = chunks[2].height.saturating_sub(2).max(1) as usize;
    let note_start = if tasks.len() > note_capacity && app.cursor_vertical == 2 {
        selected
            .unwrap_or(0)
            .saturating_sub(note_capacity / 2)
            .min(tasks.len() - note_capacity)
    } else {
        0
    };
    let items = tasks
        .iter()
        .enumerate()
        .skip(note_start)
        .take(note_capacity)
        .map(|(index, location)| {
            let list = &app.lists[location.file];
            let note = &list.notes[location.section][location.note];
            let prefix = match note.state {
                NoteEnum::Open => "[ ] ",
                NoteEnum::Done => "[x] ",
                NoteEnum::Rejected => "[-] ",
            };
            let marker = if Some(index) == selected && app.cursor_vertical == 2 {
                "> "
            } else {
                "- "
            };
            let context = if app.task_view == TaskView::Lists {
                String::new()
            } else {
                format!(
                    "{} / {}: ",
                    app.files[location.file], list.titles[location.section]
                )
            };
            ListItem::new(Line::from(vec![
                Span::raw(format!(
                    "{marker}{}{prefix}",
                    " ".repeat(list.note_depth(location.section, location.note).min(40))
                )),
                Span::styled(context, Style::default().fg(Color::Cyan)),
                Span::styled(
                    note.content.as_str(),
                    Style::default().fg({
                        let metadata = note.metadata();
                        if note.state == NoteEnum::Open
                            && metadata
                                .due
                                .is_some_and(|due| due < chrono::Local::now().date_naive())
                        {
                            Color::Red
                        } else if metadata.priority == crate::metadata::Priority::High {
                            Color::Yellow
                        } else {
                            Color::White
                        }
                    }),
                ),
            ]))
            .style(Style::default().fg(Color::White))
        })
        .collect::<Vec<_>>();
    let description = current_list
        .descriptions
        .get(app.list_index)
        .map(String::as_str)
        .unwrap_or("");
    let counts = if app.task_view == TaskView::Lists {
        app.list_counts(app.file_index, app.list_index)
    } else {
        crate::query::TaskCounts::from_notes(
            app.lists
                .iter()
                .flat_map(|list| list.notes.iter().flatten()),
        )
    };
    let mut task_title = format!("{} | {}", app.task_view.label(), app.task_filter.label());
    if !app.filter_query.is_empty() {
        task_title.push_str(&format!(" | {}", app.filter_query));
    }
    task_title.push_str(&format!(
        " | {} | {} open, {} done, {} rejected",
        app.task_sort.label(),
        counts.open,
        counts.done,
        counts.rejected
    ));
    let mut task_block = Block::default().borders(Borders::ALL).title(task_title);
    if app.task_view == TaskView::Lists && !description.is_empty() {
        task_block = task_block.title_bottom(description);
    }
    let note_list = List::new(if tasks.is_empty() {
        vec![ListItem::new("No tasks match this view")]
    } else {
        items
    })
    .block(task_block)
    .highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );
    let mut visible_note_state = ListState::default();
    if app.cursor_vertical == 2 {
        visible_note_state.select(selected.map(|index| index.saturating_sub(note_start)));
    }
    frame.render_stateful_widget(note_list, chunks[2], &mut visible_note_state);

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
                        EditorMode::Filter => "Filter",
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
                format!(
                    "Save conflict at {}. r reload, o overwrite, Esc cancel",
                    conflict.path.display()
                )
            })
        })
        .or_else(|| app.status_message.clone())
        .unwrap_or_else(|| {
            "?: help  f: filter  g: all files  Ctrl+S: save  Ctrl+Z: undo  q: quit".into()
        });
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
        .filter(|_| app.visible_tasks().contains(&app.selected_task()))
        .map(|note| note.to_string())
        .unwrap_or_else(|| "(no matching task)".into());
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
            EditorMode::Filter => "Filter",
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
        Line::from(format!(
            "{} / {}: {note}",
            app.task_view.label(),
            app.task_filter.label()
        )),
        Line::from("Resize terminal for full view"),
    ];
    frame.render_widget(
        Paragraph::new(content)
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::White)),
        Rect::new(0, 0, area.width, body_height),
    );
}

fn render_help(frame: &mut Frame<'_>, area: Rect, app: &mut App<'_>) {
    let width = area.width.min(72);
    let height = area.height.min(32);
    let rect = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    let help = "Navigation\n  Arrows or h/j/k/l or WASD move between files, lists, and tasks.\n\nEditing\n  e edit the selected item; Ctrl+E edits a list description.\n  u create a file, i create a list, o create a task.\n  J/K reorder tasks, m moves to a list/file, Tab/Shift+Tab indent/outdent.\n  Enter/Space toggles a task. r deletes; Ctrl+Z undoes edits; Ctrl+Y redoes them. History survives saves and clears on reload.\n\nSearch and save\n  f cycles states; g toggles all files; F filters tags/text/metadata.\n  S sorts by document/priority/due; t shows Today, v shows Overdue.\n  / searches names, list titles, descriptions, and tasks. n/N moves through matches.\n  Ctrl+S saves without quitting; * marks changed files. q saves and quits. Ctrl+Q or Ctrl+C asks before discarding changes.\n  E opens the selected file in VISUAL/EDITOR after saving. R refreshes files.\n  External changes refresh clean files automatically; local edits stay available.\n  F1 or ? opens this help. Esc closes help or cancels an editor.";
    let rows = wrap_help_text(help, width.saturating_sub(2) as usize);
    let maximum = rows.len().saturating_sub(height.saturating_sub(2) as usize);
    let paragraph = Paragraph::new(rows.into_iter().map(Line::from).collect::<Vec<_>>());
    app.help_scroll = app.help_scroll.min(maximum.min(u16::MAX as usize) as u16);
    frame.render_widget(Clear, rect);
    frame.render_widget(
        paragraph
            .scroll((app.help_scroll, 0))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Help: Up/Down scroll, Esc closes"),
            )
            .style(Style::default().fg(Color::White)),
        rect,
    );
}

fn wrap_help_text(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let mut rows = Vec::new();
    for line in text.lines() {
        let indent = " "
            .repeat(line.len() - line.trim_start().len())
            .chars()
            .take(width.saturating_sub(1))
            .collect::<String>();
        let mut row = indent.clone();
        let mut cells = indent.len();
        for word in line.split_whitespace() {
            let word_width = UnicodeWidthStr::width(word);
            if cells > indent.len() && cells + 1 + word_width > width {
                rows.push(row);
                row = indent.clone();
                cells = indent.len();
            }
            if cells > indent.len() {
                row.push(' ');
                cells += 1;
            }
            for grapheme in word.graphemes(true) {
                let grapheme_width = UnicodeWidthStr::width(grapheme);
                if cells > indent.len() && cells + grapheme_width > width {
                    rows.push(row);
                    row = indent.clone();
                    cells = indent.len();
                }
                row.push_str(grapheme);
                cells += grapheme_width;
            }
        }
        rows.push(row);
    }
    rows
}

fn render_move_picker(frame: &mut Frame<'_>, area: Rect, app: &App<'_>) {
    let Some(picker) = &app.move_picker else {
        return;
    };
    let items = picker
        .destinations
        .iter()
        .map(|&(file, section)| {
            ListItem::new(format!(
                "{} / {}",
                app.files[file], app.lists[file].titles[section]
            ))
        })
        .collect::<Vec<_>>();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title("Move task: Enter selects, Esc cancels"),
        )
        .highlight_symbol("> ")
        .highlight_style(Style::default().bg(Color::DarkGray));
    let mut state = ListState::default().with_selected(Some(picker.index));
    frame.render_stateful_widget(list, area, &mut state);
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

fn make_visible_tab_items(
    values: &[String],
    selected: usize,
    available_width: usize,
) -> (Vec<Line<'static>>, Option<usize>) {
    if values.is_empty() || available_width == 0 {
        return (Vec::new(), None);
    }
    let selected = selected.min(values.len() - 1);
    let max_label_width = available_width.saturating_sub(2).clamp(1, 24);
    let item_width =
        |index: usize| UnicodeWidthStr::width(values[index].as_str()).min(max_label_width) + 2;
    let mut start = selected;
    let mut end = selected + 1;
    let mut used_width = item_width(selected);
    let mut prefer_left = true;
    loop {
        let mut added = false;
        for left_first in [prefer_left, !prefer_left] {
            let candidate = if left_first {
                start.checked_sub(1)
            } else if end < values.len() {
                Some(end)
            } else {
                None
            };
            let Some(candidate) = candidate else {
                continue;
            };
            let new_width = used_width + item_width(candidate) + 1;
            if new_width <= available_width {
                if candidate < start {
                    start = candidate;
                } else {
                    end += 1;
                }
                used_width = new_width;
                added = true;
                break;
            }
        }
        if !added {
            break;
        }
        prefer_left = !prefer_left;
    }

    let items = values[start..end]
        .iter()
        .map(|value| make_tab_line(&truncate_tab(value, max_label_width)))
        .collect();
    (items, Some(selected - start))
}

fn truncate_tab(value: &str, max_width: usize) -> String {
    if UnicodeWidthStr::width(value) <= max_width {
        return value.to_owned();
    }
    if max_width == 0 {
        return String::new();
    }
    let target_width = max_width.saturating_sub(1);
    let mut output = String::new();
    let mut width = 0;
    for grapheme in value.graphemes(true) {
        let grapheme_width = UnicodeWidthStr::width(grapheme);
        if width + grapheme_width > target_width {
            break;
        }
        output.push_str(grapheme);
        width += grapheme_width;
    }
    output.push('…');
    output
}

fn make_tab_line(value: &str) -> Line<'static> {
    let split_at = value
        .grapheme_indices(true)
        .nth(1)
        .map_or(value.len(), |(index, _)| index);
    let (first, rest) = value.split_at(split_at);
    Line::from(vec![
        Span::styled(first.to_owned(), Style::default().fg(Color::Yellow)),
        Span::styled(rest.to_owned(), Style::default().fg(Color::Green)),
    ])
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
    fn selected_tab_stays_in_the_visible_window() {
        let names = (0..40)
            .map(|index| format!("long tab {index} {}", "界".repeat(20)))
            .collect::<Vec<_>>();
        let (visible, selected) = super::make_visible_tab_items(&names, 39, 25);
        let visible_text = visible
            .iter()
            .flat_map(|line| line.spans.iter().map(|span| span.content.as_ref()))
            .collect::<Vec<_>>()
            .concat();
        assert!(visible.len() < names.len());
        assert!(visible_text.contains("39"));
        assert!(selected.is_some_and(|index| index < visible.len()));
    }

    #[test]
    fn large_task_lists_render_the_selected_task_into_the_viewport() {
        let mut files = vec!["work".into()];
        let mut paths = vec![std::path::PathBuf::from("work.md")];
        let mut lists = vec![FileList::from_parts(
            vec!["Tasks".into()],
            vec![String::new()],
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
        app.note_index = 499;
        app.cursor_vertical = 2;
        let mut terminal = Terminal::new(TestBackend::new(40, 15)).unwrap();
        terminal.draw(|frame| ui(frame, &mut app)).unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .concat();
        assert!(text.contains("task 499"));
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
