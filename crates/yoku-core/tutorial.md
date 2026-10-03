# Start

Your tasks live in portable Markdown files. Follow these checklists to explore Yoku.

- [ ] Move between files and lists with arrows, WASD, or HJKL; press Down to enter tasks
- [ ] Change a task state with Enter or Space; x completes it and - rejects it
- [ ] Open help with ? or F1; scroll with Up/Down and PageUp/PageDown

# Create and edit

- [ ] Press u to create a file, i for a list, or o for a task; Enter confirms and Escape cancels
- [ ] Press e to edit the selection; Ctrl+E edits a list description
- [ ] Press r to remove the selection; file deletion asks for confirmation
- [ ] Undo with Ctrl+Z and redo with Ctrl+Y, including edits already saved

# Organize

- [ ] Press J/K to reorder a task, m to move it to another list or file
- [ ] Press Tab/Shift+Tab to indent/outdent; subtasks move with their parent
  - [ ] This is a subtask; try moving its parent
- [ ] Press f to filter All/Open/Done/Rejected, g to see tasks across every file
- [ ] Press / to search, then n/N for the next/previous match

# Metadata and agendas

- [ ] Add tags such as `#work`, a priority such as `priority:high`, and a deadline such as `due:2026-10-03` to your own tasks
- [ ] Press F to filter with `tag:work priority:high due:today`; an empty filter clears it
- [ ] Press S to change sorting, t for Today, or v for Overdue
- [ ] Add `repeat:weekly` or `repeat:2w` to your own task; completing it creates the next open occurrence

# Save and external editors

- [ ] Save with Ctrl+S; a star beside the file name marks unsaved changes
- [ ] Press E to save and open the selected file in your external editor, then return to Yoku
- [ ] Press R to refresh; clean files also reload automatically while browsing
- [ ] Press q to save and quit; Ctrl+Q or Ctrl+C confirms before discarding unsaved edits

# Use the shell

- [ ] Capture without opening the TUI with `yoku add "Buy milk"`
- [ ] Choose a destination with `yoku add "Review changes" --file work --list Inbox`
- [ ] Inspect open tasks with `yoku list --open --json`
- [ ] Find your data directory with `yoku --data-path`
