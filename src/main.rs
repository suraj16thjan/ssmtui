mod app;
mod aws_ssm;
mod editor_tools;
mod models;
mod text_edit;
mod ui;

use std::{
    env,
    io,
    process::Command,
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};

use app::App;
use aws_ssm::{fetch_parameter_value_from_ssm, put_parameter_value_to_ssm};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use editor_tools::{copy_to_clipboard, open_value_in_editor};
use models::{CreateField, ValueEditorMode};
use ratatui::{Terminal, backend::CrosstermBackend};
use text_edit::{
    backspace_at_cursor, delete_at_cursor, insert_char_at_cursor, move_cursor_down,
    move_cursor_left, move_cursor_line_end, move_cursor_line_start, move_cursor_right,
    move_cursor_up,
};
use ui::{draw, draw_loading};

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_help() {
    println!(
        "\
ssmtui {VERSION} — Terminal UI for AWS SSM Parameter Store

USAGE:
    ssmtui [OPTIONS]
    ssmtui [COMMAND]

COMMANDS:
    update           Update to the latest version from crates.io

OPTIONS:
    -h, --help       Print this help message and exit
    -v, --version    Print version and exit

AWS CONFIGURATION:
    Uses the standard AWS SDK config chain.

    Environment variables:
        AWS_PROFILE           AWS profile to use
        AWS_REGION            AWS region (overrides config)
        AWS_DEFAULT_REGION    Fallback region if AWS_REGION is not set

KEYBINDINGS:
    /          Search/filter parameters
    j / Down   Move selection down
    k / Up     Move selection up
    R          Refresh all parameters and values
    y          Yank/copy selected value to clipboard
    e          Edit selected value in external editor
    a          Create new parameter
    Ctrl+C     Quit

MORE INFO:
    https://github.com/suraj16thjan/ssmtui"
    );
}

fn self_update() {
    println!("Current version: {VERSION}");
    println!("Updating ssmtui from crates.io...\n");

    let status = Command::new("cargo")
        .args(["install", "ssmtui"])
        .status();

    match status {
        Ok(s) if s.success() => {
            println!("\nUpdate complete.");
        }
        Ok(s) => {
            eprintln!("\ncargo install exited with {s}");
            std::process::exit(1);
        }
        Err(err) => {
            eprintln!("Failed to run cargo install: {err}");
            eprintln!("Make sure cargo is installed and in your PATH.");
            std::process::exit(1);
        }
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();

    for arg in &args {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-v" | "--version" => {
                println!("ssmtui {VERSION}");
                return Ok(());
            }
            "update" => {
                self_update();
                return Ok(());
            }
            other => {
                eprintln!("ssmtui: unknown option '{other}'");
                eprintln!("Try 'ssmtui --help' for more information.");
                std::process::exit(1);
            }
        }
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.hide_cursor()?;

    let app_result = run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    app_result
}

fn run(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> io::Result<()> {
    let (init_tx, init_rx) = mpsc::channel::<App>();
    thread::spawn(move || {
        let app = App::new();
        let _ = init_tx.send(app);
    });

    let spinner = ["|", "/", "-", "\\"];
    let mut spinner_idx = 0usize;
    let started = Instant::now();

    loop {
        if let Ok(mut app) = init_rx.try_recv() {
            return run_app_loop(terminal, &mut app);
        }

        let frame_spinner = spinner[spinner_idx % spinner.len()];
        spinner_idx = spinner_idx.wrapping_add(1);
        let elapsed_secs = started.elapsed().as_secs();

        terminal.draw(|frame| draw_loading(frame, frame_spinner, elapsed_secs))?;
        if event::poll(Duration::from_millis(120))? {
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind == KeyEventKind::Press
                && key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('c'))
            {
                return Ok(());
            }
        }
    }
}

fn run_app_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> io::Result<()> {
    loop {
        app.pump_full_refresh_updates();
        app.pump_value_updates();
        app.prefetch_near_selected();

        terminal.draw(|frame| draw(frame, app))?;
        if event::poll(Duration::from_millis(120))? {
            let Event::Key(key) = event::read()? else {
                continue;
            };
            if key.kind != KeyEventKind::Press {
                continue;
            }

            if app.create_mode {
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && matches!(key.code, KeyCode::Char('s') | KeyCode::Char('S'))
                {
                    app.submit_create();
                    continue;
                }

                match app.create_field {
                    CreateField::Name => match key.code {
                        KeyCode::Esc => app.cancel_create(),
                        KeyCode::Tab | KeyCode::Down | KeyCode::Up => app.switch_create_field(),
                        KeyCode::Enter => {
                            app.create_field = CreateField::Value;
                            app.create_value_mode = ValueEditorMode::Insert;
                        }
                        KeyCode::Left => {
                            move_cursor_left(&app.create_name, &mut app.create_name_cursor)
                        }
                        KeyCode::Right => {
                            move_cursor_right(&app.create_name, &mut app.create_name_cursor)
                        }
                        KeyCode::Home => {
                            move_cursor_line_start(&app.create_name, &mut app.create_name_cursor)
                        }
                        KeyCode::End => {
                            move_cursor_line_end(&app.create_name, &mut app.create_name_cursor)
                        }
                        KeyCode::Backspace => {
                            backspace_at_cursor(&mut app.create_name, &mut app.create_name_cursor)
                        }
                        KeyCode::Delete => {
                            delete_at_cursor(&mut app.create_name, &mut app.create_name_cursor)
                        }
                        KeyCode::Char(c) => {
                            if !key.modifiers.contains(KeyModifiers::CONTROL) {
                                insert_char_at_cursor(
                                    &mut app.create_name,
                                    &mut app.create_name_cursor,
                                    c,
                                );
                            }
                        }
                        _ => {}
                    },
                    CreateField::Value => match app.create_value_mode {
                        ValueEditorMode::Insert => match key.code {
                            KeyCode::Esc => app.create_value_mode = ValueEditorMode::Normal,
                            KeyCode::Tab => app.switch_create_field(),
                            KeyCode::Left => {
                                move_cursor_left(&app.create_value, &mut app.create_value_cursor)
                            }
                            KeyCode::Right => {
                                move_cursor_right(&app.create_value, &mut app.create_value_cursor)
                            }
                            KeyCode::Up => {
                                move_cursor_up(&app.create_value, &mut app.create_value_cursor)
                            }
                            KeyCode::Down => {
                                move_cursor_down(&app.create_value, &mut app.create_value_cursor)
                            }
                            KeyCode::Home => {
                                move_cursor_line_start(&app.create_value, &mut app.create_value_cursor)
                            }
                            KeyCode::End => {
                                move_cursor_line_end(&app.create_value, &mut app.create_value_cursor)
                            }
                            KeyCode::Enter => insert_char_at_cursor(
                                &mut app.create_value,
                                &mut app.create_value_cursor,
                                '\n',
                            ),
                            KeyCode::Backspace => {
                                backspace_at_cursor(&mut app.create_value, &mut app.create_value_cursor)
                            }
                            KeyCode::Delete => {
                                delete_at_cursor(&mut app.create_value, &mut app.create_value_cursor)
                            }
                            KeyCode::Char(c) => {
                                if !key.modifiers.contains(KeyModifiers::CONTROL) {
                                    insert_char_at_cursor(
                                        &mut app.create_value,
                                        &mut app.create_value_cursor,
                                        c,
                                    );
                                }
                            }
                            _ => {}
                        },
                        ValueEditorMode::Normal => match key.code {
                            KeyCode::Esc => app.cancel_create(),
                            KeyCode::Tab => app.switch_create_field(),
                            KeyCode::Char('i') => app.create_value_mode = ValueEditorMode::Insert,
                            KeyCode::Char('a') => {
                                move_cursor_right(&app.create_value, &mut app.create_value_cursor);
                                app.create_value_mode = ValueEditorMode::Insert;
                            }
                            KeyCode::Char('h') | KeyCode::Left => {
                                move_cursor_left(&app.create_value, &mut app.create_value_cursor)
                            }
                            KeyCode::Char('l') | KeyCode::Right => {
                                move_cursor_right(&app.create_value, &mut app.create_value_cursor)
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                move_cursor_up(&app.create_value, &mut app.create_value_cursor)
                            }
                            KeyCode::Char('j') | KeyCode::Down => {
                                move_cursor_down(&app.create_value, &mut app.create_value_cursor)
                            }
                            KeyCode::Char('0') | KeyCode::Home => {
                                move_cursor_line_start(&app.create_value, &mut app.create_value_cursor)
                            }
                            KeyCode::Char('$') | KeyCode::End => {
                                move_cursor_line_end(&app.create_value, &mut app.create_value_cursor)
                            }
                            KeyCode::Char('x') | KeyCode::Delete => {
                                delete_at_cursor(&mut app.create_value, &mut app.create_value_cursor)
                            }
                            KeyCode::Enter => app.submit_create(),
                            _ => {}
                        },
                    },
                }
                continue;
            }

            if app.search_mode {
                match key.code {
                    KeyCode::Esc | KeyCode::Enter => app.end_search(),
                    KeyCode::Down | KeyCode::Char('j') => app.next(),
                    KeyCode::Up | KeyCode::Char('k') => app.previous(),
                    KeyCode::Backspace => {
                        app.query.pop();
                        app.apply_filter();
                    }
                    KeyCode::Char(c) => {
                        app.query.push(c);
                        app.apply_filter();
                    }
                    _ => {}
                }
            } else {
                match key.code {
                    KeyCode::Char('c')
                        if key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        return Ok(())
                    }
                    KeyCode::Char('/') => app.start_search(),
                    KeyCode::Char('a') => app.start_create(),
                    KeyCode::Char('R') => app.start_full_refresh(),
                    KeyCode::Char('y') => {
                        if let Some(param) = app.selected_parameter() {
                            let param_name = param.name.clone();
                            let param_value = param.value.clone();

                            if let Some(value) = param_value {
                                match copy_to_clipboard(&value) {
                                    Ok(()) => {
                                        app.status =
                                            format!("Copied value of {param_name} to clipboard");
                                    }
                                    Err(err) => {
                                        app.status = format!("Clipboard error: {err}");
                                    }
                                }
                            } else {
                                app.request_value_for_name(&param_name);
                                app.status =
                                    format!("Value for {param_name} not loaded yet. Loading in background");
                            }
                        } else {
                            app.status = String::from("No selection to copy");
                        }
                    }
                    KeyCode::Char('e') => {
                        if let Some(param) = app.selected_parameter() {
                            let param_name = param.name.clone();
                            let param_meta = param.meta.clone();

                            match fetch_parameter_value_from_ssm(
                                &param_name,
                                app.configured_region_owned(),
                            ) {
                                Ok(fetched_value) => {
                                    app.set_value_for_name(&param_name, fetched_value.clone());
                                    match open_value_in_editor(terminal, &fetched_value) {
                                        Ok(Some(edited_value)) => {
                                            match put_parameter_value_to_ssm(
                                                &param_name,
                                                &edited_value,
                                                &param_meta,
                                                app.configured_region_owned(),
                                            ) {
                                                Ok(()) => {
                                                    app.set_value_for_name(&param_name, edited_value);
                                                    app.status = format!(
                                                        "Updated {} in SSM and local state",
                                                        param_name
                                                    );
                                                }
                                                Err(err) => {
                                                    app.status = format!("SSM update error: {err}");
                                                }
                                            }
                                        }
                                        Ok(None) => {
                                            app.status = format!("No changes for {}", param_name);
                                        }
                                        Err(err) => {
                                            app.status = format!("Editor error: {err}");
                                        }
                                    }
                                }
                                Err(err) => {
                                    app.status = format!("Fetch before edit failed: {err}");
                                }
                            }
                        } else {
                            app.status = String::from("No selection to edit");
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => app.next(),
                    KeyCode::Up | KeyCode::Char('k') => app.previous(),
                    _ => {}
                }
            }
        }
    }
}
