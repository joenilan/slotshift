use crate::{theme as p, ui::Launcher};
use gpui_kit::base::Selectable;
use gpui_kit::component::{
    Disableable, Sizable, WindowExt,
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use slotshift::{
    launch::{self, LaunchPlan},
    model::Account,
    session_bridge::{self, SavedSession},
};
use std::{
    path::PathBuf,
    time::{Duration, SystemTime},
};

pub struct HandoffPicker {
    owner: WeakEntity<Launcher>,
    accounts: Vec<Account>,
    sessions: Vec<SavedSession>,
    selected: Option<(String, String)>,
    target: Option<String>,
    search: Entity<InputState>,
    project: Entity<InputState>,
    visible_limit: usize,
    loading: bool,
    busy: bool,
    error: String,
    demo: bool,
    executable: Option<PathBuf>,
    _search_subscription: Subscription,
    _load: Option<Task<()>>,
    _transfer: Option<Task<()>>,
}

impl HandoffPicker {
    pub fn new(
        owner: WeakEntity<Launcher>,
        accounts: Vec<Account>,
        target: Option<String>,
        executable: Option<PathBuf>,
        demo: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| {
            InputState::new(window, cx).placeholder("Find a saved conversation or project...")
        });
        let subscription = cx.subscribe(&search, |_, _, _: &InputEvent, cx| cx.notify());
        let project = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Folder in which Codex should continue (you can change it)")
        });
        let selected_target = target.or_else(|| accounts.first().map(|a| a.id.clone()));
        let scan_accounts = accounts.clone();
        let load = cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    if demo {
                        Ok(Self::example_sessions(&scan_accounts))
                    } else {
                        session_bridge::list_sessions(&scan_accounts)
                    }
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(sessions) => this.sessions = sessions,
                    Err(error) => this.error = error.to_string(),
                }
                cx.notify();
            });
        });
        Self {
            owner,
            accounts,
            sessions: Vec::new(),
            selected: None,
            target: selected_target,
            search,
            project,
            visible_limit: 70,
            loading: true,
            busy: false,
            error: String::new(),
            demo,
            executable,
            _search_subscription: subscription,
            _load: Some(load),
            _transfer: None,
        }
    }

    fn example_sessions(accounts: &[Account]) -> Vec<SavedSession> {
        let now = SystemTime::now();
        [
            "Improve renderer performance",
            "Fix inventory interactions",
            "Polish camera transitions",
            "Refine account manager",
            "Release readiness review",
        ]
        .iter()
        .enumerate()
        .filter_map(|(index, title)| {
            let account = accounts.get(index % accounts.len().max(1))?;
            Some(SavedSession {
                account_id: account.id.clone(),
                id: uuid::Uuid::new_v4().to_string(),
                title: (*title).into(),
                cwd: PathBuf::from(format!(
                    "E:\\Projects\\{}",
                    ["game", "client", "sandbox"][index % 3]
                )),
                path: PathBuf::new(),
                modified: now
                    .checked_sub(Duration::from_secs(index as u64 * 2400))
                    .unwrap_or(now),
                bytes: 12_000,
            })
        })
        .collect()
    }

    fn name(&self, id: &str) -> String {
        self.accounts
            .iter()
            .find(|a| a.id == id)
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "Unknown account".into())
    }
    fn refresh(&mut self, cx: &mut Context<Self>) {
        if self.loading || self.busy {
            return;
        }
        self.loading = true;
        self.error.clear();
        let accounts = self.accounts.clone();
        let demo = self.demo;
        self._load = Some(cx.spawn(async move |view, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    if demo {
                        Ok(Self::example_sessions(&accounts))
                    } else {
                        session_bridge::list_sessions(&accounts)
                    }
                })
                .await;
            let _ = view.update(cx, |this, cx| {
                this.loading = false;
                match result {
                    Ok(sessions) => this.sessions = sessions,
                    Err(e) => this.error = e.to_string(),
                }
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn browse_project(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Select the project folder for the continued session".into()),
        });
        cx.spawn_in(window, async move |view, cx| {
            if let Ok(Ok(Some(paths))) = paths.await
                && let Some(path) = paths.first()
            {
                let selected = path.to_string_lossy().into_owned();
                let _ = view.update_in(cx, |this, window, cx| {
                    this.project
                        .update(cx, |input, cx| input.set_value(selected, window, cx));
                    cx.notify();
                });
            }
        })
        .detach();
    }

    fn continue_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.busy || self.loading {
            return;
        }
        if self.demo {
            self.error = "Demo mode cannot access real account history or launch Codex.".into();
            cx.notify();
            return;
        }
        let Some((source_account, id)) = self.selected.clone() else {
            self.error = "Select a conversation first.".into();
            cx.notify();
            return;
        };
        let Some(target_id) = self.target.clone() else {
            self.error = "Choose the account that should continue this work.".into();
            cx.notify();
            return;
        };
        if source_account == target_id {
            self.error =
                "Choose a different destination account, or use Resume session normally.".into();
            cx.notify();
            return;
        }
        let Some(session) = self
            .sessions
            .iter()
            .find(|s| s.id == id && s.account_id == source_account)
            .cloned()
        else {
            self.error = "That saved conversation is no longer available. Refresh the list.".into();
            cx.notify();
            return;
        };
        let Some(source) = self
            .accounts
            .iter()
            .find(|a| a.id == source_account)
            .cloned()
        else {
            return;
        };
        let Some(target) = self.accounts.iter().find(|a| a.id == target_id).cloned() else {
            return;
        };
        let accounts = self.accounts.clone();
        let executable = self.executable.clone();
        let project_path = PathBuf::from(self.project.read(cx).value().trim());
        if !project_path.is_absolute() || !project_path.is_dir() {
            self.error = "Choose an existing folder for the continued work. Use Browse to pick your main checkout.".into();
            cx.notify();
            return;
        }
        let owner = self.owner.clone();
        self.busy = true;
        self.error.clear();
        cx.notify();
        self._transfer=Some(cx.spawn_in(window,async move |view,cx|{
            let result=cx.background_executor().spawn(async move {
                let cli=launch::locate_codex(executable.as_deref())?;
                // All failure-prone account / login checks happen before copying history.
                let plan=LaunchPlan::prepare_fork(&target,&accounts,cli,&session.id,&project_path)?;
                let imported=session_bridge::import_into(&session,&source,&target)?;
                plan.spawn()?;
                anyhow::Ok((target.name.clone(),imported.files_added,imported.bytes_added))
            }).await;
            let _=view.update_in(cx,|this,window,cx|{
                this.busy=false;
                match result {
                    Ok((name,files,bytes))=>{
                        window.close_dialog(cx);
                        let _=owner.update(cx,|main,cx|{
                            main.feedback(format!(
                                "Opened a fork under {name}. Imported {files} history file(s), {} MB. The original is unchanged.",
                                bytes/1_000_000
                            ),false,cx);
                        });
                    }
                    Err(err)=>{
                        this.error=format!("{err:#}. If the snapshot was copied but Codex could not open, retry or resume from the destination account.");
                        cx.notify();
                    }
                }
            });
        }));
    }
}
impl Render for HandoffPicker {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let target_name = self
            .target
            .as_deref()
            .map(|id| self.name(id))
            .unwrap_or_default();
        let needle = self.search.read(cx).value().to_lowercase();
        let filtered: Vec<&SavedSession> = self
            .sessions
            .iter()
            .filter(|s| {
                needle.is_empty()
                    || s.title.to_lowercase().contains(&needle)
                    || s.cwd.to_string_lossy().to_lowercase().contains(&needle)
                    || s.id.contains(&needle)
                    || self.name(&s.account_id).to_lowercase().contains(&needle)
            })
            .collect();
        let count = filtered.len();
        let mut targets = div()
            .id("handoff-targets")
            .flex()
            .flex_wrap()
            .gap_2()
            .max_h(px(112.))
            .overflow_y_scroll();
        for (index, account) in self.accounts.iter().enumerate() {
            let id = account.id.clone();
            let active = self.target.as_deref() == Some(&id);
            targets = targets.child(
                Button::new(("handoff-target", index))
                    .outline()
                    .selected(active)
                    .small()
                    .label(account.name.clone())
                    .accessibility_label(format!("Continue as {}", account.name))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.target = Some(id.clone());
                        this.error.clear();
                        cx.notify();
                    })),
            );
        }
        let mut list = div()
            .id("handoff-sessions")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1()
            .pr_2();
        for (index, s) in filtered.iter().take(self.visible_limit).enumerate() {
            let id = s.id.clone();
            let source = s.account_id.clone();
            let initial_project = s.cwd.to_string_lossy().into_owned();
            let highlighted = self
                .selected
                .as_ref()
                .is_some_and(|x| x.0 == source && x.1 == id);
            let source_name = self.name(&s.account_id);
            let folder = s
                .cwd
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Unknown folder".into());
            let age = s.modified.elapsed().unwrap_or_default().as_secs();
            let since = if age < 3600 {
                format!("{}m ago", age / 60)
            } else if age < 86400 {
                format!("{}h ago", age / 3600)
            } else {
                format!("{}d ago", age / 86400)
            };
            let row = div()
                .flex()
                .flex_col()
                .w_full()
                .items_start()
                .gap_1()
                .min_w_0()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .text_ellipsis()
                        .child(s.title.clone()),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(rgb(p::MUTED))
                        .flex()
                        .gap_2()
                        .child(source_name)
                        .child(" / ")
                        .child(folder)
                        .child(" / ")
                        .child(since),
                );
            list = list.child(
                Button::new(("handoff-session", index))
                    .ghost()
                    .selected(highlighted)
                    .w_full()
                    .h(px(64.))
                    .px_3()
                    .accessibility_label(format!(
                        "Select {} from {}",
                        s.title,
                        self.name(&s.account_id)
                    ))
                    .child(row)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.selected = Some((source.clone(), id.clone()));
                        this.project.update(cx, |input, cx| {
                            input.set_value(initial_project.clone(), window, cx)
                        });
                        this.error.clear();
                        cx.notify();
                    })),
            );
        }
        if count > self.visible_limit {
            list = list.child(
                Button::new("handoff-more")
                    .ghost()
                    .label(format!("Show more ({} of {})", self.visible_limit, count))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.visible_limit += 70;
                        cx.notify();
                    })),
            );
        }
        if count == 0 && !self.loading {
            list = list.child(div().py_7().px_4().child(p::muted(if needle.is_empty() {
                "No saved conversations found across linked accounts."
            } else {
                "No conversations match this search."
            })));
        }
        let selected = self
            .selected
            .as_ref()
            .map(|(a, id)| {
                let title = self
                    .sessions
                    .iter()
                    .find(|s| s.account_id == *a && s.id == *id)
                    .map(|s| s.title.as_str())
                    .unwrap_or("Saved session");
                format!("Selected: {title}")
            })
            .unwrap_or_else(|| "Choose a conversation above".into());
        div().w_full().h(px(690.)).flex().flex_col().gap_3().pt_2()
            .child(p::muted(
                "Choose an existing conversation, then choose which signed-in account takes over. \
                 Slotshift copies its history, and Codex creates an independent fork under that account."
            ))
            .child(div().flex().flex_col().gap_2()
                .child(p::label("CONTINUE WITH ACCOUNT"))
                .child(targets))
            .child(div().h(px(1.)).bg(rgb(p::BORDER)))
            .child(div().flex().flex_col().gap_2()
                .child(div().flex().items_center().justify_between()
                    .child(p::label("SAVED CONVERSATIONS / ALL ACCOUNTS"))
                    .child(Button::new("handoff-refresh").ghost().small().label("Refresh")
                        .disabled(self.loading||self.busy)
                        .on_click(cx.listener(|this,_,_,cx|this.refresh(cx)))))
                .child(Input::new(&self.search)))
            .child(div().flex_1().min_h_0().flex().flex_col()
                .child(if self.loading{"Scanning local history...".to_string()}
                    else{format!("{count} matching conversation(s)")})
                .child(list))
            .child(div().flex().flex_col().gap_2()
                .child(p::label("WORKING FOLDER FOR CONTINUATION"))
                .child(div().flex().items_center().gap_2()
                    .child(div().flex_1().min_w_0().child(Input::new(&self.project)))
                    .child(Button::new("handoff-browse-project").outline().label("Browse...")
                        .on_click(cx.listener(|this,_,w,cx|this.browse_project(w,cx)))))
                .child(div().text_size(px(11.)).text_color(rgb(p::MUTED))
                    .child("Defaults to the original folder. Change it to your main checkout if the old session used a worktree.")))
            .child(div().h(px(1.)).bg(rgb(p::BORDER)))
            .child(div().flex().flex_col().gap_2()
                .child(p::muted("Original history stays untouched. The new fork is a separate conversation. \
                    Parallel agents can still edit the same project files."))
                .child(div().flex().gap_3().items_center().justify_between()
                    .child(div().flex_1().min_w_0().flex().flex_col().gap_1()
                        .child(div().text_size(px(11.)).text_color(rgb(p::MUTED)).text_ellipsis()
                            .child(selected))
                        .when(!self.error.is_empty(),|d|d.child(
                            div().text_xs().text_color(rgb(p::ERROR)).child(self.error.clone())
                        )))
                    .child(Button::new("handoff-cancel").ghost().label("Cancel")
                        .disabled(self.busy).on_click(|_,w,cx|w.close_dialog(cx)))
                    .child(Button::new("handoff-continue").custom(p::primary(cx))
                        .label(if self.busy{"Preparing...".to_string()}
                            else{format!("Continue as {target_name}")})
                        .disabled(self.loading||self.busy||self.selected.is_none()
                            ||self.selected.as_ref().is_some_and(|(a,_)|self.target.as_deref()==Some(a)))
                        .on_click(cx.listener(|this,_,w,cx|this.continue_session(w,cx))))))
    }
}
