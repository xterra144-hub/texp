use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Command,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FileState {
    Modifired,
    Added,
    Untracked,
    Renamed,
    Conflicted,
    Ignored,
    Deleted
}

/// Возвращает корень git-репозитория для указанного пути, если он есть.
pub fn git_root(repo: &Path) -> Option<PathBuf> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let root = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if root.is_empty() {
        None
    } else {
        Some(PathBuf::from(root))
    }
}

pub fn git_statuses(repo: &Path) -> std::io::Result<HashMap<PathBuf, FileState>> {
    // Пути в выводе `git status --porcelain` указываются относительно корня
    // репозитория, поэтому нам нужен именно корень, а не текущая директория.
    let root = match git_root(repo) {
        Some(r) => r,
        None => return Ok(HashMap::new()),
    };

    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["status", "--porcelain=v1", "-z", "--untracked-files=all"])
        .output()?;
    if !out.status.success() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            String::from_utf8_lossy(&out.stderr).to_string(),
        ));
    }
    let mut map = HashMap::new();
    let mut parts = out.stdout.split(|&b| b == 0);
    while let Some(rec) = parts.next() {
        if rec.len() < 4 {continue;}
        let (x,y) = (rec[0] as char, rec[1] as char);
        let path = String::from_utf8_lossy(&rec[3..]).into_owned();
        if x == 'R' || x == 'C' {parts.next();}
        let state = match (x,y) {
            ('?', '?') => FileState::Untracked,
            ('!', '!') => FileState::Ignored,
            ('U', _) | (_, 'U') | ('A', 'A') | ('D', 'D') => FileState::Conflicted,
            ('A', _) => FileState::Added,
            ('R', _) | (_, 'R') => FileState::Renamed,
            ('D', _) | (_, 'D') => FileState::Deleted,
            _ => FileState::Modifired
        };
        // Склеиваем с корнем репозитория и канонизируем, чтобы путь совпадал
        // с путями в nav.files (fs::read_dir отдаёт абсолютные пути).
        let full_path = root.join(&path);
        let canonical = full_path.canonicalize().unwrap_or(full_path);
        map.insert(canonical, state);
    }
    Ok(map)
}

pub fn git_branch(repo: &Path) -> Option<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;
    if out.status.success() {
        Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
    } else {
        None
    }
}

pub fn git_add(repo: &Path, file: &Path) -> std::io::Result<()> {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["add", &file.to_string_lossy()])
        .output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            String::from_utf8_lossy(&out.stderr).to_string(),
        ))
    }
}

pub fn git_commit(repo: &Path, title: &str, description: &str) -> std::io::Result<()> {
    let mut args = vec!["commit", "-m", title];
    if !description.is_empty() {
        args.push("-m");
        args.push(description);
    }
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(&args)
        .output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            String::from_utf8_lossy(&out.stderr).to_string(),
        ))
    }
}
