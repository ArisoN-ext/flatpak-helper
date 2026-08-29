use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{Event, KeyCode, KeyEventKind, KeyModifiers, read},
    execute, queue,
    style::{Attribute, Print, SetAttribute},
    terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode, size},
};
use serde::Deserialize;
use std::collections::HashSet;
use std::env;
use std::io::{Write, stdout};
use std::os::unix::process::CommandExt;
use std::process::{Command, exit};

#[derive(Deserialize, Debug)]
struct FlatpakItem {
    application_id: Option<String>,
    name: Option<String>,
    remotes: Option<String>,
    origin: Option<String>,
}

#[derive(Deserialize, Debug)]
struct RemoteItem {
    name: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
struct Match {
    app_id: String,
    name: String,
    remote: String,
}

fn get_remotes() -> HashSet<String> {
    let mut remotes = HashSet::new();
    if let Ok(output) = Command::new("flatpak").args(["remotes", "--json"]).output() {
        if let Ok(text) = String::from_utf8(output.stdout) {
            if let Ok(items) = serde_json::from_str::<Vec<RemoteItem>>(&text) {
                for item in items {
                    if let Some(name) = item.name {
                        remotes.insert(name.trim().to_string());
                    }
                }
            }
        }
    }
    remotes
}

fn search_remote(keyword: &str) -> Vec<Match> {
    let mut matches = Vec::new();
    let mut seen = HashSet::new();

    if let Ok(output) = Command::new("flatpak")
        .args(["search", keyword, "--columns=application,name,remotes"])
        .output()
    {
        if let Ok(text) = String::from_utf8(output.stdout) {
            for line in text.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 2 {
                    let app_id = parts[0].trim().to_string();
                    let name = parts[1].trim().to_string();
                    let remote = if parts.len() >= 3 {
                        parts[2].split(',').next().unwrap_or("").trim().to_string()
                    } else {
                        String::new()
                    };
                    if !app_id.is_empty() && seen.insert(app_id.clone()) {
                        matches.push(Match {
                            app_id,
                            name,
                            remote,
                        });
                    }
                }
            }
        }
    }
    matches
}

fn search_installed(keyword: &str) -> Vec<Match> {
    let mut matches = Vec::new();
    let mut seen = HashSet::new();
    let keyword_lower = keyword.to_lowercase();

    if let Ok(output) = Command::new("flatpak")
        .args(["list", "--columns=application,name,origin"])
        .output()
    {
        if let Ok(text) = String::from_utf8(output.stdout) {
            for line in text.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 2 {
                    let app_id = parts[0].trim().to_string();
                    let name = parts[1].trim().to_string();
                    let remote = if parts.len() >= 3 {
                        parts[2].trim().to_string()
                    } else {
                        String::new()
                    };

                    if app_id.to_lowercase().contains(&keyword_lower)
                        || name.to_lowercase().contains(&keyword_lower)
                    {
                        if !app_id.is_empty() && seen.insert(app_id.clone()) {
                            matches.push(Match {
                                app_id,
                                name,
                                remote,
                            });
                        }
                    }
                }
            }
        }
    }
    matches
}

fn prompt_choice(matches: &[Match], keyword: &str) -> Option<Match> {
    let mut stdout = stdout();
    if enable_raw_mode().is_err() {
        return None;
    }
    let _ = execute!(stdout, Hide);

    let mut current_row: usize = 0;
    let mut offset: usize = 0;
    let mut result = None;
    let mut search_query = String::new();
    let mut is_searching = false;
    let mut last_key_g = false;

    loop {
        let (cols, rows) = size().unwrap_or((80, 24));
        let max_visible = (rows.saturating_sub(1)) as usize;

        if max_visible == 0 {
            break;
        }

        let filtered: Vec<&Match> = if search_query.is_empty() {
            matches.iter().collect()
        } else {
            let q = search_query.to_lowercase();
            matches
                .iter()
                .filter(|m| {
                    m.name.to_lowercase().contains(&q) || m.app_id.to_lowercase().contains(&q)
                })
                .collect()
        };

        if current_row >= filtered.len() {
            current_row = filtered.len().saturating_sub(1);
        }

        if current_row < offset {
            offset = current_row;
        } else if current_row >= offset + max_visible {
            offset = current_row - max_visible + 1;
        }

        let max_name_len = filtered
            .iter()
            .map(|m| m.name.chars().count())
            .max()
            .unwrap_or(0);

        let _ = queue!(stdout, Clear(ClearType::All));

        let header = if is_searching {
            format!("Select '{}' (Search: {}_):", keyword, search_query)
        } else if !search_query.is_empty() {
            format!("Select '{}' (Search: {}):", keyword, search_query)
        } else {
            format!("Select '{}' (/ to search):", keyword)
        };

        let header_display = if header.chars().count() > cols as usize {
            header.chars().take(cols as usize).collect::<String>()
        } else {
            header
        };

        let _ = queue!(
            stdout,
            MoveTo(0, 0),
            SetAttribute(Attribute::Bold),
            Print(header_display),
            SetAttribute(Attribute::Reset)
        );

        let name_col_width = std::cmp::min(max_name_len, (cols as usize / 2).max(15));

        for idx in 0..max_visible {
            let actual_idx = offset + idx;
            if actual_idx >= filtered.len() {
                break;
            }

            let m = &filtered[actual_idx];

            let name_chars: Vec<char> = m.name.chars().collect();
            let display_name = if name_chars.len() > name_col_width {
                let mut t: String = name_chars
                    .into_iter()
                    .take(name_col_width.saturating_sub(1))
                    .collect();
                t.push('…');
                t
            } else {
                m.name.clone()
            };

            let padding_len = name_col_width.saturating_sub(display_name.chars().count());
            let padding = " ".repeat(padding_len);

            let text = format!("{}{padding}  {}", display_name, m.app_id);

            let display_text = if text.chars().count() > cols as usize {
                text.chars().take(cols as usize).collect::<String>()
            } else {
                text
            };

            let _ = queue!(stdout, MoveTo(0, (idx + 1) as u16));

            if actual_idx == current_row {
                let _ = queue!(
                    stdout,
                    SetAttribute(Attribute::Reverse),
                    Print(display_text),
                    SetAttribute(Attribute::Reset)
                );
            } else {
                let _ = queue!(stdout, Print(display_text));
            }
        }

        let _ = stdout.flush();

        if let Ok(Event::Key(key_event)) = read() {
            if key_event.kind == KeyEventKind::Press {
                if key_event.code == KeyCode::Char('c')
                    && key_event.modifiers.contains(KeyModifiers::CONTROL)
                {
                    break;
                }

                if is_searching {
                    match key_event.code {
                        KeyCode::Char(c)
                            if c.is_alphanumeric() || c.is_ascii_punctuation() || c == ' ' =>
                        {
                            search_query.push(c);
                            current_row = 0;
                            offset = 0;
                        }
                        KeyCode::Backspace => {
                            search_query.pop();
                            current_row = 0;
                            offset = 0;
                        }
                        KeyCode::Esc => {
                            is_searching = false;
                            search_query.clear();
                            current_row = 0;
                            offset = 0;
                        }
                        KeyCode::Enter => {
                            if !filtered.is_empty() {
                                result = Some((*filtered[current_row]).clone());
                                break;
                            }
                        }
                        KeyCode::Up => {
                            if current_row > 0 {
                                current_row -= 1;
                            }
                        }
                        KeyCode::Down => {
                            if current_row < filtered.len().saturating_sub(1) {
                                current_row += 1;
                            }
                        }
                        KeyCode::PageUp => {
                            current_row = current_row.saturating_sub(max_visible);
                        }
                        KeyCode::PageDown => {
                            current_row = std::cmp::min(
                                current_row + max_visible,
                                filtered.len().saturating_sub(1),
                            );
                        }
                        _ => {}
                    }
                } else {
                    match key_event.code {
                        KeyCode::Char('/') => {
                            is_searching = true;
                            current_row = 0;
                            offset = 0;
                            last_key_g = false;
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            if current_row > 0 {
                                current_row -= 1;
                            }
                            last_key_g = false;
                        }
                        KeyCode::Char('j') | KeyCode::Down => {
                            if current_row < filtered.len().saturating_sub(1) {
                                current_row += 1;
                            }
                            last_key_g = false;
                        }
                        KeyCode::PageUp => {
                            current_row = current_row.saturating_sub(max_visible);
                            last_key_g = false;
                        }
                        KeyCode::PageDown => {
                            current_row = std::cmp::min(
                                current_row + max_visible,
                                filtered.len().saturating_sub(1),
                            );
                            last_key_g = false;
                        }
                        KeyCode::Char('G') => {
                            current_row = filtered.len().saturating_sub(1);
                            last_key_g = false;
                        }
                        KeyCode::Char('g') => {
                            if last_key_g {
                                current_row = 0; // gg - в начало
                                last_key_g = false;
                            } else {
                                last_key_g = true;
                            }
                        }
                        KeyCode::Enter => {
                            if !filtered.is_empty() {
                                result = Some((*filtered[current_row]).clone());
                                break;
                            }
                        }
                        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                            break;
                        }
                        _ => {
                            last_key_g = false;
                        }
                    }
                }
            }
        }
    }

    let _ = execute!(stdout, Clear(ClearType::All), MoveTo(0, 0), Show);
    let _ = disable_raw_mode();

    result
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        let err = Command::new("flatpak").arg("--help").exec();
        eprintln!("Error executing flatpak: {}", err);
        exit(1);
    }

    let cmd = &args[1];

    // Commands that ACTUALLY need keyword resolution.
    // Anything else (like build, remote-add, repair, etc.) is passed through as-is safely.
    let resolve_cmds = [
        "install",
        "uninstall",
        "remove",
        "run",
        "info",
        "mask",
        "override",
        "make-current",
        "history",
        "enter",
        "kill",
        "search",
    ];

    if !resolve_cmds.contains(&cmd.as_str()) {
        let err = Command::new("flatpak").args(&args[1..]).exec();
        eprintln!("Error executing flatpak: {}", err);
        exit(1);
    }

    let remotes = get_remotes();
    let mut new_args = vec![cmd.clone()];

    for arg in &args[2..] {
        if arg.starts_with('-') || remotes.contains(arg) || arg.contains('.') {
            new_args.push(arg.clone());
        } else {
            let matches = if cmd == "install" || cmd == "search" {
                search_remote(arg)
            } else {
                search_installed(arg)
            };

            if matches.is_empty() {
                eprintln!("Error: No matches found for '{}'", arg);
                exit(1);
            }

            let selected_match = if matches.len() == 1 {
                matches[0].clone()
            } else {
                if let Some(m) = prompt_choice(&matches, arg) {
                    m
                } else {
                    exit(1);
                }
            };

            if cmd == "search" {
                new_args = vec!["remote-info".to_string()];
                if !selected_match.remote.is_empty() {
                    new_args.push(selected_match.remote.clone());
                }
                new_args.push(selected_match.app_id.clone());
            } else {
                new_args.push(selected_match.app_id.clone());
            }
        }
    }

    let err = Command::new("flatpak").args(&new_args).exec();
    eprintln!("Error executing flatpak: {}", err);
    exit(1);
}
