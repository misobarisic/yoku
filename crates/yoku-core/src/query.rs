use crate::metadata::{parse_date, valid_tag, Priority};
use crate::todo::{FileList, Note, NoteEnum};
use chrono::NaiveDate;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TaskFilter {
    #[default]
    All,
    Open,
    Done,
    Rejected,
}

impl TaskFilter {
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Open => "Open",
            Self::Done => "Done",
            Self::Rejected => "Rejected",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Self::All => Self::Open,
            Self::Open => Self::Done,
            Self::Done => Self::Rejected,
            Self::Rejected => Self::All,
        }
    }

    pub fn accepts(self, state: NoteEnum) -> bool {
        match self {
            Self::All => true,
            Self::Open => state == NoteEnum::Open,
            Self::Done => state == NoteEnum::Done,
            Self::Rejected => state == NoteEnum::Rejected,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TaskView {
    #[default]
    Lists,
    AllTasks,
    Today,
    Overdue,
}

impl TaskView {
    pub fn label(self) -> &'static str {
        match self {
            Self::Lists => "List",
            Self::AllTasks => "All files",
            Self::Today => "Today",
            Self::Overdue => "Overdue",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DueFilter {
    Today,
    Overdue,
    None,
    On(NaiveDate),
}

impl FromStr for DueFilter {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "today" => Ok(Self::Today),
            "overdue" => Ok(Self::Overdue),
            "none" => Ok(Self::None),
            _ => parse_date(value).map(Self::On),
        }
    }
}

impl DueFilter {
    pub fn accepts(self, due: Option<NaiveDate>, today: NaiveDate) -> bool {
        match self {
            Self::Today => due == Some(today),
            Self::Overdue => due.is_some_and(|due| due < today),
            Self::None => due.is_none(),
            Self::On(date) => due == Some(date),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TaskCriteria {
    pub tags: Vec<String>,
    pub priority: Option<Priority>,
    pub due: Option<DueFilter>,
    pub text: Vec<String>,
}

impl TaskCriteria {
    pub fn parse(query: &str) -> Result<Self, String> {
        let mut criteria = Self::default();
        for token in query.split_whitespace() {
            if let Some(tag) = token
                .strip_prefix("tag:")
                .or_else(|| token.strip_prefix('#'))
            {
                let tag = tag.trim_start_matches('#');
                if !valid_tag(tag) {
                    return Err(
                        "Use a non-empty tag containing letters, numbers, -, _, or /".into(),
                    );
                }
                criteria.tags.push(tag.to_lowercase());
            } else if let Some(priority) = token.strip_prefix("priority:") {
                criteria.priority = Some(priority.parse()?);
            } else if let Some(due) = token.strip_prefix("due:") {
                criteria.due = Some(due.parse()?);
            } else {
                criteria.text.push(token.to_lowercase());
            }
        }
        Ok(criteria)
    }

    pub fn accepts(&self, note: &Note, today: NaiveDate) -> bool {
        if self.tags.is_empty()
            && self.priority.is_none()
            && self.due.is_none()
            && self.text.is_empty()
        {
            return true;
        }
        let metadata = note.metadata();
        self.tags.iter().all(|wanted| {
            metadata
                .tags
                .iter()
                .any(|tag| tag.to_lowercase() == wanted.to_lowercase())
        }) && self
            .priority
            .is_none_or(|priority| metadata.priority == priority)
            && self.due.is_none_or(|due| due.accepts(metadata.due, today))
            && self
                .text
                .iter()
                .all(|text| note.content.to_lowercase().contains(text))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TaskSort {
    #[default]
    Document,
    Priority,
    Due,
}

impl TaskSort {
    pub fn cycle(self) -> Self {
        match self {
            Self::Document => Self::Priority,
            Self::Priority => Self::Due,
            Self::Due => Self::Document,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Document => "document order",
            Self::Priority => "priority",
            Self::Due => "due date",
        }
    }
}

pub fn sort_tasks(tasks: &mut [TaskLocation], lists: &[FileList], sort: TaskSort) {
    match sort {
        TaskSort::Document => tasks.sort(),
        TaskSort::Priority => tasks.sort_by_cached_key(|location| {
            let metadata = lists[location.file].notes[location.section][location.note].metadata();
            (
                metadata.priority,
                metadata.due.unwrap_or(NaiveDate::MAX),
                *location,
            )
        }),
        TaskSort::Due => tasks.sort_by_cached_key(|location| {
            let metadata = lists[location.file].notes[location.section][location.note].metadata();
            (
                metadata.due.unwrap_or(NaiveDate::MAX),
                metadata.priority,
                *location,
            )
        }),
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskLocation {
    pub file: usize,
    pub section: usize,
    pub note: usize,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TaskCounts {
    pub open: usize,
    pub done: usize,
    pub rejected: usize,
}

impl TaskCounts {
    pub fn from_notes<'a>(notes: impl IntoIterator<Item = &'a Note>) -> Self {
        notes.into_iter().fold(Self::default(), |mut counts, note| {
            match note.state {
                NoteEnum::Open => counts.open += 1,
                NoteEnum::Done => counts.done += 1,
                NoteEnum::Rejected => counts.rejected += 1,
            }
            counts
        })
    }

    pub fn total(self) -> usize {
        self.open + self.done + self.rejected
    }
}
