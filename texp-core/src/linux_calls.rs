use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::state::OpenWithEntry;

pub struct ContextMenuEntry {
    pub label: String,
    pub verb: String,
    pub is_separator: bool,
    pub indent: u32,
    pub cmd_id: u32,
}

struct DesktopEntry {
    name: String,
    exec: String,
    icon: String,
    mimes: Vec<String>,
    no_display: bool,
}

fn entry(label: &str, verb: &str) -> ContextMenuEntry {
    ContextMenuEntry {
        label: label.to_string(),
        verb: verb.to_string(),
        is_separator: false,
        indent: 0,
        cmd_id: 0,
    }
}

fn separator() -> ContextMenuEntry {
    ContextMenuEntry {
        label: String::new(),
        verb: String::new(),
        is_separator: true,
        indent: 0,
        cmd_id: 0,
    }
}

fn mime_type(path: &Path) -> String {
    if path.is_dir() {
        return "inode/directory".to_string();
    }
    if let Ok(out) = Command::new("xdg-mime").args(["query", "filetype"]).arg(path).output()
        && out.status.success()
    {
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !s.is_empty() && s != "application/octet-stream" {
            return s;
        }
    }
    mime_guess::from_path(path)
        .first_raw()
        .unwrap_or("application/octet-stream")
        .to_string()
}

fn desktop_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = std::env::var_os("HOME") {
        let base = PathBuf::from(home);
        dirs.push(base.join(".local/share/applications"));
    }
    if let Ok(xdg) = std::env::var("XDG_DATA_HOME")
        && !xdg.is_empty()
    {
        dirs.push(PathBuf::from(xdg).join("applications"));
    }    dirs.push(PathBuf::from("/usr/local/share/applications"));
    dirs.push(PathBuf::from("/usr/share/applications"));
    dirs
}

fn parse_desktop(path: &Path) -> Option<DesktopEntry> {
    let content = fs::read_to_string(path).ok()?;
    let mut section = String::new();
    let mut fields = std::collections::HashMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].to_string();
            continue;
        }
        if section != "Desktop Entry" {
            continue;
        }
        if let Some(eq) = line.find('=') {
            fields.insert(line[..eq].trim().to_string(), line[eq + 1..].trim().to_string());
        }
    }
    let get = |k: &str| fields.get(k).cloned();
    if get("Type").as_deref() != Some("Application") {
        return None;
    }
    if get("NoDisplay").as_deref() == Some("true") || get("Hidden").as_deref() == Some("true") {
        return None;
    }
    let exec = get("Exec")?;
    if exec.is_empty() {
        return None;
    }
    let name = get("Name").unwrap_or_else(|| {
        path.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    let mimes = get("MimeType")
        .unwrap_or_default()
        .split(';')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    Some(DesktopEntry {
        name,
        exec,
        icon: get("Icon").unwrap_or_default(),
        mimes,
        no_display: get("NoDisplay").as_deref() == Some("true"),
    })
}

fn apps_for_mime(mime: &str) -> Vec<(String, PathBuf)> {
    let mut out: Vec<(String, PathBuf)> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for dir in desktop_dirs() {
        let Ok(entries) = fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.extension().and_then(|e| e.to_str()) != Some("desktop") {
                continue;
            }
            let Some(desktop) = parse_desktop(&p) else { continue };
            if desktop.no_display {
                continue;
            }
            if !desktop.mimes.iter().any(|m| m == mime) {
                continue;
            }
            if !seen.insert(desktop.name.clone()) {
                continue;
            }
            out.push((desktop.name, p));
        }
    }
    out.sort_by_key(|(name, _)| name.to_lowercase());
    out
}

pub fn query_context_menu_entries(paths: &[PathBuf]) -> Result<Vec<ContextMenuEntry>, String> {
    if paths.is_empty() {
        return Err("No files selected".into());
    }
    let mut entries = vec![
        entry("Открыть", "open"),
        separator(),
        entry("Вырезать", "cut"),
        entry("Копировать", "copy"),
        entry("Вставить", "paste"),
        entry("Переименовать", "rename"),
        entry("Удалить", "delete"),
        separator(),
    ];

    let mime = mime_type(&paths[0]);
    let apps = apps_for_mime(&mime);
    if !apps.is_empty() {
        for (name, desktop_path) in apps {
            entries.push(entry(&name, &desktop_path.to_string_lossy()));
        }
        entries.push(separator());
    }

    entries.push(entry("Свойства", "properties"));
    Ok(entries)
}

fn quote_shell(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn expand_exec(exec: &str, paths: &[PathBuf], name: &str, icon: &str, desktop: &Path) -> String {
    let files: Vec<String> = paths
        .iter()
        .map(|p| quote_shell(&p.to_string_lossy()))
        .collect();
    let mut out = String::new();
    let mut chars = exec.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('f') | Some('u') => out.push_str(files.first().map(|s| s.as_str()).unwrap_or("")),
            Some('F') | Some('U') => out.push_str(&files.join(" ")),
            Some('i') => {
                if !icon.is_empty() {
                    out.push_str(&format!("--icon {}", quote_shell(icon)));
                }
            }
            Some('c') => out.push_str(&quote_shell(name)),
            Some('k') => out.push_str(&quote_shell(&desktop.to_string_lossy())),
            Some('%') => out.push('%'),
            Some(_) | None => {}
        }
    }
    out
}

fn launch_desktop(desktop: &Path, paths: &[PathBuf]) -> Result<(), String> {
    if !paths.is_empty() {
        let mut cmd = Command::new("gio");
        cmd.args(["launch", desktop.to_str().unwrap_or("")]);
        for p in paths {
            cmd.arg(p);
        }
        if let Ok(out) = cmd.output()
            && out.status.success()
        {
            return Ok(());
        }
    }

    let Some(de) = parse_desktop(desktop) else {
        return Err(format!("Failed to parse desktop file: {}", desktop.display()));
    };
    let cmd = expand_exec(&de.exec, paths, &de.name, &de.icon, desktop);
    let status = Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .spawn()
        .map_err(|e| format!("Failed to launch '{}': {}", de.name, e))?;
    drop(status);
    Ok(())
}

fn run_open(path: &Path) -> Result<(), String> {
    if let Ok(out) = Command::new("xdg-open").arg(path).output()
        && out.status.success()
    {
        return Ok(());
    }
    for open in ["gio", "exo-open", "gnome-open", "kde-open5", "kde-open"] {
        if let Ok(out) = Command::new(open).arg("open").arg(path).output()
            && out.status.success()
        {
            return Ok(());
        }
    }
    Err(format!("No opener found for {}", path.display()))
}

pub fn invoke_verb(paths: &[PathBuf], verb: &str, _cmd_id: u32) -> Result<(), String> {
    if verb.is_empty() {
        return Err("Invalid arguments".into());
    }
    match verb {
        "open" => {
            let mut errors: Vec<String> = Vec::new();
            for p in paths {
                if let Err(e) = run_open(p) {
                    errors.push(e);
                }
            }
            if errors.len() == paths.len() && !errors.is_empty() {
                return Err(errors.join("; "));
            }
            Ok(())
        }
        v => {
            let desktop = PathBuf::from(v);
            if !desktop.exists() {
                return Err(format!("Unknown verb: {}", v));
            }
            launch_desktop(&desktop, paths)
        }
    }
}

pub fn query_open_with_entries(path: &Path) -> Vec<OpenWithEntry> {
    let mut entries: Vec<OpenWithEntry> = Vec::new();
    entries.push(OpenWithEntry {
        name: "Открыть по умолчанию".into(),
        exe_path: String::new(),
    });
    entries.push(OpenWithEntry {
        name: "Открыть в файловом менеджере".into(),
        exe_path: String::new(),
    });

    let mime = mime_type(path);
    let mut seen = HashSet::new();
    for (name, desktop_path) in apps_for_mime(&mime) {
        if seen.insert(name.clone()) {
            entries.push(OpenWithEntry {
                name,
                exe_path: desktop_path.to_string_lossy().into_owned(),
            });
        }
    }
    entries
}


