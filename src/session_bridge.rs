use crate::model::{Account, path_key};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OpenFlags};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{BufRead, BufReader, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    time::SystemTime,
};
use uuid::Uuid;

const FIRST_LINE_LIMIT: u64 = 1_048_576;
const MAX_ROLLOUTS_PER_HOME: usize = 5_000;
const MAX_ANCESTORS: usize = 24;
const MAX_TRANSFER_BYTES: u64 = 1_000_000_000;

#[derive(Clone, Debug)]
pub struct SavedSession {
    pub account_id: String,
    pub id: String,
    pub title: String,
    pub cwd: PathBuf,
    pub path: PathBuf,
    pub modified: SystemTime,
    pub bytes: u64,
}

#[derive(Debug)]
pub struct ImportResult {
    pub session_id: String,
    pub project: PathBuf,
    pub files_added: usize,
    pub bytes_added: u64,
}

#[derive(Debug)]
struct RolloutHead {
    id: String,
    cwd: PathBuf,
    history_base: Option<String>,
}

fn parse_head(path: &Path) -> Result<RolloutHead> {
    let file = File::open(path)?;
    let mut line = String::new();
    let count = BufReader::new(file.take(FIRST_LINE_LIMIT + 1)).read_line(&mut line)?;
    ensure!(
        count > 0 && (count as u64) <= FIRST_LINE_LIMIT,
        "Rollout header is missing or unusually large."
    );
    let value: Value = serde_json::from_str(&line).context("Invalid session metadata.")?;
    ensure!(
        value["type"] == "session_meta",
        "This file does not begin with Codex session metadata."
    );
    let p = &value["payload"];
    let id = p["id"].as_str().context("Session ID is missing.")?;
    Uuid::parse_str(id).context("Invalid session ID.")?;
    let cwd = p["cwd"].as_str().unwrap_or("");
    let base = p["history_base"]["thread_id"]
        .as_str()
        .map(|s| Uuid::parse_str(s).map(|_| s.to_string()))
        .transpose()
        .context("Malformed ancestor thread ID.")?;
    Ok(RolloutHead {
        id: id.to_string(),
        cwd: PathBuf::from(cwd),
        history_base: base,
    })
}

fn id_from_filename(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    if !name.starts_with("rollout-") || !name.ends_with(".jsonl") {
        return None;
    }
    let id = name.strip_suffix(".jsonl")?.get(name.len() - 42..)?;
    Uuid::parse_str(id).ok().map(|_| id.to_string())
}

fn discover_paths(root: &Path) -> Result<Vec<PathBuf>> {
    let base = root.join("sessions");
    if !base.is_dir() {
        return Ok(Vec::new());
    }
    let mut stack = vec![base];
    let mut found = Vec::new();
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            let kind = entry.file_type()?;
            if kind.is_symlink() {
                continue;
            }
            if kind.is_dir() {
                if stack.len() < MAX_ROLLOUTS_PER_HOME {
                    stack.push(entry.path());
                }
            } else if kind.is_file() && id_from_filename(&entry.path()).is_some() {
                found.push(entry.path());
                ensure!(
                    found.len() <= MAX_ROLLOUTS_PER_HOME,
                    "Too many Codex sessions in one profile."
                );
            }
        }
    }
    Ok(found)
}

fn names_from_index(home: &Path) -> HashMap<String, String> {
    let mut names = HashMap::new();
    let path = home.join("session_index.jsonl");
    let Ok(meta) = fs::metadata(&path) else {
        return names;
    };
    if meta.len() > 16 * 1024 * 1024 {
        return names;
    }
    let Ok(file) = File::open(path) else {
        return names;
    };
    for line in BufReader::new(file).lines().map_while(Result::ok) {
        if let Ok(v) = serde_json::from_str::<Value>(&line)
            && let (Some(id), Some(name)) = (v["id"].as_str(), v["thread_name"].as_str())
        {
            let title = display_title(name);
            if !title.is_empty() && Uuid::parse_str(id).is_ok() {
                names.insert(id.to_owned(), title);
            }
        }
    }
    names
}

fn display_title(raw: &str) -> String {
    let normalized = raw
        .split_whitespace()
        .map(|word| {
            if let Some((local, domain)) = word.split_once('@')
                && !local.is_empty()
                && domain.contains('.')
            {
                return "[email hidden]".to_string();
            }
            word.chars()
                .filter(|ch| {
                    !ch.is_control()
                        && !matches!(ch, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(" ");
    normalized.chars().take(90).collect()
}

/// Queries the official CLI's session index read-only. SQLite falls back to
/// session_index.jsonl when a profile comes from an older Codex version.
fn titles_from_db(home: &Path) -> HashMap<String, String> {
    let mut names = HashMap::new();
    let path = home.join("state_5.sqlite");
    if !path.is_file() {
        return names;
    }
    let Ok(connection) = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) else {
        return names;
    };
    let Ok(mut query) = connection.prepare(
        "SELECT id, COALESCE(NULLIF(TRIM(name),''), NULLIF(TRIM(title),''),'')          FROM threads WHERE archived IS NULL OR archived = 0"
    ) else { return names };
    let Ok(rows) = query.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    }) else {
        return names;
    };
    for row in rows.flatten() {
        let clean = display_title(&row.1);
        if !clean.is_empty() && Uuid::parse_str(&row.0).is_ok() {
            names.insert(row.0, clean);
        }
    }
    names
}

pub fn list_sessions(accounts: &[Account]) -> Result<Vec<SavedSession>> {
    let mut sessions = Vec::new();
    for account in accounts {
        let mut titles = names_from_index(&account.home);
        titles.extend(titles_from_db(&account.home));
        for path in discover_paths(&account.home)? {
            let Some(file_id) = id_from_filename(&path) else {
                continue;
            };
            let Ok(head) = parse_head(&path) else {
                continue;
            };
            if head.id != file_id {
                continue;
            }
            let Ok(metadata) = fs::metadata(&path) else {
                continue;
            };
            let short = &head.id[..8];
            sessions.push(SavedSession {
                account_id: account.id.clone(),
                id: head.id.clone(),
                title: titles
                    .get(&head.id)
                    .cloned()
                    .unwrap_or_else(|| format!("Session {short}")),
                cwd: head.cwd,
                path,
                modified: metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                bytes: metadata.len(),
            });
        }
    }
    sessions.sort_unstable_by_key(|a| std::cmp::Reverse(a.modified));
    Ok(sessions)
}
fn files_equal(a: &Path, b: &Path) -> Result<bool> {
    if fs::metadata(a)?.len() != fs::metadata(b)?.len() {
        return Ok(false);
    }
    let mut left = BufReader::new(File::open(a)?);
    let mut right = BufReader::new(File::open(b)?);
    let mut ab = [0u8; 65_536];
    let mut bb = [0u8; 65_536];
    loop {
        let count = left.read(&mut ab)?;
        if count == 0 {
            return Ok(right.read(&mut bb)? == 0);
        }
        right.read_exact(&mut bb[..count])?;
        if ab[..count] != bb[..count] {
            return Ok(false);
        }
    }
}

fn complete_line(path: &Path) -> Result<()> {
    let mut f = File::open(path)?;
    let len = f.metadata()?.len();
    ensure!(len > 0, "The selected thread has no saved history.");
    f.seek(SeekFrom::End(-1))?;
    let mut last = [0];
    f.read_exact(&mut last)?;
    ensure!(
        last[0] == b'\n',
        "The thread is still writing an unfinished history record. Retry after it becomes idle."
    );
    Ok(())
}

/// Copy a stable snapshot into the destination account without touching either
/// profile's credentials, configuration, SQLite database or existing conversations.
/// Codex itself discovers the snapshot and forks it into its own new thread.
pub fn import_into(source: &SavedSession, from: &Account, to: &Account) -> Result<ImportResult> {
    ensure!(
        source.account_id == from.id,
        "The source account changed. Refresh the session list."
    );
    ensure!(
        path_key(&from.home) != path_key(&to.home),
        "Select a different destination account."
    );
    ensure!(
        to.home.is_dir(),
        "The destination account directory is missing."
    );
    ensure!(
        source.cwd.is_absolute(),
        "The source session has no absolute project path."
    );
    let source_root = fs::canonicalize(from.home.join("sessions"))
        .context("The source account has no saved sessions.")?;
    let mut paths: HashMap<String, PathBuf> = HashMap::new();
    for path in discover_paths(&from.home)? {
        if let Some(id) = id_from_filename(&path) {
            paths.insert(id, path);
        }
    }
    let mut chain = Vec::new();
    let mut visited = HashSet::new();
    let mut next = Some(source.id.clone());
    while let Some(id) = next {
        ensure!(
            chain.len() < MAX_ANCESTORS,
            "This session has too many linked history ancestors."
        );
        ensure!(
            visited.insert(id.clone()),
            "Cyclic thread history is not supported."
        );
        let path = paths
            .get(&id)
            .context("A linked history ancestor is missing.")?
            .clone();
        ensure!(
            fs::canonicalize(&path)?.starts_with(&source_root),
            "History path escapes the source account."
        );
        let head = parse_head(&path)?;
        ensure!(
            head.id == id,
            "The source history filename and metadata disagree."
        );
        complete_line(&path)?;
        next = head.history_base;
        chain.push(path);
    }
    let target_root = to.home.join("sessions");
    let mut pending = Vec::new();
    let mut total = 0u64;
    for src in chain.iter().rev() {
        let relative = src
            .strip_prefix(from.home.join("sessions"))
            .context("Unexpected source history folder.")?;
        let destination = target_root.join(relative);
        let source_size = fs::metadata(src)?.len();
        total = total
            .checked_add(source_size)
            .context("History size overflow.")?;
        ensure!(
            total <= MAX_TRANSFER_BYTES,
            "This handoff is larger than 1 GB. Transfer stopped without changing any sessions."
        );
        if destination.exists() {
            ensure!(
                files_equal(src, &destination)?,
                "The destination already has a different version of this session. No history was overwritten."
            );
        } else {
            pending.push((src.clone(), destination));
        }
    }
    // Stage every file on the same volume as the target before publishing any.
    let stage = tempfile::Builder::new()
        .prefix(".slotshift-transfer-")
        .tempdir_in(&to.home)?;
    for (i, (src, _)) in pending.iter().enumerate() {
        let staged = stage.path().join(format!("{i}.jsonl"));
        let before = fs::metadata(src)?;
        let mut input = File::open(src)?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged)?;
        let copied = std::io::copy(
            &mut std::io::Read::by_ref(&mut input).take(before.len()),
            &mut output,
        )?;
        output.flush()?;
        output.sync_all()?;
        let after = fs::metadata(src)?;
        ensure!(
            copied == before.len()
                && before.len() == after.len()
                && before.modified()? == after.modified()?,
            "Source history changed during transfer. Nothing was imported; retry once it is idle."
        );
        complete_line(&staged)?;
        ensure!(
            files_equal(src, &staged)?,
            "The source history changed during transfer. Nothing was imported."
        );
    }
    let mut added = Vec::new();
    let publish = (|| -> Result<()> {
        for (i, (_, destination)) in pending.iter().enumerate() {
            ensure!(
                !destination.exists(),
                "Destination history changed during transfer. No existing file was overwritten."
            );
            fs::create_dir_all(
                destination
                    .parent()
                    .context("Invalid destination history path.")?,
            )?;
            // Atomically create without replacement. A racing Codex writer must
            // never lose an existing session to this handoff.
            fs::hard_link(stage.path().join(format!("{i}.jsonl")), destination).with_context(
                || {
                    format!(
                        "Could not publish history snapshot for session {}",
                        source.id
                    )
                },
            )?;
            added.push(destination.clone());
        }
        Ok(())
    })();
    if let Err(error) = publish {
        // Undo only paths created by this operation; never touch preexisting files.
        for path in added {
            let _ = fs::remove_file(path);
        }
        return Err(error);
    }
    Ok(ImportResult {
        session_id: source.id.clone(),
        project: source.cwd.clone(),
        files_added: pending.len(),
        bytes_added: pending
            .iter()
            .map(|(src, _)| fs::metadata(src).map(|m| m.len()).unwrap_or(0))
            .sum(),
    })
}
