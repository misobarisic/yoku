use chrono::{Local, NaiveDate};
use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use yoku_core::metadata::{parse_date, set_field, valid_tag, Priority};
use yoku_core::query::{sort_tasks, DueFilter, TaskCriteria, TaskFilter, TaskLocation, TaskSort};
use yoku_core::storage::{valid_file_stem, Workspace};
use yoku_core::todo::{FileList, Note, NoteEnum};

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Capture a task without opening the terminal UI.
    Add(AddArgs),
    /// Print tasks as text or JSON without opening the terminal UI.
    List(ListArgs),
}

#[derive(Debug, Args)]
pub struct AddArgs {
    /// Task text, optionally including portable inline metadata.
    pub text: String,
    /// Markdown file name, with an optional .md extension.
    #[arg(short, long, default_value = "inbox")]
    pub file: String,
    /// List heading; created if it does not exist.
    #[arg(short, long, default_value = "Inbox")]
    pub list: String,
    /// Add a tag; repeat this option for multiple tags.
    #[arg(long = "tag")]
    pub tags: Vec<String>,
    /// Set the priority (high, normal, low).
    #[arg(long)]
    pub priority: Option<Priority>,
    /// Set a due date in YYYY-MM-DD format.
    #[arg(long, value_parser = parse_date)]
    pub due: Option<NaiveDate>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum State {
    Open,
    Done,
    Rejected,
}

impl State {
    fn filter(self) -> TaskFilter {
        match self {
            Self::Open => TaskFilter::Open,
            Self::Done => TaskFilter::Done,
            Self::Rejected => TaskFilter::Rejected,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum Sort {
    Document,
    Priority,
    Due,
}
impl Sort {
    fn order(self) -> TaskSort {
        match self {
            Self::Document => TaskSort::Document,
            Self::Priority => TaskSort::Priority,
            Self::Due => TaskSort::Due,
        }
    }
}

#[derive(Debug, Args)]
pub struct ListArgs {
    /// Show only open tasks.
    #[arg(long, conflicts_with = "state")]
    pub open: bool,
    /// Select a task state.
    #[arg(long, value_enum)]
    pub state: Option<State>,
    /// Limit tasks to a file name (without .md).
    #[arg(short, long)]
    pub file: Option<String>,
    /// Limit tasks to an exact list heading.
    #[arg(short, long)]
    pub list: Option<String>,
    /// Emit a JSON array, including file, list, task index, and state.
    #[arg(long)]
    pub json: bool,
    /// Require a tag; repeat this option to require several tags.
    #[arg(long = "tag")]
    pub tags: Vec<String>,
    /// Filter by priority.
    #[arg(long)]
    pub priority: Option<Priority>,
    /// Filter by today, overdue, none, or YYYY-MM-DD.
    #[arg(long)]
    pub due: Option<DueFilter>,
    /// Sort the view without rewriting Markdown.
    #[arg(long, value_enum, default_value = "document")]
    pub sort: Sort,
}

#[derive(Serialize)]
struct TaskRecord<'a> {
    file: &'a str,
    list: &'a str,
    index: usize,
    state: &'static str,
    text: &'a str,
    depth: usize,
    tags: Vec<String>,
    priority: &'static str,
    due: Option<String>,
    repeat: Option<String>,
}

pub fn run(command: Command, root: &Path) -> io::Result<()> {
    match command {
        Command::Add(args) => add_task(root, args),
        Command::List(args) => list_tasks(root, args, &mut io::stdout().lock()),
    }
}

fn single_line(value: &str) -> bool {
    !value.trim().is_empty() && !value.chars().any(|c| c.is_control() && c != '\t')
}

fn add_task(root: &Path, args: AddArgs) -> io::Result<()> {
    let file = if args
        .file
        .get(args.file.len().saturating_sub(3)..)
        .is_some_and(|extension| extension.eq_ignore_ascii_case(".md"))
    {
        &args.file[..args.file.len() - 3]
    } else {
        &args.file
    };
    if !valid_file_stem(file) || !single_line(&args.text) || !single_line(&args.list) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Use a valid file name and non-empty, single-line task and list text",
        ));
    }
    let mut text = args.text;
    for tag in args.tags {
        let tag = tag.trim_start_matches('#');
        if !valid_tag(tag) {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid tag"));
        }
        if !yoku_core::metadata::TaskMetadata::parse(&text)
            .tags
            .iter()
            .any(|existing| existing.to_lowercase() == tag.to_lowercase())
        {
            text.push_str(&format!(" #{tag}"));
        }
    }
    if let Some(priority) = args.priority {
        text = set_field(&text, "priority", priority.label());
    }
    if let Some(due) = args.due {
        text = set_field(&text, "due", &due.format("%Y-%m-%d").to_string());
    }
    fs::create_dir_all(root)?;
    let mut workspace = Workspace::load(root)?;
    let file_index = if let Some(index) = workspace.files.iter().position(|name| name == file) {
        index
    } else {
        workspace.files.push(file.to_owned());
        workspace.paths.push(root.join(format!("{file}.md")));
        workspace
            .lists
            .push(FileList::from_parts(Vec::new(), Vec::new(), Vec::new()));
        workspace.files.len() - 1
    };
    let list = &mut workspace.lists[file_index];
    let section = if let Some(index) = list.titles.iter().position(|title| title == &args.list) {
        index
    } else {
        list.push_section(args.list);
        list.titles.len() - 1
    };
    list.push_note(
        section,
        Note {
            content: text,
            state: NoteEnum::Open,
        },
    );
    workspace.save(root)?;
    println!(
        "Added to {} / {}",
        workspace.files[file_index], workspace.lists[file_index].titles[section]
    );
    Ok(())
}

fn list_tasks(root: &Path, args: ListArgs, output: &mut impl Write) -> io::Result<()> {
    let workspace = Workspace::load(root)?;
    let filter = if args.open {
        TaskFilter::Open
    } else {
        args.state.map_or(TaskFilter::All, State::filter)
    };
    let criteria = TaskCriteria {
        tags: args
            .tags
            .iter()
            .map(|tag| tag.trim_start_matches('#').to_owned())
            .collect(),
        priority: args.priority,
        due: args.due,
        text: Vec::new(),
    };
    if criteria.tags.iter().any(|tag| !valid_tag(tag)) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Invalid tag"));
    }
    let today = Local::now().date_naive();
    let mut locations = Vec::new();
    for (file, list) in workspace.lists.iter().enumerate() {
        if args
            .file
            .as_ref()
            .is_some_and(|name| name != &workspace.files[file])
        {
            continue;
        }
        for (section, notes) in list.notes.iter().enumerate() {
            if args
                .list
                .as_ref()
                .is_some_and(|title| title != &list.titles[section])
            {
                continue;
            }
            for (index, note) in notes.iter().enumerate() {
                if !filter.accepts(note.state) || !criteria.accepts(note, today) {
                    continue;
                }
                locations.push(TaskLocation {
                    file,
                    section,
                    note: index,
                });
            }
        }
    }
    sort_tasks(&mut locations, &workspace.lists, args.sort.order());
    let records = locations
        .iter()
        .map(|location| {
            let list = &workspace.lists[location.file];
            let note = &list.notes[location.section][location.note];
            let metadata = note.metadata();
            TaskRecord {
                file: &workspace.files[location.file],
                list: &list.titles[location.section],
                index: location.note,
                state: match note.state {
                    NoteEnum::Open => "open",
                    NoteEnum::Done => "done",
                    NoteEnum::Rejected => "rejected",
                },
                text: &note.content,
                depth: list.note_depth(location.section, location.note),
                tags: metadata.tags,
                priority: metadata.priority.label(),
                due: metadata.due.map(|date| date.format("%Y-%m-%d").to_string()),
                repeat: metadata.recurrence.map(|rule| rule.to_string()),
            }
        })
        .collect::<Vec<_>>();
    if args.json {
        serde_json::to_writer_pretty(&mut *output, &records)?;
        writeln!(output)?;
    } else {
        for task in records {
            let marker = match task.state {
                "done" => "x",
                "rejected" => "-",
                _ => " ",
            };
            writeln!(
                output,
                "{} / {} #{}: {}[{marker}] {}",
                task.file,
                task.list,
                task.index,
                " ".repeat(task.depth),
                task.text
            )?;
        }
    }
    Ok(())
}
