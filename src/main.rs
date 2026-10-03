use clap::Parser;
use dirs::{data_dir, home_dir};
use std::collections::HashMap;
use std::error::Error;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use yoku_core::todo::{
    extract_naked_filename, parse_lines, FileList, MAIN_DIR, STARTER_FILE, STARTER_FILE_CONTENT,
};
use yoku_core::ui::app::App;
use yoku_core::ui::run_app;
use yoku_core::util::calculate_hash;

#[derive(Debug, Parser)]
#[command(name = "yoku", version, about = "TUI Markdown Todo")]
struct Opt {
    /// Specify a custom data directory.
    #[arg(short, long, value_name = "PATH")]
    main_path: Option<PathBuf>,

    /// Print the default data directory and exit.
    #[arg(short = 'd', long = "data-path")]
    check_path: bool,
}

fn default_data_path() -> io::Result<PathBuf> {
    let base_path = data_dir().or_else(home_dir).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "could not find a data directory")
    })?;
    Ok(base_path.join(MAIN_DIR))
}

fn markdown_files(data_path: &Path) -> io::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(data_path)? {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            let path = entry.path();
            if path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("md"))
            {
                paths.push(path);
            }
        }
    }
    paths.sort();
    Ok(paths)
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
    let default_path = default_data_path()?;
    if opt.check_path {
        println!("Default data path: {}", default_path.display());
        return Ok(());
    }

    let main_path = opt.main_path.unwrap_or(default_path);
    if main_path.exists() && !main_path.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            format!("data path is not a directory: {}", main_path.display()),
        )
        .into());
    }
    fs::create_dir_all(&main_path)?;

    let mut paths = markdown_files(&main_path)?;
    if paths.is_empty() {
        create_starter_file(&main_path.join(STARTER_FILE))?;
        paths = markdown_files(&main_path)?;
    }

    let mut files = Vec::with_capacity(paths.len());
    let mut lists: Vec<FileList> = Vec::with_capacity(paths.len());
    for path in &paths {
        let contents = fs::read_to_string(path)?;
        files.push(extract_naked_filename(path)?);
        lists.push(parse_lines(contents.lines().map(str::to_owned).collect()));
    }

    let mut hashes: HashMap<PathBuf, u64> = paths
        .iter()
        .zip(&lists)
        .map(|(path, list)| (path.clone(), calculate_hash(list)))
        .collect();
    let mut to_remove = Vec::new();

    let app = App::new(
        &mut files,
        &mut paths,
        &mut lists,
        &mut hashes,
        &main_path,
        &mut to_remove,
    );

    ratatui::run(|mut terminal| run_app(&mut terminal, app))?;
    Ok(())
}
