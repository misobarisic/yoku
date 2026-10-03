use crate::todo::{
    FileList, Note, NoteEnum, STARTER_FILE_DESCRIPTION, STARTER_FILE_NOTE, STARTER_FILE_TITLE,
};
use crate::util::calculate_hash;
use ratatui::widgets::ListState;
use std::collections::HashMap;
use std::fs::remove_file;
use std::io;
use std::path::{Path, PathBuf};

pub const EMPTY_LIST: &FileList = &FileList::empty_const();
pub const EMPTY_NOTE_VEC: &Vec<Note> = &vec![];

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum EditorMode {
    Nothing,
    CreateFile,
    CreateList,
    CreateNote,
    ChangeFileName,
    ChangeListName,
    ChangeListDescription,
    ChangeNoteContent,
}

pub struct App<'a> {
    pub main_path: &'a Path,
    pub files: &'a mut Vec<String>,
    pub paths: &'a mut Vec<PathBuf>,
    pub lists: &'a mut Vec<FileList>,
    pub hashes: &'a mut HashMap<PathBuf, u64>,
    pub cursor_vertical: usize,
    pub list_index: usize,
    pub file_index: usize,
    pub note_index: usize,
    pub files_state: ListState,
    pub lists_state: ListState,
    pub notes_state: ListState,
    pub mode: EditorMode,
    pub input: String,
    to_remove: &'a mut Vec<PathBuf>,
}

impl<'a> App<'a> {
    pub fn new(
        files: &'a mut Vec<String>,
        paths: &'a mut Vec<PathBuf>,
        lists: &'a mut Vec<FileList>,
        hashes: &'a mut HashMap<PathBuf, u64>,
        main_path: &'a Path,
        to_remove: &'a mut Vec<PathBuf>,
    ) -> Self {
        let mut app = Self {
            main_path,
            hashes,
            lists,
            files,
            paths,
            to_remove,
            cursor_vertical: 0,
            list_index: 0,
            file_index: 0,
            note_index: 0,
            files_state: Default::default(),
            lists_state: Default::default(),
            notes_state: Default::default(),
            mode: EditorMode::Nothing,
            input: String::new(),
        };
        app.validate_and_update_indices();
        app
    }

    fn selected_notes(&self) -> &[Note] {
        self.lists
            .get(self.file_index)
            .and_then(|list| list.notes.get(self.list_index))
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn validate_and_update_indices(&mut self) {
        if self.files.is_empty() {
            self.file_index = 0;
            self.list_index = 0;
            self.note_index = 0;
            self.cursor_vertical = 0;
            self.files_state.select(None);
            self.lists_state.select(None);
            self.notes_state.select(None);
            return;
        }

        self.file_index = self.file_index.min(self.files.len() - 1);
        self.files_state.select(Some(self.file_index));

        let Some(list) = self.lists.get(self.file_index) else {
            self.list_index = 0;
            self.note_index = 0;
            self.cursor_vertical = 0;
            self.lists_state.select(None);
            self.notes_state.select(None);
            return;
        };

        if list.titles.is_empty() {
            self.list_index = 0;
            self.note_index = 0;
            self.cursor_vertical = 0;
            self.lists_state.select(None);
            self.notes_state.select(None);
            return;
        }

        self.list_index = self.list_index.min(list.titles.len() - 1);
        self.lists_state.select(Some(self.list_index));

        let note_count = list
            .notes
            .get(self.list_index)
            .map(Vec::len)
            .unwrap_or_default();
        if note_count == 0 {
            self.note_index = 0;
            if self.cursor_vertical > 1 {
                self.cursor_vertical = 1;
            }
            self.notes_state.select(None);
        } else {
            self.note_index = self.note_index.min(note_count - 1);
            if self.cursor_vertical == 2 {
                self.notes_state.select(Some(self.note_index));
            } else {
                self.notes_state.select(None);
            }
        }
        self.cursor_vertical = self.cursor_vertical.min(2);
    }

    pub fn navigate_down(&mut self) {
        match self.cursor_vertical {
            0 if self
                .lists
                .get(self.file_index)
                .is_some_and(|list| !list.titles.is_empty()) =>
            {
                self.cursor_vertical = 1;
            }
            1 if !self.selected_notes().is_empty() => self.cursor_vertical = 2,
            2 => self.next_note(),
            _ => {}
        }
        self.validate_and_update_indices();
    }

    pub fn navigate_up(&mut self) {
        match self.cursor_vertical {
            0 => {}
            1 => self.cursor_vertical = 0,
            2 if self.note_index > 0 => self.previous_note(),
            2 => {
                self.cursor_vertical = 1;
                self.notes_state.select(None);
            }
            _ => self.cursor_vertical = 0,
        }
        self.validate_and_update_indices();
    }

    pub fn next(&mut self) {
        match self.cursor_vertical {
            0 if !self.files.is_empty() => {
                self.file_index = (self.file_index + 1).min(self.files.len() - 1);
                self.list_index = 0;
                self.note_index = 0;
            }
            1 => {
                if let Some(list) = self.lists.get(self.file_index) {
                    if !list.titles.is_empty() {
                        self.list_index = (self.list_index + 1).min(list.titles.len() - 1);
                    }
                }
                self.note_index = 0;
            }
            _ => {}
        }
        self.validate_and_update_indices();
    }

    pub fn previous(&mut self) {
        match self.cursor_vertical {
            0 => self.file_index = self.file_index.saturating_sub(1),
            1 => self.list_index = self.list_index.saturating_sub(1),
            _ => {}
        }
        self.note_index = 0;
        self.validate_and_update_indices();
    }

    pub fn next_note(&mut self) {
        let note_count = self.selected_notes().len();
        if note_count > 0 {
            self.note_index = (self.note_index + 1).min(note_count - 1);
        }
        self.validate_and_update_indices();
    }

    pub fn previous_note(&mut self) {
        self.note_index = self.note_index.saturating_sub(1);
        self.validate_and_update_indices();
    }

    pub fn cycle_note_state(&mut self) {
        if let Some(note) = self
            .lists
            .get_mut(self.file_index)
            .and_then(|list| list.notes.get_mut(self.list_index))
            .and_then(|notes| notes.get_mut(self.note_index))
        {
            let state = match note.state {
                NoteEnum::Open => NoteEnum::Done,
                NoteEnum::Done => NoteEnum::Rejected,
                NoteEnum::Rejected => NoteEnum::Open,
            };
            note.set_state(state);
        }
    }

    pub fn set_note_state(&mut self, state: NoteEnum) {
        if let Some(note) = self
            .lists
            .get_mut(self.file_index)
            .and_then(|list| list.notes.get_mut(self.list_index))
            .and_then(|notes| notes.get_mut(self.note_index))
        {
            note.set_state(state);
        }
    }

    pub fn save(&self) -> io::Result<()> {
        for (path, list) in self.paths.iter().zip(self.lists.iter()) {
            let stored_hash = self.hashes.get(path);
            if stored_hash.copied() != Some(calculate_hash(list)) {
                list.write(path)?;
            }
        }

        for path in self.to_remove.iter() {
            match remove_file(path) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    pub fn change(&mut self) {
        self.mode = match self.cursor_vertical {
            0 => match self.files.get(self.file_index) {
                Some(name) => {
                    self.input = name.clone();
                    EditorMode::ChangeFileName
                }
                None => EditorMode::Nothing,
            },
            1 => match self
                .lists
                .get(self.file_index)
                .and_then(|list| list.titles.get(self.list_index))
            {
                Some(title) => {
                    self.input = title.clone();
                    EditorMode::ChangeListName
                }
                None => EditorMode::Nothing,
            },
            2 => match self
                .lists
                .get(self.file_index)
                .and_then(|list| list.notes.get(self.list_index))
                .and_then(|notes| notes.get(self.note_index))
            {
                Some(note) => {
                    self.input = note.content.clone();
                    EditorMode::ChangeNoteContent
                }
                None => EditorMode::Nothing,
            },
            _ => EditorMode::Nothing,
        };
    }

    pub fn change_description(&mut self) {
        if let Some(description) = self
            .lists
            .get(self.file_index)
            .and_then(|list| list.descriptions.get(self.list_index))
        {
            self.input = description.clone();
            self.mode = EditorMode::ChangeListDescription;
        }
    }

    pub fn remove(&mut self) {
        match self.cursor_vertical {
            0 => {
                if self.file_index < self.paths.len() {
                    self.to_remove.push(self.paths.remove(self.file_index));
                }
                if self.file_index < self.files.len() {
                    self.files.remove(self.file_index);
                }
                if self.file_index < self.lists.len() {
                    self.lists.remove(self.file_index);
                }
            }
            1 => {
                if let Some(list) = self.lists.get_mut(self.file_index) {
                    list.remove(self.list_index);
                }
            }
            2 => {
                if let Some(list) = self.lists.get_mut(self.file_index) {
                    list.remove_note(self.list_index, self.note_index);
                }
            }
            _ => {}
        }
        self.validate_and_update_indices();
    }

    pub fn create_file(&mut self) {
        self.mode = EditorMode::CreateFile;
        self.input.clear();
    }

    pub fn create_list(&mut self) {
        self.mode = EditorMode::CreateList;
        self.input.clear();
    }

    pub fn create_note(&mut self) {
        self.mode = EditorMode::CreateNote;
        self.input.clear();
    }

    pub fn handle_enter(&mut self) {
        match self.mode {
            EditorMode::CreateFile => {
                if !self.input.trim().is_empty() && valid_file_stem(&self.input) {
                    let path = self.main_path.join(format!("{}.md", self.input));
                    let path_is_pending_removal =
                        self.to_remove.iter().any(|removed| removed == &path);
                    let path_is_active = self.paths.iter().any(|active| active == &path);
                    if !path_is_active && (!path.exists() || path_is_pending_removal) {
                        self.to_remove.retain(|removed| removed != &path);
                        self.files.push(self.input.clone());
                        self.paths.push(path);
                        self.lists.push(FileList::from_parts(
                            vec![STARTER_FILE_TITLE.to_owned()],
                            vec![STARTER_FILE_DESCRIPTION.to_owned()],
                            vec![vec![Note {
                                content: STARTER_FILE_NOTE.to_owned(),
                                state: NoteEnum::Open,
                            }]],
                        ));
                        self.file_index = self.files.len() - 1;
                        self.list_index = 0;
                        self.note_index = 0;
                        self.cursor_vertical = 0;
                        self.finish_input();
                    }
                }
            }
            EditorMode::CreateList => {
                if !self.input.is_empty() {
                    if let Some(current_list) = self.lists.get_mut(self.file_index) {
                        current_list.push_section(self.input.clone());
                        self.list_index = current_list.titles.len() - 1;
                        self.cursor_vertical = 1;
                        self.finish_input();
                    }
                }
            }
            EditorMode::CreateNote => {
                if !self.input.is_empty() {
                    if let Some(current_list) = self.lists.get_mut(self.file_index) {
                        current_list.push_note(
                            self.list_index,
                            Note {
                                content: self.input.clone(),
                                state: NoteEnum::Open,
                            },
                        );
                        self.note_index = current_list.notes[self.list_index].len() - 1;
                        self.cursor_vertical = 2;
                        self.finish_input();
                    }
                }
            }
            EditorMode::ChangeFileName => {
                if !self.input.trim().is_empty() && valid_file_stem(&self.input) {
                    let Some(current_path) = self.paths.get(self.file_index).cloned() else {
                        return;
                    };
                    let new_path = self.main_path.join(format!("{}.md", self.input));
                    let path_is_pending_removal =
                        self.to_remove.iter().any(|removed| removed == &new_path);
                    let path_is_active_elsewhere = self
                        .paths
                        .iter()
                        .enumerate()
                        .any(|(index, active)| index != self.file_index && active == &new_path);
                    if !path_is_active_elsewhere
                        && (new_path == current_path
                            || !new_path.exists()
                            || path_is_pending_removal)
                    {
                        self.to_remove.retain(|removed| removed != &new_path);
                        if new_path != current_path {
                            self.to_remove.push(current_path);
                            if let Some(path) = self.paths.get_mut(self.file_index) {
                                *path = new_path;
                            }
                        }
                        if let Some(file_name) = self.files.get_mut(self.file_index) {
                            *file_name = self.input.clone();
                        }
                        self.finish_input();
                    }
                }
            }
            EditorMode::ChangeListName => {
                if let Some(title) = self
                    .lists
                    .get_mut(self.file_index)
                    .and_then(|list| list.titles.get_mut(self.list_index))
                {
                    *title = self.input.clone();
                    self.finish_input();
                }
            }
            EditorMode::ChangeListDescription => {
                if let Some(description) = self
                    .lists
                    .get_mut(self.file_index)
                    .and_then(|list| list.descriptions.get_mut(self.list_index))
                {
                    *description = self.input.clone();
                    self.finish_input();
                }
            }
            EditorMode::ChangeNoteContent => {
                if let Some(note) = self
                    .lists
                    .get_mut(self.file_index)
                    .and_then(|list| list.notes.get_mut(self.list_index))
                    .and_then(|notes| notes.get_mut(self.note_index))
                {
                    note.content = self.input.clone();
                    self.finish_input();
                }
            }
            EditorMode::Nothing => {}
        }
        self.validate_and_update_indices();
    }

    fn finish_input(&mut self) {
        self.input.clear();
        self.mode = EditorMode::Nothing;
    }
}

fn valid_file_stem(name: &str) -> bool {
    !name.trim().is_empty()
        && name == name.trim()
        && name != "."
        && name != ".."
        && !name.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                )
        })
}
