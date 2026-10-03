mod cli;

use clap::Parser;
use dirs::{data_dir, home_dir};
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use yoku_core::storage::{markdown_files, Workspace};
use yoku_core::todo::{MAIN_DIR, STARTER_FILE, STARTER_FILE_CONTENT};
use yoku_core::ui::app::App;
use yoku_core::ui::run_app;

#[derive(Debug, Parser)]
#[command(name = "yoku", version, about = "TUI Markdown Todo")]
struct Opt {
    /// Specify a custom data directory.
    #[arg(short, long, global = true, value_name = "PATH")]
    main_path: Option<PathBuf>,

    /// Print the default data directory and exit.
    #[arg(short = 'd', long = "data-path")]
    check_path: bool,

    #[command(subcommand)]
    command: Option<cli::Command>,
}

fn default_data_path() -> io::Result<PathBuf> {
    let base_path = data_dir().or_else(home_dir).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "could not find a data directory")
    })?;
    Ok(base_path.join(MAIN_DIR))
}

fn create_starter_file(path: &Path) -> io::Result<()> {
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => file.write_all(STARTER_FILE_CONTENT.as_bytes()),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error),
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let opt = Opt::parse();
    if opt.check_path {
        println!("Default data path: {}", default_data_path()?.display());
        return Ok(());
    }

    let main_path = match opt.main_path {
        Some(path) => path,
        None => default_data_path()?,
    };
    if let Some(command) = opt.command {
        return match cli::run(command, &main_path) {
            Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(()),
            result => result.map_err(Into::into),
        };
    }
    if main_path.exists() && !main_path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            format!("data path is not a directory: {}", main_path.display()),
        )
        .into());
    }
    fs::create_dir_all(&main_path)?;

    if markdown_files(&main_path)?.is_empty() {
        create_starter_file(&main_path.join(STARTER_FILE))?;
    }
    let mut workspace = Workspace::load(&main_path)?;
    let app = App::new(
        &mut workspace.files,
        &mut workspace.paths,
        &mut workspace.lists,
        &mut workspace.hashes,
        &mut workspace.disk_hashes,
        &main_path,
        &mut workspace.removed,
    );

    ratatui::run(|terminal| run_app(terminal, app))?;
    Ok(())
}
