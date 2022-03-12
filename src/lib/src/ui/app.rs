use crate::todo::{
    FileList, Note, NoteEnum, STARTER_FILE_DESCRIPTION, STARTER_FILE_NOTE, STARTER_FILE_TITLE,
};
use crate::util::calculate_hash;
use std::collections::HashMap;
use std::fs::remove_file;
use std::path::{Path, PathBuf};
use tui::widgets::ListState;

pub const EMPTY_LIST: &FileList = &FileList {
    titles: vec![],
    descriptions: vec![],
    notes: vec![],
};
pub const EMPTY_NOTE_VEC: &Vec<Note> = &vec![];
pub const EMPTY_STRING: &String = &String::new();

#[derive(Copy, Clone, PartialEq)]
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
    pub hashes: &'a mut HashMap<&'a PathBuf, u64>,
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
        hashes: &'a mut HashMap<&'a PathBuf, u64>,
        main_path: &'a Path,
        to_remove: &'a mut Vec<PathBuf>,
    ) -> App<'a> {
        App {
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
        }
    }

    fn validate_and_update_indices(&mut self) {
        let size = self.files.len();
        if self.file_index >= size - 1 {
            self.file_index = size - 1;
            self.files_state.select(Option::from(self.file_index));
        }

        let size = self.lists.get(self.file_index).unwrap().titles.len();
        if self.list_index > 0 && self.list_index > size - 1 {
            self.list_index = size - 1;
            self.lists_state.select(Option::from(self.list_index));
        }

        let size = self
            .lists
            .get(self.file_index)
            .unwrap()
            .notes
            .get(self.list_index)
            .unwrap_or(EMPTY_NOTE_VEC)
            .len();
        if self.note_index > 0 && self.note_index > size - 1 {
            self.note_index = size - 1;
            self.notes_state.select(Option::from(self.note_index));
        }
    }

    pub fn navigate_down(&mut self) {
        if self.files.is_empty() || self.lists.is_empty() {
            return;
        }
        match self.cursor_vertical {
            0 => {
                let size = self.lists.get(self.file_index).unwrap().titles.len();
                if size > 0 {
                    self.cursor_vertical += 1
                }
            }
            1 => {
                let size = self
                    .lists
                    .get(self.file_index)
                    .unwrap()
                    .notes
                    .get(self.list_index)
                    .unwrap_or(EMPTY_NOTE_VEC)
                    .len();
                if size > 0 {
                    self.cursor_vertical += 1;
                    self.notes_state.select(Option::from(self.note_index));
                }
            }
            2 => {
                self.next_note();
            }
            _ => self.cursor_vertical += 1,
        }
        self.validate_and_update_indices();
    }

    pub fn navigate_up(&mut self) {
        if self.files.is_empty() || self.lists.is_empty() {
            return;
        }
        match self.cursor_vertical {
            0 => {}
            2 => {
                if self.note_index != 0 {
                    self.previous_note();
                } else {
                    self.cursor_vertical -= 1;
                    self.notes_state.select(None)
                }
            }
            _ => self.cursor_vertical -= 1,
        }
        self.validate_and_update_indices();
    }

    pub fn next(&mut self) {
        match self.cursor_vertical {
            0 => {
                self.file_index += 1;
                self.note_index = 0;
                self.files_state.select(Option::from(self.list_index));
                // self.notes_state.select(Option::from(self.note_index));
                self.notes_state.select(None);
            }
            1 => {
                self.list_index += 1;
                self.note_index = 0;
            }
            _ => {}
        }
        self.validate_and_update_indices();
    }

    pub fn previous(&mut self) {
        match self.cursor_vertical {
            0 => {
                if self.file_index > 0 {
                    self.file_index -= 1;
                    self.note_index = 0;
                    self.files_state.select(Option::from(self.list_index));
                    self.notes_state.select(None);
                }
            }
            1 => {
                if self.list_index > 0 {
                    self.list_index -= 1;
                    self.note_index = 0;
                    self.files_state.select(Option::from(self.list_index));
                    self.notes_state.select(None);
                }
            }
            _ => {}
        }
        self.validate_and_update_indices();
    }

    pub fn next_note(&mut self) {
        self.note_index += 1;
        self.notes_state.select(Option::from(self.note_index));
        self.validate_and_update_indices();
    }

    pub fn previous_note(&mut self) {
        if self.note_index > 0 {
            self.note_index -= 1;
        }
        self.notes_state.select(Option::from(self.note_index));
        self.validate_and_update_indices();
    }

    pub fn cycle_note_state(&mut self) {
        let note = self
            .lists
            .get_mut(self.file_index)
            .unwrap()
            .notes
            .get_mut(self.list_index)
            .unwrap()
            .get_mut(self.note_index)
            .unwrap();
        match note.state {
            NoteEnum::OPEN => note.set_state(NoteEnum::DONE),
            NoteEnum::DONE => note.set_state(NoteEnum::REJECTED),
            NoteEnum::REJECTED => note.set_state(NoteEnum::OPEN),
        };
    }

    pub fn set_note_state(&mut self, state: NoteEnum) {
        let note = self
            .lists
            .get_mut(self.file_index)
            .unwrap()
            .notes
            .get_mut(self.list_index)
            .unwrap()
            .get_mut(self.note_index)
            .unwrap();
        note.set_state(state);
    }
    pub fn save(&self) {
        for file in self.to_remove.iter() {
            remove_file(file).unwrap();
        }
        for (i, list) in self.lists.iter().enumerate() {
            let path = PathBuf::from(self.paths.get(i).unwrap());
            let stored_hash = self.hashes.get(&path).unwrap_or(&0);
            let new_hash = calculate_hash(&list);
            if *stored_hash != new_hash {
                list.write(&path);
            }
        }
    }
    pub fn change(&mut self) {
        self.mode = match self.cursor_vertical {
            0 => {
                let current_name = self.files.get(self.file_index).unwrap();
                self.input = current_name.clone();
                EditorMode::ChangeFileName
            }
            1 => {
                let current_list = self.lists.get_mut(self.file_index).unwrap();
                self.input = current_list
                    .titles
                    .get_mut(self.list_index)
                    .unwrap()
                    .clone();
                EditorMode::ChangeListName
            }
            2 => {
                let note = self
                    .lists
                    .get_mut(self.file_index)
                    .unwrap()
                    .notes
                    .get_mut(self.list_index)
                    .unwrap()
                    .get_mut(self.note_index)
                    .unwrap();
                self.input = note.content.clone();
                EditorMode::ChangeNoteContent
            }
            _ => EditorMode::Nothing,
        }
    }
    pub fn change_description(&mut self) {
        let current_list = self.lists.get_mut(self.file_index).unwrap();
        self.input = current_list
            .descriptions
            .get_mut(self.list_index)
            .unwrap()
            .clone();
        self.mode = EditorMode::ChangeListDescription;
    }
    pub fn remove(&mut self) {
        match self.cursor_vertical {
            0 => {
                self.to_remove
                    .push(self.paths.get(self.file_index).unwrap().clone());
                self.paths.remove(self.file_index);
                self.files.remove(self.file_index);
                self.lists.remove(self.file_index);
                if !self.files.is_empty() && self.file_index >= self.files.len() {
                    self.file_index = self.files.len() - 1
                } else if self.files.is_empty() {
                    self.file_index = 0;
                    self.cursor_vertical = 0;
                }
            }
            1 => {
                self.lists
                    .get_mut(self.file_index)
                    .unwrap()
                    .titles
                    .remove(self.list_index);
                self.lists
                    .get_mut(self.file_index)
                    .unwrap()
                    .descriptions
                    .remove(self.list_index);
                self.lists
                    .get_mut(self.file_index)
                    .unwrap()
                    .notes
                    .remove(self.list_index);
                if !self
                    .lists
                    .get_mut(self.file_index)
                    .unwrap()
                    .titles
                    .is_empty()
                    && self.list_index == self.lists.get_mut(self.file_index).unwrap().titles.len()
                {
                    self.list_index = self.lists.get_mut(self.file_index).unwrap().titles.len() - 1;
                } else if self
                    .lists
                    .get_mut(self.file_index)
                    .unwrap()
                    .titles
                    .is_empty()
                {
                    self.list_index = 0;
                    self.cursor_vertical = 0;
                }
            }
            2 => {
                let notes = self
                    .lists
                    .get_mut(self.file_index)
                    .unwrap()
                    .notes
                    .get_mut(self.list_index)
                    .unwrap();
                notes.remove(self.note_index);
                if !notes.is_empty() && self.note_index >= notes.len() {
                    self.note_index = notes.len() - 1;
                    self.notes_state.select(Option::from(self.note_index))
                } else if notes.is_empty() {
                    self.note_index = 0;
                    self.cursor_vertical = 1;
                    self.notes_state.select(Option::from(self.note_index));
                }
            }
            _ => {}
        }
    }
    pub fn create_file(&mut self) {
        self.mode = EditorMode::CreateFile;
    }
    pub fn create_list(&mut self) {
        self.mode = EditorMode::CreateList;
    }
    pub fn create_note(&mut self) {
        self.mode = EditorMode::CreateNote;
    }
    pub fn handle_enter(&mut self) {
        match self.mode {
            EditorMode::CreateFile => {
                if !self.input.is_empty() {
                    let input = self.input.clone();
                    let path = self.main_path.join(format!("{}.md", input));
                    self.files.push(input);
                    self.paths.push(path);
                    let list = FileList {
                        titles: vec![STARTER_FILE_TITLE.to_string()],
                        descriptions: vec![STARTER_FILE_DESCRIPTION.to_string()],
                        notes: vec![vec![Note {
                            content: STARTER_FILE_NOTE.to_string(),
                            state: NoteEnum::OPEN,
                        }]],
                    };
                    self.lists.push(list);
                    self.input = String::new();
                    self.mode = EditorMode::Nothing;
                }
            }
            EditorMode::CreateList => {
                if !self.input.is_empty() {
                    let input = self.input.clone();
                    let current_list = self.lists.get_mut(self.file_index).unwrap();
                    current_list.titles.push(input);
                    current_list.descriptions.push(String::new());
                    current_list.notes.push(vec![]);
                    self.input = String::new();
                    self.mode = EditorMode::Nothing;
                }
            }
            EditorMode::CreateNote => {
                if !self.input.is_empty() {
                    let input = self.input.clone();
                    let current_notes = self
                        .lists
                        .get_mut(self.file_index)
                        .unwrap()
                        .notes
                        .get_mut(self.list_index)
                        .unwrap();
                    current_notes.push(Note {
                        content: input,
                        state: NoteEnum::OPEN,
                    });
                    self.input = String::new();
                    self.mode = EditorMode::Nothing;
                }
            }
            EditorMode::ChangeFileName => {
                if !self.input.is_empty() {
                    let input = self.input.clone();
                    let current_file_name = self.files.get_mut(self.file_index).unwrap();
                    *current_file_name = input.clone();
                    let current_path = self.paths.get_mut(self.file_index).unwrap();
                    remove_file(&current_path).unwrap();
                    current_path.set_file_name(format!("{}.md", input));
                    self.input = String::new();
                    self.mode = EditorMode::Nothing;
                }
            }
            EditorMode::ChangeListName => {
                if !self.input.is_empty() {
                    let input = self.input.clone();
                    let current_list = self.lists.get_mut(self.file_index).unwrap();
                    *current_list.titles.get_mut(self.list_index).unwrap() = input;
                    self.input = String::new();
                    self.mode = EditorMode::Nothing;
                }
            }
            EditorMode::ChangeListDescription => {
                if !self.input.is_empty() {
                    let input = self.input.clone();
                    let current_list = self.lists.get_mut(self.file_index).unwrap();
                    *current_list.descriptions.get_mut(self.list_index).unwrap() = input;
                    self.input = String::new();
                    self.mode = EditorMode::Nothing;
                }
            }
            EditorMode::ChangeNoteContent => {
                if !self.input.is_empty() {
                    let input = self.input.clone();
                    let note = self
                        .lists
                        .get_mut(self.file_index)
                        .unwrap()
                        .notes
                        .get_mut(self.list_index)
                        .unwrap()
                        .get_mut(self.note_index)
                        .unwrap();
                    *note = Note {
                        content: input,
                        state: note.state.clone(),
                    };
                    self.input = String::new();
                    self.mode = EditorMode::Nothing;
                }
            }
            _ => {}
        }
        self.input = String::new();
    }
}
