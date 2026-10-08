# Contributing to Slotshift

Slotshift is a focused Windows-first account launcher, not a proxy or an embedded agent runtime. Keep changes small, understandable, and testable.

## Local development

Use stable Rust, Visual Studio C++ tools, and a Windows SDK. Run `cargo fmt --all`, `cargo test --no-default-features --locked`, and `cargo clippy --all-targets --locked -- -D warnings` before submitting a change. Build the GUI with `cargo build --locked`.

Use `slotshift --demo` for visual changes. Demo identities are fictional and model launches are disabled. Use `--data-dir <empty-absolute-folder>` for isolated functional tests. Never use real account homes in automated tests or publish screenshots with revealed login identities.

Run `powershell -NoProfile -File scripts/drive-slotshift.ps1 -Action smoke` to exercise the actual GPUI controls and rename workflow without real accounts. The [driving and debugging guide](docs/DRIVING_AND_DEBUGGING.md) documents inspect/click/set/wait/screenshot/diagnose. Live-process interaction is deliberately opt-in; do not automate real session launches, logins, or destructive actions without the user's approval.

## Boundaries to preserve

- Account removal only removes launcher metadata; it must not delete logins, history, worktrees, or active terminals.
- Keep launch options typed. Do not accept arbitrary shell snippets or mutate global environment variables.
- YOLO off must explicitly request workspace-write permissions and on-request approvals. A fresh account starts with YOLO off and worktrees off.
- Keep account identities masked unless the user explicitly reveals the selected identity. Never persist reveal state or put hidden emails in tooltips or accessibility labels.
- Do not print, commit, or copy credentials. Keep imported homes in place. A cached label is not live authentication or usage verification.

Include tests for changed behavior and a demo-mode screenshot for visible UI changes. See `docs/VALIDATION.md` for the current validation scope.