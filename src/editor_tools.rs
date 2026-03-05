use std::{
    env, fs, io,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, ExitStatus},
    time::{SystemTime, UNIX_EPOCH},
};

use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

enum Editor {
    EnvCommand(String),
    Binary(&'static str),
}

enum ClipboardTool {
    Binary(&'static str, Vec<&'static str>),
}

pub fn open_value_in_editor(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    value: &str,
) -> io::Result<Option<String>> {
    suspend_terminal(terminal)?;
    let edit_result = edit_value_in_external_editor(value);
    let resume_result = resume_terminal(terminal);

    match (edit_result, resume_result) {
        (Ok(edit_out), Ok(())) => Ok(edit_out),
        (Err(edit_err), Ok(())) => Err(edit_err),
        (Ok(_), Err(resume_err)) => Err(resume_err),
        (Err(edit_err), Err(resume_err)) => Err(io::Error::other(format!(
            "editor error: {edit_err}; terminal restore error: {resume_err}"
        ))),
    }
}

fn suspend_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn resume_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    enable_raw_mode()?;
    terminal.hide_cursor()?;
    terminal.clear()?;
    Ok(())
}

fn edit_value_in_external_editor(value: &str) -> io::Result<Option<String>> {
    let tmp_edit_file = build_temp_edit_file();
    fs::write(&tmp_edit_file, value)?;

    let edit_result = (|| -> io::Result<Option<String>> {
        let editor = select_editor().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "No editor found: set $EDITOR or install vim/vi",
            )
        })?;

        let status = run_editor(&editor, &tmp_edit_file)?;
        if status.success() {
            let edited_content = fs::read_to_string(&tmp_edit_file)?;
            if edited_content != value {
                Ok(Some(edited_content))
            } else {
                Ok(None)
            }
        } else {
            Err(io::Error::other(format!("Editor exited with status: {status}")))
        }
    })();

    let cleanup_result = fs::remove_file(&tmp_edit_file);
    match (edit_result, cleanup_result) {
        (Ok(edit_out), Ok(())) => Ok(edit_out),
        (Ok(edit_out), Err(err)) if err.kind() == io::ErrorKind::NotFound => Ok(edit_out),
        (Ok(_), Err(err)) => Err(err),
        (Err(edit_err), Ok(())) => Err(edit_err),
        (Err(edit_err), Err(err)) if err.kind() == io::ErrorKind::NotFound => Err(edit_err),
        (Err(edit_err), Err(clean_err)) => Err(io::Error::other(format!(
            "{edit_err}; cleanup failed for {}: {clean_err}",
            tmp_edit_file.display()
        ))),
    }
}

fn build_temp_edit_file() -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let pid = std::process::id();
    PathBuf::from(format!(
        "/tmp/epochtime-{}-{}-{}.txt",
        now.as_secs(),
        now.subsec_nanos(),
        pid
    ))
}

fn select_editor() -> Option<Editor> {
    if let Ok(raw) = env::var("EDITOR") {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            let binary = trimmed.split_whitespace().next().unwrap_or_default();
            if command_exists(binary) {
                return Some(Editor::EnvCommand(trimmed.to_string()));
            }
        }
    }

    if command_exists("vim") {
        Some(Editor::Binary("vim"))
    } else if command_exists("vi") {
        Some(Editor::Binary("vi"))
    } else {
        None
    }
}

fn run_editor(editor: &Editor, file_path: &Path) -> io::Result<ExitStatus> {
    let file_path = file_path.to_string_lossy();
    match editor {
        Editor::EnvCommand(cmd) => {
            let quoted = shell_quote(&file_path);
            Command::new("sh")
                .arg("-lc")
                .arg(format!("{cmd} {quoted}"))
                .status()
        }
        Editor::Binary(bin) => Command::new(bin).arg(file_path.as_ref()).status(),
    }
}

pub fn copy_to_clipboard(value: &str) -> io::Result<()> {
    let tool = select_clipboard_tool().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "No clipboard tool found (tried pbcopy, wl-copy, xclip, xsel)",
        )
    })?;

    match tool {
        ClipboardTool::Binary(bin, args) => {
            let mut child = Command::new(bin)
                .args(args)
                .stdin(std::process::Stdio::piped())
                .spawn()?;

            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(value.as_bytes())?;
            }

            let status = child.wait()?;
            if status.success() {
                Ok(())
            } else {
                Err(io::Error::other(format!(
                    "clipboard command {bin} failed with status {status}"
                )))
            }
        }
    }
}

fn select_clipboard_tool() -> Option<ClipboardTool> {
    if command_exists("pbcopy") {
        return Some(ClipboardTool::Binary("pbcopy", vec![]));
    }
    if command_exists("wl-copy") {
        return Some(ClipboardTool::Binary("wl-copy", vec![]));
    }
    if command_exists("xclip") {
        return Some(ClipboardTool::Binary(
            "xclip",
            vec!["-selection", "clipboard"],
        ));
    }
    if command_exists("xsel") {
        return Some(ClipboardTool::Binary(
            "xsel",
            vec!["--clipboard", "--input"],
        ));
    }
    None
}

fn shell_quote(input: &str) -> String {
    format!("'{}'", input.replace('\'', "'\\''"))
}

fn command_exists(cmd: &str) -> bool {
    if cmd.is_empty() {
        return false;
    }

    let path = Path::new(cmd);
    if cmd.contains('/') {
        return is_executable(path);
    }

    let Some(path_var) = env::var_os("PATH") else {
        return false;
    };

    env::split_paths(&path_var)
        .map(|dir| dir.join(cmd))
        .any(|candidate| is_executable(&candidate))
}

fn is_executable(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = path.metadata() {
            return meta.permissions().mode() & 0o111 != 0;
        }
        false
    }

    #[cfg(not(unix))]
    {
        true
    }
}
