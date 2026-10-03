use crate::todo::{extract_naked_filename, parse_markdown, FileList};
use crate::ui::app::App;
use crate::util::calculate_hash;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn markdown_files(data_path: &Path) -> io::Result<Vec<PathBuf>> {
    let entries = match fs::read_dir(data_path) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut paths = Vec::new();
    for entry in entries {
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

pub fn valid_file_stem(name: &str) -> bool {
    !name.trim().is_empty()
        && name == name.trim()
        && name != "."
        && name != ".."
        && !name.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                )
        })
}

#[derive(Default)]
pub struct Workspace {
    pub files: Vec<String>,
    pub paths: Vec<PathBuf>,
    pub lists: Vec<FileList>,
    pub hashes: HashMap<PathBuf, u64>,
    pub disk_hashes: HashMap<PathBuf, u64>,
    pub removed: Vec<PathBuf>,
}

impl Workspace {
    pub fn load(root: &Path) -> io::Result<Self> {
        let paths = markdown_files(root)?;
        let mut workspace = Self::default();
        for path in paths {
            let contents = fs::read_to_string(&path)?;
            let list = parse_markdown(&contents);
            workspace.files.push(extract_naked_filename(&path)?);
            workspace.hashes.insert(path.clone(), calculate_hash(&list));
            workspace
                .disk_hashes
                .insert(path.clone(), calculate_hash(&contents.as_bytes()));
            workspace.paths.push(path);
            workspace.lists.push(list);
        }
        Ok(workspace)
    }

    /// CLI writes share the TUI's atomic writes and conflict checks.
    pub fn save(&mut self, root: &Path) -> io::Result<()> {
        let mut app = App::new(
            &mut self.files,
            &mut self.paths,
            &mut self.lists,
            &mut self.hashes,
            &mut self.disk_hashes,
            root,
            &mut self.removed,
        );
        if app.save()? {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "{} changed on disk; retry the command",
                app.save_conflict
                    .as_ref()
                    .map(|conflict| conflict.path.display().to_string())
                    .unwrap_or_default()
            )))
        }
    }
}
