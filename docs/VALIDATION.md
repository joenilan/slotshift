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

## Not claimed by this alpha

A live model response, browser reauthorization, organization-managed policy behavior, long-duration stability, and a broad GPU/driver matrix were not tested in this pass. Existing provider logins are linked in place, but cached identity labels are not live validation. macOS/Linux terminal launching and code signing are not implemented.

The GitHub Windows workflow builds and checks the source independently; consult the actual workflow status rather than treating these local results as a CI result.

## Reproduction

Run the commands in the README for core, formatting, lint, and build checks. `--demo` uses fictional identities and disables real launches. Use a separate `--data-dir` and the `tests/fixtures/mock_codex.rs` executable when testing terminal actions. That fixture records only launch arguments, account-home/current-directory paths, and a boolean indicating whether API environment entries are absent; it does not contact a service or execute project commands.

Never run automated tests against real credential files. Do not publish local test logs or revealed identity screenshots.