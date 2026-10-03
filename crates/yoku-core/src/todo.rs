use crate::metadata::{parse_date, set_field};
use chrono::NaiveDate;
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use std::collections::{HashMap, HashSet};
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
    "# Start\n\nThis is a simple todo list\n\n- [ ] you may check a note state with Enter, Spacebar, x, + or -; delete it with r\n- [ ] navigation keys include WASD, HJKL and arrow keys\n\n# Create\n\nThis list contains shortcuts related to creating new files\n\n- [ ] u = create new file (press enter to confirm)\n- [ ] i = create new list (press enter to confirm)\n- [ ] o = create new note (press enter to confirm)\n\n# Modify\n\nThis list contains shortcuts related to modifying data\n\n- [ ] e = edit current file, note or list\n- [ ] Ctrl + e = edit current list's description\n- [ ] r = remove current file, note or list (file removal asks for confirmation)\n- [ ] Ctrl + z = undo the last edit; Ctrl + y = redo (history survives saves)\n- [ ] use Escape to unselect the current note\n\n# Search and help\n\n- [ ] / = search file names, list titles, descriptions and tasks\n- [ ] n/N = next/previous search match\n- [ ] ? or F1 = show keyboard help\n\n# Exiting\n\n- [ ] q = exit and save\n- [ ] Ctrl + q or Ctrl + C = confirm before discarding unsaved changes\n";

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
    heading: Option<SourceLine>,
    heading_prefix: String,
    heading_suffix: String,
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
        span_lines: usize,
    },
}

#[derive(Clone, Debug)]
struct SourceLine {
    text: String,
    ending: String,
}

#[derive(Clone, Debug)]
pub struct RemovedSection {
    title: String,
    description: String,
    notes: Vec<Note>,
    source_section: Option<SourceSection>,
}

#[derive(Clone, Debug)]
pub struct RemovedNote {
    note: Note,
    body_index: Option<usize>,
    source_block: Option<SourceBlock>,
}

/// A task, its subtasks, and continuation Markdown travel together.
pub struct TaskGroup {
    lines: Vec<SourceLine>,
    base_indent: usize,
}

impl TaskGroup {
    fn reindent(&mut self, depth: usize) {
        for line in &mut self.lines {
            let columns = indentation(&line.text);
            if columns >= self.base_indent && !line.text.is_empty() {
                line.text = format!(
                    "{}{}",
                    " ".repeat(columns - self.base_indent + depth),
                    line.text.trim_start_matches([' ', '\t'])
                );
            }
        }
        self.base_indent = depth;
    }
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
                heading: Some(SourceLine {
                    text: format!("# {title}"),
                    ending: source.newline.clone(),
                }),
                heading_prefix: "# ".into(),
                heading_suffix: String::new(),
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
                            span_lines: 1,
                        },
                    },
                );
            }
        }
    }

    pub fn remove_note(&mut self, section_index: usize, note_index: usize) -> Option<RemovedNote> {
        let section_notes = self.notes.get_mut(section_index)?;
        if note_index >= section_notes.len() {
            return None;
        }
        let note = section_notes.remove(note_index);
        let mut removed = RemovedNote {
            note,
            body_index: None,
            source_block: None,
        };
        if let Some(section) = self
            .source
            .as_mut()
            .and_then(|source| source.sections.get_mut(section_index))
        {
            if let Some(body_index) = section.body.iter().position(|block| {
                matches!(block.kind, SourceBlockKind::Note { index, .. } if index == note_index)
            }) {
                removed.body_index = Some(body_index);
                removed.source_block = Some(section.body.remove(body_index));
            }
            section.body.retain_mut(|block| match &mut block.kind {
                SourceBlockKind::Note { index, .. } if *index > note_index => {
                    *index -= 1;
                    true
                }
                _ => true,
            });
        }
        Some(removed)
    }

    pub fn restore_note(&mut self, section_index: usize, note_index: usize, removed: RemovedNote) {
        let Some(section_notes) = self.notes.get_mut(section_index) else {
            return;
        };
        let note_index = note_index.min(section_notes.len());
        section_notes.insert(note_index, removed.note);
        if let Some(section) = self
            .source
            .as_mut()
            .and_then(|source| source.sections.get_mut(section_index))
        {
            for block in &mut section.body {
                if let SourceBlockKind::Note { index, .. } = &mut block.kind {
                    if *index >= note_index {
                        *index += 1;
                    }
                }
            }
            if let Some(mut block) = removed.source_block {
                if let SourceBlockKind::Note { index, .. } = &mut block.kind {
                    *index = note_index;
                }
                let body_index = removed.body_index.unwrap_or(section.body.len());
                section
                    .body
                    .insert(body_index.min(section.body.len()), block);
            }
        }
    }

    pub fn take_section(&mut self, index: usize) -> Option<RemovedSection> {
        if index >= self.titles.len() {
            return None;
        }
        let title = self.titles.remove(index);
        let description = if index < self.descriptions.len() {
            self.descriptions.remove(index)
        } else {
            String::new()
        };
        let notes = if index < self.notes.len() {
            self.notes.remove(index)
        } else {
            Vec::new()
        };
        let source_section = self.source.as_mut().and_then(|source| {
            (index < source.sections.len()).then(|| source.sections.remove(index))
        });
        Some(RemovedSection {
            title,
            description,
            notes,
            source_section,
        })
    }

    pub fn restore_section(&mut self, index: usize, removed: RemovedSection) {
        let index = index.min(self.titles.len());
        self.titles.insert(index, removed.title);
        self.descriptions.insert(index, removed.description);
        self.notes.insert(index, removed.notes);
        if let Some(section) = self.source.as_mut() {
            if let Some(source_section) = removed.source_section {
                section
                    .sections
                    .insert(index.min(section.sections.len()), source_section);
            }
        }
    }

    pub fn note_depth(&self, section_index: usize, note_index: usize) -> usize {
        self.source
            .as_ref()
            .and_then(|source| source.sections.get(section_index))
            .and_then(|section| {
                section.body.iter().find(|block| {
                matches!(block.kind, SourceBlockKind::Note { index, .. } if index == note_index)
            })
            })
            .map_or(0, |block| indentation(&block.line.text))
    }

    pub fn section_level(&self, index: usize) -> usize {
        self.source
            .as_ref()
            .and_then(|source| source.sections.get(index))
            .map_or(1, |section| {
                section
                    .heading_prefix
                    .bytes()
                    .filter(|byte| *byte == b'#')
                    .count()
            })
    }

    pub fn remove_section_tree(&mut self, index: usize) {
        if index >= self.titles.len() {
            return;
        }
        let level = self.section_level(index);
        let count = 1
            + (index + 1..self.titles.len())
                .take_while(|child| self.section_level(*child) > level)
                .count();
        for _ in 0..count {
            self.take_section(index);
        }
    }

    fn reparse(&mut self) {
        *self = parse_markdown(&self.to_string());
    }

    pub fn take_task_group(
        &mut self,
        section_index: usize,
        note_index: usize,
    ) -> Option<TaskGroup> {
        self.reparse();
        let section = self.source.as_mut()?.sections.get_mut(section_index)?;
        let start = section.body.iter().position(|block| {
            matches!(block.kind, SourceBlockKind::Note { index, .. } if index == note_index)
        })?;
        let SourceBlockKind::Note { span_lines, .. } = section.body[start].kind else {
            return None;
        };
        let mut end = (start + span_lines).min(section.body.len());
        while end > start + 1 && section.body[end - 1].line.text.trim().is_empty() {
            end -= 1;
        }
        let base_indent = indentation(&section.body[start].line.text);
        let lines = section
            .body
            .drain(start..end)
            .map(|block| block.line)
            .collect();
        self.reparse();
        Some(TaskGroup { lines, base_indent })
    }

    pub fn insert_task_group(
        &mut self,
        section_index: usize,
        note_index: usize,
        mut group: TaskGroup,
        depth: usize,
    ) -> bool {
        self.reparse();
        let Some(source) = self.source.as_mut() else {
            return false;
        };
        let Some(section) = source.sections.get_mut(section_index) else {
            return false;
        };
        let insert_at = section.body.iter().position(|block| {
            matches!(block.kind, SourceBlockKind::Note { index, .. } if index == note_index)
        }).unwrap_or_else(|| {
            if let Some((start, block)) = section.body.iter().enumerate().rev()
                .find(|(_, block)| matches!(block.kind, SourceBlockKind::Note { .. })) {
                if let SourceBlockKind::Note { span_lines, .. } = block.kind {
                    let mut end = (start + span_lines).min(section.body.len());
                    while end > start + 1 && section.body[end - 1].line.text.trim().is_empty() { end -= 1; }
                    return end;
                }
            }
            let mut end = section.body.len();
            while end > 0 && section.body[end - 1].line.text.trim().is_empty() { end -= 1; }
            end
        });
        group.reindent(depth);
        let blocks = group
            .lines
            .into_iter()
            .map(|mut line| {
                line.ending.clone_from(&source.newline);
                SourceBlock {
                    line,
                    kind: SourceBlockKind::Raw,
                }
            })
            .collect::<Vec<_>>();
        section.body.splice(insert_at..insert_at, blocks);
        self.reparse();
        true
    }

    pub fn reorder_task(&mut self, section: usize, index: usize, down: bool) -> Option<usize> {
        self.reparse();
        let count = self.notes.get(section)?.len();
        if index >= count {
            return None;
        }
        let depth = self.note_depth(section, index);
        let target = if down {
            let sibling = (index + 1..count).find(|i| self.note_depth(section, *i) <= depth)?;
            if self.note_depth(section, sibling) != depth {
                return None;
            }
            (sibling + 1..count)
                .find(|i| self.note_depth(section, *i) <= depth)
                .unwrap_or(count)
        } else {
            let sibling = (0..index)
                .rev()
                .find(|i| self.note_depth(section, *i) <= depth)?;
            if self.note_depth(section, sibling) != depth {
                return None;
            }
            sibling
        };
        let group = self.take_task_group(section, index)?;
        let removed_count = count - self.notes.get(section).map_or(0, Vec::len);
        let target = if down { target - removed_count } else { target };
        self.insert_task_group(section, target, group, depth)
            .then_some(target)
    }

    pub fn indent_task(&mut self, section: usize, index: usize, outdent: bool) -> bool {
        self.reparse();
        let depth = self.note_depth(section, index);
        let parent = (0..index).rev().find(|i| {
            let previous = self.note_depth(section, *i);
            if outdent {
                previous < depth
            } else {
                previous <= depth
            }
        });
        let Some(parent) = parent else {
            return false;
        };
        let new_depth = if outdent {
            self.note_depth(section, parent)
        } else {
            let Some(block) = self.source.as_ref().and_then(|source| source.sections.get(section))
                .and_then(|section| section.body.iter().find(|block| matches!(block.kind, SourceBlockKind::Note { index, .. } if index == parent))) else { return false; };
            let Some(parts) = task_line_parts(&block.line.text) else {
                return false;
            };
            block.line.text[..parts.marker_start]
                .chars()
                .fold(0, |columns, c| {
                    if c == '\t' {
                        (columns / 4 + 1) * 4
                    } else {
                        columns + 1
                    }
                })
        };
        let Some(group) = self.take_task_group(section, index) else {
            return false;
        };
        self.insert_task_group(section, index, group, new_depth)
    }

    pub fn set_task_state(
        &mut self,
        section: usize,
        index: usize,
        state: NoteEnum,
        today: NaiveDate,
    ) -> Result<(), String> {
        let Some(note) = self.notes.get(section).and_then(|notes| notes.get(index)) else {
            return Ok(());
        };
        let metadata = note.metadata();
        if state != NoteEnum::Done || note.state == NoteEnum::Done || metadata.recurrence.is_none()
        {
            self.notes[section][index].state = state;
            return Ok(());
        }
        let base = metadata.due.unwrap_or(today);
        let next = metadata
            .recurrence
            .and_then(|repeat| repeat.next_due(base, today))
            .ok_or("The next recurring due date is outside the supported calendar")?;
        let content = set_field(&note.content, "due", &next.format("%Y-%m-%d").to_string());
        if self.notes[section]
            .iter()
            .any(|note| note.content == content)
        {
            self.notes[section][index].state = state;
            return Ok(());
        }

        let before = self.clone();
        let mut copy = self.clone();
        let mut group = copy
            .take_task_group(section, index)
            .ok_or("Could not copy the recurring task")?;
        let depth = group.base_indent;
        group.reindent(0);
        // Parse the copied subtree so examples in code blocks stay untouched.
        let text = group
            .lines
            .iter()
            .map(|line| format!("{}{}", line.text, line.ending))
            .collect::<String>();
        let mut upcoming = parse_markdown(&text);
        let notes = upcoming
            .notes
            .first_mut()
            .ok_or("Could not parse the recurring task")?;
        let count = notes.len();
        let shift = next.signed_duration_since(base);
        for (child, note) in notes.iter_mut().enumerate() {
            note.state = NoteEnum::Open;
            if child == 0 {
                note.content = content.clone();
            } else if let Some(due) = note.metadata().due {
                let due = due
                    .checked_add_signed(shift)
                    .ok_or("A recurring subtask date is outside the supported calendar")?;
                let date = due.format("%Y-%m-%d").to_string();
                parse_date(&date)?;
                note.content = set_field(&note.content, "due", &date);
            }
        }
        let group = upcoming
            .take_task_group(0, 0)
            .ok_or("Could not copy the next occurrence")?;
        for note in self.notes[section].iter_mut().skip(index).take(count) {
            note.state = NoteEnum::Done;
        }
        if !self.insert_task_group(section, index + count, group, depth) {
            *self = before;
            return Err("Could not insert the next occurrence".into());
        }
        Ok(())
    }
}

fn indentation(line: &str) -> usize {
    line.chars()
        .take_while(|c| matches!(c, ' ' | '\t'))
        .fold(0, |columns, c| {
            if c == '\t' {
                (columns / 4 + 1) * 4
            } else {
                columns + 1
            }
        })
}

impl FileList {
    pub fn remove(&mut self, index: usize) {
        let _ = self.take_section(index);
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

    /// Create a new destination without replacing a file created by another process.
    pub fn write_new(&self, path: &Path) -> io::Result<()> {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let mut temporary = NamedTempFile::new_in(parent)?;
        temporary.write_all(self.to_string().as_bytes())?;
        temporary.as_file().sync_all()?;
        temporary
            .persist_noclobber(path)
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
                if let Some(mut heading) = section.heading.clone() {
                    if title != &section.original_title {
                        heading.text = format!(
                            "{}{title}{}",
                            section.heading_prefix, section.heading_suffix
                        );
                    }
                    lines.push(heading);
                } else if title != &section.original_title {
                    lines.push(SourceLine {
                        text: format!("# {title}"),
                        ending: source.newline.clone(),
                    });
                }

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
                        SourceBlockKind::Note {
                            index, original, ..
                        } => {
                            if *index < note_count {
                                let note = &self.notes[section_index][*index];
                                let mut line = block.line.clone();
                                if original.as_ref() != Some(note) {
                                    line.text = render_task_line(&line.text, note);
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
    pub fn metadata(&self) -> crate::metadata::TaskMetadata {
        crate::metadata::TaskMetadata::parse(&self.content)
    }
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
    if lines.is_empty() {
        FileList::default()
    } else {
        parse_markdown(&format!("{}\n", lines.join("\n")))
    }
}

pub fn parse_markdown(contents: &str) -> FileList {
    let lines = split_source_lines(contents);
    let shape = markdown_shape(contents, &lines);
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

    for (line_index, line) in lines.into_iter().enumerate() {
        if shape.headings.contains(&line_index) {
            if let Some((title, prefix, suffix)) = heading_parts(&line.text) {
                if let Some(section) = current.take() {
                    finish_source_section(section, &mut file_list, &mut source_sections);
                }
                current = Some(SourceSection {
                    heading: Some(line),
                    heading_prefix: prefix,
                    heading_suffix: suffix,
                    original_title: title,
                    body: Vec::new(),
                });
                continue;
            }
        }
        let parsed_note = shape.tasks.get(&line_index);
        if current.is_none() && parsed_note.is_some() {
            current = Some(SourceSection {
                heading: None,
                heading_prefix: String::new(),
                heading_suffix: String::new(),
                original_title: "Inbox".into(),
                body: Vec::new(),
            });
        }
        let Some(section) = current.as_mut() else {
            preamble.push(line);
            continue;
        };
        let kind = if let Some((note, span_lines)) = parsed_note {
            SourceBlockKind::Note {
                index: 0,
                original: Some(note.clone()),
                span_lines: *span_lines,
            }
        } else if shape.raw.contains(&line_index) || is_raw_markdown(&line.text) {
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

#[derive(Default)]
struct MarkdownShape {
    headings: HashSet<usize>,
    tasks: HashMap<usize, (Note, usize)>,
    raw: HashSet<usize>,
}

fn markdown_shape(contents: &str, lines: &[SourceLine]) -> MarkdownShape {
    let mut offsets = Vec::with_capacity(lines.len());
    let mut offset = 0;
    for line in lines {
        offsets.push(offset);
        offset += line.text.len() + line.ending.len();
    }
    let mut shape = MarkdownShape::default();
    let mut item_depth = 0;
    for (event, range) in Parser::new(contents).into_offset_iter() {
        let first = offsets
            .partition_point(|start| *start <= range.start)
            .saturating_sub(1);
        let end = offsets.partition_point(|start| *start < range.end);
        match event {
            Event::Start(Tag::Heading { .. }) if item_depth == 0 => {
                shape.headings.insert(first);
            }
            Event::Start(Tag::Item) => {
                item_depth += 1;
                if let Some(task) = lines
                    .get(first)
                    .and_then(|line| task_line_parts(&line.text))
                {
                    shape
                        .tasks
                        .insert(first, (task.note, end.saturating_sub(first).max(1)));
                }
                shape.raw.extend(first..end);
            }
            Event::End(TagEnd::Item) => item_depth -= 1,
            Event::Start(Tag::CodeBlock(_) | Tag::HtmlBlock | Tag::BlockQuote(_)) => {
                shape.raw.extend(first..end);
            }
            _ => {}
        }
    }
    shape
}

fn heading_parts(line: &str) -> Option<(String, String, String)> {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 {
        return None;
    }
    let level = trimmed.bytes().take_while(|byte| *byte == b'#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &trimmed[level..];
    if !rest.is_empty() && !rest.starts_with([' ', '\t']) {
        return None;
    }
    let text = rest.trim_start_matches([' ', '\t']);
    let start = line.len() - text.len();
    let mut title = text.trim_end_matches([' ', '\t']);
    let without_hashes = title.trim_end_matches('#');
    if without_hashes.len() != title.len() && without_hashes.ends_with([' ', '\t']) {
        title = without_hashes.trim_end_matches([' ', '\t']);
    }
    Some((
        title.to_owned(),
        line[..start].to_owned(),
        line[start + title.len()..].to_owned(),
    ))
}

struct TaskLineParts {
    note: Note,
    marker_start: usize,
    marker_end: usize,
    content_start: usize,
}

fn task_line_parts(line: &str) -> Option<TaskLineParts> {
    let trimmed = line.trim_start_matches([' ', '\t']);
    let bullet_len = if trimmed.starts_with(['-', '*', '+']) {
        1
    } else {
        let digits = trimmed.bytes().take_while(u8::is_ascii_digit).count();
        if !(1..=9).contains(&digits) || !trimmed[digits..].starts_with(['.', ')']) {
            return None;
        }
        digits + 1
    };
    let rest = &trimmed[bullet_len..];
    if !rest.starts_with([' ', '\t']) {
        return None;
    }
    let marker = rest.trim_start_matches([' ', '\t']);
    let marker_start = line.len() - marker.len();
    let inner = marker.strip_prefix('[')?.split_once(']')?.0;
    let state = match inner {
        "x" | "X" => NoteEnum::Done,
        "-" => NoteEnum::Rejected,
        value if value.chars().all(|c| matches!(c, ' ' | '\t')) => NoteEnum::Open,
        _ => return None,
    };
    let marker_end = marker_start + inner.len() + 2;
    let rest = &line[marker_end..];
    if !rest.is_empty() && !rest.starts_with([' ', '\t']) {
        return None;
    }
    let content_start = marker_end + usize::from(!rest.is_empty());
    Some(TaskLineParts {
        note: Note {
            content: line[content_start..].to_owned(),
            state,
        },
        marker_start,
        marker_end,
        content_start,
    })
}

fn render_task_line(template: &str, note: &Note) -> String {
    let Some(parts) = task_line_parts(template) else {
        return note.to_string();
    };
    let marker = if parts.note.state == note.state {
        &template[parts.marker_start..parts.marker_end]
    } else {
        match note.state {
            NoteEnum::Open => "[ ]",
            NoteEnum::Done => "[x]",
            NoteEnum::Rejected => "[-]",
        }
    };
    let space = if parts.content_start > parts.marker_end {
        &template[parts.marker_end..parts.content_start]
    } else {
        " "
    };
    format!(
        "{}{marker}{space}{}",
        &template[..parts.marker_start],
        note.content
    )
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
            SourceBlockKind::Note {
                index, original, ..
            } => {
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
    let bytes = contents.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        let ending = match bytes[index] {
            b'\n' => Some((index, index + 1, "\n")),
            b'\r' if bytes.get(index + 1) == Some(&b'\n') => Some((index, index + 2, "\r\n")),
            b'\r' => Some((index, index + 1, "\r")),
            _ => None,
        };
        if let Some((line_end, next_start, ending)) = ending {
            lines.push(SourceLine {
                text: contents[start..line_end].to_owned(),
                ending: ending.to_owned(),
            });
            start = next_start;
            index = next_start;
        } else {
            index += 1;
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
    use super::{parse_lines, parse_markdown, NoteEnum};

    #[test]
    fn deeply_nested_tasks_can_repeat_without_losing_indentation() {
        let mut list = parse_markdown(
            "# Work\n- [ ] root\n  - [ ] parent\n    + [ ] recurring repeat:daily\n",
        );
        list.set_task_state(
            0,
            2,
            NoteEnum::Done,
            crate::metadata::parse_date("2026-10-03").unwrap(),
        )
        .unwrap();
        assert_eq!(list.notes[0].len(), 4);
        assert_eq!(list.note_depth(0, 3), 4);
        assert_eq!(list.notes[0][3].state, NoteEnum::Open);
    }

    #[test]
    fn recurring_completion_copies_subtasks_and_advances_their_dates() {
        let source = "# Home\r\n- [ ] chore #home due:2026-10-03 repeat:weekly\r\n  * [x] child due:2026-10-02\r\n\r\n> keep\r\n";
        let mut list = parse_markdown(source);
        list.set_task_state(
            0,
            0,
            NoteEnum::Done,
            crate::metadata::parse_date("2026-10-03").unwrap(),
        )
        .unwrap();
        assert_eq!(list.notes[0].len(), 4);
        assert_eq!(list.notes[0][0].state, NoteEnum::Done);
        assert_eq!(list.notes[0][2].state, NoteEnum::Open);
        assert_eq!(
            list.notes[0][2].metadata().due,
            Some(crate::metadata::parse_date("2026-10-10").unwrap())
        );
        assert_eq!(list.notes[0][3].state, NoteEnum::Open);
        assert_eq!(
            list.notes[0][3].metadata().due,
            Some(crate::metadata::parse_date("2026-10-09").unwrap())
        );
        assert!(list
            .to_string()
            .contains("\r\n  * [ ] child due:2026-10-09\r\n\r\n> keep\r\n"));
        let saved = list.to_string();
        list.set_task_state(
            0,
            0,
            NoteEnum::Done,
            crate::metadata::parse_date("2026-10-03").unwrap(),
        )
        .unwrap();
        assert_eq!(list.to_string(), saved);
        list.set_task_state(
            0,
            0,
            NoteEnum::Open,
            crate::metadata::parse_date("2026-10-03").unwrap(),
        )
        .unwrap();
        list.set_task_state(
            0,
            0,
            NoteEnum::Done,
            crate::metadata::parse_date("2026-10-03").unwrap(),
        )
        .unwrap();
        assert_eq!(list.notes[0].len(), 4);
    }

    #[test]
    fn recurrence_without_due_uses_today_and_overflow_keeps_the_task_open() {
        let today = crate::metadata::parse_date("2026-10-03").unwrap();
        let mut list = parse_markdown("- [ ] Water plants repeat:daily\n");
        list.set_task_state(0, 0, NoteEnum::Done, today).unwrap();
        assert_eq!(
            list.notes[0][1].metadata().due,
            Some(crate::metadata::parse_date("2026-10-04").unwrap())
        );
        let source = "# End\n- [ ] task due:9999-12-31 repeat:daily\n";
        let mut list = parse_markdown(source);
        assert!(list.set_task_state(0, 0, NoteEnum::Done, today).is_err());
        assert_eq!(list.to_string(), source);
    }

    #[test]
    fn recognizes_headerless_nested_and_alternate_markers_without_reformatting() {
        let source =
            "intro\r\n\r\n* [X] parent\r\n  + [ ] child\r\n\r\n## Next ##\r\n1. [-] rejected\r\n";
        let mut list = parse_markdown(source);
        assert_eq!(list.titles, ["Inbox", "Next"]);
        assert_eq!(list.notes[0].len(), 2);
        assert_eq!(list.note_depth(0, 1), 2);
        assert_eq!(list.to_string(), source);
        list.notes[0][0].content = "renamed parent".into();
        list.notes[0][1].state = NoteEnum::Done;
        list.titles[1] = "Later".into();
        assert_eq!(
            list.to_string(),
            source
                .replace("parent", "renamed parent")
                .replace("+ [ ] child", "+ [x] child")
                .replace("## Next ##", "## Later ##")
        );
    }

    #[test]
    fn code_blocks_are_not_tasks_or_descriptions() {
        let source = "# Work\nOld description\n~~~md\n## example heading\n- [ ] example task\nordinary code text\n~~~\n- [ ] real task\n";
        let mut list = parse_markdown(source);
        assert_eq!(list.titles, ["Work"]);
        assert_eq!(list.notes[0].len(), 1);
        list.descriptions[0] = "New description".into();
        assert_eq!(
            list.to_string(),
            source.replace("Old description", "New description")
        );
    }

    #[test]
    fn reordering_moves_a_subtree_with_its_continuation_markdown() {
        let source = "# Work\n- [ ] a\n  + [ ] child\n    continuation **text**\n- [ ] b\n";
        let mut list = parse_markdown(source);
        assert_eq!(list.reorder_task(0, 0, true), Some(1));
        assert_eq!(
            list.to_string(),
            "# Work\n- [ ] b\n- [ ] a\n  + [ ] child\n    continuation **text**\n"
        );
        assert_eq!(list.reorder_task(0, 1, false), Some(0));
        assert_eq!(list.to_string(), source);
        assert_eq!(list.notes[0].len(), 3);
    }

    #[test]
    fn indenting_and_outdenting_keep_subtasks_attached() {
        let source = "# Work\n- [ ] a\n- [ ] b\n  - [ ] child\n";
        let mut list = parse_markdown(source);
        assert!(list.indent_task(0, 1, false));
        assert_eq!(list.note_depth(0, 1), 2);
        assert_eq!(list.note_depth(0, 2), 4);
        assert!(list.indent_task(0, 1, true));
        assert_eq!(list.to_string(), source);
    }

    #[test]
    fn moving_between_lists_preserves_unrelated_markdown_and_target_line_endings() {
        let mut from = parse_markdown("# Work\n- [X] parent\n  - [ ] child\n\n> untouched\n");
        let mut to = parse_markdown("## Inbox\r\n");
        let group = from.take_task_group(0, 0).unwrap();
        assert!(to.insert_task_group(0, 0, group, 0));
        assert!(from.notes[0].is_empty());
        assert_eq!(from.to_string(), "# Work\n\n> untouched\n");
        assert_eq!(
            to.to_string(),
            "## Inbox\r\n- [X] parent\r\n  - [ ] child\r\n"
        );
    }

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
        list.notes[1][0].state = NoteEnum::Done;
        assert_eq!(
            list.to_string(),
            "# Roadmap\nA description\n\n## Details\n> keep this quote\n- [x] first\n"
        );
    }

    #[test]
    fn editing_a_description_keeps_unrecognized_markdown_blocks() {
        let source = "# Plans\nOld description\n## Details\n> keep this quote\n- [ ] first\n";
        let mut list = parse_markdown(source);
        list.descriptions[0] = "New description".to_owned();
        assert_eq!(
            list.to_string(),
            "# Plans\nNew description\n## Details\n> keep this quote\n- [ ] first\n"
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

    #[test]
    fn preserves_bare_carriage_return_line_endings() {
        let source = "# One\r- [ ] task\r";
        let mut list = parse_markdown(source);
        list.notes[0][0].state = NoteEnum::Done;
        assert_eq!(list.to_string(), "# One\r- [x] task\r");
    }

    #[test]
    fn parse_lines_keeps_the_legacy_newline_terminated_output() {
        let list = parse_lines(vec!["# One".into(), "- [ ] task".into()]);
        assert_eq!(list.to_string(), "# One\n- [ ] task\n");
    }
}
