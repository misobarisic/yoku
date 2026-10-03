use crate::todo::{Note, NoteEnum};

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
}

impl TaskView {
    pub fn label(self) -> &'static str {
        match self {
            Self::Lists => "List",
            Self::AllTasks => "All files",
        }
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
