# Security and privacy

Slotshift is an alpha launcher. It is not a credential vault or a security boundary between agents running as the same Windows user.

Managed accounts use the official CLI's file credential storage. The account home's `auth.json` contains sensitive plain-text tokens. Protect it like a password. The owned data folder is restricted to the current Windows user, SYSTEM, and administrators. Removing an account entry does not revoke or delete its tokens.

A YOLO session can read and modify files allowed by the current Windows user, including files outside the selected project. Do not enable it for untrusted work. Folder trust, operating-system permissions, and provider controls are separate from the launch toggle.

Login identity text is masked by default. Reveal is temporary and limited to the selected account's header/settings view; the sidebar stays masked. This is display privacy, not encryption. Custom nicknames, folder paths, and the separate Codex terminal output are not automatically redacted. Avoid sensitive account nicknames when streaming.

Do not attach credentials, account databases, full session histories, or unredacted screenshots to issues. For a suspected vulnerability, use GitHub's private vulnerability reporting when available; otherwise open a minimal issue asking for a private reporting channel without publishing exploit details or sensitive data.

The optional local UI Automation driver (`scripts/drive-slotshift.ps1`) runs only when invoked. It opens no network ports, has no unattended background listener, defaults to disposable demo instances, and rejects live access without `-AllowLive`. Actions that could alter accounts or launch sessions need `-AllowSensitiveActions` too. UI automation has the same Windows desktop permissions as its caller; treat scripts and access permissions accordingly. Live screenshots might expose user-customized labels, project folders, or voluntarily revealed identities.

The alpha binary is unsigned. Check the release checksum. Do not disable Windows security protections to run it.