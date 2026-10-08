# Alpha validation

Validated on Windows 11 x64 on 2026-10-07, using Rust 1.99, GPUI Kit 0.7.1, and the installed Codex CLI 0.160.0.

## Completed

- **46 automated core tests passed.** They cover dynamic lists (including 1,000 accounts), migration, atomic persistence/reopening, duplicate identities and homes, safe removal/re-import, argument combinations, UTF-16 transport, private storage, masked identities, and rejection of unrelated data directories.
- **Formatting and Clippy passed**, including `cargo clippy --all-targets --locked -- -D warnings`.
- **Native debug and optimized Windows builds succeeded.** The release window was rendered and inspected, including its masked-identity screenshot.
- **Actual GUI controls were exercised through Windows accessibility automation:** add, rename, remove, search/clear, reveal, hide, account-switch concealment, and the reveal timeout.
- **New and Resume buttons opened real Windows Terminal windows with a no-network mock CLI.** Fifteen checks passed for selected account homes, the exact project path (including spaces, an apostrophe and an ampersand), YOLO on/off, explicit safe permissions, daemon bypass, resume selection, worktree behavior, and inherited API environment removal.
- **The installed official Codex CLI accepted all four New/Resume and YOLO-on/off argument combinations in help-only checks.** No project was edited and no model request was sent.

The native-terminal test caught a null-versus-empty environment handling edge case. The wrapper now removes inherited API environment entries explicitly with PowerShell's process-local environment provider; the repeated terminal tests confirmed they were absent in the child CLI.

## Cross-account continuation validation (2026-10-08)

- **55 automated tests passed** (46 existing core tests and 9 new handoff tests). Tests cover read-only SQLite titles, indexed-title fallback, masked email display, ancestry copying, duplicate destination refusal, missing worktrees, unfinished history, source preservation, and target-account launch argument construction.
- Rust formatting and Clippy `--all-targets --locked -- -D warnings` passed.
- **No-network Codex CLI 0.161.0 protocol check:** a synthetic session placed into a separate temporary account home was visible via `thread/list`, readable via `thread/read`, and forked via `thread/fork` with a new ID. Another test confirmed forking a synthetic child session referencing a paginated parent; no model calls or account tokens were supplied.
- Actual GPU-rendered GPUI demo displayed the **Continue across accounts** dialog with five fictional sessions, searchable list, target-account choices, selectable source session and working-folder override. No real account history was opened in the demo.
- Imported session JSONL history is snapshotted with a bounded size, checked before publishing, and linked without replacing existing files. Private account settings, tokens, and the SQLite indexes are neither transferred nor modified.

Live continuation with real model inference and account-specific service entitlements has **not** been verified. A saved-history fork is not the same as reattaching to an actively running process. Wait until the original session's current turn has finished before continuing.

## Native rename controls and on-demand UI driving (2026-10-08)

- **56 Rust tests passed:** 47 core tests and 9 cross-account transfer tests. The rename test confirms a newly added account's display name can change without modifying its identifier, login, history, preferences, or the selected account.
- The GPU-rendered GPUI application was driven through Windows UI Automation to add an account, rename it using the pencil icon, rename an account that was not selected, and open/cancel removal. The isolated demo smoke test passed.
- The opt-in local driver supports inspect, click, set, wait, screenshot, diagnose, and a demo smoke test. Read-only live diagnostics confirmed the original installation was responsive without changing account data. Live access requires `-AllowLive`; higher-impact clicks also require `-AllowSensitiveActions`.
- Fictional screenshots demonstrate both pencil/trash icons and the rename dialog. The capture code temporarily foregrounds the targeted window and restores normal stacking to prevent partially occluded screenshots.
- Rust formatting, tests, and strict Clippy checks passed. No real account logins or Codex model requests were made in these UI tests.

## YOLO launch regression investigation (2026-10-08)

- Inspected real Slotshift-launched Windows Codex CLI 0.161.0 processes: `resume` and `fork` both received the actual `--yolo` flag. Slotshift's account settings also had YOLO enabled, so the flag was not being dropped by the launcher.
- Read only permission metadata (no conversation content) from local Codex rollouts. Some managed sessions recorded `approval_policy=on-request` and `sandbox_policy.type=workspace-write`, despite earlier YOLO turns recording `never` and `danger-full-access`.
- Existing managed account `config.toml` files default to `on-request`/`workspace-write`. A disposable `CODEX_HOME` with Codex's own app-server `config/read` confirmed that process-only `-c approval_policy=\"never\" -c sandbox_mode=\"danger-full-access\"` overrides changed effective defaults to `never`/`danger-full-access` without touching user credentials or live sessions.
- The installed CLI accepted the amended resume/fork flag combinations in help-only checks. Rust tests assert YOLO and both explicit config overrides on new/resume/fork, while login/status never inherit execution permissions.
- Codex issue https://github.com/openai/codex/issues/49088 documents the reconnect regression addressed by this defensive override. Windows UAC and plugin/MCP approvals are separate; they are not bypassed by YOLO and are not tested as part of this mitigation.

## Not claimed by this alpha

A live model response, browser reauthorization, organization-managed policy behavior, long-duration stability, and a broad GPU/driver matrix were not tested in this pass. Existing provider logins are linked in place, but cached identity labels are not live validation. macOS/Linux terminal launching and code signing are not implemented.

The GitHub Windows workflow builds and checks the source independently; consult the actual workflow status rather than treating these local results as a CI result.

## Reproduction

Run the commands in the README for core, formatting, lint, and build checks. `--demo` uses fictional identities and disables real launches. Use a separate `--data-dir` and the `tests/fixtures/mock_codex.rs` executable when testing terminal actions. That fixture records only launch arguments, account-home/current-directory paths, and a boolean indicating whether API environment entries are absent; it does not contact a service or execute project commands.

Never run automated tests against real credential files. Do not publish local test logs or revealed identity screenshots.