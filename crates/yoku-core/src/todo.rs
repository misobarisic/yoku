use std::hash::{Hash, Hasher};
use std::{
    fmt, fs,
    io::{self, Write},
    path::Path,
};
use tempfile::NamedTempFile;

pub const MAIN_DIR: &str = "yoku";
pub const STARTER_FILE: &str = "tutorial.md";
pub const STARTER_FILE_CONTENT: &str =
    "# Start\n\nThis is a simple todo list\n\n- [ ] you may check a note state with Enter, Spacebar, x, + or -; delete it with r\n- [ ] navigation keys include WASD, HJKL and arrow keys\n\n# Create\n\nThis list contains shortcuts related to creating new files\n\n- [ ] u = create new file (press enter to confirm)\n- [ ] i = create new list (press enter to confirm)\n- [ ] o = create new note (press enter to confirm)\n\n# Modify\n\nThis list contains shortcuts related to modifying data\n\n- [ ] e = edit current file, note or list\n- [ ] Ctrl + e = edit current list's description\n- [ ] r = remove current file, note or list\n- [ ] use Escape to unselect the current note\n\n# Exiting\n\n- [ ] q = exit and save\n- [ ] Ctrl + q = exit and discard changes\n- [ ] Ctrl + C = exit and discard changes\n";

pub const STARTER_FILE_TITLE: &str = "Todo";
pub const STARTER_FILE_DESCRIPTION: &str = "This is a simple todo list";
pub const STARTER_FILE_NOTE: &str = "you may check this";

#[derive(Clone, Debug, Default)]
pub struct FileList {
    pub titles: Vec<String>,
    pub descriptions: Vec<String>,
    pub notes: Vec<Vec<Note>>,
}

impl FileList {
    pub fn remove(&mut self, index: usize) {
        if index < self.titles.len() {
            self.titles.remove(index);
        }
        if index < self.descriptions.len() {
            self.descriptions.remove(index);
        }
        if index < self.notes.len() {
            self.notes.remove(index);
        }
    }

    /// Write a complete replacement beside the destination, then rename it into place.
    pub fn write(&self, path: &Path) -> io::Result<()> {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut temporary = NamedTempFile::new_in(parent)?;
        temporary.write_all(self.to_string().as_bytes())?;
        if let Ok(metadata) = fs::metadata(path) {
            temporary
                .as_file()
                .set_permissions(metadata.permissions())?;
        }
        temporary.as_file().sync_all()?;
        temporary
            .persist(path)
            .map(|_| ())
            .map_err(|error| error.error)
    }
}

impl fmt::Display for FileList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, title) in self.titles.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
            }
            writeln!(f, "# {title}")?;

            let description = self
                .descriptions
                .get(index)
                .map(String::as_str)
                .unwrap_or("");
            if !description.is_empty() {
                writeln!(f, "{description}")?;
                writeln!(f)?;
            }

            if let Some(notes) = self.notes.get(index) {
                for note in notes {
                    writeln!(f, "{note}")?;
                }
            }
        }
        Ok(())
    }
}

impl Hash for FileList {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.to_string().hash(state);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteEnum {
    Open,
    Done,
    Rejected,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Note {
    pub content: String,
    pub state: NoteEnum,
}

impl Note {
    pub fn set_content(&mut self, content: String) -> &mut Self {
        self.content = content;
        self
    }

    pub fn set_state(&mut self, state: NoteEnum) -> &mut Self {
        self.state = state;
        self
    }

    pub fn to_string_custom(&self, start: &str) -> String {
        match self.state {
            NoteEnum::Done => format!("{start} [x] {}", self.content),
            NoteEnum::Open => format!("{start} [ ] {}", self.content),
            NoteEnum::Rejected => format!("{start} [-] {}", self.content),
        }
    }
}

impl fmt::Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.state {
            NoteEnum::Done => write!(f, "- [x] {}", self.content),
            NoteEnum::Open => write!(f, "- [ ] {}", self.content),
            NoteEnum::Rejected => write!(f, "- [-] {}", self.content),
        }
    }
}

pub fn extract_naked_filename(path: &Path) -> io::Result<String> {
    path.file_stem()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "file name is not valid UTF-8"))
}

pub fn parse_lines(lines: Vec<String>) -> FileList {
    let mut file_list = FileList::default();
    let mut current: Option<(String, String, Vec<Note>)> = None;

    for line in lines {
        if let Some(title) = line.strip_prefix("# ") {
            if let Some((title, description, notes)) = current.take() {
                file_list.titles.push(title);
                file_list.descriptions.push(description);
                file_list.notes.push(notes);
            }
            current = Some((title.to_owned(), String::new(), Vec::new()));
            continue;
        }

        let Some((_, description, notes)) = current.as_mut() else {
            continue;
        };

        if let Some(note_line) = line.strip_prefix("- ") {
            let parsed_note = [
                ("[x] ", NoteEnum::Done),
                ("[ ] ", NoteEnum::Open),
                ("[] ", NoteEnum::Open),
                ("[-] ", NoteEnum::Rejected),
            ]
            .into_iter()
            .find_map(|(prefix, state)| {
                note_line.strip_prefix(prefix).map(|content| Note {
                    content: content.to_owned(),
                    state,
                })
            });
            if let Some(note) = parsed_note {
                notes.push(note);
            }
        } else {
            if !description.is_empty() {
                description.push(' ');
            }
            description.push_str(&line);
        }
    }

    if let Some((title, description, notes)) = current {
        file_list.titles.push(title);
        file_list.descriptions.push(description);
        file_list.notes.push(notes);
    }

    // Keep the per-section vectors aligned even when reading malformed input.
    while file_list.descriptions.len() < file_list.titles.len() {
        file_list.descriptions.push(String::new());
    }
    while file_list.notes.len() < file_list.titles.len() {
        file_list.notes.push(Vec::new());
    }

    file_list
}
