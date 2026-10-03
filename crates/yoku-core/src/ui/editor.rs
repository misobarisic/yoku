use crossterm::cursor::{Hide, Show};
use crossterm::execute;
use crossterm::terminal::{enable_raw_mode, EnterAlternateScreen};
use ratatui::{backend::Backend, Terminal};
use std::io;
use std::path::Path;
use std::process::Command;

fn command_words(command: &str) -> io::Result<Vec<String>> {
    let words = shell_words::split(command)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    if words.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "The editor command is empty",
        ));
    }
    Ok(words)
}

pub(super) fn edit_file<B: Backend>(terminal: &mut Terminal<B>, path: &Path) -> io::Result<()> {
    let editor = std::env::var("VISUAL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("EDITOR")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| {
            if cfg!(windows) {
                "notepad.exe".into()
            } else {
                "vi".into()
            }
        });
    let words = command_words(&editor)?;
    let path = path.canonicalize()?;
    ratatui::try_restore()?;
    execute!(io::stdout(), Show)?;
    let status = Command::new(&words[0]).args(&words[1..]).arg(path).status();
    // Restore the TUI even if starting the editor failed or it exited unsuccessfully.
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen, Hide)?;
    terminal
        .clear()
        .map_err(|error| io::Error::other(error.to_string()))?;
    let status = status?;
    if status.success() {
        Ok(())
    } else {
        Err(io::Error::other(format!("Editor exited with {status}")))
    }
}

#[cfg(test)]
mod tests {
    use super::command_words;

    #[test]
    fn editor_arguments_support_quotes_without_shell_expansion() {
        assert_eq!(
            command_words("'/path with spaces/editor' --wait '$literal'").unwrap(),
            ["/path with spaces/editor", "--wait", "$literal"]
        );
        assert!(command_words("'unterminated").is_err());
        assert!(command_words(" ").is_err());
    }
}
