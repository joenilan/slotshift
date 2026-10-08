use crate::model::{
    Account, CredentialPolicy, LoginState, assert_distinct_identity, login_summary,
};
use anyhow::{Context, Result, bail, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::path::{Path, PathBuf};
#[cfg(windows)]
use std::process::Command;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    New,
    Resume,
    Login,
    DeviceLogin,
    Status,
}
impl Action {
    pub fn interactive(self) -> bool {
        matches!(self, Self::New | Self::Resume)
    }
    pub fn login(self) -> bool {
        matches!(self, Self::Login | Self::DeviceLogin)
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::New => "New session",
            Self::Resume => "Resume session",
            Self::Login => "Sign in",
            Self::DeviceLogin => "Device sign-in",
            Self::Status => "Login status",
        }
    }
}
pub const CLEARED_ENV: &[&str] = &[
    "OPENAI_API_KEY",
    "CODEX_API_KEY",
    "CODEX_ACCESS_TOKEN",
    "OPENAI_BASE_URL",
    "CODEX_THREAD_ID",
];
pub fn arguments(account: &Account, action: Action) -> Vec<String> {
    let mut args = Vec::new();
    if account.credentials == CredentialPolicy::IsolatedFile {
        args.extend(
            [
                "-c",
                "cli_auth_credentials_store=\"file\"",
                "-c",
                "forced_login_method=\"chatgpt\"",
            ]
            .map(str::to_string),
        );
    }
    if action.interactive() {
        args.push("--no-daemon".into());
        if action == Action::Resume {
            args.extend(["resume", "--all"].map(str::to_string));
        }
        if account.options.yolo {
            args.push("--yolo".into());
        } else {
            args.extend(
                [
                    "--sandbox",
                    "workspace-write",
                    "--ask-for-approval",
                    "on-request",
                ]
                .map(str::to_string),
            );
        }
        if account.options.live_search {
            args.push("--search".into());
        }
        if account.options.inline_terminal {
            args.push("--no-alt-screen".into());
        }
        if action == Action::New {
            args.extend(["-C".into(), account.project.clone()]);
            if account.options.worktree {
                args.push("--worktree".into());
            }
        }
    } else {
        args.push("login".into());
        match action {
            Action::DeviceLogin => args.push("--device-auth".into()),
            Action::Status => args.push("status".into()),
            _ => {}
        }
    }
    args
}
#[derive(Debug, Clone)]
pub struct LaunchPlan {
    pub action: Action,
    pub name: String,
    pub executable: PathBuf,
    pub home: PathBuf,
    pub working_directory: PathBuf,
    pub args: Vec<String>,
    pub yolo: bool,
}
impl LaunchPlan {
    pub fn prepare(
        account: &Account,
        action: Action,
        all: &[Account],
        executable: PathBuf,
    ) -> Result<Self> {
        ensure!(
            executable.is_file(),
            "Codex executable was not found. Select it in Settings."
        );
        ensure!(
            account.home.is_dir(),
            "This account's Codex home is missing. Import its existing folder or add a new account."
        );
        if action.login() && account.protect_login {
            bail!(
                "This entry uses your default Codex login, which may also be used by the desktop app. Manage its sign-in in Codex itself."
            );
        }
        if action.interactive() {
            assert_distinct_identity(account, all)?;
            ensure!(
                !matches!(
                    login_summary(account).state,
                    LoginState::SignedOut | LoginState::Unreadable
                ),
                "Sign in to this account first, then launch or resume a session."
            );
        }
        let working_directory = if action == Action::New {
            let path = PathBuf::from(account.project.trim());
            ensure!(
                path.is_absolute() && path.is_dir(),
                "Choose an existing, absolute project folder."
            );
            ensure!(
                !account.project.contains(['\n', '\r', '\0']),
                "The project path contains a control character."
            );
            if account.options.worktree {
                ensure!(
                    path.join(".git").exists(),
                    "Separate worktree mode needs a Git repository root. Choose its root or turn worktree mode off."
                );
            }
            path
        } else {
            account.home.clone()
        };
        Ok(Self {
            action,
            name: account.name.clone(),
            executable,
            home: account.home.clone(),
            working_directory,
            args: arguments(account, action),
            yolo: account.options.yolo && action.interactive(),
        })
    }
    /// Continue a copied thread as a new fork under the destination login.
    /// The original thread remains available in its original account home.
    pub fn prepare_fork(
        account: &Account,
        all: &[Account],
        executable: PathBuf,
        source_thread_id: &str,
        source_project: &Path,
    ) -> Result<Self> {
        uuid::Uuid::parse_str(source_thread_id).context("Invalid source session ID.")?;
        ensure!(
            source_project.is_absolute() && source_project.is_dir(),
            "The original project directory is missing."
        );
        let mut plan = Self::prepare(account, Action::Resume, all, executable)?;
        let command = plan
            .args
            .iter()
            .position(|arg| arg == "resume")
            .context("Codex resume arguments were not generated.")?;
        plan.args[command] = "fork".into();
        ensure!(
            plan.args.get(command + 1).is_some_and(|arg| arg == "--all"),
            "Unexpected session picker arguments."
        );
        plan.args[command + 1] = source_thread_id.into();
        plan.args
            .extend(["-C".into(), source_project.to_string_lossy().into_owned()]);
        plan.working_directory = source_project.to_path_buf();
        Ok(plan)
    }

    /// Environment selection occurs inside the new terminal, never in the UI or global environment.
    pub fn powershell(&self) -> String {
        let mut script = String::from(
            "$ErrorActionPreference='Stop'; $exitCode=0; $held=$false; $lock=$null;\n",
        );
        script.push_str(&format!(
            "$env:CODEX_HOME={};\n",
            ps_quote(&self.home.to_string_lossy())
        ));
        for key in CLEARED_ENV {
            script.push_str(&format!(
                "Remove-Item -LiteralPath 'Env:\\{}' -ErrorAction SilentlyContinue;\n",
                key
            ));
        }
        script.push_str(&format!(
            "$cli={}; $cliArgs=@({});\n",
            ps_quote(&self.executable.to_string_lossy()),
            self.args
                .iter()
                .map(|a| ps_quote(a))
                .collect::<Vec<_>>()
                .join(",")
        ));
        script.push_str("try {\n");
        script.push_str(&format!(
            "Set-Location -LiteralPath {};\n",
            ps_quote(&self.working_directory.to_string_lossy())
        ));
        script.push_str(&format!("$Host.UI.RawUI.WindowTitle={}; Write-Host {}; Write-Host ('Account home: '+$env:CODEX_HOME);\n",ps_quote(&format!("Slotshift - {}",self.name)),ps_quote(&format!("SLOTSHIFT / {}",self.name))));
        if self.action.login() {
            script.push_str("$lock=New-Object Threading.Mutex($false,'Local\\CodexAccountsBrowserLogin');\ntry {$held=$lock.WaitOne(0)} catch [Threading.AbandonedMutexException] {$held=$true};\nif(-not $held){throw 'Another account sign-in is open. Complete it first.'};\nWrite-Host 'Choose the intended account in your browser, not an already signed-in account.';\n");
        }
        if self.action.interactive() {
            script.push_str("$oldPreference=$ErrorActionPreference; $ErrorActionPreference='Continue';\ntry {$helpText=(& $cli --help 2>&1 | Out-String); $helpExit=$LASTEXITCODE} finally {$ErrorActionPreference=$oldPreference};\nif($helpExit -ne 0 -or $helpText -notmatch '--no-daemon'){throw 'This Codex version does not support --no-daemon. Update Codex before launching isolated sessions.'};\n");
            script.push_str(if self.yolo {"Write-Host 'YOLO ON: command approvals and sandbox restrictions are disabled.' -ForegroundColor Yellow;\n"} else {"Write-Host 'YOLO OFF: workspace-write sandbox, approvals on request.' -ForegroundColor Green;\n"});
        }
        // Native programs commonly write progress to stderr; do not turn that into a PowerShell terminating error.
        script.push_str("$ErrorActionPreference='Continue'; & $cli @cliArgs; $exitCode=$LASTEXITCODE; $ErrorActionPreference='Stop';\nif($exitCode -ne 0){Write-Host ('Codex exited with code '+$exitCode) -ForegroundColor Yellow};\n} catch {$exitCode=1; Write-Host $_.Exception.Message -ForegroundColor Red}\nfinally {if($held -and $lock){[void]$lock.ReleaseMutex()}; if($lock){$lock.Dispose()}};\n[void](Read-Host 'Press Enter to close this terminal'); exit $exitCode;\n");
        script
    }
    pub fn encoded_powershell(&self) -> String {
        STANDARD.encode(
            self.powershell()
                .encode_utf16()
                .flat_map(u16::to_le_bytes)
                .collect::<Vec<_>>(),
        )
    }
    pub fn spawn(&self) -> Result<()> {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let system = std::env::var_os("SystemRoot")
                .map(PathBuf::from)
                .context("SystemRoot is unavailable.")?;
            let pwsh = std::env::var_os("ProgramFiles")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join("PowerShell/7/pwsh.exe");
            let shell = if pwsh.is_file() {
                pwsh
            } else {
                system.join("System32/WindowsPowerShell/v1.0/powershell.exe")
            };
            ensure!(shell.is_file(), "PowerShell was not found.");
            let mut cmd = if let Some(wt) = find_in_path("wt.exe") {
                let mut c = Command::new(wt);
                c.args([
                    "-w",
                    "new",
                    "new-tab",
                    "--title",
                    &terminal_title(&self.name),
                    "--suppressApplicationTitle",
                ])
                .arg(&shell)
                .creation_flags(0x08000000);
                c
            } else {
                let mut c = Command::new(&shell);
                c.creation_flags(0x00000010);
                c
            };
            cmd.args([
                "-NoLogo",
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-EncodedCommand",
            ])
            .arg(self.encoded_powershell())
            .current_dir(&self.working_directory);
            cmd.spawn()
                .context("Windows could not open the account terminal.")?;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            bail!(
                "This alpha supports terminal launching on Windows. Other platforms are not implemented yet."
            )
        }
    }
}
pub fn ps_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}
pub fn terminal_title(name: &str) -> String {
    format!(
        "Slotshift - {}",
        name.chars()
            .take(64)
            .map(|c| if c.is_alphanumeric() || " _-".contains(c) {
                c
            } else {
                '-'
            })
            .collect::<String>()
    )
}
pub fn find_in_path(name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|p| {
        std::env::split_paths(&p)
            .filter(|d| d.is_absolute())
            .map(|d| d.join(name))
            .find(|p| p.is_file())
    })
}
pub fn locate_codex(custom: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = custom {
        ensure!(
            path.is_absolute() && path.is_file(),
            "The selected Codex executable is missing."
        );
        #[cfg(windows)]
        ensure!(
            path.extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("exe")),
            "Select codex.exe, not a command script."
        );
        return Ok(path.to_path_buf());
    }
    #[cfg(windows)]
    {
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let path = PathBuf::from(local).join("Programs/OpenAI/Codex/bin/codex.exe");
            if path.is_file() {
                return Ok(path);
            }
        }
        if let Some(path) = find_in_path("codex.exe") {
            return Ok(path);
        }
        let paths = std::env::var_os("PATH").unwrap_or_default();
        let platform = if cfg!(target_arch = "aarch64") {
            "codex-win32-arm64"
        } else {
            "codex-win32-x64"
        };
        let triple = if cfg!(target_arch = "aarch64") {
            "aarch64-pc-windows-msvc"
        } else {
            "x86_64-pc-windows-msvc"
        };
        for dir in std::env::split_paths(&paths).filter(|d| d.is_absolute()) {
            for package in [
                dir.join("node_modules/@openai").join(platform),
                dir.join("node_modules/@openai/codex/node_modules/@openai")
                    .join(platform),
                dir.join("node_modules/@openai/codex"),
            ] {
                for subdir in ["bin", "codex"] {
                    let path = package
                        .join("vendor")
                        .join(triple)
                        .join(subdir)
                        .join("codex.exe");
                    if path.is_file() {
                        return Ok(path);
                    }
                }
            }
        }
    }
    #[cfg(not(windows))]
    if let Some(path) = find_in_path("codex") {
        return Ok(path);
    }
    bail!("Codex CLI was not found. Install the standalone CLI, or choose codex.exe in Settings.")
}
pub fn preview(account: &Account, action: Action) -> String {
    let args = arguments(account, action);
    let skip = if account.credentials == CredentialPolicy::IsolatedFile {
        4
    } else {
        0
    };
    format!(
        "codex {}",
        args.into_iter()
            .skip(skip)
            .map(|s| {
                if s.chars()
                    .any(|c| c.is_whitespace() || ['\'', '\"', '$', ';', '&'].contains(&c))
                {
                    ps_quote(&s)
                } else {
                    s
                }
            })
            .collect::<Vec<_>>()
            .join(" ")
    )
}
