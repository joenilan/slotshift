use gpui_kit::component::{Theme, ThemeMode};
use gpui_kit::*;
pub const BACKGROUND: u32 = 0x131517;
pub const SIDEBAR: u32 = 0x0E1012;
pub const SURFACE: u32 = 0x1A1D20;
pub const BORDER: u32 = 0x2B3034;
pub const TEXT: u32 = 0xE8EBED;
pub const MUTED: u32 = 0x969FA7;
pub const ACCENT: u32 = 0xB2E5A0;
pub const AMBER: u32 = 0xE8BB74;
pub const ERROR: u32 = 0xF09B9B;
pub fn apply(cx: &mut App) {
    Theme::change(ThemeMode::Dark, None, cx);
    Theme::update(cx, |t| {
        t.font_family = "Segoe UI".into();
        t.font_size = px(14.);
        t.mono_font_family = "Cascadia Mono".into();
        t.radius = px(5.);
        t.radius_lg = px(8.);
        t.colors.background = rgb(BACKGROUND).into();
        t.colors.foreground = rgb(TEXT).into();
        t.colors.border = rgb(BORDER).into();
        t.colors.input = rgb(BORDER).into();
        t.colors.primary = rgb(ACCENT).into();
        t.colors.primary_foreground = rgb(0x162013).into();
        t.colors.primary_hover = rgb(0xC4EDB6).into();
        t.colors.primary_active = rgb(0x98CC86).into();
        t.colors.secondary = rgb(SURFACE).into();
        t.colors.secondary_foreground = rgb(TEXT).into();
        t.colors.secondary_hover = rgb(0x282D31).into();
        t.colors.secondary_active = rgb(0x32393D).into();
        t.colors.muted = rgb(SURFACE).into();
        t.colors.muted_foreground = rgb(MUTED).into();
        t.colors.accent = rgb(0x252D27).into();
        t.colors.accent_foreground = rgb(ACCENT).into();
        t.colors.ring = rgb(ACCENT).into();
        t.colors.sidebar = rgb(SIDEBAR).into();
        t.colors.sidebar_foreground = rgb(TEXT).into();
        t.colors.sidebar_border = rgb(BORDER).into();
        t.colors.sidebar_accent = rgb(0x202B23).into();
        t.colors.sidebar_accent_foreground = rgb(ACCENT).into();
    });
}
pub fn label(text: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(11.))
        .text_color(rgb(MUTED))
        .font_weight(FontWeight::SEMIBOLD)
        .child(text.into())
}
pub fn muted(text: impl Into<SharedString>) -> Div {
    div().text_sm().text_color(rgb(MUTED)).child(text.into())
}
pub fn mark() -> Div {
    div()
        .relative()
        .w(px(30.))
        .h(px(30.))
        .flex_shrink_0()
        .child(
            div()
                .absolute()
                .left(px(2.))
                .top(px(4.))
                .w(px(20.))
                .h(px(7.))
                .rounded(px(1.))
                .bg(rgb(ACCENT)),
        )
        .child(
            div()
                .absolute()
                .left(px(9.))
                .top(px(16.))
                .w(px(20.))
                .h(px(7.))
                .rounded(px(1.))
                .bg(rgb(ACCENT)),
        )
}
pub fn primary(cx: &App) -> gpui_kit::component::button::ButtonCustomVariant {
    gpui_kit::component::button::ButtonCustomVariant::new(cx)
        .color(rgb(ACCENT).into())
        .foreground(rgb(ACCENT).into())
        .hover(rgb(0xC4EDB6).into())
        .active(rgb(0x98CC86).into())
}
