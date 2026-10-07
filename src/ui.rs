use crate::theme as palette;
use gpui_kit::component::{
    WindowExt,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    switch::Switch,
};
use gpui_kit::*;
use slotshift::{
    launch::{self, Action, LaunchPlan},
    model::{Account, LoginSummary, Settings, validate_name},
    storage::{Store, open_folder},
};
use std::{collections::HashMap, path::PathBuf, time::Duration};
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Launcher,
    Settings,
}
pub struct Launcher {
    pub store: Store,
    pub settings: Settings,
    pub logins: HashMap<String, LoginSummary>,
    pub project: Entity<InputState>,
    pub search: Entity<InputState>,
    pub page: Page,
    pub notice: String,
    pub error: bool,
    pub demo: bool,
    pub cli: Option<PathBuf>,
    _subscriptions: Vec<Subscription>,
    _refresh: Task<()>,
    _save: Option<Task<()>>,
    // Session-only privacy state. Never included in settings.json.
    revealed_account: Option<String>,
    _reveal_hide: Option<Task<()>>,
}
impl Launcher {
    pub fn new(
        store: Store,
        settings: Settings,
        demo: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let value = settings
            .current()
            .map(|a| a.project.clone())
            .unwrap_or_default();
        let project = cx.new(|cx| {
            let mut i = InputState::new(window, cx).placeholder("Choose your project folder");
            i.set_value(value, window, cx);
            i
        });
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Find an account..."));
        let search_sub = cx.subscribe(&search, |_, _, _: &InputEvent, cx| cx.notify());
        let project_sub = cx.subscribe(&project, |this, input, event, cx| {
            if matches!(event, InputEvent::Change) {
                let value = input.read(cx).value().to_string();
                if let Some(account) = this.settings.current_mut() {
                    if account.project == value {
                        return;
                    }
                    account.project = value;
                }
                this._save = Some(cx.spawn(async move |view, cx| {
                    smol::Timer::after(Duration::from_millis(400)).await;
                    let _ = view.update(cx, |this, cx| {
                        if let Err(e) = this.store.save(&this.settings) {
                            this.feedback(e.to_string(), true, cx);
                        }
                    });
                }));
                cx.notify();
            }
        });
        let cli = launch::locate_codex(settings.codex_executable.as_deref()).ok();
        let logins = Self::summaries(&settings.accounts, demo);
        let refresh = cx.spawn(async move |view, cx| {
            loop {
                smol::Timer::after(Duration::from_secs(5)).await;
                let Ok((accounts, demo)) =
                    view.update(cx, |this, _| (this.settings.accounts.clone(), this.demo))
                else {
                    break;
                };
                let result = cx
                    .background_executor()
                    .spawn(async move { Self::summaries(&accounts, demo) })
                    .await;
                if view
                    .update(cx, |this, cx| {
                        if this.logins != result {
                            this.logins = result;
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        let notice=if demo {"Demo workspace. Account identities are fictional; terminal launches are disabled."}
            else if settings.migrated_legacy {"Your existing accounts are linked in place. Credentials and session history have not moved."}
            else {"Choose an account and a project. Your running sessions stay independent."}.to_string();
        Self {
            store,
            settings,
            logins,
            project,
            search,
            page: Page::Launcher,
            notice,
            error: false,
            demo,
            cli,
            _subscriptions: vec![search_sub, project_sub],
            _refresh: refresh,
            _save: None,
            revealed_account: None,
            _reveal_hide: None,
        }
    }
    fn summaries(accounts: &[Account], demo: bool) -> HashMap<String, LoginSummary> {
        accounts
            .iter()
            .enumerate()
            .map(|(i, a)| {
                (
                    a.id.clone(),
                    if demo {
                        let mut summary = LoginSummary::demo(if i == 0 { "pro" } else { "plus" });
                        let local = ["personal", "work", "side"]
                            .get(i)
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| format!("account{}", i + 1));
                        summary.email = Some(format!("{local}@example.com"));
                        summary
                    } else {
                        slotshift::model::login_summary(a)
                    },
                )
            })
            .collect()
    }
    pub fn feedback(&mut self, text: impl Into<String>, error: bool, cx: &mut Context<Self>) {
        self.notice = text.into();
        self.error = error;
        cx.notify();
    }
    pub fn persist(&mut self, cx: &mut Context<Self>) -> bool {
        self._save = None;
        match self.store.save(&self.settings) {
            Ok(()) => true,
            Err(e) => {
                self.feedback(e.to_string(), true, cx);
                false
            }
        }
    }
    pub fn identity_revealed(&self) -> bool {
        self.revealed_account.is_some() && self.revealed_account == self.settings.selected
    }
    pub fn toggle_identity(&mut self, cx: &mut Context<Self>) {
        self._reveal_hide = None;
        if self.identity_revealed() {
            self.revealed_account = None;
            cx.notify();
            return;
        }
        let Some(id) = self.settings.selected.clone() else {
            return;
        };
        self.revealed_account = Some(id.clone());
        self._reveal_hide = Some(cx.spawn(async move |view, cx| {
            smol::Timer::after(Duration::from_secs(15)).await;
            let _ = view.update(cx, |this, cx| {
                if this.revealed_account.as_ref() == Some(&id) {
                    this.revealed_account = None;
                    cx.notify();
                }
            });
        }));
        cx.notify();
    }
    pub fn select(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.revealed_account = None;
        self._reveal_hide = None;
        if !self.persist(cx) {
            return;
        }
        if let Err(e) = self.store.commit(&mut self.settings, |s| {
            s.selected = Some(id);
            Ok(())
        }) {
            self.feedback(e.to_string(), true, cx);
            return;
        }
        let value = self
            .settings
            .current()
            .map(|a| a.project.clone())
            .unwrap_or_default();
        self.project
            .update(cx, |input, cx| input.set_value(value, window, cx));
        self.page = Page::Launcher;
        cx.notify();
    }
    pub fn toggle(&mut self, key: &str, value: bool, cx: &mut Context<Self>) {
        let result = self.store.commit(&mut self.settings, |s| {
            if let Some(a) = s.current_mut() {
                match key {
                    "yolo" => a.options.yolo = value,
                    "worktree" => a.options.worktree = value,
                    "search" => a.options.live_search = value,
                    "inline" => a.options.inline_terminal = value,
                    _ => anyhow::bail!("Unknown launch option."),
                }
            }
            Ok(())
        });
        match result {
            Ok(()) => self.feedback(
                "Saved for this account. Applies to the next new or resumed session.",
                false,
                cx,
            ),
            Err(e) => self.feedback(e.to_string(), true, cx),
        }
    }
    pub fn launch(&mut self, action: Action, window: &mut Window, cx: &mut Context<Self>) {
        if self.demo {
            self.feedback(
                "Demo mode never signs in or launches a real Codex process.",
                false,
                cx,
            );
            return;
        }
        let Some(account) = self.settings.current().cloned() else {
            self.feedback("Add or select an account first.", true, cx);
            return;
        };
        if action.login() && self.logins.get(&account.id).is_some_and(|s| s.saved()) {
            let view = cx.entity().downgrade();
            let name = account.name.clone();
            window.open_dialog(cx,move |dialog,_,dialog_cx|{
                let view=view.clone();
                dialog.title(format!("Sign in again to {name}?")).w(px(460.)).child(palette::muted("This replaces the saved login for this account home. Finish or close sessions using this account before continuing. Other account homes are unchanged."))
                    .footer(div().flex().justify_end().gap_2()
                        .child(Button::new("cancel-login").label("Cancel").on_click(|_,w,cx|w.close_dialog(cx)))
                        .child(Button::new("confirm-login").custom(palette::primary(dialog_cx)).label("Open sign-in").on_click(move |_,w,cx|{w.close_dialog(cx);let _=view.update(cx,|this,cx|this.launch_now(action,cx));})))
            });
            return;
        }
        self.launch_now(action, cx);
    }
    fn launch_now(&mut self, action: Action, cx: &mut Context<Self>) {
        if !self.persist(cx) {
            return;
        }
        let Some(account) = self.settings.current().cloned() else {
            return;
        };
        let result = (|| -> anyhow::Result<()> {
            let executable = launch::locate_codex(self.settings.codex_executable.as_deref())?;
            self.cli = Some(executable.clone());
            let plan = LaunchPlan::prepare(&account, action, &self.settings.accounts, executable)?;
            if action == Action::New {
                self.store.commit(&mut self.settings, |s| {
                    s.remember_project(&account.project);
                    Ok(())
                })?;
            }
            plan.spawn()?;
            Ok(())
        })();
        match result {
            Ok(()) => self.feedback(
                format!(
                    "{} terminal opened for {}. Check that terminal for Codex status.",
                    action.description(),
                    account.name
                ),
                false,
                cx,
            ),
            Err(e) => self.feedback(e.to_string(), true, cx),
        }
    }
    pub fn browse_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose project folder".into()),
        });
        cx.spawn_in(window, async move |view, cx| match paths.await {
            Ok(Ok(Some(paths))) => {
                if let Some(path) = paths.first() {
                    let value = path.to_string_lossy().into_owned();
                    let _ = view.update_in(cx, |this, w, cx| this.set_project(value, w, cx));
                }
            }
            Ok(Err(e)) => {
                let _ = view.update(cx, |this, cx| this.feedback(e.to_string(), true, cx));
            }
            _ => {}
        })
        .detach();
    }
    pub fn set_project(&mut self, value: String, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(a) = self.settings.current_mut() {
            a.project = value.clone();
        }
        self.project
            .update(cx, |i, cx| i.set_value(value, window, cx));
        self.persist(cx);
        cx.notify();
    }
    pub fn browse_cli(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Choose the standalone codex.exe".into()),
        });
        cx.spawn_in(window, async move |view, cx| {
            if let Ok(Ok(Some(paths))) = paths.await
                && let Some(path) = paths.first()
            {
                let path = path.clone();
                let _ = view.update(cx, |this, cx| match launch::locate_codex(Some(&path)) {
                    Ok(path) => {
                        let result = this.store.commit(&mut this.settings, |s| {
                            s.codex_executable = Some(path.clone());
                            Ok(())
                        });
                        match result {
                            Ok(()) => {
                                this.cli = Some(path);
                                this.feedback("Standalone Codex executable saved.", false, cx);
                            }
                            Err(e) => this.feedback(e.to_string(), true, cx),
                        }
                    }
                    Err(e) => this.feedback(e.to_string(), true, cx),
                });
            }
        })
        .detach();
    }
    pub fn reset_cli(&mut self, cx: &mut Context<Self>) {
        let result = self.store.commit(&mut self.settings, |s| {
            s.codex_executable = None;
            Ok(())
        });
        match result {
            Ok(()) => {
                self.cli = launch::locate_codex(None).ok();
                self.feedback("Using automatic standalone CLI detection.", false, cx);
            }
            Err(e) => self.feedback(e.to_string(), true, cx),
        }
    }
    pub fn open_home(&mut self, cx: &mut Context<Self>) {
        if let Some(a) = self.settings.current() {
            let result = open_folder(&a.home);
            if let Err(e) = result {
                self.feedback(e.to_string(), true, cx);
            }
        }
    }
    pub fn edit_account(&mut self, rename: bool, window: &mut Window, cx: &mut Context<Self>) {
        let account = if rename {
            self.settings.current().cloned()
        } else {
            None
        };
        let title = if rename {
            "Rename account"
        } else {
            "Add an account"
        };
        let owner = cx.entity().downgrade();
        let editor = cx.new(|cx| AccountEditor::new(owner, account, window, cx));
        window.open_dialog(cx, move |dialog, _, _| {
            dialog
                .title(title)
                .w(px(480.))
                .overlay_closable(false)
                .child(editor.clone())
        });
    }
    pub fn remove_account(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(account) = self.settings.current().cloned() else {
            return;
        };
        let owner = cx.entity().downgrade();
        window.open_dialog(cx,move |dialog,_,_|{
            let owner=owner.clone();let id=account.id.clone();
            dialog.title(format!("Remove {} from Slotshift?",account.name)).w(px(500.))
                .child(div().flex().flex_col().gap_3()
                    .child(palette::muted("Only the entry in this launcher is removed. Credentials, session history, worktrees, and running terminals stay untouched. You can link this home again later."))
                    .child(div().text_xs().font_family("Cascadia Mono").child(account.home.to_string_lossy().into_owned())))
                .footer(div().flex().justify_end().gap_2()
                    .child(Button::new("cancel-remove").label("Keep account").on_click(|_,w,cx|w.close_dialog(cx)))
                    .child(Button::new("confirm-remove").danger().label("Remove entry").on_click(move |_,w,cx|{
                        let success=owner.update(cx,|this,cx|{
                            let result=this.store.commit(&mut this.settings,|s|s.remove(&id));
                            match result {Ok(_)=>{this.logins.remove(&id);let value=this.settings.current().map(|a|a.project.clone()).unwrap_or_default();this.project.update(cx,|i,cx|i.set_value(value,w,cx));this.feedback("Account entry removed. Its files and saved login were kept.",false,cx);true},Err(e)=>{this.feedback(e.to_string(),true,cx);false}}
                        }).unwrap_or(false);if success{w.close_dialog(cx);}
                    })))
        });
    }
}
impl Drop for Launcher {
    fn drop(&mut self) {
        let _ = self.store.save(&self.settings);
    }
}
struct AccountEditor {
    owner: WeakEntity<Launcher>,
    name: Entity<InputState>,
    path: Entity<InputState>,
    rename: Option<String>,
    link: bool,
    error: String,
}
impl AccountEditor {
    fn new(
        owner: WeakEntity<Launcher>,
        account: Option<Account>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let name = cx.new(|cx| {
            let mut i =
                InputState::new(window, cx).placeholder("e.g. Personal, Work, Side projects");
            if let Some(a) = &account {
                i.set_value(a.name.clone(), window, cx);
            }
            i
        });
        let path =
            cx.new(|cx| InputState::new(window, cx).placeholder("Existing CODEX_HOME folder"));
        Self {
            owner,
            name,
            path,
            rename: account.map(|a| a.id),
            link: false,
            error: String::new(),
        }
    }
    fn browse(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose an existing Codex home".into()),
        });
        cx.spawn_in(window, async move |view, cx| {
            if let Ok(Ok(Some(paths))) = paths.await
                && let Some(path) = paths.first()
            {
                let value = path.to_string_lossy().into_owned();
                let _ = view.update_in(cx, |this, w, cx| {
                    this.path.update(cx, |i, cx| i.set_value(value, w, cx));
                    cx.notify();
                });
            }
        })
        .detach();
    }
    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.name.read(cx).value().to_string();
        let path = PathBuf::from(self.path.read(cx).value().trim());
        let rename = self.rename.clone();
        let link = self.link;
        let result = self.owner.update(cx, |owner, cx| -> anyhow::Result<()> {
            if let Some(id) = rename {
                let name = validate_name(&name)?;
                owner.store.commit(&mut owner.settings, |s| {
                    let a = s
                        .accounts
                        .iter_mut()
                        .find(|a| a.id == id)
                        .ok_or_else(|| anyhow::anyhow!("Account not found."))?;
                    a.name = name;
                    Ok(())
                })?;
                owner.feedback(
                    "Account renamed. Its credentials and history are unchanged.",
                    false,
                    cx,
                );
            } else {
                owner.store.add_account(
                    &mut owner.settings,
                    &name,
                    if link { Some(path.as_path()) } else { None },
                )?;
                let value = owner
                    .settings
                    .current()
                    .map(|a| a.project.clone())
                    .unwrap_or_default();
                owner
                    .project
                    .update(cx, |i, cx| i.set_value(value, window, cx));
                owner.logins = Launcher::summaries(&owner.settings.accounts, owner.demo);
                owner.page = Page::Launcher;
                owner.feedback(
                    if link {
                        "Existing Codex home linked. No files were copied."
                    } else {
                        "Account added. Use Sign in to authorize it with Codex."
                    },
                    false,
                    cx,
                );
            }
            Ok(())
        });
        match result {
            Ok(Ok(())) => window.close_dialog(cx),
            Ok(Err(e)) => {
                self.error = e.to_string();
                cx.notify();
            }
            Err(_) => {
                self.error = "The launcher is no longer available.".into();
                cx.notify();
            }
        }
    }
}
impl Render for AccountEditor {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut content = div()
            .flex()
            .flex_col()
            .gap_4()
            .pt_2()
            .child(palette::muted(if self.rename.is_some() {
                "Choose a label that makes this account easy to recognize."
            } else {
                "One account, one Codex home. Add as many as you need."
            }))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(palette::label("ACCOUNT NAME"))
                    .child(Input::new(&self.name)),
            );
        if self.rename.is_none() {
            content = content.child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child("Link an existing Codex home")
                            .child(palette::muted(
                                "Reuse its login and session history in place.",
                            )),
                    )
                    .child(
                        Switch::new("link-home")
                            .accessibility_label("Link an existing Codex home")
                            .checked(self.link)
                            .on_change(cx.listener(|this, value, _, cx| {
                                this.link = *value;
                                cx.notify();
                            })),
                    ),
            );
            if self.link {
                content = content.child(
                    div()
                        .flex()
                        .gap_2()
                        .child(div().flex_1().min_w_0().child(Input::new(&self.path)))
                        .child(
                            Button::new("browse-home")
                                .label("Browse")
                                .on_click(cx.listener(|this, _, w, cx| this.browse(w, cx))),
                        ),
                );
            } else {
                content=content.child(palette::muted("A private account folder is created for you. Sign in afterward using Codex's browser authorization. YOLO starts off."));
            }
        }
        if !self.error.is_empty() {
            content = content.child(
                div()
                    .text_sm()
                    .text_color(rgb(palette::ERROR))
                    .child(self.error.clone()),
            );
        }
        content.child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .pt_3()
                .child(
                    Button::new("cancel-account")
                        .label("Cancel")
                        .on_click(|_, w, cx| w.close_dialog(cx)),
                )
                .child(
                    Button::new("save-account")
                        .custom(palette::primary(cx))
                        .label(if self.rename.is_some() {
                            "Save name"
                        } else if self.link {
                            "Link account"
                        } else {
                            "Add account"
                        })
                        .on_click(cx.listener(|this, _, w, cx| this.submit(w, cx))),
                ),
        )
    }
}
