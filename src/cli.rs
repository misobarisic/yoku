use clap::{Args, Subcommand, ValueEnum};
use serde::Serialize;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use yoku_core::query::TaskFilter;
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
}

#[derive(Serialize)]
struct TaskRecord<'a> {
    file: &'a str,
    list: &'a str,
    index: usize,
    state: &'static str,
    text: &'a str,
    depth: usize,
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
            content: args.text,
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
    let mut records = Vec::new();
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
                if !filter.accepts(note.state) {
                    continue;
                }
                records.push(TaskRecord {
                    file: &workspace.files[file],
                    list: &list.titles[section],
                    index,
                    state: match note.state {
                        NoteEnum::Open => "open",
                        NoteEnum::Done => "done",
                        NoteEnum::Rejected => "rejected",
                    },
                    text: &note.content,
                    depth: list.note_depth(section, index),
                });
            }
        }
    }
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
