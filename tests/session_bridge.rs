use serde_json::json;
use slotshift::{
    model::Account,
    session_bridge::{import_into, list_sessions},
};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tempfile::tempdir;
use uuid::Uuid;

fn account(label: &str, home: &Path) -> Account {
    fs::create_dir_all(home).unwrap();
    Account::linked(label, home.to_path_buf(), false).unwrap()
}
fn fixture(home: &Path, project: &Path, id: &str, parent: Option<&str>) -> PathBuf {
    let dir = home.join("sessions/2026/10/08");
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!("rollout-2026-10-08T07-00-00-{id}.jsonl"));
    let meta = json!({
        "ordinal":0,"timestamp":"2026-10-08T07:00:00Z","type":"session_meta",
        "payload":{
            "id":id,"session_id":id,"timestamp":"2026-10-08T07:00:00Z",
            "cwd":project.to_string_lossy(),"source":"cli","originator":"codex",
            "cli_version":"0.161.0","history_mode":"paginated",
            "model_provider":"openai",
            "history_base":parent.map(|thread_id|json!({"thread_id":thread_id,"end_ordinal_exclusive":2,"end_byte_offset":123}))
        }
    });
    let user = json!({"ordinal":1,"timestamp":"2026-10-08T07:00:01Z","type":"response_item",
        "payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"synthetic test; no model requests"}]}});
    fs::write(&file, format!("{meta}\n{user}\n")).unwrap();
    file
}
fn setup() -> (tempfile::TempDir, Account, Account, PathBuf) {
    let temp = tempdir().unwrap();
    let from = account("Pro", &temp.path().join("pro"));
    let to = account("Plus", &temp.path().join("plus"));
    let project = temp.path().join("project");
    fs::create_dir(&project).unwrap();
    (temp, from, to, project)
}
#[test]
fn discovers_sessions_from_all_accounts_without_login_access() {
    let (_tmp, from, to, project) = setup();
    let id = Uuid::new_v4().to_string();
    fixture(&from.home, &project, &id, None);
    let sessions = list_sessions(&[from.clone(), to]).unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].account_id, from.id);
    assert_eq!(sessions[0].id, id);
    assert_eq!(sessions[0].cwd, project);
}
#[test]
fn imports_snapshot_with_no_changes_to_login_or_source() {
    let (_tmp, from, to, project) = setup();
    let id = Uuid::new_v4().to_string();
    let src = fixture(&from.home, &project, &id, None);
    fs::write(to.home.join("auth.json"), "protected target fixture").unwrap();
    fs::write(to.home.join("config.toml"), "protected settings fixture").unwrap();
    let before = fs::read(&src).unwrap();
    let session = list_sessions(std::slice::from_ref(&from))
        .unwrap()
        .remove(0);
    let result = import_into(&session, &from, &to).unwrap();
    assert_eq!(result.files_added, 1);
    assert_eq!(result.session_id, id);
    assert_eq!(result.project, project);
    let target = to
        .home
        .join("sessions/2026/10/08")
        .join(src.file_name().unwrap());
    assert_eq!(fs::read(target).unwrap(), before);
    assert_eq!(fs::read(src).unwrap(), before);
    assert_eq!(
        fs::read_to_string(to.home.join("auth.json")).unwrap(),
        "protected target fixture"
    );
    assert_eq!(
        fs::read_to_string(to.home.join("config.toml")).unwrap(),
        "protected settings fixture"
    );
    let again = import_into(&session, &from, &to).unwrap();
    assert_eq!(again.files_added, 0);
}
#[test]
fn ancestry_is_imported_without_losing_pagination_records() {
    let (_tmp, from, to, project) = setup();
    let parent = Uuid::new_v4().to_string();
    let child = Uuid::new_v4().to_string();
    let original = fixture(&from.home, &project, &parent, None);
    let current = fixture(&from.home, &project, &child, Some(&parent));
    let session = list_sessions(std::slice::from_ref(&from))
        .unwrap()
        .into_iter()
        .find(|s| s.id == child)
        .unwrap();
    let result = import_into(&session, &from, &to).unwrap();
    assert_eq!(result.files_added, 2);
    for file in [original, current] {
        let dst = to
            .home
            .join("sessions/2026/10/08")
            .join(file.file_name().unwrap());
        assert_eq!(fs::read(dst).unwrap(), fs::read(file).unwrap());
    }
}
#[test]
fn existing_different_history_is_never_overwritten() {
    let (_tmp, from, to, project) = setup();
    let id = Uuid::new_v4().to_string();
    let original = fixture(&from.home, &project, &id, None);
    let destination = fixture(&to.home, &project, &id, None);
    fs::write(&destination, "a different, important thread\n").unwrap();
    let session = list_sessions(std::slice::from_ref(&from))
        .unwrap()
        .remove(0);
    assert!(import_into(&session, &from, &to).is_err());
    assert_eq!(
        fs::read_to_string(destination).unwrap(),
        "a different, important thread\n"
    );
    assert!(original.exists());
}
#[test]
fn unfinished_line_is_rejected_but_missing_old_worktree_can_be_retargeted() {
    let (_tmp, from, to, project) = setup();
    let id = Uuid::new_v4().to_string();
    let path = fixture(&from.home, &project, &id, None);
    let session = list_sessions(std::slice::from_ref(&from))
        .unwrap()
        .remove(0);
    fs::write(&path, "{\"type\":\"session_meta\",\"payload\":").unwrap();
    assert!(import_into(&session, &from, &to).is_err());
    fs::remove_file(path).unwrap();
    let new_id = Uuid::new_v4().to_string();
    fixture(&from.home, &project, &new_id, None);
    let session = list_sessions(std::slice::from_ref(&from))
        .unwrap()
        .remove(0);
    fs::remove_dir(&project).unwrap();
    assert_eq!(import_into(&session, &from, &to).unwrap().files_added, 1);
    let exe = to.home.join("codex.exe");
    fs::write(&exe, "").unwrap();
    assert!(
        slotshift::launch::LaunchPlan::prepare_fork(
            &to,
            &[from, to.clone()],
            exe,
            &session.id,
            &project
        )
        .is_err()
    );
}
#[test]
fn cannot_import_back_into_the_same_account_home() {
    let (_tmp, from, _to, project) = setup();
    fixture(&from.home, &project, &Uuid::new_v4().to_string(), None);
    let session = list_sessions(std::slice::from_ref(&from))
        .unwrap()
        .remove(0);
    assert!(import_into(&session, &from, &from).is_err());
}
#[test]
fn uses_read_only_sqlite_titles_and_masks_email_addresses() {
    let (_tmp, from, _to, project) = setup();
    let id = Uuid::new_v4().to_string();
    fixture(&from.home, &project, &id, None);
    let db_path = from.home.join("state_5.sqlite");
    let conn = rusqlite::Connection::open(&db_path).unwrap();
    conn.execute_batch("CREATE TABLE threads (id TEXT, name TEXT, title TEXT, archived INTEGER);")
        .unwrap();
    conn.execute(
        "INSERT INTO threads VALUES (?1, ?2, ?3, 0)",
        rusqlite::params![id, "", "Continue test for alice@example.com repo"],
    )
    .unwrap();
    drop(conn);
    let before = fs::read(&db_path).unwrap();
    let found = list_sessions(std::slice::from_ref(&from)).unwrap();
    assert_eq!(found.len(), 1);
    assert!(found[0].title.contains("[email hidden]"));
    assert!(!found[0].title.contains("alice@example.com"));
    assert_eq!(fs::read(db_path).unwrap(), before);
}
#[test]
fn falls_back_to_jsonl_titles_without_sqlite() {
    let (_tmp, from, _to, project) = setup();
    let id = Uuid::new_v4().to_string();
    fixture(&from.home, &project, &id, None);
    fs::write(
        from.home.join("session_index.jsonl"),
        format!(
            "{}\n",
            json!({"id":id,"thread_name":"Fix cross-account login"})
        ),
    )
    .unwrap();
    let found = list_sessions(std::slice::from_ref(&from)).unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].title, "Fix cross-account login");
}
#[test]
fn fork_plan_uses_destination_login_and_source_project() {
    let (_tmp, from, mut to, project) = setup();
    let id = Uuid::new_v4().to_string();
    fixture(&from.home, &project, &id, None);
    fs::write(
        to.home.join("auth.json"),
        r#"{"tokens":{"access_token":"synthetic-not-real"}}"#,
    )
    .unwrap();
    to.options.yolo = true;
    to.options.worktree = true;
    let exe = to.home.join("codex.exe");
    fs::write(&exe, "").unwrap();
    let plan =
        slotshift::launch::LaunchPlan::prepare_fork(&to, &[from, to.clone()], exe, &id, &project)
            .unwrap();
    assert_eq!(plan.home, to.home);
    assert_eq!(plan.working_directory, project);
    assert!(plan.args.iter().any(|arg| arg == "fork"));
    assert!(plan.args.iter().any(|arg| arg == &id));
    assert!(plan.args.iter().any(|arg| arg == "--yolo"));
    assert!(
        plan.args
            .iter()
            .any(|arg| arg == "approval_policy=\"never\"")
    );
    assert!(
        plan.args
            .iter()
            .any(|arg| arg == "sandbox_mode=\"danger-full-access\"")
    );
    assert!(!plan.args.iter().any(|arg| arg == "--worktree"));
}
