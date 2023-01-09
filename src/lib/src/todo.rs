use crate::ui::app::EMPTY_NOTE_VEC;
use std::hash::{Hash, Hasher};
use std::process::exit;
use std::{
    fmt,
    fs::File,
    io::{prelude::*, BufReader},
    path::Path,
};

pub const MAIN_DIR: &str = "yoku";
pub const STARTER_FILE: &str = "tutorial.md";
pub const STARTER_FILE_CONTENT: &str =
    "# Start\n\nThis is a simple todo list\n\n- [ ] you may change note state with Enter, Spacebar, x, +, - or delete it with r\n- [ ] navigation keys include WASD, HJKL and arrow keys\n\
    \n# Create\n\nThis list contains shortcuts related to creating new files\n\n- [ ] u = create new file (press enter to confirm)\n- [ ] i = create new list (press enter to confirm)\n- [ ] o = create new note (press enter to confirm)\n\
    \n# Modify\n\nThis list contains shortcuts related to modifying data\n\n- [ ] e = edit current file/note/list\n- [ ] Ctrl + e = edit current list's description \n- [ ] r = remove current file/note/list\n- [ ] use the Escape key to unselect the current note\n\
    \n# Exiting\n\n- [ ] q = exit and save\n- [ ] Ctrl + q  = exit and discard changes\n- [ ] Ctrl + C  = exit and discard changes\n\n";

pub const STARTER_FILE_TITLE: &str = "Todo";
pub const STARTER_FILE_DESCRIPTION: &str = "This is a simple todo list";
pub const STARTER_FILE_NOTE: &str = "you may check this";

#[derive(Clone, Debug)]
pub struct FileList {
    pub titles: Vec<String>,
    pub descriptions: Vec<String>,
    pub notes: Vec<Vec<Note>>,
}

impl FileList {
    pub fn remove(&mut self, index: usize) {
        if self.titles.len() <= index + 1 {
            self.titles.remove(index);
        }
        if self.descriptions.len() <= index + 1 {
            self.descriptions.remove(index);
        }
        if self.notes.len() <= index + 1 {
            self.notes.remove(index);
        }
    }
    pub fn write(&self, path: &Path) {
        let mut write_string = self.to_string();
        // Add trailing newline
        write_string.push('\n');

        let mut file = File::open(&path).unwrap_or_else(|_| {
            File::create(&path).unwrap_or_else(|_| panic!("Could not create file {:?}", path))
        });

        file.write_all(write_string.as_bytes())
            .unwrap_or_else(|_| panic!("Could not write to file {:?}", path));
    }
}

impl fmt::Display for FileList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut write_string = String::new();
        for i in 0..self.titles.len() {
            if i > 0 {
                write_string.push('\n');
            }
            let title = &format!("# {}\n", self.titles.get(i).unwrap());
            write_string.push_str(title);
            write_string = write_string.trim().to_string();
            write_string.push('\n');

            let description = &self
                .descriptions
                .get(i)
                .unwrap_or(&"".to_string())
                .to_string();
            if !description.is_empty() {
                write_string.push_str(description);
                write_string = write_string.trim().to_string();
                write_string.push_str("\n\n");
            }

            let note_vec = self.notes.get(i).unwrap_or(EMPTY_NOTE_VEC);
            for note in note_vec.iter() {
                write_string.push_str(&note.to_string());
                write_string = write_string.trim().to_string();
                write_string.push('\n');
            }
        }

        write!(f, "{}", write_string)
    }
}

impl Hash for FileList {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.to_string().hash(state);
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum NoteEnum {
    OPEN,
    DONE,
    REJECTED,
}

#[derive(Clone, Debug)]
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
            NoteEnum::DONE => format!("{} [x] {}", start, self.content),
            NoteEnum::OPEN => format!("{} [ ] {}", start, self.content),
            NoteEnum::REJECTED => format!("{} [-] {}", start, self.content),
        }
    }
}

impl fmt::Display for Note {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            match self.state {
                NoteEnum::DONE => format!("- [x] {}", self.content),
                NoteEnum::OPEN => format!("- [ ] {}", self.content),
                NoteEnum::REJECTED => format!("- [-] {}", self.content),
            }
        )
    }
}

pub fn extract_filename(path: &Path) -> String {
    match path.file_name() {
        Some(value) => match value.to_os_string().into_string() {
            Ok(value) => value,
            _ => exit(1),
        },
        _ => exit(1),
    }
}

pub fn extract_naked_filename(path: &Path) -> String {
    let mut working_path = path.to_path_buf();
    match path.file_name() {
        Some(_value) => {
            working_path.set_extension("");
            match working_path.file_name() {
                Some(value) => match value.to_os_string().into_string() {
                    Ok(value) => value,
                    _ => exit(1),
                },
                _ => exit(1),
            }
        }
        _ => exit(1),
    }
}

// Read lines from file
pub fn lines_from_file(filename: impl AsRef<Path>) -> Vec<String> {
    let file = File::open(filename).expect("no such file");
    let buf = BufReader::new(file);
    buf.lines()
        .map(|l| l.expect("Could not parse line"))
        .collect()
}

pub fn parse_lines(lines: Vec<String>) -> FileList {
    let mut titles: Vec<String> = Vec::new();
    let mut descriptions: Vec<String> = Vec::new();
    let mut notes: Vec<Vec<Note>> = Vec::new();

    // Working data designed to be consumed
    let mut working_title: &String = &String::new();
    let mut working_description: String = String::new();
    let mut working_notes: Vec<Note> = Vec::new();

    for line in lines {
        if line.starts_with("# ") {
            if !working_title.is_empty() {
                descriptions.push((&working_description).to_string());
                notes.push(working_notes)
            }
            let title = line.split("# ").collect();
            titles.push(title);
            working_title = titles.last().unwrap();
            working_description = String::new();
            working_notes = Vec::new();
        } else if line.starts_with("- ") {
            if !working_title.is_empty() {
                if line.starts_with("- [x]") {
                    let content = line.split("- [x] ").collect();
                    working_notes.push(Note {
                        content,
                        state: NoteEnum::DONE,
                    });
                } else if line.starts_with("- [ ]") {
                    let content = line.split("- [ ] ").collect();
                    working_notes.push(Note {
                        content,
                        state: NoteEnum::OPEN,
                    });
                } else if line.starts_with("- []") {
                    let content = line.split("- [] ").collect();
                    working_notes.push(Note {
                        content,
                        state: NoteEnum::OPEN,
                    });
                } else if line.starts_with("- [-]") {
                    let content = line.split("- [-] ").collect();
                    working_notes.push(Note {
                        content,
                        state: NoteEnum::REJECTED,
                    });
                }
            }
        } else if !working_title.is_empty() {
            // Add space on new line
            if !working_description.is_empty() {
                working_description.push(' ')
            }
            working_description.push_str(&line);
        }
    }

    // Add last
    // if !descriptions.is_empty() || descriptions.len() + 1 == titles.len() {
    //     descriptions.push(String::new());
    // } else {
    descriptions.push(working_description);
    // }
    // if !notes.is_empty() || descriptions.len() + 1 == titles.len() {
    //     notes.push(Vec::new());
    // } else {
    notes.push(working_notes);
    // }

    FileList {
        titles,
        descriptions,
        notes,
    }
}
