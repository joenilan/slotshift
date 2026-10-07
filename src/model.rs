use anyhow::{Context, Result, bail, ensure};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;
pub const SCHEMA_VERSION: u32 = 1;
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct LaunchOptions {
    pub yolo: bool,
    pub worktree: bool,
    pub live_search: bool,
    pub inline_terminal: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialPolicy {
    Existing,
    IsolatedFile,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub name: String,
    pub home: PathBuf,
    pub credentials: CredentialPolicy,
    #[serde(default)]
    pub protect_login: bool,
    #[serde(default)]
    pub project: String,
    #[serde(default)]
    pub options: LaunchOptions,
}
impl Account {
    pub fn linked(name: &str, home: PathBuf, is_default: bool) -> Result<Self> {
        Ok(Self {
            id: Uuid::new_v4().to_string(),
            name: validate_name(name)?,
            home,
            credentials: if is_default {
                CredentialPolicy::Existing
            } else {
                CredentialPolicy::IsolatedFile
            },
            protect_login: is_default,
            project: String::new(),
            options: LaunchOptions::default(),
        })
    }
    pub fn initials(&self) -> String {
        let mut letters = self
            .name
            .split_whitespace()
            .filter_map(|s| s.chars().next());
        let first = letters.next().unwrap_or('S');
        let second = letters.next().or_else(|| self.name.chars().nth(1));
        match second {
            Some(c) => format!("{first}{c}").to_uppercase(),
            None => first.to_uppercase().to_string(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub schema_version: u32,
    pub accounts: Vec<Account>,
    pub selected: Option<String>,
    pub recent_projects: Vec<String>,
    pub codex_executable: Option<PathBuf>,
    pub migrated_legacy: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            accounts: vec![],
            selected: None,
            recent_projects: vec![],
            codex_executable: None,
            migrated_legacy: false,
        }
    }
}
impl Settings {
    pub fn current(&self) -> Option<&Account> {
        self.selected
            .as_deref()
            .and_then(|id| self.accounts.iter().find(|a| a.id == id))
    }
    pub fn current_mut(&mut self) -> Option<&mut Account> {
        let id = self.selected.as_deref()?;
        self.accounts.iter_mut().find(|a| a.id == id)
    }
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == SCHEMA_VERSION,
            "This settings file uses a different Slotshift version. It has not been overwritten."
        );
        let mut ids = HashSet::new();
        let mut homes = HashSet::new();
        for a in &self.accounts {
            validate_name(&a.name)?;
            ensure!(
                !a.id.is_empty() && ids.insert(a.id.clone()),
                "Duplicate or missing account identifier."
            );
            ensure!(
                a.home.is_absolute(),
                "An account home must be an absolute folder path."
            );
            ensure!(
                homes.insert(path_key(&a.home)),
                "Two entries point to the same Codex home."
            );
            ensure!(
                !a.project.contains(['\n', '\r', '\0']),
                "The project path contains a control character."
            );
        }
        if let Some(selected) = &self.selected {
            ensure!(
                ids.contains(selected),
                "The selected account does not exist."
            );
        }
        Ok(())
    }
    pub fn remember_project(&mut self, path: &str) {
        if path.trim().is_empty() {
            return;
        }
        let key = path_key(Path::new(path));
        self.recent_projects
            .retain(|p| path_key(Path::new(p)) != key);
        self.recent_projects.insert(0, path.to_string());
        self.recent_projects.truncate(8);
    }
    /// Metadata-only removal. Never delete credentials, history, files, or running processes.
    pub fn remove(&mut self, id: &str) -> Result<Account> {
        let index = self
            .accounts
            .iter()
            .position(|a| a.id == id)
            .context("Account not found.")?;
        let account = self.accounts.remove(index);
        if self.selected.as_deref() == Some(id) {
            self.selected = self
                .accounts
                .get(index.min(self.accounts.len().saturating_sub(1)))
                .map(|a| a.id.clone());
        }
        Ok(account)
    }
}
pub fn validate_name(name: &str) -> Result<String> {
    let name = name.trim();
    ensure!(!name.is_empty(), "Give the account a name.");
    ensure!(
        name.chars().count() <= 64,
        "Use an account name of 64 characters or fewer."
    );
    ensure!(
        !name.chars().any(char::is_control),
        "Account names cannot contain control characters."
    );
    Ok(name.to_string())
}
pub fn path_key(path: &Path) -> String {
    let resolved = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let text = resolved.to_string_lossy().to_string();
    #[cfg(windows)]
    {
        text.trim_start_matches(r"\\?\")
            .trim_end_matches(['\\', '/'])
            .replace('/', "\\")
            .to_lowercase()
    }
    #[cfg(not(windows))]
    {
        text
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginState {
    Saved,
    SignedOut,
    ExistingStore,
    Unreadable,
}
/// Cached display claims only. Tokens are not retained here or serialized.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginSummary {
    pub state: LoginState,
    pub email: Option<String>,
    pub plan: Option<String>,
    pub(crate) identity: Option<String>,
}
impl LoginSummary {
    pub fn label(&self) -> &'static str {
        match self.state {
            LoginState::Saved => "Login saved",
            LoginState::SignedOut => "Not signed in",
            LoginState::ExistingStore => "Existing Codex setup",
            LoginState::Unreadable => "Check sign-in",
        }
    }
    pub fn saved(&self) -> bool {
        self.state == LoginState::Saved
    }
    /// Never infer the login identity from the user-supplied account nickname.
    pub fn identity_label(&self, revealed: bool) -> String {
        if !self.saved() {
            return self.label().into();
        }
        match self
            .email
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        {
            Some(value) if revealed => value.into(),
            Some(value) => masked_identity(value),
            None => "Identity unavailable".into(),
        }
    }
    pub fn demo(plan: &str) -> Self {
        Self {
            state: LoginState::Saved,
            email: None,
            plan: Some(plan.into()),
            identity: None,
        }
    }
}
pub fn login_summary(account: &Account) -> LoginSummary {
    let mut summary = LoginSummary {
        state: LoginState::SignedOut,
        email: None,
        plan: None,
        identity: None,
    };
    let path = account.home.join("auth.json");
    let metadata = match fs::metadata(&path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            if account.credentials == CredentialPolicy::Existing {
                summary.state = LoginState::ExistingStore;
            }
            return summary;
        }
        Err(_) => {
            summary.state = LoginState::Unreadable;
            return summary;
        }
    };
    if metadata.len() > 128 * 1024 {
        summary.state = LoginState::Unreadable;
        return summary;
    }
    let value: serde_json::Value = match fs::read(&path)
        .ok()
        .and_then(|v| serde_json::from_slice(&v).ok())
    {
        Some(v) => v,
        None => {
            summary.state = LoginState::Unreadable;
            return summary;
        }
    };
    if let Some(tokens) = value.get("tokens") {
        let has_token = ["access_token", "id_token"].iter().any(|k| {
            tokens
                .get(k)
                .and_then(|v| v.as_str())
                .is_some_and(|s| !s.is_empty())
        });
        if has_token {
            summary.state = LoginState::Saved;
            summary.identity = tokens
                .get("account_id")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            if let Some(payload) = tokens
                .get("id_token")
                .and_then(|v| v.as_str())
                .and_then(|s| s.split('.').nth(1))
                && let Some(claims) = URL_SAFE_NO_PAD
                    .decode(payload.trim_end_matches('='))
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            {
                summary.email = claims
                    .get("email")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .or_else(|| {
                        claims
                            .get("preferred_username")
                            .and_then(|v| v.as_str())
                            .filter(|s| !s.trim().is_empty())
                    })
                    .map(|s| {
                        s.trim()
                            .chars()
                            .filter(|c| {
                                !c.is_control()
                                    && !matches!(*c,'\u{202a}'..='\u{202e}'|'\u{2066}'..='\u{2069}')
                            })
                            .take(256)
                            .collect()
                    });
                summary.plan = claims
                    .get("https://api.openai.com/auth")
                    .and_then(|v| v.get("chatgpt_plan_type"))
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
            }
        }
    } else if account.credentials == CredentialPolicy::Existing
        && value
            .get("OPENAI_API_KEY")
            .and_then(|v| v.as_str())
            .is_some_and(|s| !s.is_empty())
    {
        summary.state = LoginState::ExistingStore;
    }
    summary
}
pub fn assert_distinct_identity(account: &Account, all: &[Account]) -> Result<()> {
    let Some(identity) = login_summary(account).identity else {
        return Ok(());
    };
    for other in all {
        if other.id != account.id
            && login_summary(other).identity.as_deref() == Some(identity.as_str())
        {
            bail!(
                "This entry has the same cached account/workspace identity as '{}'. Sign in with the intended account instead.",
                other.name
            );
        }
    }
    Ok(())
}
/// Fixed-length masking does not reveal the domain or original address length.
pub fn masked_identity(value: &str) -> String {
    let first = value
        .trim()
        .chars()
        .next()
        .filter(|c| c.is_alphanumeric())
        .unwrap_or('*');
    if value.contains('@') {
        format!("{first}***@***")
    } else {
        format!("{first}***")
    }
}
