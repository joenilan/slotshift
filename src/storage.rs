use crate::model::{Account, Settings, path_key, validate_name};
use anyhow::{Context, Result, ensure};
use fs2::FileExt as _;
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::NamedTempFile;
pub struct Store {
    root: PathBuf,
    _lock: File,
}
impl Store {
    pub fn default_root() -> Result<PathBuf> {
        Ok(directories::BaseDirs::new()
            .context("Could not find the user data folder.")?
            .data_local_dir()
            .join("Slotshift"))
    }
    pub fn open(root: PathBuf, user_home: Option<&Path>) -> Result<(Self, Settings)> {
        ensure!(
            root.is_absolute() && root.parent().is_some(),
            "Choose an absolute application data directory, not a drive root."
        );
        if root.is_dir() && !root.join("settings.json").is_file() {
            let entries = fs::read_dir(&root)?.collect::<std::io::Result<Vec<_>>>()?;
            ensure!(
                entries.iter().all(|e| e.file_name() == "settings.lock"),
                "Choose an empty data folder or an existing Slotshift data folder. This directory has unrelated files and has not been changed."
            );
        }
        fs::create_dir_all(&root)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join("settings.lock"))?;
        lock.try_lock_exclusive()
            .context("Slotshift is already open for this data folder. Use its existing window.")?;
        // The owned root is private on every launch, including a retry after interrupted setup.
        secure_directory(&root)?;
        let store = Self { root, _lock: lock };
        let path = store.root.join("settings.json");
        let settings = if path.exists() {
            let mut bytes = Vec::new();
            File::open(&path)?
                .take(16 * 1024 * 1024 + 1)
                .read_to_end(&mut bytes)?;
            ensure!(
                bytes.len() <= 16 * 1024 * 1024,
                "The settings file is unexpectedly large; it has not been changed."
            );
            serde_json::from_slice::<Settings>(&bytes).context(
                "Could not read Slotshift settings. The original file has been left untouched.",
            )?
        } else {
            let settings = match user_home {
                Some(home) => discover_existing(home)?,
                None => Settings::default(),
            };
            store.save(&settings)?;
            settings
        };
        settings.validate()?;
        Ok((store, settings))
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn save(&self, settings: &Settings) -> Result<()> {
        settings.validate()?;
        let mut temp =
            NamedTempFile::new_in(&self.root).context("Could not prepare the settings file.")?;
        serde_json::to_writer_pretty(&mut temp, settings)?;
        temp.write_all(b"\n")?;
        temp.as_file().sync_all()?;
        temp.persist(self.root.join("settings.json"))
            .map_err(|e| e.error)
            .context("Could not save settings. Your previous settings are still available.")?;
        Ok(())
    }
    pub fn commit<T>(
        &self,
        settings: &mut Settings,
        change: impl FnOnce(&mut Settings) -> Result<T>,
    ) -> Result<T> {
        let mut next = settings.clone();
        let result = change(&mut next)?;
        self.save(&next)?;
        *settings = next;
        Ok(result)
    }
    pub fn add_account(
        &self,
        settings: &mut Settings,
        name: &str,
        existing: Option<&Path>,
    ) -> Result<String> {
        let name = validate_name(name)?;
        let mut account = match existing {
            Some(path) => {
                ensure!(
                    path.is_absolute() && path.is_dir(),
                    "Choose an existing, absolute Codex home folder."
                );
                ensure!(
                    path.join("config.toml").is_file() || path.join("auth.json").is_file(),
                    "That folder does not contain config.toml or auth.json. Choose a Codex home, not a project folder."
                );
                let canonical = fs::canonicalize(path)?;
                let default_home =
                    directories::BaseDirs::new().map(|b| b.home_dir().join(".codex"));
                let is_default = default_home
                    .as_ref()
                    .is_some_and(|d| path_key(d) == path_key(&canonical));
                Account::linked(&name, canonical, is_default)?
            }
            None => {
                let id = uuid::Uuid::new_v4().to_string();
                let home = self.root.join("accounts").join(&id);
                fs::create_dir_all(home.parent().context("Invalid account directory.")?)?;
                fs::create_dir(&home).context("Could not create a new account home.")?;
                let mut config = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(home.join("config.toml"))?;
                config.write_all(b"# This account's credentials and history stay in this Codex home.\ncli_auth_credentials_store = \"file\"\nforced_login_method = \"chatgpt\"\napproval_policy = \"on-request\"\nsandbox_mode = \"workspace-write\"\n")?;
                let mut account = Account::linked(&name, home, false)?;
                account.id = id;
                account
            }
        };
        ensure!(
            !settings
                .accounts
                .iter()
                .any(|a| path_key(&a.home) == path_key(&account.home)),
            "That Codex home is already in your account list."
        );
        account.options = Default::default();
        let id = account.id.clone();
        self.commit(settings, |next| {
            next.selected = Some(id.clone());
            next.accounts.push(account);
            Ok(())
        })?;
        Ok(id)
    }
}
/// Link old homes in place. Never copy credentials or history, or edit imported account configs.
pub fn discover_existing(user_home: &Path) -> Result<Settings> {
    let mut settings = Settings::default();
    let legacy_root = user_home.join(".codex-accounts");
    let legacy_state = fs::read_to_string(legacy_root.join("launcher-state.json"))
        .ok()
        .and_then(|text| {
            serde_json::from_str::<serde_json::Value>(text.trim_start_matches('\u{feff}')).ok()
        });
    let legacy_yolo = legacy_state
        .as_ref()
        .map(|v| v.get("yolo").and_then(|v| v.as_bool()).unwrap_or(true))
        .unwrap_or(false);
    let mut linked_names = Vec::new();
    let default_home = user_home.join(".codex");
    if default_home.is_dir() {
        let mut main = Account::linked("Main account", default_home, true)?;
        main.options.yolo = legacy_yolo;
        linked_names.push(("main".to_string(), main.id.clone()));
        settings.accounts.push(main);
    }
    if legacy_root.is_dir() {
        let mut entries: Vec<_> = fs::read_dir(&legacy_root)?.filter_map(Result::ok).collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            if !path.is_dir()
                || !(path.join("config.toml").is_file() || path.join("auth.json").is_file())
            {
                continue;
            }
            if settings
                .accounts
                .iter()
                .any(|a| path_key(&a.home) == path_key(&path))
            {
                continue;
            }
            let old_name = entry.file_name().to_string_lossy().into_owned();
            let label = match old_name.strip_prefix("plus") {
                Some(n) if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) => {
                    format!("Plus {n}")
                }
                _ => old_name.clone(),
            };
            let mut account = Account::linked(&label, path, false)?;
            account.options.yolo = legacy_yolo;
            linked_names.push((old_name, account.id.clone()));
            settings.accounts.push(account);
        }
    }
    if let Some(old) = legacy_state {
        settings.migrated_legacy = true;
        if let Some(selected) = old.get("account").and_then(|v| v.as_str()) {
            settings.selected = linked_names
                .iter()
                .find(|(name, _)| name == selected)
                .map(|(_, id)| id.clone());
        }
        if let Some(project) = old
            .get("project")
            .and_then(|v| v.as_str())
            .filter(|s| !s.contains(['\n', '\r', '\0']))
        {
            if let Some(account) = settings.current_mut() {
                account.project = project.into();
            }
            settings.remember_project(project);
        }
    }
    if settings.selected.is_none() {
        settings.selected = settings.accounts.first().map(|a| a.id.clone());
    }
    settings.validate()?;
    Ok(settings)
}
fn secure_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let system = std::env::var_os("SystemRoot")
            .map(PathBuf::from)
            .context("SystemRoot is unavailable.")?;
        let shell = system.join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let script = format!(
            "$ErrorActionPreference='Stop'; $p={}; $acl=New-Object Security.AccessControl.DirectorySecurity; $acl.SetAccessRuleProtection($true,$false); $sid=[Security.Principal.WindowsIdentity]::GetCurrent().User; foreach($s in @($sid.Value,'S-1-5-18','S-1-5-32-544')) {{$principal=New-Object Security.Principal.SecurityIdentifier($s); $rule=New-Object Security.AccessControl.FileSystemAccessRule($principal,'FullControl','ContainerInherit,ObjectInherit','None','Allow'); $acl.AddAccessRule($rule)}}; [IO.Directory]::SetAccessControl($p,$acl);",
            crate::launch::ps_quote(&path.to_string_lossy())
        );
        let result = Command::new(shell)
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                &script,
            ])
            .creation_flags(0x08000000)
            .output()
            .context("Could not protect the Slotshift data folder.")?;
        ensure!(
            result.status.success(),
            "Windows could not apply private-folder permissions. No account credentials have been created. {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    Ok(())
}
pub fn open_folder(path: &Path) -> Result<()> {
    ensure!(path.is_dir(), "That folder no longer exists.");
    #[cfg(windows)]
    let program = "explorer.exe";
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(all(not(windows), not(target_os = "macos")))]
    let program = "xdg-open";
    Command::new(program)
        .arg(path)
        .spawn()
        .context("Could not open the folder.")?;
    Ok(())
}
