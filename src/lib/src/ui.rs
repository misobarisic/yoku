pub mod app;

use crate::todo::NoteEnum;
use crate::ui::app::{App, EditorMode, EMPTY_LIST, EMPTY_NOTE_VEC, EMPTY_STRING};
use crossterm::event::{self};
use crossterm::event::{Event, KeyCode, KeyModifiers};
use std::io;
use tui::widgets::{List, ListItem, Paragraph};
use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Tabs},
    Frame, Terminal,
};

pub fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> io::Result<()> {
    loop {
        terminal.draw(|f| ui(f, &mut app))?;

        if let Event::Key(key) = event::read().unwrap() {
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
                            app.change()
                        }
                    }
                    KeyCode::Char('q') => {
                        if key.modifiers != KeyModifiers::CONTROL {
                            app.save();
                        }
                        return Ok(());
                    }
                    KeyCode::Char('c') => {
                        if key.modifiers == KeyModifiers::CONTROL {
                            // app.save();
                            return Ok(());
                        }
                    }
                    KeyCode::Right => app.next(),
                    KeyCode::Left => app.previous(),
                    KeyCode::Up => app.navigate_up(),
                    KeyCode::Down => app.navigate_down(),
                    KeyCode::Char('w') => app.navigate_up(),
                    KeyCode::Char('s') => app.navigate_down(),
                    KeyCode::Char('d') => app.next(),
                    KeyCode::Char('a') => app.previous(),
                    KeyCode::Char('k') => app.navigate_up(),
                    KeyCode::Char('h') => app.previous(),
                    KeyCode::Char('j') => app.navigate_down(),
                    KeyCode::Char('l') => app.next(),
                    KeyCode::Esc => {
                        if app.cursor_vertical == 2 {
                            app.note_index = 0;
                            app.navigate_up();
                        }
                    }
                    KeyCode::Enter => {
                        if app.cursor_vertical == 2 {
                            app.cycle_note_state()
                        }
                    }
                    KeyCode::Char(' ') => {
                        if app.cursor_vertical == 2 {
                            app.cycle_note_state()
                        }
                    }
                    KeyCode::Char('x') => {
                        if app.cursor_vertical == 2 {
                            app.set_note_state(NoteEnum::DONE)
                        }
                    }
                    KeyCode::Char('+') => {
                        if app.cursor_vertical == 2 {
                            app.set_note_state(NoteEnum::DONE)
                        }
                    }
                    KeyCode::Char('-') => {
                        if app.cursor_vertical == 2 {
                            app.set_note_state(NoteEnum::REJECTED)
                        }
                    }
                    _ => {}
                },
                _ => match key.code {
                    KeyCode::Char('q') => {
                        app.save();
                        return Ok(());
                    }
                    KeyCode::Char('c') => {
                        if key.modifiers == KeyModifiers::CONTROL {
                            // app.save();
                            return Ok(());
                        } else {
                            app.input.push('c');
                        }
                    }
                    KeyCode::Char(c) => {
                        app.input.push(c);
                    }
                    KeyCode::Backspace => {
                        app.input.pop();
                    }
                    KeyCode::Enter => app.handle_enter(),
                    KeyCode::Esc => {
                        app.mode = EditorMode::Nothing;
                        app.input = String::new();
                    }
                    _ => {}
                },
            }
        }
    }
}

pub fn ui<B: Backend>(f: &mut Frame<B>, app: &mut App) {
    let size = f.size();
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
            .split(size)
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
            .split(size)
    };

    let block =
        Block::default().style(Style::default().bg(Color::Rgb(31, 41, 55)).fg(Color::White));
    f.render_widget(block, size);
    let mut list_strings: Vec<String> = Vec::new();
    let ls = if !app.lists.is_empty() {
        app.lists.get(app.file_index).unwrap()
    } else {
        EMPTY_LIST
    };
    for file in &ls.titles {
        list_strings.push(file.parse().unwrap());
    }

    let list_tab_items = make_tab_items(&list_strings);
    let file_tab_items = make_tab_items(app.files);

    let mut list_tabs = Tabs::new(list_tab_items)
        .block(Block::default().borders(Borders::ALL).title("Lists"))
        .select(app.list_index)
        .style(Style::default().fg(Color::Cyan));

    if app.cursor_vertical == 1 {
        list_tabs = list_tabs.highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(Color::DarkGray),
        );
    }

    f.render_widget(list_tabs, chunks[1]);

    let mut file_tabs = Tabs::new(file_tab_items)
        .block(Block::default().borders(Borders::ALL).title("Files"))
        .select(app.file_index)
        .style(Style::default().fg(Color::Cyan));

    if app.cursor_vertical == 0 {
        file_tabs = file_tabs.highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(Color::DarkGray),
        );
    }

    f.render_widget(file_tabs, chunks[0]);

    // NOTES
    let items = app
        .lists
        .get(app.file_index)
        .unwrap_or(EMPTY_LIST)
        .notes
        .get(app.list_index)
        .unwrap_or(EMPTY_NOTE_VEC);
    let items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, note)| {
            let lines = if i == app.note_index && app.cursor_vertical == 2 {
                vec![Spans::from(note.to_string_custom(">"))]
            } else {
                vec![Spans::from(note.to_string())]
            };
            ListItem::new(lines).style(Style::default().fg(Color::White))
        })
        .collect();

    let items = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(
            if !app.lists.is_empty() || !app.files.is_empty() {
                &*app
                    .lists
                    .get(app.file_index)
                    .unwrap()
                    .descriptions
                    .get(app.list_index)
                    .unwrap_or(EMPTY_STRING)
            } else {
                ""
            },
        ))
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(items, chunks[2], &mut app.notes_state);

    // Input
    if app.mode != EditorMode::Nothing {
        let input = Paragraph::new(app.input.as_ref())
            .style(match app.mode {
                EditorMode::Nothing => Style::default(),
                _ => Style::default().fg(Color::White),
            })
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
                        _ => "",
                    })
                    .style(Style::default().fg(Color::LightCyan)),
            );
        f.render_widget(input, chunks[3]);
    }
}

pub fn make_tab_items(v: &[String]) -> Vec<Spans> {
    v.iter()
        .map(|t| {
            let (first, rest) = t.split_at(1);
            Spans::from(vec![
                Span::styled(first, Style::default().fg(Color::Yellow)),
                Span::styled(rest, Style::default().fg(Color::Green)),
            ])
        })
        .collect()
}
