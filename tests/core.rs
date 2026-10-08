use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use slotshift::{
    launch::{self, Action, LaunchPlan},
    model::*,
    storage::{Store, discover_existing},
};
use std::{fs, path::Path};
use tempfile::{TempDir, tempdir};
fn account() -> (TempDir, Account) {
    let t = tempdir().unwrap();
    let home = t.path().join("account");
    fs::create_dir(&home).unwrap();
    let a = Account::linked("Personal", home, false).unwrap();
    (t, a)
}
fn fake_login(a: &Account, id: &str) {
    fs::write(
        a.home.join("auth.json"),
        serde_json::json!({"tokens":{"account_id":id,"access_token":"fixture-not-a-real-token"}})
            .to_string(),
    )
    .unwrap();
}
fn store() -> (TempDir, Store, Settings) {
    let t = tempdir().unwrap();
    let (store, settings) = Store::open(t.path().join("app"), None).unwrap();
    (t, store, settings)
}
fn settings(a: Account) -> Settings {
    Settings {
        selected: Some(a.id.clone()),
        accounts: vec![a],
        ..Settings::default()
    }
}
fn plan(a: &Account, action: Action) -> LaunchPlan {
    LaunchPlan {
        action,
        name: a.name.clone(),
        executable: a.home.join("codex.exe"),
        home: a.home.clone(),
        working_directory: a.home.clone(),
        args: launch::arguments(a, action),
        yolo: a.options.yolo && action.interactive(),
    }
}
#[test]
fn fresh_install_is_not_yolo() {
    let o = LaunchOptions::default();
    assert!(!o.yolo && !o.worktree && !o.live_search && !o.inline_terminal);
}
#[test]
fn no_fixed_account_count() {
    let t = tempdir().unwrap();
    let mut s = Settings::default();
    for i in 0..1000 {
        s.accounts.push(
            Account::linked(
                &format!("Account {i}"),
                t.path().join(format!("a{i}")),
                false,
            )
            .unwrap(),
        );
    }
    s.validate().unwrap();
    assert_eq!(s.accounts.len(), 1000);
}
#[test]
fn names_trim_but_never_become_paths() {
    assert_eq!(validate_name(" Work / team ").unwrap(), "Work / team");
    assert!(validate_name("\n").is_err());
    assert!(validate_name(&"x".repeat(65)).is_err());
    assert!(validate_name("x\0y").is_err());
}
#[test]
fn duplicate_homes_rejected() {
    let (_t, a) = account();
    let mut b = a.clone();
    b.id = "other".into();
    let s = Settings {
        accounts: vec![a, b],
        ..Settings::default()
    };
    assert!(s.validate().is_err());
}
#[test]
fn duplicate_ids_rejected() {
    let (t, a) = account();
    let mut b = a.clone();
    b.home = t.path().join("other");
    let s = Settings {
        accounts: vec![a, b],
        ..Settings::default()
    };
    assert!(s.validate().is_err());
}
#[test]
fn relative_account_home_rejected() {
    let (_t, mut a) = account();
    a.home = "relative/home".into();
    assert!(settings(a).validate().is_err());
}
#[test]
fn bad_selected_id_rejected() {
    let s = Settings {
        selected: Some("absent".into()),
        ..Default::default()
    };
    assert!(s.validate().is_err());
}
#[test]
fn old_settings_link_all_homes_and_preserve_originals() {
    let t = tempdir().unwrap();
    let main = t.path().join(".codex");
    fs::create_dir(&main).unwrap();
    fs::write(main.join("auth.json"), "unchanged main fixture").unwrap();
    let legacy = t.path().join(".codex-accounts");
    for n in ["plus1", "plus2", "work", "plus15"] {
        fs::create_dir_all(legacy.join(n)).unwrap();
        fs::write(legacy.join(n).join("config.toml"), "original config").unwrap();
    }
    fs::write(
        legacy.join("launcher-state.json"),
        "\u{feff}{\"account\":\"plus2\",\"project\":\"E:\\\\Projects\\\\demo\"}",
    )
    .unwrap();
    let s = discover_existing(t.path()).unwrap();
    assert_eq!(s.accounts.len(), 5);
    assert!(s.migrated_legacy);
    assert_eq!(s.current().unwrap().name, "Plus 2");
    assert!(
        s.accounts
            .iter()
            .all(|a| a.options.yolo && !a.options.worktree)
    );
    assert!(s.accounts.iter().any(|a| a.protect_login));
    assert_eq!(
        fs::read_to_string(main.join("auth.json")).unwrap(),
        "unchanged main fixture"
    );
    assert_eq!(
        fs::read_to_string(legacy.join("plus2/config.toml")).unwrap(),
        "original config"
    );
}
#[test]
fn legacy_explicit_yolo_off_is_respected() {
    let t = tempdir().unwrap();
    fs::create_dir(t.path().join(".codex")).unwrap();
    fs::create_dir(t.path().join(".codex-accounts")).unwrap();
    fs::write(
        t.path().join(".codex-accounts/launcher-state.json"),
        r#"{"account":"main","yolo":false}"#,
    )
    .unwrap();
    assert!(
        !discover_existing(t.path())
            .unwrap()
            .current()
            .unwrap()
            .options
            .yolo
    );
}
#[test]
fn normal_codex_import_does_not_enable_yolo() {
    let t = tempdir().unwrap();
    fs::create_dir(t.path().join(".codex")).unwrap();
    let s = discover_existing(t.path()).unwrap();
    assert!(!s.current().unwrap().options.yolo);
    assert!(!s.migrated_legacy);
}
#[test]
fn removal_preserves_login_history_and_directory() {
    let (_t, a) = account();
    fake_login(&a, "fixture-a");
    fs::write(a.home.join("history.jsonl"), "saved work").unwrap();
    let mut s = settings(a.clone());
    s.remove(&a.id).unwrap();
    assert!(a.home.join("auth.json").exists());
    assert_eq!(
        fs::read_to_string(a.home.join("history.jsonl")).unwrap(),
        "saved work"
    );
    assert!(s.accounts.is_empty());
    assert!(s.selected.is_none());
    s.validate().unwrap();
}
#[test]
fn removal_selects_a_neighbor() {
    let (t, a) = account();
    let b = Account::linked("Second", t.path().join("two"), false).unwrap();
    let mut s = settings(a.clone());
    s.accounts.push(b.clone());
    s.remove(&a.id).unwrap();
    assert_eq!(s.current().unwrap().id, b.id);
}
#[test]
fn recents_are_deduplicated_and_bounded() {
    let mut s = Settings::default();
    for i in 0..12 {
        s.remember_project(&format!("project-{i}"));
    }
    s.remember_project("project-11");
    assert_eq!(s.recent_projects.len(), 8);
    assert_eq!(s.recent_projects[0], "project-11");
}
#[test]
fn settings_round_trip_after_atomic_replacement() {
    let (t, store, mut s) = store();
    store.add_account(&mut s, "Personal", None).unwrap();
    store
        .commit(&mut s, |s| {
            s.current_mut().unwrap().options.yolo = true;
            Ok(())
        })
        .unwrap();
    let root = t.path().join("app");
    drop(store);
    let (_store, loaded) = Store::open(root, None).unwrap();
    assert_eq!(s, loaded);
}
#[test]
fn second_gui_cannot_overwrite_settings() {
    let (t, _store, _s) = store();
    assert!(Store::open(t.path().join("app"), None).is_err());
}
#[test]
fn failed_transaction_does_not_mutate_memory_or_disk() {
    let (_t, store, mut s) = store();
    let original = fs::read(store.root().join("settings.json")).unwrap();
    assert!(
        store
            .commit(&mut s, |s| {
                s.schema_version = 999;
                Ok(())
            })
            .is_err()
    );
    assert_eq!(s.schema_version, 1);
    assert_eq!(
        fs::read(store.root().join("settings.json")).unwrap(),
        original
    );
}
#[test]
fn malformed_settings_are_not_overwritten() {
    let (t, store, _s) = store();
    let root = store.root().to_path_buf();
    drop(store);
    fs::write(root.join("settings.json"), "{broken").unwrap();
    assert!(Store::open(root.clone(), None).is_err());
    assert_eq!(
        fs::read_to_string(root.join("settings.json")).unwrap(),
        "{broken"
    );
    drop(t);
}
#[test]
fn new_account_is_private_uuid_and_not_a_model_copy() {
    let (_t, store, mut s) = store();
    store.add_account(&mut s, "../../not a path", None).unwrap();
    let a = s.current().unwrap();
    assert!(a.home.starts_with(store.root().join("accounts")));
    assert!(uuid::Uuid::parse_str(&a.id).is_ok());
    assert!(!a.options.yolo);
    let config = fs::read_to_string(a.home.join("config.toml")).unwrap();
    assert!(!config.contains("model ="));
    assert!(!a.home.join("auth.json").exists());
}
#[test]
fn additional_accounts_do_not_inherit_yolo() {
    let (_t, store, mut s) = store();
    store.add_account(&mut s, "One", None).unwrap();
    s.current_mut().unwrap().options.yolo = true;
    store.add_account(&mut s, "Two", None).unwrap();
    assert!(!s.current().unwrap().options.yolo);
}
#[test]
fn import_existing_home_and_duplicate_rejection() {
    let (t, store, mut s) = store();
    let home = t.path().join("old");
    fs::create_dir(&home).unwrap();
    fs::write(home.join("config.toml"), "keep me").unwrap();
    store.add_account(&mut s, "Old", Some(&home)).unwrap();
    assert!(store.add_account(&mut s, "Again", Some(&home)).is_err());
    assert_eq!(
        fs::read_to_string(home.join("config.toml")).unwrap(),
        "keep me"
    );
}
#[test]
fn cannot_import_arbitrary_project_as_codex_home() {
    let (t, store, mut s) = store();
    let home = t.path().join("project");
    fs::create_dir(&home).unwrap();
    assert!(store.add_account(&mut s, "No", Some(&home)).is_err());
}
#[test]
fn all_launch_options_and_account_types_form_correct_arguments() {
    let (_t, mut a) = account();
    for existing in [false, true] {
        a.credentials = if existing {
            CredentialPolicy::Existing
        } else {
            CredentialPolicy::IsolatedFile
        };
        for mask in 0..16 {
            a.options = LaunchOptions {
                yolo: mask & 1 != 0,
                worktree: mask & 2 != 0,
                live_search: mask & 4 != 0,
                inline_terminal: mask & 8 != 0,
            };
            for action in [Action::New, Action::Resume] {
                let args = launch::arguments(&a, action);
                assert!(args.contains(&"--no-daemon".into()));
                assert_eq!(args.contains(&"--yolo".into()), a.options.yolo);
                assert_eq!(args.contains(&"workspace-write".into()), !a.options.yolo);
                assert_eq!(args.contains(&"on-request".into()), !a.options.yolo);
                assert_eq!(
                    args.contains(&"--worktree".into()),
                    a.options.worktree && action == Action::New
                );
                assert_eq!(args.contains(&"--search".into()), a.options.live_search);
                assert_eq!(
                    args.contains(&"--no-alt-screen".into()),
                    a.options.inline_terminal
                );
                assert_eq!(
                    args.contains(&"forced_login_method=\"chatgpt\"".into()),
                    !existing
                );
            }
        }
    }
}
#[test]
fn resume_never_moves_project_or_creates_worktree() {
    let (_t, mut a) = account();
    a.options.worktree = true;
    a.project = "must not be used".into();
    let args = launch::arguments(&a, Action::Resume);
    assert!(args.contains(&"--all".into()));
    assert!(!args.contains(&"-C".into()));
    assert!(!args.contains(&a.project));
    assert!(!args.contains(&"--worktree".into()));
}
#[test]
fn login_and_status_do_not_receive_execution_permissions() {
    let (_t, mut a) = account();
    a.options = LaunchOptions {
        yolo: true,
        worktree: true,
        live_search: true,
        inline_terminal: true,
    };
    for action in [Action::Login, Action::DeviceLogin, Action::Status] {
        let args = launch::arguments(&a, action);
        for bad in [
            "--yolo",
            "--worktree",
            "--search",
            "--no-daemon",
            "--sandbox",
        ] {
            assert!(!args.contains(&bad.into()));
        }
        assert!(args.contains(&"login".into()));
    }
}
#[test]
fn powershell_literals_escape_apostrophes_and_expressions() {
    assert_eq!(
        launch::ps_quote("O'Brien & $(oops);"),
        "'O''Brien & $(oops);'"
    );
    assert_eq!(launch::ps_quote("trailing\\"), "'trailing\\'");
}
#[test]
fn terminal_titles_cannot_inject_windows_terminal_commands() {
    let title = launch::terminal_title("Personal; new-tab & $(oops)");
    assert!(!title.contains([';', '&', '$', '(', ')']));
}
#[test]
fn encoded_script_round_trips_unicode() {
    let (_t, mut a) = account();
    a.name = "Føx 東京 O'Brien".into();
    let p = plan(&a, Action::New);
    let bytes = STANDARD.decode(p.encoded_powershell()).unwrap();
    let words: Vec<_> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect();
    assert_eq!(String::from_utf16(&words).unwrap(), p.powershell());
}
#[test]
fn child_script_sets_only_process_scoped_environment() {
    let (_t, a) = account();
    let s = plan(&a, Action::New).powershell();
    assert!(s.contains("$env:CODEX_HOME="));
    for key in launch::CLEARED_ENV {
        assert!(s.contains(&format!("Remove-Item -LiteralPath 'Env:\\{}'", key)));
    }
    assert!(!s.contains("'Machine'"));
    assert!(!s.contains("'User'"));
    assert!(s.contains("does not support --no-daemon"));
}
#[test]
fn browser_logins_use_single_shared_mutex() {
    let (_t, a) = account();
    assert!(
        plan(&a, Action::Login)
            .powershell()
            .contains("Local\\CodexAccountsBrowserLogin")
    );
    assert!(
        !plan(&a, Action::New)
            .powershell()
            .contains("New-Object Threading.Mutex")
    );
}
#[test]
fn duplicate_cached_identity_is_blocked() {
    let (t, a) = account();
    fake_login(&a, "same");
    let home = t.path().join("two");
    fs::create_dir(&home).unwrap();
    let b = Account::linked("Second", home, false).unwrap();
    fake_login(&b, "same");
    assert!(assert_distinct_identity(&a, &[a.clone(), b.clone()]).is_err());
    fake_login(&b, "different");
    assert_distinct_identity(&a, &[a.clone(), b]).unwrap();
}
#[test]
fn cached_identity_does_not_leak_tokens_to_settings() {
    let (_t, a) = account();
    let payload=URL_SAFE_NO_PAD.encode(br#"{"email":"example@example.invalid","https://api.openai.com/auth":{"chatgpt_plan_type":"plus"}}"#);
    fs::write(a.home.join("auth.json"),serde_json::json!({"tokens":{"id_token":format!("fixture.{payload}.signature"),"account_id":"fixture-a"}}).to_string()).unwrap();
    let s = login_summary(&a);
    assert_eq!(s.email.as_deref(), Some("example@example.invalid"));
    assert_eq!(s.plan.as_deref(), Some("plus"));
    let json = serde_json::to_string(&settings(a)).unwrap();
    assert!(!json.contains("id_token"));
    assert!(!json.contains("example@example.invalid"));
    assert!(!json.contains("signature"));
}
#[test]
fn malformed_or_huge_auth_is_not_treated_as_logged_in() {
    let (_t, a) = account();
    fs::write(a.home.join("auth.json"), "bad json").unwrap();
    assert_eq!(login_summary(&a).state, LoginState::Unreadable);
    fs::write(a.home.join("auth.json"), vec![b' '; 129 * 1024]).unwrap();
    assert_eq!(login_summary(&a).state, LoginState::Unreadable);
}
#[test]
fn absent_default_auth_can_use_existing_keychain() {
    let (_t, mut a) = account();
    a.credentials = CredentialPolicy::Existing;
    assert_eq!(login_summary(&a).state, LoginState::ExistingStore);
    a.credentials = CredentialPolicy::IsolatedFile;
    assert_eq!(login_summary(&a).state, LoginState::SignedOut);
}
#[test]
fn launching_uses_selected_project_and_signed_in_account() {
    let (t, mut a) = account();
    fake_login(&a, "fixture");
    let exe = t.path().join("codex.exe");
    fs::write(&exe, "").unwrap();
    let project = t.path().join("project");
    fs::create_dir(&project).unwrap();
    a.project = project.to_string_lossy().into_owned();
    let p = LaunchPlan::prepare(&a, Action::New, &[a.clone()], exe.clone()).unwrap();
    assert_eq!(p.working_directory, project);
    assert_eq!(p.home, a.home);
    a.options.worktree = true;
    assert!(LaunchPlan::prepare(&a, Action::New, &[a.clone()], exe.clone()).is_err());
    fs::create_dir(project.join(".git")).unwrap();
    LaunchPlan::prepare(&a, Action::New, &[a.clone()], exe).unwrap();
}
#[test]
fn protected_default_login_cannot_be_replaced_in_launcher() {
    let (t, mut a) = account();
    a.protect_login = true;
    let exe = t.path().join("codex.exe");
    fs::write(&exe, "").unwrap();
    assert!(LaunchPlan::prepare(&a, Action::Login, &[a.clone()], exe).is_err());
}
#[test]
fn sign_in_can_run_before_auth_exists() {
    let (t, a) = account();
    let exe = t.path().join("codex.exe");
    fs::write(&exe, "").unwrap();
    LaunchPlan::prepare(&a, Action::Login, std::slice::from_ref(&a), exe).unwrap();
}
#[test]
fn missing_cli_never_silently_uses_another_executable() {
    let (_t, a) = account();
    assert!(
        LaunchPlan::prepare(
            &a,
            Action::Status,
            std::slice::from_ref(&a),
            a.home.join("missing.exe")
        )
        .is_err()
    );
    assert!(launch::locate_codex(Some(Path::new("relative.exe"))).is_err());
}
#[test]
fn removed_account_can_be_relinked_without_losing_its_state() {
    let (_t, store, mut s) = store();
    store.add_account(&mut s, "First", None).unwrap();
    let a = s.current().unwrap().clone();
    fake_login(&a, "fixture");
    s.remove(&a.id).unwrap();
    store.save(&s).unwrap();
    store
        .add_account(&mut s, "Returned", Some(&a.home))
        .unwrap();
    assert!(login_summary(s.current().unwrap()).saved());
}
#[test]
fn masked_email_hides_domain_and_length() {
    assert_eq!(
        masked_identity("person@private-company.invalid"),
        "p***@***"
    );
    assert_eq!(masked_identity("p@x.io"), "p***@***");
    assert!(!masked_identity("person@private-company.invalid").contains("private"));
}
#[test]
fn masked_username_and_unicode_are_safe() {
    assert_eq!(masked_identity("creator-name"), "c***");
    assert_eq!(masked_identity("東京@example.invalid"), "東***@***");
    assert_eq!(masked_identity(""), "****");
}
#[test]
fn identity_is_only_clear_with_explicit_reveal() {
    let mut summary = LoginSummary::demo("plus");
    summary.email = Some("person@example.invalid".into());
    assert_eq!(summary.identity_label(false), "p***@***");
    assert_eq!(summary.identity_label(true), "person@example.invalid");
    assert_eq!(summary.identity_label(false), "p***@***");
}
#[test]
fn missing_identity_is_not_invented_from_nickname() {
    assert_eq!(
        LoginSummary::demo("plus").identity_label(false),
        "Identity unavailable"
    );
}
#[test]
fn stale_identity_never_makes_a_signed_out_account_look_ready() {
    let mut summary = LoginSummary::demo("plus");
    summary.email = Some("person@example.invalid".into());
    summary.state = LoginState::SignedOut;
    assert_eq!(summary.identity_label(false), "Not signed in");
    assert_eq!(summary.identity_label(true), "Not signed in");
}
#[test]
fn cached_username_is_used_when_email_claim_is_missing() {
    let (_t, a) = account();
    let payload = URL_SAFE_NO_PAD.encode(br#"{"preferred_username":"sample-creator"}"#);
    fs::write(
        a.home.join("auth.json"),
        serde_json::json!({"tokens":{"id_token":format!("fixture.{payload}.signature")}})
            .to_string(),
    )
    .unwrap();
    let summary = login_summary(&a);
    assert_eq!(summary.identity_label(false), "s***");
    assert_eq!(summary.identity_label(true), "sample-creator");
}
#[test]
fn cached_identity_strips_control_and_direction_overrides() {
    let (_t, a) = account();
    let payload = URL_SAFE_NO_PAD
        .encode(serde_json::json!({"email":"safe\u{202e}\n@example.invalid"}).to_string());
    fs::write(
        a.home.join("auth.json"),
        serde_json::json!({"tokens":{"id_token":format!("fixture.{payload}.signature")}})
            .to_string(),
    )
    .unwrap();
    let summary = login_summary(&a);
    assert_eq!(summary.identity_label(true), "safe@example.invalid");
    assert_eq!(summary.identity_label(false), "s***@***");
}
#[test]
fn unrelated_data_folder_is_not_taken_over() {
    let t = tempdir().unwrap();
    fs::write(t.path().join("unrelated.txt"), "keep this").unwrap();
    assert!(Store::open(t.path().to_path_buf(), None).is_err());
    assert!(!t.path().join("settings.lock").exists());
    assert_eq!(
        fs::read_to_string(t.path().join("unrelated.txt")).unwrap(),
        "keep this"
    );
}
#[test]
fn renaming_added_account_preserves_login_history_and_current_selection() {
    let (_tmp, store, mut state) = store();
    store.add_account(&mut state, "Account A", None).unwrap();
    let a = state.current().unwrap().clone();
    store.add_account(&mut state, "Account B", None).unwrap();
    let selected_before = state.selected.clone();
    fake_login(&a, "fixture-login");
    let history = a.home.join("history.jsonl");
    fs::write(&history, "saved conversation").unwrap();
    let auth_before = fs::read(a.home.join("auth.json")).unwrap();
    let history_before = fs::read(&history).unwrap();
    store
        .commit(&mut state, |s| {
            let target = s.accounts.iter_mut().find(|item| item.id == a.id).unwrap();
            target.name = validate_name(" Work & Play ")?;
            Ok(())
        })
        .unwrap();
    let after = state.accounts.iter().find(|item| item.id == a.id).unwrap();
    assert_eq!(after.name, "Work & Play");
    assert_eq!(after.id, a.id);
    assert_eq!(after.home, a.home);
    assert_eq!(after.credentials, a.credentials);
    assert_eq!(after.options, a.options);
    assert_eq!(state.selected, selected_before);
    assert_eq!(fs::read(a.home.join("auth.json")).unwrap(), auth_before);
    assert_eq!(fs::read(history).unwrap(), history_before);
    let settings_json = fs::read_to_string(store.root().join("settings.json")).unwrap();
    assert!(settings_json.contains("Work & Play"));
}
