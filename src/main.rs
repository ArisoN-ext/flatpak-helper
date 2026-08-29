use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{Event, KeyCode, KeyEventKind, KeyModifiers, read},
    execute, queue,
    style::{Attribute, Print, SetAttribute},
    terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode, size},
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
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
    branch: Option<String>,
    arch: Option<String>,
}

#[derive(Deserialize, Debug)]
struct RemoteItem {
    name: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BranchInfo {
    branch: String,
    arch: String,
    remote: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AppGroup {
    app_id: String,
    name: String,
    branches: Vec<BranchInfo>,
}

#[derive(Clone)]
struct DisplayItem {
    idx: usize,
    col1: String,
    col2: String,
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

fn search_remote(keyword: &str) -> Vec<AppGroup> {
    let mut map: HashMap<String, AppGroup> = HashMap::new();

    if let Ok(output) = Command::new("flatpak")
        .args(["search", keyword, "--json"])
        .output()
    {
        if let Ok(text) = String::from_utf8(output.stdout) {
            if let Ok(items) = serde_json::from_str::<Vec<FlatpakItem>>(&text) {
                for item in items {
                    if let (Some(app_id), Some(name)) = (item.application_id, item.name) {
                        let remote = item
                            .remotes
                            .and_then(|r| r.split(',').next().map(|s| s.trim().to_string()))
                            .unwrap_or_default();
                        let branch = item.branch.unwrap_or_default();
                        let arch = item.arch.unwrap_or_else(|| "x86_64".to_string());

                        if !app_id.is_empty() {
                            let entry = map.entry(app_id.clone()).or_insert_with(|| AppGroup {
                                app_id: app_id.clone(),
                                name,
                                branches: Vec::new(),
                            });
                            let b_info = BranchInfo {
                                branch,
                                arch,
                                remote,
                            };
                            if !entry.branches.contains(&b_info) {
                                entry.branches.push(b_info);
                            }
                        }
                    }
                }
            }
        }
    }
    map.into_values().collect()
}

fn search_installed(keyword: &str) -> Vec<AppGroup> {
    let mut map: HashMap<String, AppGroup> = HashMap::new();
    let keyword_lower = keyword.to_lowercase();

    if let Ok(output) = Command::new("flatpak").args(["list", "--json"]).output() {
        if let Ok(text) = String::from_utf8(output.stdout) {
            if let Ok(items) = serde_json::from_str::<Vec<FlatpakItem>>(&text) {
                for item in items {
                    if let (Some(app_id), Some(name)) = (item.application_id, item.name) {
                        let remote = item.origin.unwrap_or_default().trim().to_string();
                        let branch = item.branch.unwrap_or_default();
                        let arch = item.arch.unwrap_or_else(|| "x86_64".to_string());

                        if app_id.to_lowercase().contains(&keyword_lower)
                            || name.to_lowercase().contains(&keyword_lower)
                        {
                            if !app_id.is_empty() {
                                let entry = map.entry(app_id.clone()).or_insert_with(|| AppGroup {
                                    app_id: app_id.clone(),
                                    name,
                                    branches: Vec::new(),
                                });
                                let b_info = BranchInfo {
                                    branch,
                                    arch,
                                    remote,
                                };
                                if !entry.branches.contains(&b_info) {
                                    entry.branches.push(b_info);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    map.into_values().collect()
}

fn tui_select(items: &[DisplayItem], keyword: &str, title_prefix: &str) -> Option<usize> {
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

        let filtered: Vec<&DisplayItem> = if search_query.is_empty() {
            items.iter().collect()
        } else {
            let q = search_query.to_lowercase();
            items
                .iter()
                .filter(|m| {
                    m.col1.to_lowercase().contains(&q) || m.col2.to_lowercase().contains(&q)
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

        let max_col1_len = filtered
            .iter()
            .map(|m| m.col1.chars().count())
            .max()
            .unwrap_or(0);

        let _ = queue!(stdout, Clear(ClearType::All));

        let header = if is_searching {
            format!(
                "{} '{}' (Search: {}_):",
                title_prefix, keyword, search_query
            )
        } else if !search_query.is_empty() {
            format!("{} '{}' (Search: {}):", title_prefix, keyword, search_query)
        } else {
            format!("{} '{}' (/ to search):", title_prefix, keyword)
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

        let col1_width = std::cmp::min(max_col1_len, (cols as usize / 2).max(15));

        for idx in 0..max_visible {
            let actual_idx = offset + idx;
            if actual_idx >= filtered.len() {
                break;
            }

            let m = &filtered[actual_idx];

            let col1_chars: Vec<char> = m.col1.chars().collect();
            let display_col1 = if col1_chars.len() > col1_width {
                let mut t: String = col1_chars
                    .into_iter()
                    .take(col1_width.saturating_sub(1))
                    .collect();
                t.push('…');
                t
            } else {
                m.col1.clone()
            };

            let padding_len = col1_width.saturating_sub(display_col1.chars().count());
            let padding = " ".repeat(padding_len);

            let text = format!("{}{padding}  {}", display_col1, m.col2);

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
                                result = Some(filtered[current_row].idx);
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
                                current_row = 0;
                                last_key_g = false;
                            } else {
                                last_key_g = true;
                            }
                        }
                        KeyCode::Enter => {
                            if !filtered.is_empty() {
                                result = Some(filtered[current_row].idx);
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
            let mut groups = if cmd == "install" || cmd == "search" {
                search_remote(arg)
            } else {
                search_installed(arg)
            };

            if groups.is_empty() {
                eprintln!("Error: No matches found for '{}'", arg);
                exit(1);
            }

            groups.sort_by(|a, b| a.name.cmp(&b.name));

            let selected_group = if groups.len() == 1 {
                &groups[0]
            } else {
                let display_items: Vec<DisplayItem> = groups
                    .iter()
                    .enumerate()
                    .map(|(i, g)| DisplayItem {
                        idx: i,
                        col1: g.name.clone(),
                        col2: format!(
                            "{} [{}]",
                            g.app_id,
                            if g.branches.len() > 1 {
                                format!("{} branches", g.branches.len())
                            } else {
                                g.branches
                                    .first()
                                    .map(|b| b.branch.clone())
                                    .unwrap_or_default()
                            }
                        ),
                    })
                    .collect();

                let idx = tui_select(&display_items, arg, "Select app").unwrap_or_else(|| exit(1));
                &groups[idx]
            };

            let selected_branch = if selected_group.branches.len() <= 1 {
                selected_group.branches.first().cloned()
            } else {
                let mut branches = selected_group.branches.clone();
                branches.sort_by(|a, b| a.branch.cmp(&b.branch));

                let branch_items: Vec<DisplayItem> = branches
                    .iter()
                    .enumerate()
                    .map(|(i, b)| DisplayItem {
                        idx: i,
                        col1: b.branch.clone(),
                        col2: format!("{} ({})", b.arch, b.remote),
                    })
                    .collect();

                let idx = tui_select(&branch_items, &selected_group.name, "Select branch")
                    .unwrap_or_else(|| exit(1));
                Some(branches[idx].clone())
            };

            let ref_str = if let Some(b) = selected_branch.as_ref() {
                if b.arch.is_empty() || b.branch.is_empty() {
                    selected_group.app_id.clone()
                } else {
                    format!("{}/{}/{}", selected_group.app_id, b.arch, b.branch)
                }
            } else {
                selected_group.app_id.clone()
            };

            if cmd == "search" {
                new_args = vec!["remote-info".to_string()];
                if let Some(b) = selected_branch.as_ref() {
                    if !b.remote.is_empty() {
                        new_args.push(b.remote.clone());
                    }
                }
                new_args.push(ref_str);
            } else {
                new_args.push(ref_str);
            }
        }
    }

    let err = Command::new("flatpak").args(&new_args).exec();
    eprintln!("Error executing flatpak: {}", err);
    exit(1);
}
