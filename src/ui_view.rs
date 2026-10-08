use crate::{
    theme as p,
    ui::{Launcher, Page},
};
use gpui_kit::base::Selectable;
use gpui_kit::component::{
    Disableable, Icon, IconName, Sizable, TitleBar,
    button::{Button, ButtonVariants},
    input::Input,
    switch::Switch,
};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::*;
use slotshift::{
    launch::{Action, preview},
    model::Account,
};
use std::path::Path;

const TRASH_ICON: &[u8] = br#"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='M3 6h18'/><path d='M8 6V4h8v2'/><path d='m19 6-1 14H6L5 6'/><path d='M10 11v5'/><path d='M14 11v5'/></svg>"#;
const PENCIL_ICON: &[u8] = br#"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='currentColor' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='M12 20h9'/><path d='M16 4 20 8'/><path d='M18.5 5.5 7 17l-4 1 1-4L15.5 2.5a2.1 2.1 0 0 1 3 3Z'/></svg>"#;

impl Launcher {
    fn sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let needle = self.search.read(cx).value().to_lowercase();
        let mut rows = div()
            .id("account-list")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1()
            .px_3();
        let mut visible = 0;
        for (index, account) in self.settings.accounts.iter().enumerate() {
            if !needle.is_empty() && !account.name.to_lowercase().contains(&needle) {
                continue;
            }
            visible += 1;
            let id = account.id.clone();
            let selected = self.settings.selected.as_deref() == Some(id.as_str());
            let login = self.logins.get(&id);
            let detail = login
                .map(|s| {
                    if s.saved() {
                        s.plan
                            .as_ref()
                            .map(|plan| format!("{} / cached plan", plan.to_uppercase()))
                            .unwrap_or_else(|| "Login saved".into())
                    } else {
                        String::new()
                    }
                })
                .unwrap_or_default();
            // The sidebar stays masked even while a header identity is deliberately revealed.
            let identity = login
                .map(|s| s.identity_label(false))
                .unwrap_or_else(|| "Checking login".into());
            let row = div()
                .flex()
                .w_full()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .w(px(36.))
                        .h(px(36.))
                        .flex_shrink_0()
                        .rounded(px(5.))
                        .border_1()
                        .border_color(rgb(if selected { 0x495D44 } else { p::BORDER }))
                        .bg(rgb(if selected { 0x293C27 } else { p::SURFACE }))
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(12.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(if selected { p::ACCENT } else { p::MUTED }))
                        .child(account.initials()),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::MEDIUM)
                                .text_ellipsis()
                                .child(account.name.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(rgb(p::MUTED))
                                .text_ellipsis()
                                .child(identity),
                        )
                        .when(!detail.is_empty(), |d| {
                            d.child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(rgb(p::MUTED))
                                    .text_ellipsis()
                                    .child(detail),
                            )
                        }),
                )
                .when(selected, |d| {
                    d.child(div().w(px(5.)).h(px(5.)).rounded_full().bg(rgb(p::ACCENT)))
                });
            let rename_id = id.clone();
            let remove_id = id.clone();
            rows = rows.child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        Button::new(("account", index))
                            .ghost()
                            .selected(selected)
                            .flex_1()
                            .min_w_0()
                            .h(px(82.))
                            .px_3()
                            .accessibility_label(format!("Select {}", account.name))
                            .child(row)
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.select(id.clone(), window, cx)
                            })),
                    )
                    .child(
                        Button::new(("rename-account-row", index))
                            .ghost()
                            .w(px(30.))
                            .h(px(32.))
                            .p_0()
                            .accessibility_label(format!("Rename {}", account.name))
                            .tooltip("Rename account")
                            .child(
                                Icon::default()
                                    .data(PENCIL_ICON)
                                    .size(px(15.))
                                    .text_color(rgb(p::MUTED)),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.rename_account(rename_id.clone(), window, cx)
                            })),
                    )
                    .child(
                        Button::new(("remove-account-row", index))
                            .ghost()
                            .w(px(32.))
                            .h(px(32.))
                            .p_0()
                            .accessibility_label(format!("Remove {}", account.name))
                            .tooltip("Remove account from Slotshift")
                            .child(
                                Icon::default()
                                    .data(TRASH_ICON)
                                    .size(px(15.))
                                    .text_color(rgb(p::MUTED)),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.remove_account(remove_id.clone(), window, cx)
                            })),
                    ),
            );
        }
        if visible == 0 {
            rows = rows.child(
                p::muted(if self.settings.accounts.is_empty() {
                    "No accounts yet."
                } else {
                    "No matching accounts."
                })
                .px_2()
                .py_4(),
            );
        }
        div()
            .w(px(286.))
            .h_full()
            .flex_shrink_0()
            .bg(rgb(p::SIDEBAR))
            .border_r_1()
            .border_color(rgb(p::BORDER))
            .flex()
            .flex_col()
            .child(
                div()
                    .px_5()
                    .pt_6()
                    .pb_7()
                    .flex()
                    .gap_3()
                    .items_center()
                    .child(p::mark())
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(21.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Slotshift"),
                            )
                            .child(p::label("ACCOUNTS, SIDE BY SIDE")),
                    ),
            )
            .child(
                div()
                    .px_5()
                    .pb_3()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(p::label("YOUR ACCOUNTS"))
                    .child(p::label(self.settings.accounts.len().to_string())),
            )
            .child(div().px_4().pb_3().child(Input::new(&self.search).small()))
            .child(rows)
            .child(
                div()
                    .p_4()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        Button::new("add-account")
                            .outline()
                            .icon(IconName::Plus)
                            .label("Add account")
                            .w_full()
                            .on_click(
                                cx.listener(|this, _, w, cx| this.edit_account(false, w, cx)),
                            ),
                    )
                    .child(div().h(px(1.)).bg(rgb(p::BORDER)))
                    .child(
                        Button::new("settings")
                            .ghost()
                            .icon(IconName::Settings)
                            .label("Settings & account tools")
                            .w_full()
                            .selected(self.page == Page::Settings)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.page = Page::Settings;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .px_1()
                            .pt_2()
                            .child(p::label("0.2.2 / ALPHA"))
                            .child(p::label("zombie.digital")),
                    ),
            )
    }
    fn option_row(
        &self,
        key: &'static str,
        title: &'static str,
        description: &'static str,
        checked: bool,
        code: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .py_3()
            .border_b_1()
            .border_color(rgb(p::BORDER))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(div().font_weight(FontWeight::MEDIUM).child(title))
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .font_family("Cascadia Mono")
                                    .text_color(rgb(p::MUTED))
                                    .child(code),
                            ),
                    )
                    .child(p::muted(description)),
            )
            .child(
                Switch::new(key)
                    .accessibility_label(title)
                    .checked(checked)
                    .when(key == "yolo", |s| s.color(rgb(p::AMBER)))
                    .on_change(cx.listener(move |this, value, _, cx| this.toggle(key, *value, cx))),
            )
    }
    fn launch_page(&self, account: &Account, cx: &mut Context<Self>) -> AnyElement {
        let login = self.logins.get(&account.id);
        let options = &account.options;
        let mut recents = div().flex().flex_wrap().gap_2();
        for (index, path) in self.settings.recent_projects.iter().take(5).enumerate() {
            let path = path.clone();
            let name = Path::new(&path)
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone());
            recents = recents.child(
                Button::new(("recent", index))
                    .ghost()
                    .small()
                    .label(name)
                    .tooltip(path.clone())
                    .on_click(
                        cx.listener(move |this, _, w, cx| this.set_project(path.clone(), w, cx)),
                    ),
            );
        }
        let sign_label = if login.is_some_and(|s| s.saved()) {
            "Sign in again"
        } else {
            "Sign in"
        };
        div().flex().flex_col().size_full().min_w_0()
            .child(div().px_7().pt_7().pb_5().flex().justify_between().gap_4().items_start()
                .child(div().flex_1().min_w_0().flex().flex_col().gap_2()
                    .child(p::label("WORKSPACE / CODEX CLI"))
                    .child(div().text_size(px(27.)).font_weight(FontWeight::SEMIBOLD).text_ellipsis().child(account.name.clone()))
                    .child(div().flex().gap_2().items_center()
                        .child(div().w(px(6.)).h(px(6.)).rounded_full().bg(rgb(if login.is_some_and(|s|s.saved()){p::ACCENT}else{p::MUTED})))
                        .child(p::muted(login.map(|s|s.label()).unwrap_or("Checking login")))
                        .when_some(login.filter(|s|s.saved()),|d,summary|d
                            .child(div().text_size(px(13.)).text_color(rgb(p::TEXT)).text_ellipsis().child(format!("/ {}",summary.identity_label(self.identity_revealed()))))
                            .when(summary.email.is_some(),|d|d.child(Button::new("reveal-identity").ghost().small()
                                .label(if self.identity_revealed(){"Hide"}else{"Reveal"})
                                .tooltip("Show this login identity for 15 seconds. Sidebar identities stay masked.")
                                .on_click(cx.listener(|this,_,_,cx|this.toggle_identity(cx))))))))
                .child(div().flex().items_center().gap_2().pt_1()
                    .child(Button::new("rename-selected").ghost().small().label("Rename")
                        .on_click(cx.listener(|this,_,w,cx|this.edit_account(true,w,cx))))
                    .child(Button::new("login-status").ghost().small().label("Check login").on_click(cx.listener(|this,_,w,cx|this.launch(Action::Status,w,cx))))
                    .when(!account.protect_login,|d|d.child(Button::new("sign-in").outline().small().label(sign_label).on_click(cx.listener(|this,_,w,cx|this.launch(Action::Login,w,cx)))))))
            .child(div().id("launch-scroll").flex_1().min_h_0().overflow_y_scroll().px_7().pb_5().flex().flex_col().gap_5()
                .child(div().flex().flex_col().gap_3()
                    .child(div().flex().justify_between().child(p::label("PROJECT FOLDER")).child(div().text_xs().text_color(rgb(p::MUTED)).child(if options.worktree{"New sessions: separate worktree"}else{"New sessions: work in this folder"})))
                    .child(div().flex().items_center().gap_2().child(div().flex_1().min_w_0().child(Input::new(&self.project))).child(Button::new("browse-project").outline().label("Browse...").on_click(cx.listener(|this,_,w,cx|this.browse_project(w,cx)))))
                    .when(!self.settings.recent_projects.is_empty(),|d|d.child(div().flex().items_center().gap_2().child(p::label("RECENT")).child(recents))))
                .child(div().w_full().flex().items_center().justify_between().gap_4()
                    .p_4().bg(rgb(p::SURFACE)).border_1().border_color(rgb(p::BORDER)).rounded(px(6.))
                    .child(div().flex_1().min_w_0().flex().flex_col().gap_1()
                        .child(div().font_weight(FontWeight::MEDIUM).child("Continue with another account"))
                        .child(p::muted("Browse saved Codex conversations across every linked login.")))
                    .child(Button::new("cross-account-continue").outline().label("Choose session")
                        .on_click(cx.listener(|this,_,w,cx|this.open_handoff(w,cx)))))
                .child(div().flex().flex_col()
                    .child(div().flex().justify_between().pb_1().child(p::label("LAUNCH OPTIONS")).child(p::muted("Saved for this account")))
                    .child(self.option_row("yolo","YOLO mode","Use no Codex approvals and full access; Windows UAC and plugin prompts are separate.",options.yolo,"--yolo",cx))
                    .child(self.option_row("worktree","Separate worktree","Create another checkout. New sessions only.",options.worktree,"--worktree",cx))
                    .child(self.option_row("search","Live web search","Allow Codex to use live web search.",options.live_search,"--search",cx))
                    .child(self.option_row("inline","Keep terminal scrollback","Use an inline terminal instead of the alternate screen.",options.inline_terminal,"--no-alt-screen",cx)))
                .child(div().flex().gap_2().items_start().text_xs().text_color(rgb(if options.yolo{p::AMBER}else{p::MUTED}))
                    .child(if options.yolo{"!"}else{"i"})
                    .child(div().flex_1().child(if options.yolo{"YOLO requested: Codex commands run without its sandbox or approval prompts. Windows UAC, project trust, or separate plugin approvals may still appear."}else{"YOLO is off. Uses workspace-write permissions and approvals on request."})))
                .child(div().flex().flex_col().gap_2().child(p::label("NEW SESSION PREVIEW"))
                    .child(div().p_3().bg(rgb(p::SIDEBAR)).border_1().border_color(rgb(p::BORDER)).rounded(px(5.)).font_family("Cascadia Mono").text_size(px(11.)).text_color(rgb(p::MUTED)).child(preview(account,Action::New)))))
            .child(div().px_7().py_5().border_t_1().border_color(rgb(p::BORDER)).flex().items_center().justify_between().gap_4()
                .child(div().flex_1().min_w_0().child(p::muted("Your work stays in its own terminal.")).child(div().text_xs().pt_1().text_color(rgb(p::MUTED)).child("Resume opens this account's saved-session picker.")))
                .child(Button::new("resume-session").outline().h(px(42.)).label("Resume session").on_click(cx.listener(|this,_,w,cx|this.launch(Action::Resume,w,cx))))
                .child(Button::new("launch-session").custom(p::primary(cx)).h(px(42.)).px_5().label("Launch session").on_click(cx.listener(|this,_,w,cx|this.launch(Action::New,w,cx)))))
            .into_any_element()
    }
    fn settings_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut content=div().id("settings-scroll").size_full().overflow_y_scroll().p_7().flex().flex_col().gap_6()
            .child(div().flex().flex_col().gap_2().child(p::label("SLOTSHIFT / PREFERENCES")).child(div().text_size(px(27.)).font_weight(FontWeight::SEMIBOLD).child("Settings")).child(p::muted("Local accounts. Explicit choices. Nothing running in the middle.")))
            .child(div().flex().flex_col().gap_3().child(p::label("CODEX EXECUTABLE"))
                .child(p::muted("Uses your installed standalone CLI. Slotshift does not install, update, or modify ChatGPT Desktop."))
                .child(div().p_3().bg(rgb(p::SIDEBAR)).rounded(px(5.)).text_xs().font_family("Cascadia Mono").child(self.cli.as_ref().map(|p|p.to_string_lossy().into_owned()).unwrap_or_else(||"Not detected. Install Codex CLI or choose codex.exe below.".into())))
                .child(div().flex().gap_2().child(Button::new("select-cli").outline().label("Choose executable").on_click(cx.listener(|this,_,w,cx|this.browse_cli(w,cx))))
                    .child(Button::new("auto-cli").ghost().label("Use auto-detect").on_click(cx.listener(|this,_,_,cx|this.reset_cli(cx))))));
        if let Some(account) = self.settings.current() {
            let remove_id = account.id.clone();
            content=content.child(div().h(px(1.)).bg(rgb(p::BORDER)))
                .child(div().flex().flex_col().gap_3().child(p::label(format!("SELECTED ACCOUNT / {}",account.name.to_uppercase())))
                    .child(div().flex().items_center().gap_3()
                        .child(p::label("LOGIN IDENTITY"))
                        .child(div().text_sm().child(self.logins.get(&account.id).map(|s|s.identity_label(self.identity_revealed())).unwrap_or_else(||"Checking login".into())))
                        .when(self.logins.get(&account.id).is_some_and(|s|s.saved()&&s.email.is_some()),|d|d.child(Button::new("settings-reveal-identity").ghost().small()
                            .label(if self.identity_revealed(){"Hide"}else{"Reveal"})
                            .tooltip("Temporarily reveal this identity; automatically hidden after 15 seconds.")
                            .on_click(cx.listener(|this,_,_,cx|this.toggle_identity(cx))))))
                    .child(div().p_3().bg(rgb(p::SIDEBAR)).rounded(px(5.)).text_xs().font_family("Cascadia Mono").child(account.home.to_string_lossy().into_owned()))
                    .child(p::muted(if account.protect_login{"This is your default Codex home. Re-login is protected here to avoid disrupting other apps."}else{"Credentials and session history are stored in this account home. Removing its entry never deletes the folder."}))
                    .child(div().flex().flex_wrap().gap_2()
                        .child(Button::new("rename-account").outline().label("Rename").on_click(cx.listener(|this,_,w,cx|this.edit_account(true,w,cx))))
                        .child(Button::new("open-home").outline().label("Open account folder").on_click(cx.listener(|this,_,_,cx|this.open_home(cx))))
                        .child(Button::new("device-login").outline().disabled(account.protect_login).label("Device sign-in").on_click(cx.listener(|this,_,w,cx|this.launch(Action::DeviceLogin,w,cx))))
                        .child(Button::new("remove-account").danger().label("Remove account").on_click(cx.listener(move |this,_,w,cx|this.remove_account(remove_id.clone(),w,cx))))));
        }
        content.child(div().h(px(1.)).bg(rgb(p::BORDER)))
            .child(div().flex().flex_col().gap_3().child(p::label("SMALL BY DESIGN"))
                .child(p::muted("No API proxy. No account rotation. No combined quotas. Each terminal uses the account you chose. Your provider's terms and usage limits still apply."))
                .child(p::muted("Native Rust + GPUI Kit. Windows-first alpha. No telemetry from Slotshift. Cached login labels are not live subscription or usage checks."))
                .child(div().flex().gap_2()
                    .child(Button::new("github").outline().label("View on GitHub").on_click(|_,_,cx|cx.open_url("https://github.com/joenilan/slotshift")))
                    .child(Button::new("open-settings-folder").ghost().label("Open settings folder").on_click(cx.listener(|this,_,_,cx|{if let Err(e)=slotshift::storage::open_folder(this.store.root()){this.feedback(e.to_string(),true,cx);}}))))
                .child(div().pt_3().text_xs().text_color(rgb(p::MUTED)).child("Independent open-source software by zombie.digital. Not affiliated with or endorsed by OpenAI.")))
            .into_any_element()
    }
}
impl Render for Launcher {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = if self.page == Page::Settings {
            self.settings_page(cx)
        } else if let Some(account) = self.settings.current().cloned() {
            self.launch_page(&account, cx)
        } else {
            div()
                .size_full()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_4()
                .p_8()
                .child(p::mark())
                .child(
                    div()
                        .text_size(px(30.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Room for your next account."),
                )
                .child(p::muted(
                    "Add a new Codex login or link an existing home. No fixed slots.",
                ))
                .child(
                    Button::new("empty-add")
                        .custom(p::primary(cx))
                        .label("Add your first account")
                        .on_click(cx.listener(|this, _, w, cx| this.edit_account(false, w, cx))),
                )
                .into_any_element()
        };
        div()
            .size_full()
            .bg(rgb(p::BACKGROUND))
            .text_color(rgb(p::TEXT))
            .font_family("Segoe UI")
            .text_size(px(14.))
            .flex()
            .flex_col()
            .child(
                TitleBar::new().bg(rgb(p::SIDEBAR)).child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .w_full()
                        .px_4()
                        .child(
                            div()
                                .text_size(px(11.))
                                .text_color(rgb(p::MUTED))
                                .child("Slotshift"),
                        )
                        .child(div().text_size(px(10.)).text_color(rgb(p::MUTED)).child(
                            if self.demo {
                                "DEMO WORKSPACE"
                            } else {
                                "LOCAL ACCOUNT LAUNCHER"
                            },
                        )),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.sidebar(cx))
                    .child(div().flex_1().min_w_0().h_full().child(body)),
            )
            .child(
                div()
                    .min_h(px(36.))
                    .px_5()
                    .py_2()
                    .bg(rgb(p::SIDEBAR))
                    .border_t_1()
                    .border_color(rgb(p::BORDER))
                    .flex()
                    .gap_3()
                    .items_center()
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(rgb(if self.error { p::ERROR } else { p::MUTED }))
                            .flex_1()
                            .child(self.notice.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(p::MUTED))
                            .flex_shrink_0()
                            .child("RUST + GPUI"),
                    ),
            )
    }
}
