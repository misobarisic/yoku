use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};
use dirs::{data_dir, home_dir};
use lib::todo::{
    extract_naked_filename, lines_from_file, parse_lines, FileList, MAIN_DIR, STARTER_FILE,
    STARTER_FILE_CONTENT,
};
use lib::ui::app::App;
use lib::ui::run_app;
use lib::util::calculate_hash;
use regex::Regex;
use std::collections::HashMap;
use std::process::exit;
use std::thread::sleep;
use std::time::Duration;
use std::{
    error::Error,
    fs::read_dir,
    fs::{create_dir_all, File},
    io,
    io::Write,
    path::{Path, PathBuf},
};
use structopt::StructOpt;
use tui::{backend::CrosstermBackend, Terminal};

#[derive(Debug, StructOpt)]
#[structopt(name = "yoku", about = "TUI Markdown Todo")]
struct Opt {
    #[structopt(
        short = "p",
        long = "path",
        help = "Specify the data path",
        default_value = "",
        hide_default_value = true
    )]
    main_path: String,

    #[structopt(short = "d", long = "data-path", help = "Check the default data path")]
    check_path: bool,
}

fn main() -> Result<(), Box<dyn Error>> {
    let _re = Regex::new(r"^\d{4}-\d{2}-\d{2}$").unwrap();
    let opt = Opt::from_args();

    let main_path = if !opt.main_path.is_empty() {
        PathBuf::from(opt.main_path)
    } else {
        let mut path: PathBuf = PathBuf::new();
        match data_dir() {
            Some(dir) => path.push(dir.as_path()),
            _ => match home_dir() {
                Some(dir) => path.push(dir.as_path()),
                _ => path.push(Path::new("/root")),
            },
        }
        path.push(Path::new(&MAIN_DIR));
        path
    };

    if opt.check_path {
        println!(
            "Current default data path is \"{}\"",
            main_path.to_str().unwrap()
        );
        exit(0);
    }

    // Create main folder if it doesn't exist
    if !main_path.exists() && !main_path.is_dir() {
        println!(
            "Creating yoku data directory: {}",
            main_path.to_str().unwrap()
        );
        create_dir_all(&main_path).unwrap_or_else(|_| {
            panic!(
                "Error creating starting dir: {}",
                &main_path.to_str().unwrap()
            )
        });
        sleep(Duration::from_secs(3));
    }

    let mut path_entries = read_dir(&main_path)?
        .map(|res| res.map(|e| e.path()))
        .collect::<Result<Vec<_>, io::Error>>()?;
    path_entries.sort();

    if path_entries.is_empty() {
        let mut starter_path = main_path.clone();
        starter_path.push(&STARTER_FILE);
        let mut starter_file = File::create(&starter_path)
            .unwrap_or_else(|_| panic!("Could not create file {:?}", starter_path));
        starter_file
            .write_all(STARTER_FILE_CONTENT.as_ref())
            .unwrap_or_else(|_| panic!("Could not write to file {:?}", starter_path));

        // Redefine path_entries
        path_entries = read_dir(&main_path)?
            .map(|res| res.map(|e| e.path()))
            .collect::<Result<Vec<_>, io::Error>>()?;
        path_entries.sort();
    }

    let mut lists: Vec<FileList> = Vec::new();
    let mut files: Vec<String> = Vec::new();
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut hashes: HashMap<&PathBuf, u64> = HashMap::new();
    let mut to_remove: Vec<PathBuf> = Vec::new();

    // Parse all paths
    for path in path_entries {
        let lines = lines_from_file(&path);
        let filelist = parse_lines(lines);
        files.push(extract_naked_filename(&path));

        paths.push(path);
        lists.push(filelist);
    }

    // Create initial hashes (for checking whether a list needs to be written to file)
    let tmp_paths = paths.clone();
    for (i, list) in lists.iter().enumerate() {
        hashes.insert(tmp_paths.get(i).unwrap(), calculate_hash(list));
    }

    // TERMINAL
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app = App::new(
        &mut files,
        &mut paths,
        &mut lists,
        &mut hashes,
        &main_path,
        &mut to_remove,
    );
    let res = run_app(&mut terminal, app);

    // restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err)
    }

    Ok(())
}
