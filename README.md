<p align="center"><img src="assets/slotshift.svg" width="76" alt="Slotshift icon"></p>
<h1 align="center">Slotshift</h1>
<p align="center"><strong>Accounts, side by side.</strong><br>A small native launcher for your Codex CLI accounts.</p>
<p align="center"><a href="https://github.com/joenilan/slotshift/releases">Download for Windows</a> · <a href="#getting-started">Getting started</a> · <a href="#build-from-source">Build from source</a></p>

![Slotshift's GPU-rendered interface, showing fictional demo accounts](docs/screenshot.png)

Keep one account working while you start another. Slotshift opens the official Codex CLI in separate terminal windows, with a dedicated Codex home for each account and launch settings you can actually see.

**Windows-first alpha.** Built in Rust with [GPUI Kit](https://gpui-kit.com/). No Electron, embedded browser, API proxy, automatic account rotation, or combined quota pool. Independent software by **zombie.digital**, not affiliated with or endorsed by OpenAI.

## What it does

- **Your account list, not three fixed slots.** Add accounts, rename them, search them, or link existing Codex homes. Removing an entry leaves its credentials, history, and running sessions alone.
- **New and resumed sessions.** Work directly in the project folder you choose. Resume opens the selected account's saved-session picker across projects.
- **Saved options per account.** Toggle YOLO, separate worktrees, live web search, and inline terminal scrollback. The command preview shows what a new session will receive.
- **Local-first account separation.** Separate homes and file credentials for managed accounts. Interactive sessions use `--no-daemon`; unsupported CLI versions stop rather than silently connecting to another account's shared server.
- **A native interface.** GPU rendering, a resizable window, keyboard-accessible controls, system folder pickers, graphite surfaces, and a restrained mint accent.

## Getting started

Download the Windows x64 ZIP from [Releases](https://github.com/joenilan/slotshift/releases), extract it, and open `slotshift.exe`. The alpha executable is **unsigned**; Windows may display a reputation warning. Verify the published SHA-256 checksum before running downloaded software. Do not disable Windows security protections.

You also need the official **Codex CLI**, installed and available on your PC. Slotshift prefers its standalone Windows executable and can find supported npm package layouts. It does not install Codex or update the desktop app. Use **Settings → Choose executable** when automatic detection does not find your standalone `codex.exe`.

1. Click **Add account**, give it a label, and create a new private account home. Alternatively, enable **Link an existing Codex home** to reuse a folder that already contains `config.toml` or `auth.json`.
2. Click **Sign in**. Complete Codex's official browser authorization using the intended account. Finish one sign-in before starting another. **Device sign-in** in Settings is a fallback when supported by your account.
3. Choose the project folder, set your options, then **Launch session**. Select another account and repeat. Closing Slotshift does not close the terminals it opened.

An existing default `~/.codex` home is discovered automatically. It may also be used by another Codex app, so Slotshift deliberately protects its sign-in from replacement. Manage that default login in Codex itself.

The earlier `codex-accounts` PowerShell launcher's homes are linked **in place** on first startup. No authentication files, session histories, or imported account configs are copied or moved. Its selected account and project are retained, and its previous YOLO default is preserved. The old launcher can remain installed as a fallback.

## Launch options

| Option | On | Off |
|---|---|---|
| YOLO | Adds `--yolo`; skips command approvals and sandboxing. | Explicitly uses `--sandbox workspace-write --ask-for-approval on-request`. |
| Separate worktree | Adds `--worktree` for a **new** session. | Uses your selected folder and its current branch. |
| Live web search | Adds `--search`. | Does not add the live-search override; Codex's own configuration still applies. |
| Keep terminal scrollback | Adds `--no-alt-screen`. | Does not override the default terminal screen behavior. |

Fresh accounts start with **YOLO off** and **worktrees off**. Options affect the next launch or resume, not already-running sessions. Project trust is separate from command approvals, so a trusted-folder prompt can still appear with YOLO enabled. Resume retains the session's original project; it does not move old worktree sessions into another folder.

Agents working in the same project folder share editable files. Slotshift does not switch branches, merge their changes, or coordinate concurrent Git operations.

## Accounts and privacy

**Streaming-friendly identity labels.** Login emails/usernames are masked by default (for example, `p***@***`), separately from your custom account nickname. Click **Reveal** to show only the selected account's identity for 15 seconds; click **Hide** to conceal it sooner. The sidebar stays masked, changing accounts hides the identity, and reveal state never survives an app restart. Hidden addresses are not placed in tooltips or accessibility labels. Use non-sensitive nicknames: this protects login identity labels, not arbitrary project paths, custom names, or Codex's own terminal output.

Settings live in `%LOCALAPPDATA%\Slotshift\settings.json`. New account homes live under `%LOCALAPPDATA%\Slotshift\accounts\<random-id>`. Linked homes remain where they are. The owned data folder is restricted to the current Windows user, SYSTEM, and administrators.

Credentials are stored by Codex, not in a Slotshift password database. For managed accounts, Codex uses each home's **plain-text `auth.json`**. Treat these files as passwords. Cached email, plan, and account identity claims are read locally to label accounts and detect duplicate saved identities; they are **not live subscription or usage verification**. Use `/status` inside Codex for current information.

Removing an entry is **not logout or credential deletion**. Its files remain available for re-import. Account homes and worktrees are organizational boundaries, **not security sandboxes**. In particular, a YOLO session can access other files allowed by your Windows user. The default home can intentionally be shared with other Codex apps.

Slotshift sets account variables only in the new terminal and clears inherited API-key overrides there. It does not modify global environment variables, PowerShell profiles, the system PATH, or imported account configuration. Slotshift sends no telemetry and does not proxy prompts. Codex and your provider retain their own network behavior, terms, and limits.

## Build from source

Use a current stable Rust toolchain (the alpha was built with **Rust 1.99**), Visual Studio C++ build tools, and a Windows SDK. GPUI Kit is pinned to **0.7.1** with its matching GPUI snapshot; `Cargo.lock` is committed.

```powershell
cargo +stable build --release --locked
.\target\release\slotshift.exe
```

```powershell
cargo +stable fmt --all -- --check
cargo +stable test --no-default-features --locked
cargo +stable clippy --all-targets --locked -- -D warnings
```

`--demo` opens a temporary workspace with fictional account labels and **disables real Codex launches**. It is useful for screenshots. `--data-dir <absolute-folder>` uses a separate settings directory and disables automatic discovery of your existing accounts, which is useful for isolated testing.

Core tests cover dynamic account lists, private storage, atomic persistence, legacy migration, credential-safe removal, duplicate identities, command construction, YOLO on/off behavior, Unicode, and PowerShell escaping. The mock CLI in `tests/fixtures` never calls a model or provider. See [validation](docs/VALIDATION.md) for the alpha's actual test scope.

## Scope

This is a launcher, not an embedded terminal, proxy server, remote session host, or quota dashboard. It does not transfer a running conversation between accounts or preserve a process through a PC restart. Resume restores a saved Codex conversation; it is not a running-process checkpoint.

Windows x64 is the tested release target. macOS/Linux terminal adapters and signing are not part of this alpha. The GPU framework does not make Codex's model generation faster.

MIT-licensed application source. See [third-party notices](THIRD_PARTY_NOTICES.md), [security](SECURITY.md), and [contributing](CONTRIBUTING.md).