use crate::todo::{
    parse_markdown, FileList, Note, NoteEnum, STARTER_FILE_DESCRIPTION, STARTER_FILE_NOTE,
    STARTER_FILE_TITLE,
};
use crate::util::calculate_hash;
use ratatui::widgets::ListState;
use std::collections::{HashMap, HashSet};
use std::fs::{self, remove_file};
use std::io;
use std::path::{Path, PathBuf};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

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
    Search,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaveConflictKind {
    ExternalEdit,
    MissingFile,
    DestinationExists,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveConflict {
    pub path: PathBuf,
    pub kind: SaveConflictKind,
}

#[derive(Clone)]
struct EditSnapshot {
    files: Vec<String>,
    paths: Vec<PathBuf>,
    lists: Vec<FileList>,
    to_remove: Vec<PathBuf>,
    renamed_from: HashMap<PathBuf, PathBuf>,
    selection: (usize, usize, usize, usize),
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct SearchMatch {
    file_index: usize,
    section_index: Option<usize>,
    note_index: Option<usize>,
}

pub struct App<'a> {
    pub main_path: &'a Path,
    pub files: &'a mut Vec<String>,
    pub paths: &'a mut Vec<PathBuf>,
    pub lists: &'a mut Vec<FileList>,
    pub hashes: &'a mut HashMap<PathBuf, u64>,
    pub disk_hashes: &'a mut HashMap<PathBuf, u64>,
    pub cursor_vertical: usize,
    pub list_index: usize,
    pub file_index: usize,
    pub note_index: usize,
    pub files_state: ListState,
    pub lists_state: ListState,
    pub notes_state: ListState,
    pub mode: EditorMode,
    pub input: String,
    pub input_cursor: usize,
    pub status_message: Option<String>,
    pub save_conflict: Option<SaveConflict>,
    pub pending_file_delete: Option<usize>,
    pub confirm_discard: bool,
    pub show_help: bool,
    pub search_query: String,
    pub quit_after_save: bool,
    to_remove: &'a mut Vec<PathBuf>,
    renamed_from: HashMap<PathBuf, PathBuf>,
    overwrite_paths: HashSet<PathBuf>,
    undo_stack: Vec<EditSnapshot>,
    redo_stack: Vec<EditSnapshot>,
    search_matches: Vec<SearchMatch>,
    search_index: Option<usize>,
}

impl<'a> App<'a> {
    pub fn new(
        files: &'a mut Vec<String>,
        paths: &'a mut Vec<PathBuf>,
        lists: &'a mut Vec<FileList>,
        hashes: &'a mut HashMap<PathBuf, u64>,
        disk_hashes: &'a mut HashMap<PathBuf, u64>,
        main_path: &'a Path,
        to_remove: &'a mut Vec<PathBuf>,
    ) -> Self {
        let mut app = Self {
            main_path,
            hashes,
            disk_hashes,
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
            input_cursor: 0,
            status_message: None,
            save_conflict: None,
            pending_file_delete: None,
            confirm_discard: false,
            show_help: false,
            search_query: String::new(),
            quit_after_save: false,
            renamed_from: HashMap::new(),
            overwrite_paths: HashSet::new(),
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            search_matches: Vec::new(),
            search_index: None,
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
        let before = self.snapshot();
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
        self.record_change(before);
    }

    pub fn set_note_state(&mut self, state: NoteEnum) {
        let before = self.snapshot();
        if let Some(note) = self
            .lists
            .get_mut(self.file_index)
            .and_then(|list| list.notes.get_mut(self.list_index))
            .and_then(|notes| notes.get_mut(self.note_index))
        {
            note.set_state(state);
        }
        self.record_change(before);
    }

    pub fn save(&mut self) -> io::Result<bool> {
        self.save_conflict = None;
        self.status_message = None;
        let overwrite_paths = std::mem::take(&mut self.overwrite_paths);

        for (path, list) in self.paths.iter().zip(self.lists.iter()) {
            if self.hashes.get(path).copied() == Some(calculate_hash(list)) {
                continue;
            }
            let baseline = self.disk_hashes.get(path).copied();
            let on_disk = hash_file(path)?;
            let conflict = match (baseline, on_disk) {
                (Some(_), None) => Some(SaveConflictKind::MissingFile),
                (Some(expected), Some(actual)) if expected != actual => {
                    Some(SaveConflictKind::ExternalEdit)
                }
                (None, Some(_)) => Some(SaveConflictKind::DestinationExists),
                _ => None,
            };
            if let Some(kind) = conflict {
                if !overwrite_paths.contains(path) {
                    self.save_conflict = Some(SaveConflict {
                        path: path.clone(),
                        kind,
                    });
                    return Ok(false);
                }
            }
        }

        for path in self.to_remove.iter() {
            let baseline = self.disk_hashes.get(path).copied();
            let on_disk = hash_file(path)?;
            let conflict = match (baseline, on_disk) {
                (Some(_), None) => None,
                (Some(expected), Some(actual)) if expected != actual => {
                    Some(SaveConflictKind::ExternalEdit)
                }
                (None, Some(_)) => Some(SaveConflictKind::DestinationExists),
                _ => None,
            };
            if let Some(kind) = conflict {
                if !overwrite_paths.contains(path) {
                    self.save_conflict = Some(SaveConflict {
                        path: path.clone(),
                        kind,
                    });
                    return Ok(false);
                }
            }
        }

        for (path, list) in self.paths.iter().zip(self.lists.iter()) {
            let serialized = list.to_string();
            if self.hashes.get(path).copied() == Some(calculate_hash(list)) {
                continue;
            }
            if self.disk_hashes.contains_key(path) || overwrite_paths.contains(path) {
                list.write(path)?;
            } else if let Err(error) = list.write_new(path) {
                if error.kind() == io::ErrorKind::AlreadyExists {
                    self.save_conflict = Some(SaveConflict {
                        path: path.clone(),
                        kind: SaveConflictKind::DestinationExists,
                    });
                    return Ok(false);
                }
                return Err(error);
            }
            self.hashes.insert(path.clone(), calculate_hash(list));
            self.disk_hashes
                .insert(path.clone(), calculate_hash(&serialized.as_bytes()));
        }

        for path in self.to_remove.iter() {
            match remove_file(path) {
                Ok(()) => {
                    self.hashes.remove(path);
                    self.disk_hashes.remove(path);
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    self.hashes.remove(path);
                    self.disk_hashes.remove(path);
                }
                Err(error) => return Err(error),
            }
        }
        self.to_remove.clear();
        self.renamed_from.clear();
        Ok(true)
    }

    pub fn file_is_dirty(&self, index: usize) -> bool {
        self.paths
            .get(index)
            .zip(self.lists.get(index))
            .is_some_and(|(path, list)| {
                self.hashes.get(path).copied() != Some(calculate_hash(list))
            })
    }

    /// Keep the intent through a conflict retry, so Ctrl+S never closes the app.
    pub fn request_save(&mut self, quit: bool) -> io::Result<bool> {
        self.quit_after_save = quit;
        self.retry_save()
    }

    pub fn retry_save(&mut self) -> io::Result<bool> {
        if self.save()? {
            self.status_message = Some("Saved".into());
            Ok(self.quit_after_save)
        } else {
            Ok(false)
        }
    }

    pub fn overwrite_conflict(&mut self) {
        if let Some(conflict) = self.save_conflict.take() {
            self.overwrite_paths.insert(conflict.path);
            self.status_message = Some("Retrying save with overwrite for the selected path".into());
        }
    }

    pub fn reload_conflict(&mut self) -> io::Result<()> {
        let Some(conflict) = self.save_conflict.take() else {
            return Ok(());
        };
        let contents = fs::read_to_string(&conflict.path)?;
        let list = parse_markdown(&contents);
        let file_hash = calculate_hash(&list);
        let disk_hash = calculate_hash(&contents.as_bytes());
        let path = conflict.path;

        if let Some(index) = self.paths.iter().position(|active| active == &path) {
            self.lists[index] = list;
            if let Some(name) = path.file_stem().and_then(|name| name.to_str()) {
                self.files[index] = name.to_owned();
            }
            self.hashes.insert(path.clone(), file_hash);
            self.disk_hashes.insert(path.clone(), disk_hash);
            if let Some(original_path) = self.renamed_from.remove(&path) {
                self.to_remove.retain(|removed| removed != &original_path);
                if !self.paths.iter().any(|active| active == &original_path) {
                    if let Ok(original_contents) = fs::read_to_string(&original_path) {
                        let original_list = parse_markdown(&original_contents);
                        let original_name = original_path
                            .file_stem()
                            .and_then(|name| name.to_str())
                            .unwrap_or_default()
                            .to_owned();
                        self.files.push(original_name);
                        self.paths.push(original_path.clone());
                        self.hashes
                            .insert(original_path.clone(), calculate_hash(&original_list));
                        self.disk_hashes.insert(
                            original_path.clone(),
                            calculate_hash(&original_contents.as_bytes()),
                        );
                        self.lists.push(original_list);
                    }
                }
            }
            self.status_message = Some("Reloaded the file from disk".into());
        } else if self.to_remove.iter().any(|removed| removed == &path) {
            if let Some((new_path, index)) =
                self.renamed_from.iter().find_map(|(new_path, old_path)| {
                    (old_path == &path)
                        .then(|| {
                            self.paths
                                .iter()
                                .position(|active| active == new_path)
                                .map(|i| (new_path.clone(), i))
                        })
                        .flatten()
                })
            {
                self.paths[index] = path.clone();
                if let Some(name) = path.file_stem().and_then(|name| name.to_str()) {
                    self.files[index] = name.to_owned();
                }
                self.lists[index] = list;
                self.renamed_from.remove(&new_path);
            } else {
                let name = path
                    .file_stem()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .to_owned();
                self.files.push(name.clone());
                self.paths.push(path.clone());
                self.lists.push(list);
            }
            self.to_remove.retain(|removed| removed != &path);
            self.hashes.insert(path.clone(), file_hash);
            self.disk_hashes.insert(path, disk_hash);
            self.validate_and_update_indices();
            self.status_message =
                Some("Restored the file from disk and canceled its removal".into());
        } else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                "the conflicting file is no longer part of the pending save",
            ));
        }
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.status_message = Some("Reloaded from disk; undo history was cleared".into());
        Ok(())
    }

    pub fn cancel_conflict(&mut self) {
        self.save_conflict = None;
        self.quit_after_save = false;
        self.status_message =
            Some("Save canceled; your in-memory edits are still available".into());
    }

    pub fn change(&mut self) {
        self.mode = match self.cursor_vertical {
            0 => match self.files.get(self.file_index) {
                Some(name) => {
                    self.set_input(name.clone());
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
                    self.set_input(title.clone());
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
                    self.set_input(note.content.clone());
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
            self.set_input(description.clone());
            self.mode = EditorMode::ChangeListDescription;
        }
    }

    pub fn remove(&mut self) {
        let before = self.snapshot();
        match self.cursor_vertical {
            0 if self.file_index < self.files.len() => {
                self.pending_file_delete = Some(self.file_index);
            }
            1 => {
                if let Some(list) = self.lists.get_mut(self.file_index) {
                    list.take_section(self.list_index);
                }
            }
            2 => {
                if let Some(list) = self.lists.get_mut(self.file_index) {
                    list.remove_note(self.list_index, self.note_index);
                }
            }
            _ => {}
        }
        self.record_change(before);
        self.validate_and_update_indices();
    }

    pub fn confirm_file_delete(&mut self) {
        let Some(index) = self.pending_file_delete.take() else {
            return;
        };
        if index >= self.files.len() || index >= self.paths.len() || index >= self.lists.len() {
            return;
        }
        let before = self.snapshot();
        self.files.remove(index);
        let path = self.paths.remove(index);
        self.lists.remove(index);
        if !self.to_remove.contains(&path) {
            self.to_remove.push(path);
        }
        self.record_change(before);
        self.validate_and_update_indices();
        self.status_message = Some("File marked for deletion. Ctrl+Z restores it".into());
    }

    pub fn cancel_file_delete(&mut self) {
        self.pending_file_delete = None;
        self.status_message = Some("File deletion canceled".into());
    }

    fn snapshot(&self) -> EditSnapshot {
        EditSnapshot {
            files: self.files.clone(),
            paths: self.paths.clone(),
            lists: self.lists.clone(),
            to_remove: self.to_remove.clone(),
            renamed_from: self.renamed_from.clone(),
            selection: (
                self.file_index,
                self.list_index,
                self.note_index,
                self.cursor_vertical,
            ),
        }
    }

    fn record_change(&mut self, before: EditSnapshot) {
        let changed = before.files != *self.files
            || before.paths != *self.paths
            || before.to_remove != *self.to_remove
            || before.lists.len() != self.lists.len()
            || before
                .lists
                .iter()
                .zip(self.lists.iter())
                .any(|(a, b)| a.to_string() != b.to_string());
        if changed {
            if self.undo_stack.len() == 100 {
                self.undo_stack.remove(0);
            }
            self.undo_stack.push(before);
            self.redo_stack.clear();
        }
    }

    fn restore_snapshot(&mut self, snapshot: EditSnapshot) {
        // Paths removed by an undo after saving must be deleted on the next save.
        // Disk hashes always describe the real disk, never an earlier snapshot.
        let newly_removed = self
            .paths
            .iter()
            .filter(|path| !snapshot.paths.contains(path) && self.disk_hashes.contains_key(*path))
            .cloned()
            .collect::<Vec<_>>();
        *self.files = snapshot.files;
        *self.paths = snapshot.paths;
        *self.lists = snapshot.lists;
        *self.to_remove = snapshot.to_remove;
        for path in newly_removed {
            if !self.to_remove.contains(&path) {
                self.to_remove.push(path);
            }
        }
        self.to_remove.retain(|path| !self.paths.contains(path));
        self.renamed_from = snapshot.renamed_from;
        (
            self.file_index,
            self.list_index,
            self.note_index,
            self.cursor_vertical,
        ) = snapshot.selection;
        self.pending_file_delete = None;
        self.save_conflict = None;
        self.validate_and_update_indices();
    }

    pub fn undo(&mut self) {
        if let Some(snapshot) = self.undo_stack.pop() {
            self.redo_stack.push(self.snapshot());
            self.restore_snapshot(snapshot);
            self.status_message = Some("Undid the last edit".into());
        } else {
            self.status_message = Some("There is no edit to undo".into());
        }
    }

    pub fn redo(&mut self) {
        if let Some(snapshot) = self.redo_stack.pop() {
            self.undo_stack.push(self.snapshot());
            self.restore_snapshot(snapshot);
            self.status_message = Some("Redid the last edit".into());
        } else {
            self.status_message = Some("There is no edit to redo".into());
        }
    }

    pub fn undo_last_delete(&mut self) {
        self.undo();
    }

    pub fn has_unsaved_changes(&self) -> bool {
        (self.mode != EditorMode::Nothing && self.mode != EditorMode::Search)
            || self.pending_file_delete.is_some()
            || !self.to_remove.is_empty()
            || self
                .paths
                .iter()
                .zip(self.lists.iter())
                .any(|(path, list)| self.hashes.get(path).copied() != Some(calculate_hash(list)))
    }

    pub fn begin_discard_confirmation(&mut self) -> bool {
        if self.has_unsaved_changes() {
            self.confirm_discard = true;
            true
        } else {
            false
        }
    }

    pub fn cancel_discard(&mut self) {
        self.confirm_discard = false;
        self.status_message = Some("Discard canceled; your changes are still available".into());
    }

    pub fn begin_search(&mut self) {
        self.mode = EditorMode::Search;
        self.set_input(String::new());
        self.status_message = None;
    }

    pub fn complete_search(&mut self) {
        self.search_query = self.input.clone();
        self.input.clear();
        self.input_cursor = 0;
        self.mode = EditorMode::Nothing;
        self.search_index = None;
        self.refresh_search_matches();
        if self.search_matches.is_empty() {
            self.status_message = Some(format!("No matches for '{}'", self.search_query));
        } else {
            self.search_index = Some(0);
            self.show_search_match();
        }
    }

    pub fn search_next(&mut self, forward: bool) {
        if self.search_query.is_empty() {
            self.status_message = Some("Press / to search".into());
            return;
        }
        self.refresh_search_matches();
        if self.search_matches.is_empty() {
            self.search_index = None;
            self.status_message = Some(format!("No matches for '{}'", self.search_query));
            return;
        }
        let count = self.search_matches.len();
        let next = match (self.search_index, forward) {
            (None, _) => 0,
            (Some(current), true) => (current + 1) % count,
            (Some(0), false) => count - 1,
            (Some(current), false) => current - 1,
        };
        self.search_index = Some(next);
        self.show_search_match();
    }

    fn refresh_search_matches(&mut self) {
        if self.search_query.is_empty() {
            self.search_matches.clear();
            self.search_index = None;
            return;
        }
        let query = self.search_query.to_lowercase();
        let mut matches = Vec::new();
        let mut seen = HashSet::new();
        for (file_index, name) in self.files.iter().enumerate() {
            if name.to_lowercase().contains(&query) {
                add_search_match(
                    &mut matches,
                    &mut seen,
                    SearchMatch {
                        file_index,
                        section_index: None,
                        note_index: None,
                    },
                );
            }
            let Some(list) = self.lists.get(file_index) else {
                continue;
            };
            for section_index in 0..list.titles.len() {
                let title_match = list
                    .titles
                    .get(section_index)
                    .is_some_and(|title| title.to_lowercase().contains(&query));
                let description_match = list
                    .descriptions
                    .get(section_index)
                    .is_some_and(|description| description.to_lowercase().contains(&query));
                if title_match || description_match {
                    add_search_match(
                        &mut matches,
                        &mut seen,
                        SearchMatch {
                            file_index,
                            section_index: Some(section_index),
                            note_index: None,
                        },
                    );
                }
                if let Some(notes) = list.notes.get(section_index) {
                    for (note_index, note) in notes.iter().enumerate() {
                        if note.content.to_lowercase().contains(&query) {
                            add_search_match(
                                &mut matches,
                                &mut seen,
                                SearchMatch {
                                    file_index,
                                    section_index: Some(section_index),
                                    note_index: Some(note_index),
                                },
                            );
                        }
                    }
                }
            }
        }
        self.search_matches = matches;
        self.search_index = self
            .search_index
            .map(|index| index.min(self.search_matches.len().saturating_sub(1)));
    }

    fn show_search_match(&mut self) {
        let Some(index) = self.search_index else {
            return;
        };
        let Some(found) = self.search_matches.get(index) else {
            return;
        };
        self.file_index = found.file_index;
        if let Some(section_index) = found.section_index {
            self.list_index = section_index;
            if let Some(note_index) = found.note_index {
                self.note_index = note_index;
                self.cursor_vertical = 2;
            } else {
                self.note_index = 0;
                self.cursor_vertical = 1;
            }
        } else {
            self.list_index = 0;
            self.note_index = 0;
            self.cursor_vertical = 0;
        }
        self.validate_and_update_indices();
        self.status_message = Some(format!(
            "Search '{}': match {} of {} (n/N next/previous)",
            self.search_query,
            index + 1,
            self.search_matches.len()
        ));
    }

    pub fn create_file(&mut self) {
        self.mode = EditorMode::CreateFile;
        self.set_input(String::new());
    }

    pub fn create_list(&mut self) {
        self.mode = EditorMode::CreateList;
        self.set_input(String::new());
    }

    pub fn create_note(&mut self) {
        self.mode = EditorMode::CreateNote;
        self.set_input(String::new());
    }

    pub fn insert_input_char(&mut self, character: char) {
        let byte_index = grapheme_boundary(&self.input, self.input_cursor);
        self.input.insert(byte_index, character);
        self.input_cursor = grapheme_index_at_byte(&self.input, byte_index + character.len_utf8());
    }

    pub fn move_input_left(&mut self) {
        self.input_cursor = self.input_cursor.saturating_sub(1);
    }

    pub fn move_input_right(&mut self) {
        self.input_cursor = (self.input_cursor + 1).min(self.input.graphemes(true).count());
    }

    pub fn move_input_home(&mut self) {
        self.input_cursor = 0;
    }

    pub fn move_input_end(&mut self) {
        self.input_cursor = self.input.graphemes(true).count();
    }

    pub fn backspace_input(&mut self) {
        if self.input_cursor == 0 {
            return;
        }
        let start = grapheme_boundary(&self.input, self.input_cursor - 1);
        let end = grapheme_boundary(&self.input, self.input_cursor);
        self.input.replace_range(start..end, "");
        self.input_cursor -= 1;
    }

    pub fn delete_input(&mut self) {
        let count = self.input.graphemes(true).count();
        if self.input_cursor >= count {
            return;
        }
        let start = grapheme_boundary(&self.input, self.input_cursor);
        let end = grapheme_boundary(&self.input, self.input_cursor + 1);
        self.input.replace_range(start..end, "");
    }

    pub fn input_cursor_display_width(&self) -> usize {
        let byte_index = grapheme_boundary(&self.input, self.input_cursor);
        UnicodeWidthStr::width(&self.input[..byte_index])
    }

    fn set_input(&mut self, input: String) {
        self.input = input;
        self.move_input_end();
    }

    pub fn handle_enter(&mut self) {
        let before = self.snapshot();
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
                            if !self.to_remove.contains(&current_path) {
                                self.to_remove.push(current_path.clone());
                            }
                            let original_path = self
                                .renamed_from
                                .remove(&current_path)
                                .unwrap_or_else(|| current_path.clone());
                            if new_path != original_path {
                                self.renamed_from.insert(new_path.clone(), original_path);
                            }
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
            EditorMode::Search => self.complete_search(),
            EditorMode::Nothing => {}
        }
        self.record_change(before);
        self.validate_and_update_indices();
    }

    fn finish_input(&mut self) {
        self.input.clear();
        self.input_cursor = 0;
        self.mode = EditorMode::Nothing;
    }
}

fn grapheme_boundary(input: &str, grapheme_index: usize) -> usize {
    input
        .grapheme_indices(true)
        .nth(grapheme_index)
        .map_or(input.len(), |(byte_index, _)| byte_index)
}

fn grapheme_index_at_byte(input: &str, byte_index: usize) -> usize {
    input
        .grapheme_indices(true)
        .take_while(|(start, _)| *start < byte_index)
        .count()
}

fn add_search_match(
    matches: &mut Vec<SearchMatch>,
    seen: &mut HashSet<SearchMatch>,
    found: SearchMatch,
) {
    if seen.insert(found.clone()) {
        matches.push(found);
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

fn hash_file(path: &Path) -> io::Result<Option<u64>> {
    match fs::read(path) {
        Ok(contents) => Ok(Some(calculate_hash(&contents))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::{App, SaveConflictKind};
    use crate::todo::{parse_markdown, FileList};
    use crate::util::calculate_hash;
    use std::collections::HashMap;
    use std::fs;

    type AppData = (
        Vec<String>,
        Vec<std::path::PathBuf>,
        Vec<FileList>,
        HashMap<std::path::PathBuf, u64>,
        HashMap<std::path::PathBuf, u64>,
        Vec<std::path::PathBuf>,
    );

    fn app_data(root: &std::path::Path, contents: &str) -> AppData {
        let path = root.join("todos.md");
        fs::write(&path, contents).unwrap();
        let list = parse_markdown(contents);
        let hashes = HashMap::from([(path.clone(), calculate_hash(&list))]);
        let disk_hashes = HashMap::from([(path.clone(), calculate_hash(&contents.as_bytes()))]);
        (
            vec!["todos".into()],
            vec![path],
            vec![list],
            hashes,
            disk_hashes,
            Vec::new(),
        )
    }

    #[test]
    fn undo_redo_restores_edits_and_renames_across_saves() {
        let directory = tempfile::tempdir().unwrap();
        let original = "# Work\r\n- [ ] task\r\n";
        let (mut files, mut paths, mut lists, mut hashes, mut disk_hashes, mut removed) =
            app_data(directory.path(), original);
        let mut app = App::new(
            &mut files,
            &mut paths,
            &mut lists,
            &mut hashes,
            &mut disk_hashes,
            directory.path(),
            &mut removed,
        );
        app.cursor_vertical = 2;
        app.change();
        app.set_input("edited".into());
        app.handle_enter();
        app.save().unwrap();
        app.undo();
        app.save().unwrap();
        assert_eq!(fs::read_to_string(&app.paths[0]).unwrap(), original);
        app.redo();
        assert_eq!(app.lists[0].notes[0][0].content, "edited");
        app.cycle_note_state();
        app.undo();
        assert_eq!(app.lists[0].notes[0][0].state, crate::todo::NoteEnum::Open);
        app.redo();
        assert_eq!(app.lists[0].notes[0][0].state, crate::todo::NoteEnum::Done);

        app.cursor_vertical = 0;
        app.change();
        app.set_input("renamed".into());
        app.handle_enter();
        app.save().unwrap();
        assert!(!directory.path().join("todos.md").exists());
        app.undo();
        app.save().unwrap();
        assert!(directory.path().join("todos.md").exists());
        assert!(!directory.path().join("renamed.md").exists());
        app.redo();
        app.save().unwrap();
        assert!(directory.path().join("renamed.md").exists());
        assert!(!directory.path().join("todos.md").exists());
    }

    #[test]
    fn saved_file_creation_and_deletion_can_be_undone_and_redone() {
        let directory = tempfile::tempdir().unwrap();
        let (mut files, mut paths, mut lists, mut hashes, mut disk_hashes, mut removed) =
            app_data(directory.path(), "# Work\n- [ ] task\n");
        let mut app = App::new(
            &mut files,
            &mut paths,
            &mut lists,
            &mut hashes,
            &mut disk_hashes,
            directory.path(),
            &mut removed,
        );
        app.create_file();
        app.set_input("new".into());
        app.handle_enter();
        app.save().unwrap();
        app.undo();
        app.save().unwrap();
        assert!(!directory.path().join("new.md").exists());
        app.redo();
        app.save().unwrap();
        assert!(directory.path().join("new.md").exists());
        app.cursor_vertical = 0;
        app.remove();
        app.confirm_file_delete();
        app.save().unwrap();
        app.undo();
        app.save().unwrap();
        assert!(directory.path().join("new.md").exists());
        app.redo();
        app.save().unwrap();
        assert!(!directory.path().join("new.md").exists());
    }

    #[test]
    fn a_new_edit_discards_redo_history() {
        let directory = tempfile::tempdir().unwrap();
        let (mut files, mut paths, mut lists, mut hashes, mut disk_hashes, mut removed) =
            app_data(directory.path(), "# Work\n- [ ] task\n");
        let mut app = App::new(
            &mut files,
            &mut paths,
            &mut lists,
            &mut hashes,
            &mut disk_hashes,
            directory.path(),
            &mut removed,
        );
        app.cycle_note_state();
        app.undo();
        app.create_note();
        app.set_input("new task".into());
        app.handle_enter();
        app.redo();
        assert_eq!(app.lists[0].notes[0][0].state, crate::todo::NoteEnum::Open);
        assert_eq!(app.lists[0].notes[0].len(), 2);
    }

    #[test]
    fn save_without_quitting_preserves_intent_through_conflicts() {
        let directory = tempfile::tempdir().unwrap();
        let (mut files, mut paths, mut lists, mut hashes, mut disk_hashes, mut removed) =
            app_data(directory.path(), "# Work\n- [ ] task\n");
        let target = paths[0].clone();
        let mut app = App::new(
            &mut files,
            &mut paths,
            &mut lists,
            &mut hashes,
            &mut disk_hashes,
            directory.path(),
            &mut removed,
        );
        app.lists[0].notes[0][0].content = "local edit".into();
        assert!(app.file_is_dirty(0));
        fs::write(&target, "# Work\n- [ ] external edit\n").unwrap();
        assert!(!app.request_save(false).unwrap());
        assert!(app.save_conflict.is_some());
        app.overwrite_conflict();
        assert!(!app.retry_save().unwrap());
        assert!(!app.file_is_dirty(0));
        assert_eq!(app.status_message.as_deref(), Some("Saved"));
        assert!(fs::read_to_string(&target).unwrap().contains("local edit"));
        assert!(app.request_save(true).unwrap());
    }

    #[test]
    fn failed_save_keeps_edits_available_for_retry() {
        let directory = tempfile::tempdir().unwrap();
        let original = "# Work\n- [ ] task\n";
        let (mut files, mut paths, mut lists, mut hashes, mut disk_hashes, mut removed) =
            app_data(directory.path(), original);
        let target = paths[0].clone();
        let mut app = App::new(
            &mut files,
            &mut paths,
            &mut lists,
            &mut hashes,
            &mut disk_hashes,
            directory.path(),
            &mut removed,
        );
        app.lists[0].notes[0][0].content = "edited task".into();

        fs::remove_file(&target).unwrap();
        fs::create_dir(&target).unwrap();
        assert!(app.save().is_err());
        assert_eq!(app.lists[0].notes[0][0].content, "edited task");

        fs::remove_dir(&target).unwrap();
        fs::write(&target, original).unwrap();
        assert!(app.save().unwrap());
        assert!(fs::read_to_string(target).unwrap().contains("edited task"));
    }

    #[test]
    fn external_edits_require_an_explicit_overwrite() {
        let directory = tempfile::tempdir().unwrap();
        let original = "# Work\n- [ ] task\n";
        let (mut files, mut paths, mut lists, mut hashes, mut disk_hashes, mut removed) =
            app_data(directory.path(), original);
        let target = paths[0].clone();
        let mut app = App::new(
            &mut files,
            &mut paths,
            &mut lists,
            &mut hashes,
            &mut disk_hashes,
            directory.path(),
            &mut removed,
        );
        app.lists[0].notes[0][0].content = "local task".into();
        fs::write(&target, "# Work\n- [ ] external task\n").unwrap();

        assert!(!app.save().unwrap());
        assert_eq!(
            app.save_conflict.as_ref().unwrap().kind,
            SaveConflictKind::ExternalEdit
        );
        app.overwrite_conflict();
        assert!(app.save().unwrap());
        assert!(fs::read_to_string(target).unwrap().contains("local task"));
    }

    #[test]
    fn reloading_an_external_edit_discards_only_the_conflicted_file_edits() {
        let directory = tempfile::tempdir().unwrap();
        let original = "# Work\n- [ ] task\n";
        let (mut files, mut paths, mut lists, mut hashes, mut disk_hashes, mut removed) =
            app_data(directory.path(), original);
        let target = paths[0].clone();
        let mut app = App::new(
            &mut files,
            &mut paths,
            &mut lists,
            &mut hashes,
            &mut disk_hashes,
            directory.path(),
            &mut removed,
        );
        app.lists[0].notes[0][0].content = "local edit".into();
        fs::write(&target, "# Work\n- [ ] external edit\n").unwrap();
        app.lists
            .push(parse_markdown("# Another\n- [ ] local edit\n"));
        app.files.push("another".into());
        app.paths.push(directory.path().join("another.md"));

        assert!(!app.save().unwrap());
        app.reload_conflict().unwrap();
        assert_eq!(app.lists[0].notes[0][0].content, "external edit");
        assert_eq!(app.lists[1].notes[0][0].content, "local edit");
        assert_eq!(
            fs::read_to_string(target).unwrap(),
            "# Work\n- [ ] external edit\n"
        );
    }

    #[test]
    fn reloading_a_deleted_file_cancels_the_delete_without_duplicate_undo() {
        let directory = tempfile::tempdir().unwrap();
        let (mut files, mut paths, mut lists, mut hashes, mut disk_hashes, mut removed) =
            app_data(directory.path(), "# Work\n- [ ] task\n");
        let target = paths[0].clone();
        let mut app = App::new(
            &mut files,
            &mut paths,
            &mut lists,
            &mut hashes,
            &mut disk_hashes,
            directory.path(),
            &mut removed,
        );
        app.remove();
        app.confirm_file_delete();
        fs::write(&target, "# Work\n- [ ] external task\n").unwrap();

        assert!(!app.save().unwrap());
        app.reload_conflict().unwrap();
        assert_eq!(app.paths.len(), 1);
        assert_eq!(app.lists[0].notes[0][0].content, "external task");
        app.undo_last_delete();
        assert_eq!(app.paths.len(), 1);
        assert!(app.save().unwrap());
    }

    #[test]
    fn chained_renames_keep_the_original_file_when_reloading_a_collision() {
        let directory = tempfile::tempdir().unwrap();
        let (mut files, mut paths, mut lists, mut hashes, mut disk_hashes, mut removed) =
            app_data(directory.path(), "# Work\n- [ ] task\n");
        let original = paths[0].clone();
        let target = directory.path().join("target.md");
        let mut app = App::new(
            &mut files,
            &mut paths,
            &mut lists,
            &mut hashes,
            &mut disk_hashes,
            directory.path(),
            &mut removed,
        );

        app.change();
        app.set_input("middle".into());
        app.handle_enter();
        app.change();
        app.set_input("target".into());
        app.handle_enter();
        assert_eq!(app.renamed_from.get(&target), Some(&original));
        fs::write(&target, "# Existing target\n").unwrap();

        assert!(!app.save().unwrap());
        app.reload_conflict().unwrap();
        assert!(app.paths.contains(&original));
        assert!(original.exists());
        assert!(app.save().unwrap());
        assert!(original.exists());
        assert_eq!(fs::read_to_string(&target).unwrap(), "# Existing target\n");
    }

    #[test]
    fn new_file_collision_is_never_overwritten_without_confirmation() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("new.md");
        fs::write(&target, "external contents").unwrap();
        let mut files = vec!["new".into()];
        let mut paths = vec![target.clone()];
        let mut lists = vec![FileList::from_parts(
            vec!["Local".into()],
            vec![String::new()],
            vec![Vec::new()],
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
            directory.path(),
            &mut removed,
        );

        assert!(!app.save().unwrap());
        assert_eq!(
            app.save_conflict.as_ref().unwrap().kind,
            SaveConflictKind::DestinationExists
        );
        assert_eq!(fs::read_to_string(&target).unwrap(), "external contents");
        app.overwrite_conflict();
        assert!(app.save().unwrap());
        assert!(fs::read_to_string(target).unwrap().contains("# Local"));
    }

    #[test]
    fn destination_collision_can_reload_the_existing_file() {
        let directory = tempfile::tempdir().unwrap();
        let target = directory.path().join("new.md");
        let external = "# Existing\n- [ ] keep\n";
        fs::write(&target, external).unwrap();
        let mut files = vec!["new".into()];
        let mut paths = vec![target.clone()];
        let mut lists = vec![FileList::from_parts(
            vec!["Local".into()],
            vec![String::new()],
            vec![Vec::new()],
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
            directory.path(),
            &mut removed,
        );

        assert!(!app.save().unwrap());
        assert_eq!(
            app.save_conflict.as_ref().unwrap().kind,
            SaveConflictKind::DestinationExists
        );
        app.reload_conflict().unwrap();
        assert_eq!(app.lists[0].titles[0], "Existing");
        assert!(app.save().unwrap());
        assert_eq!(fs::read_to_string(target).unwrap(), external);
    }

    #[test]
    fn input_editing_uses_graphemes_and_keeps_cursor_on_boundaries() {
        let directory = tempfile::tempdir().unwrap();
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
            directory.path(),
            &mut removed,
        );

        app.set_input("A👩‍💻e\u{301}Z".to_owned());
        app.move_input_left();
        app.backspace_input();
        assert_eq!(app.input, "A👩‍💻Z");
        app.insert_input_char('界');
        assert_eq!(app.input, "A👩‍💻界Z");
        app.move_input_home();
        app.delete_input();
        assert_eq!(app.input, "👩‍💻界Z");
        assert_eq!(app.input_cursor_display_width(), 0);
    }

    #[test]
    fn note_and_section_deletions_can_be_undone_before_save() {
        let directory = tempfile::tempdir().unwrap();
        let contents = "# One\n- [ ] task\n\n# Two\n## preserved\n";
        let (mut files, mut paths, mut lists, mut hashes, mut disk_hashes, mut removed) =
            app_data(directory.path(), contents);
        let mut app = App::new(
            &mut files,
            &mut paths,
            &mut lists,
            &mut hashes,
            &mut disk_hashes,
            directory.path(),
            &mut removed,
        );

        app.cursor_vertical = 2;
        app.remove();
        assert!(app.lists[0].notes[0].is_empty());
        app.undo_last_delete();
        assert_eq!(app.lists[0].notes[0][0].content, "task");

        app.cursor_vertical = 1;
        app.list_index = 1;
        app.remove();
        assert_eq!(app.lists[0].titles, ["One"]);
        app.undo_last_delete();
        assert_eq!(app.lists[0].titles, ["One", "Two"]);
        assert!(app.lists[0].to_string().contains("## preserved"));
    }

    #[test]
    fn whole_file_delete_requires_confirmation_and_can_be_undone() {
        let directory = tempfile::tempdir().unwrap();
        let (mut files, mut paths, mut lists, mut hashes, mut disk_hashes, mut removed) =
            app_data(directory.path(), "# Work\n- [ ] task\n");
        let target = paths[0].clone();
        let mut app = App::new(
            &mut files,
            &mut paths,
            &mut lists,
            &mut hashes,
            &mut disk_hashes,
            directory.path(),
            &mut removed,
        );

        app.cursor_vertical = 0;
        app.remove();
        assert_eq!(app.paths.len(), 1);
        assert_eq!(app.pending_file_delete, Some(0));
        app.confirm_file_delete();
        assert!(app.paths.is_empty());
        app.undo_last_delete();
        assert_eq!(app.paths[0], target);
        assert!(app.to_remove.is_empty());
    }

    #[test]
    fn discard_confirmation_does_not_clear_in_memory_edits() {
        let directory = tempfile::tempdir().unwrap();
        let (mut files, mut paths, mut lists, mut hashes, mut disk_hashes, mut removed) =
            app_data(directory.path(), "# Work\n- [ ] task\n");
        let mut app = App::new(
            &mut files,
            &mut paths,
            &mut lists,
            &mut hashes,
            &mut disk_hashes,
            directory.path(),
            &mut removed,
        );
        app.lists[0].notes[0][0].content = "local edit".into();

        assert!(app.begin_discard_confirmation());
        app.cancel_discard();
        assert_eq!(app.lists[0].notes[0][0].content, "local edit");
        assert!(!app.confirm_discard);
    }

    #[test]
    fn search_matches_names_descriptions_and_tasks_case_insensitively() {
        let directory = tempfile::tempdir().unwrap();
        let contents = "# Planning\nQuartz notes\n- [ ] Fix quartz edge case\n";
        let (mut files, mut paths, mut lists, mut hashes, mut disk_hashes, mut removed) =
            app_data(directory.path(), contents);
        files[0] = "QuArTz".into();
        let mut app = App::new(
            &mut files,
            &mut paths,
            &mut lists,
            &mut hashes,
            &mut disk_hashes,
            directory.path(),
            &mut removed,
        );

        app.begin_search();
        for character in "QUARTZ".chars() {
            app.insert_input_char(character);
        }
        app.handle_enter();
        assert_eq!(app.cursor_vertical, 0);
        app.search_next(true);
        assert_eq!(app.cursor_vertical, 1);
        app.search_next(true);
        assert_eq!(app.cursor_vertical, 2);
        app.search_next(false);
        assert_eq!(app.cursor_vertical, 1);
    }

    #[test]
    fn search_with_no_matches_is_safe_on_empty_data() {
        let directory = tempfile::tempdir().unwrap();
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
            directory.path(),
            &mut removed,
        );

        app.begin_search();
        app.insert_input_char('x');
        app.handle_enter();
        assert_eq!(app.file_index, 0);
        assert_eq!(app.search_index, None);
        app.search_next(true);
        assert_eq!(app.file_index, 0);
    }
}
