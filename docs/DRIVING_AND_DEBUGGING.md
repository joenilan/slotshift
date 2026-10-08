# Driving and debugging Slotshift

Slotshift is native GPUI software. Windows UI Automation exposes controls and input fields. **scripts/drive-slotshift.ps1** gives local AI agents and developers a repeatable way to inspect, click, type, wait, capture screenshots, and check responsiveness.

**No always-on control service or network listener is installed.** The driver runs only when invoked and never reads Codex credentials or session history.

## Safe quick start

From the source checkout:

```powershell
.\scripts\drive-slotshift.ps1 -Action smoke
.\scripts\drive-slotshift.ps1 -Action smoke -Output "E:\git\slotshift\ui-smoke.local.png"
.\scripts\drive-slotshift.ps1 -Action start-demo
```

Smoke launches a disposable fictional --demo instance, adds an account, renames it, renames another non-selected account, tests canceling removal, then closes only the demo it started. No real login or model request is involved.

Start-demo leaves a demo open and prints its PID. Use that PID to drive a particular window:

```powershell
$demoPid = 12345   # Replace with the printed Demo PID
.\scripts\drive-slotshift.ps1 -Action inspect -ProcessId $demoPid
.\scripts\drive-slotshift.ps1 -Action click -ProcessId $demoPid -Control 'Rename Workbench'
.\scripts\drive-slotshift.ps1 -Action set -ProcessId $demoPid -Control 'e.g. Personal, Work, Side projects' -Value 'Work Test'
.\scripts\drive-slotshift.ps1 -Action click -ProcessId $demoPid -Control 'Save name'
.\scripts\drive-slotshift.ps1 -Action wait -ProcessId $demoPid -Control 'Rename Work Test' -TimeoutSeconds 10
.\scripts\drive-slotshift.ps1 -Action diagnose -ProcessId $demoPid
.\scripts\drive-slotshift.ps1 -Action screenshot -ProcessId $demoPid -Output "E:\git\slotshift\ui-debug.local.png"
```

Control names match Windows accessibility names. For duplicated labels, use **-Index -1** for the last match. Bounded **wait** replaces guessed delays; use **-Absent** to wait for a control to disappear.

## Inspect the live app

```powershell
.\scripts\drive-slotshift.ps1 -Action diagnose -ProcessId 12345 -AllowLive
.\scripts\drive-slotshift.ps1 -Action inspect -ProcessId 12345 -AllowLive
```

Replace 12345 with the Slotshift PID from Task Manager. Without **-AllowLive**, the driver refuses live app access. Live clicks that can change accounts or launch sessions additionally require **-AllowSensitiveActions**. Account confirmation dialogs still apply.

The driver reports button and input accessibility names, enabled states, process memory, and responsiveness. It can click buttons, set text fields, wait for states, capture a window image, and test the GUI. It does not open sockets, proxy prompts, alter global shell settings, or bypass local permissions.

**Privacy:** console inspection masks recognizable emails in UI labels, but does not anonymize arbitrary custom nicknames. Screenshots can contain project paths or revealed emails; only share redacted captures. Never include credential files or session history in bug reports.

The Windows release ZIP also includes drive-slotshift.ps1 beside the executable, so you can use it without a source checkout. For code-level problems, use the Rust test suite, Clippy and normal Windows diagnostics alongside this GUI driver.
