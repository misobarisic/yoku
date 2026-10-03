use std::fs;
use std::path::Path;
use std::process::{Command, Output};

fn yoku(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_yoku"))
        .arg("--main-path")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn captures_tasks_without_a_terminal_and_emits_json() {
    let directory = tempfile::tempdir().unwrap();
    let output = yoku(directory.path(), &["add", "Buy milk"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        fs::read_to_string(directory.path().join("inbox.md")).unwrap(),
        "# Inbox\n- [ ] Buy milk\n"
    );
    assert!(!directory.path().join("tutorial.md").exists());
    let output = yoku(directory.path(), &["list", "--open", "--json"]);
    assert!(output.status.success());
    let tasks: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(tasks[0]["file"], "inbox");
    assert_eq!(tasks[0]["list"], "Inbox");
    assert_eq!(tasks[0]["state"], "open");
    assert_eq!(tasks[0]["text"], "Buy milk");
}

#[test]
fn capture_preserves_existing_markdown_and_uses_requested_destination() {
    let directory = tempfile::tempdir().unwrap();
    let original = "## Work\r\n* [X] finished\r\n  + [ ] nested\r\n\r\n> keep this\r\n";
    fs::write(directory.path().join("work.MD"), original).unwrap();
    let output = yoku(
        directory.path(),
        &[
            "add",
            "Fix edge case",
            "--file",
            "work.md",
            "--list",
            "Work",
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let contents = fs::read_to_string(directory.path().join("work.MD")).unwrap();
    assert_eq!(contents, format!("{original}- [ ] Fix edge case\r\n"));
    assert!(!directory.path().join("work.md").exists());
    let output = yoku(directory.path(), &["list", "--state", "done", "--json"]);
    let tasks: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(tasks.as_array().unwrap().len(), 1);
    assert_eq!(tasks[0]["text"], "finished");
}

#[test]
fn invalid_capture_cannot_escape_the_data_directory_or_inject_lines() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("data");
    let output = yoku(&root, &["add", "task", "--file", "../escape"]);
    assert!(!output.status.success());
    assert!(!root.exists());
    let output = yoku(&root, &["add", "task\n# injected"]);
    assert!(!output.status.success());
    assert!(!root.exists());
}

#[test]
fn listing_missing_data_is_read_only_and_json_escapes_text() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("missing");
    let output = yoku(&root, &["list", "--json"]);
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "[]");
    assert!(!root.exists());
    let text = "say \"hello\" \\ world 🦀";
    assert!(yoku(&root, &["add", text]).status.success());
    let output = yoku(&root, &["list", "--json"]);
    let tasks: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(tasks[0]["text"], text);
}

#[test]
fn capture_and_listing_share_metadata_filters_and_sorting() {
    let directory = tempfile::tempdir().unwrap();
    assert!(yoku(
        directory.path(),
        &[
            "add",
            "Later",
            "--tag",
            "work",
            "--priority",
            "low",
            "--due",
            "2026-10-05"
        ]
    )
    .status
    .success());
    assert!(yoku(
        directory.path(),
        &[
            "add",
            "Urgent",
            "--tag",
            "Work",
            "--priority",
            "high",
            "--due",
            "2026-10-02"
        ]
    )
    .status
    .success());
    let output = yoku(
        directory.path(),
        &["list", "--tag", "work", "--sort", "due", "--json"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let tasks: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(tasks.as_array().unwrap().len(), 2);
    assert_eq!(tasks[0]["due"], "2026-10-02");
    assert_eq!(tasks[0]["priority"], "high");
    assert_eq!(tasks[0]["tags"][0], "Work");
    let output = yoku(
        directory.path(),
        &[
            "list",
            "--priority",
            "high",
            "--due",
            "2026-10-02",
            "--json",
        ],
    );
    let tasks: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(tasks.as_array().unwrap().len(), 1);
}

#[test]
fn invalid_metadata_options_fail_before_creating_files() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("missing");
    for args in [
        ["add", "task", "--due", "2026-02-30"],
        ["add", "task", "--priority", "urgent"],
        ["add", "task", "--tag", "bad tag"],
    ] {
        assert!(!yoku(&root, &args).status.success());
        assert!(!root.exists());
    }
}
