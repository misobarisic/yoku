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
    source: Option<SourceDocument>,
}

#[derive(Clone, Debug)]
struct SourceDocument {
    preamble: Vec<SourceLine>,
    sections: Vec<SourceSection>,
    newline: String,
    final_newline: bool,
}

#[derive(Clone, Debug)]
struct SourceSection {
    heading: SourceLine,
    original_title: String,
    body: Vec<SourceBlock>,
}

#[derive(Clone, Debug)]
struct SourceBlock {
    line: SourceLine,
    kind: SourceBlockKind,
}

#[derive(Clone, Debug)]
enum SourceBlockKind {
    Raw,
    Description,
    Note {
        index: usize,
        original: Option<Note>,
    },
}

#[derive(Clone, Debug)]
struct SourceLine {
    text: String,
    ending: String,
}

impl FileList {
    pub const fn empty_const() -> Self {
        Self {
            titles: Vec::new(),
            descriptions: Vec::new(),
            notes: Vec::new(),
            source: None,
        }
    }

    pub fn from_parts(
        titles: Vec<String>,
        descriptions: Vec<String>,
        notes: Vec<Vec<Note>>,
    ) -> Self {
        let mut file_list = Self {
            titles,
            descriptions,
            notes,
            source: None,
        };
        file_list.align_sections();
        file_list
    }

    fn align_sections(&mut self) {
        self.descriptions.resize(self.titles.len(), String::new());
        self.notes.resize_with(self.titles.len(), Vec::new);
        self.descriptions.truncate(self.titles.len());
        self.notes.truncate(self.titles.len());
    }

    pub fn push_section(&mut self, title: String) {
        self.titles.push(title.clone());
        self.descriptions.push(String::new());
        self.notes.push(Vec::new());
        if let Some(source) = &mut self.source {
            if let Some(previous) = source.sections.last_mut() {
                if previous
                    .body
                    .last()
                    .is_none_or(|block| !block.line.text.is_empty())
                {
                    previous.body.push(SourceBlock {
                        line: SourceLine {
                            text: String::new(),
                            ending: source.newline.clone(),
                        },
                        kind: SourceBlockKind::Raw,
                    });
                }
            }
            source.sections.push(SourceSection {
                heading: SourceLine {
                    text: format!("# {title}"),
                    ending: source.newline.clone(),
                },
                original_title: title,
                body: Vec::new(),
            });
        }
    }

    pub fn push_note(&mut self, section_index: usize, note: Note) {
        let Some(section_notes) = self.notes.get_mut(section_index) else {
            return;
        };
        let note_index = section_notes.len();
        section_notes.push(note);
        if let Some(source) = &mut self.source {
            if let Some(section) = source.sections.get_mut(section_index) {
                let mut insert_at = section.body.len();
                while insert_at > 0
                    && section.body[insert_at - 1].line.text.is_empty()
                    && matches!(
                        section.body[insert_at - 1].kind,
                        SourceBlockKind::Description | SourceBlockKind::Raw
                    )
                {
                    insert_at -= 1;
                }
                section.body.insert(
                    insert_at,
                    SourceBlock {
                        line: SourceLine {
                            text: String::new(),
                            ending: source.newline.clone(),
                        },
                        kind: SourceBlockKind::Note {
                            index: note_index,
                            original: None,
                        },
                    },
                );
            }
        }
    }

    pub fn remove_note(&mut self, section_index: usize, note_index: usize) {
        let Some(section_notes) = self.notes.get_mut(section_index) else {
            return;
        };
        if note_index >= section_notes.len() {
            return;
        }
        section_notes.remove(note_index);
        if let Some(section) = self
            .source
            .as_mut()
            .and_then(|source| source.sections.get_mut(section_index))
        {
            section.body.retain_mut(|block| match &mut block.kind {
                SourceBlockKind::Note { index, .. } if *index == note_index => false,
                SourceBlockKind::Note { index, .. } if *index > note_index => {
                    *index -= 1;
                    true
                }
                _ => true,
            });
        }
    }
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
        if let Some(source) = &mut self.source {
            if index < source.sections.len() {
                source.sections.remove(index);
            }
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
        if let Some(source) = &self.source {
            let mut lines = source.preamble.clone();
            for (section_index, section) in source.sections.iter().enumerate() {
                let Some(title) = self.titles.get(section_index) else {
                    continue;
                };
                let mut heading = section.heading.clone();
                if title != &section.original_title {
                    heading.text = format!("# {title}");
                }
                lines.push(heading);

                let description = self
                    .descriptions
                    .get(section_index)
                    .map(String::as_str)
                    .unwrap_or("");
                let description_changed = description != section_description(section);
                let mut wrote_description = false;
                let note_count = self.notes.get(section_index).map_or(0, Vec::len);
                for block in &section.body {
                    if matches!(block.kind, SourceBlockKind::Note { .. })
                        && description_changed
                        && !wrote_description
                    {
                        if !description.is_empty() {
                            lines.push(SourceLine {
                                text: description.to_owned(),
                                ending: source.newline.clone(),
                            });
                        }
                        wrote_description = true;
                    }
                    match &block.kind {
                        SourceBlockKind::Raw => lines.push(block.line.clone()),
                        SourceBlockKind::Description if !description_changed => {
                            lines.push(block.line.clone());
                            wrote_description = true;
                        }
                        SourceBlockKind::Description if !wrote_description => {
                            if !description.is_empty() {
                                let mut replacement = block.line.clone();
                                replacement.text = description.to_owned();
                                lines.push(replacement);
                            }
                            wrote_description = true;
                        }
                        SourceBlockKind::Description => {}
                        SourceBlockKind::Note { index, original } => {
                            if *index < note_count {
                                let note = &self.notes[section_index][*index];
                                let mut line = block.line.clone();
                                if original.as_ref() != Some(note) {
                                    line.text = note.to_string();
                                }
                                lines.push(line);
                            }
                        }
                    }
                }
                if description_changed && !wrote_description && !description.is_empty() {
                    lines.push(SourceLine {
                        text: description.to_owned(),
                        ending: source.newline.clone(),
                    });
                }
            }
            write_source_lines(f, lines, source)?;
            return Ok(());
        }

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

fn section_description(section: &SourceSection) -> String {
    let mut description = String::new();
    for block in &section.body {
        if matches!(block.kind, SourceBlockKind::Description) {
            if !description.is_empty() {
                description.push(' ');
            }
            description.push_str(&block.line.text);
        }
    }
    description
}

fn write_source_lines(
    f: &mut fmt::Formatter<'_>,
    mut lines: Vec<SourceLine>,
    source: &SourceDocument,
) -> fmt::Result {
    if let Some(last) = lines.last_mut() {
        if source.final_newline && last.ending.is_empty() {
            last.ending.clone_from(&source.newline);
        } else if !source.final_newline {
            last.ending.clear();
        }
    }
    let interior_len = lines.len().saturating_sub(1);
    for line in &mut lines[..interior_len] {
        if line.ending.is_empty() {
            line.ending.clone_from(&source.newline);
        }
    }
    for line in lines {
        write!(f, "{}{}", line.text, line.ending)?;
    }
    Ok(())
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
    parse_markdown(&lines.join("\n"))
}

pub fn parse_markdown(contents: &str) -> FileList {
    let lines = split_source_lines(contents);
    let newline = lines
        .iter()
        .find(|line| !line.ending.is_empty())
        .map(|line| line.ending.clone())
        .unwrap_or_else(|| "\n".to_owned());
    let final_newline = lines.last().is_some_and(|line| !line.ending.is_empty());
    let mut file_list = FileList::default();
    let mut preamble = Vec::new();
    let mut source_sections = Vec::new();
    let mut current: Option<SourceSection> = None;

    for line in lines {
        if let Some(title) = line.text.strip_prefix("# ") {
            let original_title = title.to_owned();
            if let Some(section) = current.take() {
                finish_source_section(section, &mut file_list, &mut source_sections);
            }
            current = Some(SourceSection {
                heading: line,
                original_title,
                body: Vec::new(),
            });
            continue;
        }

        let Some(section) = current.as_mut() else {
            preamble.push(line);
            continue;
        };
        let parsed_note = line.text.strip_prefix("- ").and_then(parse_note_line);
        let kind = if let Some(note) = parsed_note {
            SourceBlockKind::Note {
                index: 0,
                original: Some(note),
            }
        } else if is_raw_markdown(&line.text) {
            SourceBlockKind::Raw
        } else {
            SourceBlockKind::Description
        };
        section.body.push(SourceBlock { line, kind });
    }
    if let Some(section) = current {
        finish_source_section(section, &mut file_list, &mut source_sections);
    }

    file_list.source = Some(SourceDocument {
        preamble,
        sections: source_sections,
        newline,
        final_newline,
    });
    file_list
}

fn finish_source_section(
    mut section: SourceSection,
    file_list: &mut FileList,
    source_sections: &mut Vec<SourceSection>,
) {
    let mut description = String::new();
    let mut notes = Vec::new();
    for block in &mut section.body {
        match &mut block.kind {
            SourceBlockKind::Description => {
                if !description.is_empty() {
                    description.push(' ');
                }
                description.push_str(&block.line.text);
            }
            SourceBlockKind::Note { index, original } => {
                *index = notes.len();
                if let Some(note) = original {
                    notes.push(note.clone());
                }
            }
            SourceBlockKind::Raw => {}
        }
    }
    file_list.titles.push(section.original_title.clone());
    file_list.descriptions.push(description);
    file_list.notes.push(notes);
    source_sections.push(section);
}

fn split_source_lines(contents: &str) -> Vec<SourceLine> {
    if contents.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    let mut start = 0;
    for (index, byte) in contents.bytes().enumerate() {
        if byte == b'\n' {
            let mut end = index;
            let ending = if end > start && contents.as_bytes()[end - 1] == b'\r' {
                end -= 1;
                "\r\n"
            } else {
                "\n"
            };
            lines.push(SourceLine {
                text: contents[start..end].to_owned(),
                ending: ending.to_owned(),
            });
            start = index + 1;
        }
    }
    if start < contents.len() {
        lines.push(SourceLine {
            text: contents[start..].to_owned(),
            ending: String::new(),
        });
    }
    lines
}

fn parse_note_line(note_line: &str) -> Option<Note> {
    [
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
    })
}

fn is_raw_markdown(line: &str) -> bool {
    let trimmed = line.trim_start();
    [
        "#", ">", "```", "~~~", "|", "- ", "* ", "+ ", "1. ", "---", "___", "***", "<",
    ]
    .iter()
    .any(|prefix| trimmed.starts_with(prefix))
        || ["**", "__", "~~", "[", "](", "`"]
            .iter()
            .any(|marker| line.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::{parse_markdown, NoteEnum};

    #[test]
    fn preserves_unmodified_markdown_and_crlf_endings() {
        let source = "# Plans\r\nA description\r\n\r\n## Details\r\n> keep this quote\r\n- [ ] first\r\n- not a yoku task\r\n";
        assert_eq!(parse_markdown(source).to_string(), source);
    }

    #[test]
    fn edits_only_the_selected_heading_or_task_line() {
        let source = "# Plans\nA description\n\n## Details\n> keep this quote\n- [ ] first\n";
        let mut list = parse_markdown(source);
        list.titles[0] = "Roadmap".to_owned();
        list.notes[0][0].state = NoteEnum::Done;
        assert_eq!(
            list.to_string(),
            "# Roadmap\nA description\n\n## Details\n> keep this quote\n- [x] first\n"
        );
    }

    #[test]
    fn structural_edits_keep_other_sections_and_unknown_lines() {
        let source = "# One\nplain description\n- [ ] keep\n\n# Two\n## not a list\n";
        let mut list = parse_markdown(source);
        list.push_note(
            0,
            super::Note {
                content: "new task".to_owned(),
                state: NoteEnum::Open,
            },
        );
        list.remove_note(0, 0);
        list.push_section("Three".to_owned());
        assert_eq!(
            list.to_string(),
            "# One\nplain description\n- [ ] new task\n\n# Two\n## not a list\n\n# Three\n"
        );
    }

    #[test]
    fn preserves_missing_final_newline() {
        let source = "# One\n- [ ] task";
        assert_eq!(parse_markdown(source).to_string(), source);
    }
}
