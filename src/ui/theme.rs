//! Look & feel: a pastel take on the colours of the Brazilian flag (cream
//! with a hint of yellow, soft green, indigo blue), bundled fonts and icons,
//! and small painted components shared by screens.

use std::sync::Arc;

use egui::{
    Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Margin, Pos2,
    Rect, Response, RichText, Sense, Shadow, Stroke, TextStyle, Theme, ThemePreference, Ui, Vec2,
    Visuals, WidgetInfo,
};

use crate::model::EventKind;
use crate::ui::icons;

// Status and accent colours, tuned for text on cream. In dark mode they are
// lightened where they're drawn (see `fg`).
/// Indigo blue of the flag's globe: primary actions, TCC meetings, links.
pub const ACCENT: Color32 = Color32::from_rgb(0x33, 0x60, 0x9a);
/// Flag green: done, held, submitted; tutoria.
pub const GREEN: Color32 = Color32::from_rgb(0x36, 0x72, 0x49);
/// Deep gold of the flag's rhombus: waiting on something; estágio visits.
pub const AMBER: Color32 = Color32::from_rgb(0x8c, 0x60, 0x0f);
/// Pau-brasil red: overdue, no reply, destructive.
pub const RED: Color32 = Color32::from_rgb(0xa2, 0x46, 0x3f);
/// Olive grey: secondary text and neutral states; reminders.
pub const GREY: Color32 = Color32::from_rgb(0x6d, 0x6f, 0x62);
/// Jacarandá purple: postgraduate students.
pub const PLUM: Color32 = Color32::from_rgb(0x6f, 0x5a, 0x92);
/// Teal: iniciação científica.
pub const TEAL: Color32 = Color32::from_rgb(0x2b, 0x6f, 0x70);

// Flag colours, softened, for the little flag in the logo.
const FLAG_GREEN: Color32 = Color32::from_rgb(0x2e, 0x9a, 0x5c);
const FLAG_YELLOW: Color32 = Color32::from_rgb(0xf6, 0xd3, 0x4a);
const FLAG_BLUE: Color32 = Color32::from_rgb(0x2a, 0x4b, 0x8d);

// Light surfaces.
const CREAM: Color32 = Color32::from_rgb(0xf5, 0xf2, 0xe3);
const IVORY: Color32 = Color32::from_rgb(0xff, 0xfd, 0xf6);
/// Pastel green of the sidebar.
const MINT: Color32 = Color32::from_rgb(0xcf, 0xe4, 0xd2);
/// Pastel blue of selections.
const POWDER: Color32 = Color32::from_rgb(0xcd, 0xda, 0xee);
const SAND: Color32 = Color32::from_rgb(0xee, 0xea, 0xdb);
const LINE: Color32 = Color32::from_rgb(0xe2, 0xdd, 0xc6);
const INK: Color32 = Color32::from_rgb(0x2c, 0x38, 0x30);

const DARK_BG: Color32 = Color32::from_rgb(0x17, 0x1f, 0x1a);
const DARK_CARD: Color32 = Color32::from_rgb(0x20, 0x29, 0x23);
const DARK_SIDEBAR: Color32 = Color32::from_rgb(0x1e, 0x33, 0x27);
const DARK_RAISED: Color32 = Color32::from_rgb(0x2b, 0x35, 0x2e);
const DARK_LINE: Color32 = Color32::from_rgb(0x36, 0x42, 0x3a);
const DARK_INK: Color32 = Color32::from_rgb(0xe6, 0xea, 0xdf);

const SEMIBOLD: &str = "semibold";
const SERIF: &str = "serif";

pub fn apply(ctx: &egui::Context, dark: bool) {
    ctx.set_theme(if dark {
        ThemePreference::Dark
    } else {
        ThemePreference::Light
    });
}

/// Inter for text, DM Serif Display for titles and Phosphor for icons, in
/// front of egui's own fonts (kept as fallbacks for emoji and other scripts).
pub fn fonts() -> FontDefinitions {
    let mut fonts = FontDefinitions::default();
    for (name, bytes) in [
        ("inter", &include_bytes!("../../assets/fonts/Inter-Regular.ttf")[..]),
        ("inter-semibold", include_bytes!("../../assets/fonts/Inter-SemiBold.ttf")),
        ("dm-serif", include_bytes!("../../assets/fonts/DMSerifDisplay-Regular.ttf")),
        ("phosphor", include_bytes!("../../assets/fonts/Phosphor.ttf")),
    ] {
        fonts
            .font_data
            .insert(name.into(), Arc::new(FontData::from_static(bytes)));
    }
    let fallbacks = fonts.families[&FontFamily::Proportional].clone();
    // Phosphor maps a–z to blank ligature glyphs, so it must come after a text font.
    let family = |first: &str| {
        let mut keys = vec![first.to_string()];
        if first != "inter" {
            keys.push("inter".into());
        }
        keys.push("phosphor".into());
        keys.extend(fallbacks.iter().cloned());
        keys
    };
    fonts.families.insert(FontFamily::Proportional, family("inter"));
    fonts
        .families
        .insert(FontFamily::Name(SEMIBOLD.into()), family("inter-semibold"));
    fonts.families.insert(FontFamily::Name(SERIF.into()), family("dm-serif"));
    if let Some(mono) = fonts.families.get_mut(&FontFamily::Monospace) {
        mono.push("phosphor".into());
    }
    fonts
}

/// One-off style setup at start-up.
pub fn install(ctx: &egui::Context) {
    ctx.set_fonts(fonts());
    ctx.all_styles_mut(|style| {
        use egui::FontFamily::{Monospace, Proportional};
        style.text_styles = [
            (TextStyle::Heading, FontId::new(28.0, FontFamily::Name(SERIF.into()))),
            (TextStyle::Body, FontId::new(14.0, Proportional)),
            (TextStyle::Monospace, FontId::new(13.0, Monospace)),
            (TextStyle::Button, FontId::new(14.0, Proportional)),
            (TextStyle::Small, FontId::new(12.0, Proportional)),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(12.0, 5.0);
        style.spacing.interact_size.y = 30.0;
        style.spacing.window_margin = Margin::same(20);
        style.spacing.menu_margin = Margin::same(8);
    });
    for theme in [Theme::Light, Theme::Dark] {
        ctx.set_visuals_of(theme, visuals(theme == Theme::Dark));
    }
}

fn visuals(dark: bool) -> Visuals {
    let mut v = if dark { Visuals::dark() } else { Visuals::light() };
    let (bg, card, raised, line, ink) = if dark {
        (DARK_BG, DARK_CARD, DARK_RAISED, DARK_LINE, DARK_INK)
    } else {
        (CREAM, IVORY, SAND, LINE, INK)
    };
    let hover = if dark {
        Color32::from_rgb(0x38, 0x44, 0x3b)
    } else {
        Color32::from_rgb(0xe6, 0xe1, 0xcb)
    };
    let radius = CornerRadius::same(8);
    v.panel_fill = bg;
    v.window_fill = card;
    v.faint_bg_color = if dark {
        Color32::from_rgb(0x25, 0x2f, 0x28)
    } else {
        Color32::from_rgb(0xfa, 0xf8, 0xee)
    };
    v.extreme_bg_color = if dark { DARK_BG } else { Color32::from_rgb(0xfc, 0xfa, 0xf1) };
    v.text_edit_bg_color = Some(if dark {
        Color32::from_rgb(0x1b, 0x23, 0x1e)
    } else {
        Color32::from_rgb(0xfc, 0xfa, 0xf1)
    });
    v.code_bg_color = raised;
    v.window_stroke = Stroke::new(1.0, line);
    v.window_corner_radius = CornerRadius::same(16);
    v.menu_corner_radius = CornerRadius::same(10);
    let shadow = Color32::from_black_alpha(if dark { 90 } else { 22 });
    v.window_shadow = Shadow { offset: [0, 8], blur: 28, spread: 0, color: shadow };
    v.popup_shadow = Shadow { offset: [0, 4], blur: 14, spread: 0, color: shadow };
    v.warn_fg_color = fg_for(dark, AMBER);
    v.error_fg_color = fg_for(dark, RED);
    v.hyperlink_color = fg_for(dark, ACCENT);
    v.selection.bg_fill = if dark {
        Color32::from_rgb(0x2f, 0x45, 0x63)
    } else {
        POWDER
    };
    v.selection.stroke = Stroke::new(1.0, fg_for(dark, ACCENT));
    v.striped = false;
    v.collapsing_header_frame = false;

    let w = &mut v.widgets;
    w.noninteractive.bg_fill = card;
    w.noninteractive.weak_bg_fill = card;
    w.noninteractive.bg_stroke = Stroke::new(1.0, line);
    w.noninteractive.fg_stroke = Stroke::new(1.0, ink);
    for (state, fill, stroke) in [
        (&mut w.inactive, raised, line),
        (&mut w.hovered, hover, if dark { DARK_LINE } else { Color32::from_rgb(0xcc, 0xc5, 0xa8) }),
        (&mut w.active, hover, fg_for(dark, ACCENT)),
        (&mut w.open, hover, line),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = Stroke::new(1.0, stroke);
        state.fg_stroke = Stroke::new(1.0, ink);
        state.corner_radius = radius;
        state.expansion = 0.0;
    }
    w.noninteractive.corner_radius = radius;
    v
}

fn fg_for(dark: bool, c: Color32) -> Color32 {
    if dark {
        c.lerp_to_gamma(Color32::WHITE, 0.38)
    } else {
        c
    }
}

/// A palette colour made readable as text on the current theme.
pub fn fg(ui: &Ui, c: Color32) -> Color32 {
    fg_for(ui.visuals().dark_mode, c)
}

/// A soft wash of `c`, for chip and badge backgrounds.
pub fn soft(ui: &Ui, c: Color32) -> Color32 {
    if ui.visuals().dark_mode {
        c.gamma_multiply(0.28)
    } else {
        ui.visuals().window_fill.lerp_to_gamma(c, 0.16)
    }
}

pub fn sidebar_fill(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode { DARK_SIDEBAR } else { MINT }
}

/// Secondary text colour.
pub fn muted(ui: &Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(0xa3, 0xab, 0x9c)
    } else {
        GREY
    }
}

pub fn semibold(text: impl Into<String>) -> RichText {
    RichText::new(text).family(FontFamily::Name(SEMIBOLD.into()))
}

pub fn serif(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text).family(FontFamily::Name(SERIF.into())).size(size)
}

pub fn semibold_font(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(SEMIBOLD.into()))
}

/// Icon and colour for each kind of calendar item.
pub fn kind_style(kind: EventKind) -> (&'static str, Color32) {
    match kind {
        EventKind::TccMeeting => (icons::BOOK_OPEN_TEXT, ACCENT),
        EventKind::IcMeeting => (icons::FLASK, TEAL),
        EventKind::Visit => (icons::BUILDINGS, AMBER),
        EventKind::AdviseeMeeting => (icons::CHAT_CIRCLE_TEXT, GREEN),
        EventKind::PosMeeting => (icons::GRADUATION_CAP, PLUM),
        EventKind::Deadline => (icons::PUSH_PIN, RED),
        EventKind::Reminder => (icons::BELL, GREY),
    }
}

/// Small rounded pill with a soft coloured background, sized to its text
/// (so it doesn't stretch to fill a table row).
pub fn chip(ui: &mut Ui, text: impl Into<String>, color: Color32) -> Response {
    let text = text.into();
    let galley = ui.painter().layout_no_wrap(text.clone(), FontId::proportional(12.0), fg(ui, color));
    let size = galley.size() + egui::vec2(16.0, 6.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    resp.widget_info(|| WidgetInfo::labeled(egui::Role::Label, true, &text));
    ui.painter().rect_filled(rect, CornerRadius::same(10), soft(ui, color));
    ui.painter().galley(rect.center() - galley.size() / 2.0, galley, fg(ui, color));
    resp
}

/// The frame for modal dialogs.
pub fn modal_frame(ui: &Ui) -> egui::Frame {
    egui::Frame::popup(ui.style())
        .corner_radius(CornerRadius::same(16))
        .inner_margin(Margin::same(24))
}

/// Frame of a card: rounded, light border, on the card fill.
pub fn card_frame(ui: &Ui) -> egui::Frame {
    egui::Frame::new()
        .fill(ui.visuals().window_fill)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(CornerRadius::same(14))
        .inner_margin(Margin::same(16))
}

/// A titled card used to group sections in the detail panel and dashboard.
/// The app version shown in the sidebar, e.g. "v0.3.0".
pub const VERSION: &str = concat!("v", env!("CARGO_PKG_VERSION"));

pub fn card<R>(ui: &mut Ui, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    card_frame(ui)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if !title.is_empty() {
                card_title(ui, title);
            }
            add(ui)
        })
        .inner
}

/// Card heading in the small-caps style of the dashboard.
pub fn card_title(ui: &mut Ui, title: &str) {
    ui.label(
        semibold(title.to_uppercase())
            .size(11.5)
            .extra_letter_spacing(0.8)
            .color(muted(ui)),
    );
    ui.add_space(6.0);
}

/// Serif page title with a muted subtitle, and room for actions on the right.
pub fn page_header(ui: &mut Ui, title: &str, subtitle: &str, right: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        // The buttons first, so a long title is cut short rather than run under them.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            right(ui);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    ui.add(egui::Label::new(RichText::new(title).heading()).truncate());
                    if !subtitle.is_empty() {
                        ui.label(RichText::new(subtitle).color(muted(ui)));
                    }
                });
            });
        });
    });
    ui.add_space(14.0);
}

/// The filled indigo button used for each screen's main action.
pub fn primary_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(semibold(text).color(Color32::WHITE))
        .fill(ACCENT)
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(10))
        .min_size(egui::vec2(0.0, 34.0))
}

/// A button for destructive actions.
pub fn danger_button(text: impl Into<String>) -> egui::Button<'static> {
    egui::Button::new(semibold(text).color(Color32::WHITE))
        .fill(RED)
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(10))
        .min_size(egui::vec2(0.0, 34.0))
}

/// An icon centred in a softly tinted circle.
pub fn icon_badge(ui: &mut Ui, icon: &str, color: Color32, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter();
    painter.circle_filled(rect.center(), size / 2.0, soft(ui, color));
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        icon,
        FontId::proportional(size * 0.5),
        fg(ui, color),
    );
    resp
}

const AVATAR_COLORS: [Color32; 5] = [
    Color32::from_rgb(0xb8, 0xca, 0xe6), // pastel indigo
    Color32::from_rgb(0xf3, 0xe0, 0x9c), // pastel yellow
    Color32::from_rgb(0xb7, 0xdb, 0xbf), // pastel green
    Color32::from_rgb(0xe3, 0xb9, 0xb2), // pastel pau-brasil
    Color32::from_rgb(0xcf, 0xc3, 0xe2), // pastel jacarandá
];

fn initials(name: &str) -> String {
    let mut words = name.split_whitespace().filter(|w| w.chars().next().is_some_and(char::is_alphabetic));
    let first = words.next().and_then(|w| w.chars().next());
    let last = words.next_back().and_then(|w| w.chars().next());
    first.into_iter().chain(last).flat_map(char::to_uppercase).collect()
}

/// A circle with the person's initials, its colour fixed by the name.
pub fn avatar(ui: &mut Ui, name: &str, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let hash = name.bytes().fold(7u32, |h, b| h.wrapping_mul(31).wrapping_add(u32::from(b)));
    let mut fill = AVATAR_COLORS[hash as usize % AVATAR_COLORS.len()];
    if ui.visuals().dark_mode {
        fill = fill.gamma_multiply(0.55);
    }
    let text = initials(name);
    let painter = ui.painter();
    painter.circle_filled(rect.center(), size / 2.0, fill);
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        if text.is_empty() { icons::STUDENT.to_string() } else { text },
        semibold_font(size * 0.38),
        if ui.visuals().dark_mode { DARK_INK } else { INK },
    );
    resp
}

/// Width of the sidebar, and its left/right padding.
pub const SIDEBAR_WIDTH: f32 = 260.0;
pub const SIDEBAR_PADDING: f32 = 14.0;

/// Room for the label of a sidebar entry `width` wide.
fn nav_label_width(width: f32, count: bool) -> f32 {
    width - 40.0 - if count { 40.0 } else { 8.0 }
}

/// A sidebar entry: icon, label and an optional count, with the selected one
/// raised as a card-coloured pill.
pub fn nav_item(ui: &mut Ui, icon: &str, label: &str, count: Option<usize>, selected: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 36.0), Sense::click());
    resp.widget_info(|| WidgetInfo::selected(egui::Role::Button, true, selected, label));
    let dark = ui.visuals().dark_mode;
    let ink = if dark { DARK_INK } else { INK };
    let fill = if selected {
        Some(ui.visuals().window_fill)
    } else if resp.hovered() {
        Some(if dark {
            Color32::from_white_alpha(10)
        } else {
            Color32::from_white_alpha(110)
        })
    } else {
        None
    };
    let painter = ui.painter();
    if let Some(fill) = fill {
        if selected {
            painter.add(
                Shadow { offset: [0, 2], blur: 8, spread: 0, color: Color32::from_black_alpha(if dark { 60 } else { 14 }) }
                    .as_shape(rect, CornerRadius::same(10)),
            );
        }
        painter.rect_filled(rect, CornerRadius::same(10), fill);
    }
    let icon_color = if selected { fg_for(dark, ACCENT) } else { ink.gamma_multiply(0.8) };
    painter.text(
        Pos2::new(rect.left() + 14.0, rect.center().y),
        Align2::LEFT_CENTER,
        icon,
        FontId::proportional(17.0),
        icon_color,
    );
    if let Some(n) = count {
        let text = n.to_string();
        let galley = painter.layout_no_wrap(text, semibold_font(11.5), ink.gamma_multiply(0.75));
        let w = (galley.size().x + 14.0).max(24.0);
        let pill = Rect::from_center_size(
            Pos2::new(rect.right() - 10.0 - w / 2.0, rect.center().y),
            egui::vec2(w, 20.0),
        );
        let pill_fill = if selected {
            soft(ui, ACCENT)
        } else if dark {
            Color32::from_white_alpha(14)
        } else {
            Color32::from_white_alpha(150)
        };
        painter.rect_filled(pill, CornerRadius::same(10), pill_fill);
        painter.galley(pill.center() - galley.size() / 2.0, galley, ink);
    }
    // The label ends before the count, with an ellipsis when it doesn't fit.
    let font = if selected { semibold_font(14.0) } else { FontId::proportional(14.0) };
    let mut job = egui::text::LayoutJob::simple_singleline(label.to_owned(), font, ink);
    job.wrap = egui::text::TextWrapping::truncate_at_width(nav_label_width(rect.width(), count.is_some()));
    let galley = painter.layout_job(job);
    painter.galley(Pos2::new(rect.left() + 40.0, rect.center().y - galley.size().y / 2.0), galley, ink);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A row of options in one rounded track, the chosen one raised.
pub fn segmented<T: PartialEq + Clone>(ui: &mut Ui, value: &mut T, options: &[(T, &str)]) -> Response {
    let dark = ui.visuals().dark_mode;
    let track = ui.visuals().widgets.inactive.bg_fill;
    egui::Frame::new()
        .fill(track)
        .corner_radius(CornerRadius::same(10))
        .inner_margin(Margin::same(3))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.horizontal(|ui| {
                for (v, label) in options {
                    let on = *value == *v;
                    let text = if on {
                        semibold(*label)
                    } else {
                        RichText::new(*label).color(muted(ui))
                    };
                    let button = egui::Button::selectable(on, text)
                        .fill(Color32::TRANSPARENT)
                        .stroke(Stroke::NONE)
                        .corner_radius(CornerRadius::same(8))
                        .min_size(egui::vec2(0.0, 28.0));
                    let resp = ui.scope(|ui| {
                        // Keep the raised segment card-coloured rather than selection blue
                        // (the selection stroke also colours the selected text).
                        let raised = if dark { DARK_RAISED } else { ui.visuals().window_fill };
                        let ink = ui.visuals().text_color();
                        ui.visuals_mut().selection.bg_fill = raised;
                        ui.visuals_mut().selection.stroke = Stroke::new(1.0, ink);
                        ui.add(button)
                    });
                    if resp.inner.clicked() {
                        *value = v.clone();
                    }
                }
            });
        })
        .response
}

/// A filter pill that toggles on and off.
pub fn toggle_pill(ui: &mut Ui, on: &mut bool, icon: &str, label: &str, color: Color32) -> Response {
    let text = RichText::new(format!("{icon}  {label}")).color(if *on { fg(ui, color) } else { muted(ui) });
    let button = egui::Button::new(text)
        .fill(if *on { soft(ui, color) } else { Color32::TRANSPARENT })
        .stroke(if *on {
            Stroke::new(1.0, fg(ui, color).gamma_multiply(0.45))
        } else {
            ui.visuals().widgets.noninteractive.bg_stroke
        })
        .corner_radius(CornerRadius::same(15));
    let resp = ui.add(button);
    if resp.clicked() {
        *on = !*on;
    }
    resp
}

/// A thin rounded progress track.
pub fn progress(ui: &mut Ui, fraction: f32, color: Color32, height: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), height), Sense::hover());
    let r = CornerRadius::same((height / 2.0) as u8);
    let painter = ui.painter();
    painter.rect_filled(rect, r, soft(ui, color));
    let mut done = rect;
    done.set_width(rect.width() * fraction.clamp(0.0, 1.0));
    if done.width() > 0.5 {
        painter.rect_filled(done, r, fg(ui, color));
    }
    resp
}

/// A donut chart of `(value, colour)` segments with a label in the middle.
pub fn ring(ui: &mut Ui, size: f32, segments: &[(f32, Color32)], centre: &str, caption: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter();
    let width = size * 0.12;
    let radius = size / 2.0 - width / 2.0;
    let c = rect.center();
    painter.circle_stroke(c, radius, Stroke::new(width, ui.visuals().widgets.inactive.bg_fill));
    let total: f32 = segments.iter().map(|(v, _)| v).sum();
    if total > 0.0 {
        let gap = if segments.iter().filter(|(v, _)| *v > 0.0).count() > 1 { 0.05 } else { 0.0 };
        let mut start = -std::f32::consts::FRAC_PI_2;
        for (value, color) in segments {
            let sweep = value / total * std::f32::consts::TAU;
            if sweep > 0.0 {
                let a0 = start + gap / 2.0;
                let a1 = start + (sweep - gap / 2.0).max(gap / 2.0 + 0.01);
                let steps = ((a1 - a0) / 0.05).ceil().max(2.0) as usize;
                let points = (0..=steps)
                    .map(|i| {
                        let a = a0 + (a1 - a0) * i as f32 / steps as f32;
                        c + radius * egui::vec2(a.cos(), a.sin())
                    })
                    .collect();
                painter.line(points, Stroke::new(width, fg(ui, *color)));
            }
            start += sweep;
        }
    }
    let ink = ui.visuals().text_color();
    painter.text(c - egui::vec2(0.0, 7.0), Align2::CENTER_CENTER, centre, semibold_font(size * 0.2), ink);
    painter.text(
        c + egui::vec2(0.0, size * 0.13),
        Align2::CENTER_CENTER,
        caption,
        FontId::proportional(11.0),
        muted(ui),
    );
    resp
}

/// A small Brazilian flag: green field, yellow rhombus and blue globe with
/// its white band, painted into `rect` (about 10:7).
pub fn flag(painter: &egui::Painter, rect: Rect) {
    let r = CornerRadius::same((rect.height() * 0.18) as u8);
    painter.rect_filled(rect, r, FLAG_GREEN);
    let c = rect.center();
    let (dx, dy) = (rect.width() * 0.40, rect.height() * 0.36);
    painter.add(egui::Shape::convex_polygon(
        vec![
            Pos2::new(c.x - dx, c.y),
            Pos2::new(c.x, c.y - dy),
            Pos2::new(c.x + dx, c.y),
            Pos2::new(c.x, c.y + dy),
        ],
        FLAG_YELLOW,
        Stroke::NONE,
    ));
    let globe = rect.height() * 0.24;
    painter.circle_filled(c, globe, FLAG_BLUE);
    // The white band across the globe.
    let band: Vec<Pos2> = (0..=12)
        .map(|i| {
            let t = -0.85 + 1.7 * i as f32 / 12.0;
            Pos2::new(c.x + globe * t, c.y - globe * 0.05 + globe * 0.25 * t * t)
        })
        .collect();
    painter.add(egui::Shape::line(band, Stroke::new((globe * 0.22).max(0.8), Color32::WHITE)));
    painter.rect_stroke(rect, r, Stroke::new(1.0, Color32::from_white_alpha(200)), egui::StrokeKind::Outside);
}

/// The app's logo: a graduation cap on an indigo tile, with a little
/// Brazilian flag at its lower-right corner.
pub fn logo(ui: &mut Ui, size: f32) -> Response {
    let (rect, resp) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    resp.widget_info(|| WidgetInfo::labeled(egui::Role::Image, true, "Logotipo do Supervisor BR"));
    let painter = ui.painter();
    let tile = rect;
    painter.rect_filled(tile, CornerRadius::same((size * 0.29) as u8), ACCENT);
    painter.text(
        tile.center() - egui::vec2(size * 0.06, size * 0.06),
        Align2::CENTER_CENTER,
        icons::GRADUATION_CAP,
        FontId::proportional(size * 0.52),
        Color32::WHITE,
    );
    // Inside the tile, so it never spills over the text next to the logo.
    let (w, inset) = (size * 0.46, size * 0.07);
    let flag_rect = Rect::from_min_size(
        Pos2::new(tile.right() - w - inset, tile.bottom() - w * 0.7 - inset),
        egui::vec2(w, w * 0.7),
    );
    flag(painter, flag_rect);
    resp
}

#[cfg(test)]
mod tests {
    /// Every non-ASCII character shown in the UI must exist in the bundled
    /// fonts, otherwise it renders as an empty box on some systems.
    #[test]
    fn ui_glyphs_are_available() {
        let sources = [
            include_str!("../app.rs"),
            include_str!("advisee.rs"),
            include_str!("calendar.rs"),
            include_str!("dashboard.rs"),
            include_str!("ic.rs"),
            include_str!("lock.rs"),
            include_str!("placement.rs"),
            include_str!("pos.rs"),
            include_str!("steps.rs"),
            include_str!("tcc.rs"),
            include_str!("../model.rs"),
            include_str!("../pt.rs"),
            include_str!("settings.rs"),
            include_str!("student_form.rs"),
            include_str!("student_list.rs"),
            include_str!("theme.rs"),
            include_str!("widgets.rs"),
        ];
        let ctx = egui::Context::default();
        ctx.set_fonts(super::fonts());
        let mut out = ctx.run_ui(Default::default(), |_| {});
        out.textures_delta.clear();
        for family in [
            egui::FontFamily::Proportional,
            egui::FontFamily::Name(super::SEMIBOLD.into()),
            egui::FontFamily::Name(super::SERIF.into()),
        ] {
            let font = egui::FontId::new(14.0, family);
            let mut missing: Vec<char> = sources
                .iter()
                .flat_map(|src| src.lines())
                .filter(|line| !line.trim_start().starts_with("//"))
                .flat_map(str::chars)
                .chain(super::icons::ALL.iter().flat_map(|i| i.chars()))
                .filter(|c| !c.is_ascii() && !ctx.fonts_mut(|f| f.has_glyph(&font, *c)))
                .collect();
            missing.dedup();
            assert!(missing.is_empty(), "missing glyphs in {:?}: {missing:?}", font.family);
        }
    }

    /// The text fonts must not have glyphs at the icon codepoints, or they
    /// would be drawn instead of the Phosphor icons (Inter ships some in the
    /// Private Use Area; the bundled copies are subset without them).
    #[test]
    fn text_fonts_leave_icons_to_phosphor() {
        let all = super::fonts();
        for text_font in ["inter", "inter-semibold", "dm-serif"] {
            let mut fonts = egui::FontDefinitions::empty();
            fonts.font_data.insert(text_font.into(), all.font_data[text_font].clone());
            fonts.families.insert(egui::FontFamily::Proportional, vec![text_font.into()]);
            let ctx = egui::Context::default();
            ctx.set_fonts(fonts);
            let mut out = ctx.run_ui(Default::default(), |_| {});
            out.textures_delta.clear();
            let font = egui::FontId::proportional(14.0);
            let shadowed: Vec<_> = super::icons::ALL
                .iter()
                .flat_map(|i| i.chars())
                .filter(|c| ctx.fonts_mut(|f| f.has_glyph(&font, *c)))
                .collect();
            assert!(shadowed.is_empty(), "{text_font} has glyphs for icons {shadowed:?}");
        }
    }

    /// The fixed sidebar entries fit next to their counts without being cut
    /// short, even in the semibold of the selected entry.
    #[test]
    fn sidebar_labels_fit() {
        let ctx = egui::Context::default();
        ctx.set_fonts(super::fonts());
        let mut out = ctx.run_ui(Default::default(), |_| {});
        out.textures_delta.clear();
        let room = super::nav_label_width(super::SIDEBAR_WIDTH - 2.0 * super::SIDEBAR_PADDING, true);
        let labels = ["Iniciação científica", "Todos os alunos", "Configurações"]
            .into_iter()
            .chain(crate::model::PosGroup::ALL.iter().map(|g| g.label()));
        for label in labels {
            let width = ctx.fonts_mut(|f| {
                f.layout_no_wrap(label.into(), super::semibold_font(14.0), egui::Color32::WHITE).size().x
            });
            assert!(width <= room, "{label:?} is {width:.0}px wide, the sidebar has room for {room:.0}px");
        }
    }

    #[test]
    fn initials_of_names() {
        assert_eq!(super::initials("Ana Souza"), "AS");
        assert_eq!(super::initials("Joana d'Ávila"), "JD");
        assert_eq!(super::initials("Íris"), "Í");
        assert_eq!(super::initials(""), "");
    }
}
