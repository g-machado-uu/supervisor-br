//! Small reusable input widgets, including a date picker in Portuguese.

use egui::AsIdSalt;

use chrono::{Datelike, Duration, Local, Months, NaiveDate, NaiveTime, Timelike};
use egui::{Color32, RichText, Ui, WidgetInfo};

use crate::model::{Meeting, Platform, Repeat, ReplyStatus, Settings, TriState};
use crate::pt;
use crate::ui::{icons, theme};

pub fn today() -> NaiveDate {
    Local::now().date_naive()
}

pub fn fmt_date(date: Option<NaiveDate>) -> String {
    date.map_or_else(|| "—".into(), pt::date)
}

pub fn fmt_short(date: NaiveDate) -> String {
    pt::day_month(date)
}

/// Which month the open picker shows.
#[derive(Clone, Copy)]
struct PickerState {
    month: NaiveDate,
}

/// A day in the picker, and the gap between days.
const DAY: egui::Vec2 = egui::vec2(36.0, 26.0);
const DAY_GAP: f32 = 2.0;
/// The width of the picker: a week of days.
const WEEK_WIDTH: f32 = 7.0 * DAY.x + 6.0 * DAY_GAP;

fn first_of_month(date: NaiveDate) -> NaiveDate {
    date.with_day(1).unwrap_or(date)
}

/// A button showing `date` that opens a month calendar; clicking a day
/// picks it. Days are labelled in full ("ter, 03 nov 2026") for screen readers.
pub fn date_picker(ui: &mut Ui, date: &mut NaiveDate) -> egui::Response {
    let id = ui.make_persistent_id("date-picker");
    let mut state: Option<PickerState> = ui.data(|d| d.get_temp(id));
    let mut button = ui.add(egui::Button::new(format!("{}  {}", pt::date(*date), icons::CALENDAR_BLANK)));
    if button.clicked() {
        state = match state {
            Some(_) => None,
            None => Some(PickerState {
                month: first_of_month(*date),
            }),
        };
    }
    if let Some(st) = &mut state {
        let width = WEEK_WIDTH;
        let mut pos = button.rect.left_bottom() + egui::vec2(0.0, 4.0);
        if pos.x + width + 24.0 > ui.ctx().content_rect().right() {
            pos.x = button.rect.right() - width - 24.0;
        }
        let mut picked = None;
        let mut close = false;
        let area = egui::Area::new(id.with("popup"))
            .kind(egui::UiKind::Picker)
            .order(egui::Order::Foreground)
            .fixed_pos(pos)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.set_width(width);
                    picked = ui.scope(|ui| month_grid(ui, st, *date)).inner;
                    ui.horizontal(|ui| {
                        if ui.button("Hoje").clicked() {
                            picked = Some(today());
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Fechar").clicked() {
                                close = true;
                            }
                        });
                    });
                });
            });
        if let Some(day) = picked {
            *date = day;
            button.mark_changed();
            close = true;
        }
        if close
            || ui.input(|i| i.key_pressed(egui::Key::Escape))
            || (area.response.clicked_elsewhere() && !button.clicked())
        {
            state = None;
        }
    }
    ui.data_mut(|d| match state {
        Some(st) => {
            d.insert_temp(id, st);
        }
        None => d.remove::<PickerState>(id),
    });
    button
}

/// Month and year arrows, the weekday initials and the days of the month.
fn month_grid(ui: &mut Ui, st: &mut PickerState, selected: NaiveDate) -> Option<NaiveDate> {
    let mut picked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = DAY_GAP;
        let arrow = egui::vec2(26.0, 22.0);
        let nav = |ui: &mut Ui, text: &str, hover: &str| {
            ui.add_sized(arrow, egui::Button::new(text).small()).on_hover_text(hover).clicked()
        };
        if nav(ui, "«", "Ano anterior") {
            st.month = st.month - Months::new(12);
        }
        if nav(ui, icons::CARET_LEFT, "Mês anterior") {
            st.month = st.month - Months::new(1);
        }
        // The month gets what is left of the width of the days below.
        let width = WEEK_WIDTH - 4.0 * (arrow.x + DAY_GAP);
        ui.allocate_ui_with_layout(
            egui::vec2(width, arrow.y),
            egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
            |ui| ui.add(egui::Label::new(theme::semibold(pt::month_year(st.month))).truncate()),
        );
        if nav(ui, icons::CARET_RIGHT, "Próximo mês") {
            st.month = st.month + Months::new(1);
        }
        if nav(ui, "»", "Próximo ano") {
            st.month = st.month + Months::new(12);
        }
    });
    ui.spacing_mut().item_spacing = egui::vec2(DAY_GAP, DAY_GAP);
    ui.horizontal(|ui| {
        for d in pt::WEEKDAY_INITIALS {
            ui.add_sized(DAY, egui::Label::new(RichText::new(d).small().color(theme::muted(ui))));
        }
    });
    let first = st.month - Duration::days(i64::from(st.month.weekday().num_days_from_monday()));
    let today = today();
    // The theme's padding would make two-digit days wider than the cell.
    ui.spacing_mut().button_padding = egui::vec2(2.0, 2.0);
    for week in 0..6 {
        ui.horizontal(|ui| {
            for day in 0..7 {
                let date = first + Duration::days(week * 7 + day);
                let in_month = date.month() == st.month.month();
                let is_selected = date == selected;
                let mut text = RichText::new(date.day().to_string());
                if is_selected {
                    text = text.color(Color32::WHITE);
                } else if !in_month {
                    text = text.color(theme::muted(ui));
                }
                let mut button = egui::Button::new(text).min_size(DAY).corner_radius(egui::CornerRadius::same(6));
                button = if is_selected {
                    button.fill(theme::ACCENT)
                } else {
                    button.fill(Color32::TRANSPARENT)
                };
                if date == today && !is_selected {
                    button = button.stroke(egui::Stroke::new(1.0, theme::fg(ui, theme::ACCENT)));
                } else if !is_selected {
                    button = button.stroke(egui::Stroke::NONE);
                }
                let resp = ui.add_sized(DAY, button);
                resp.widget_info(|| WidgetInfo::labeled(egui::Role::Button, true, pt::date(date)));
                if resp.clicked() {
                    picked = Some(date);
                }
            }
        });
    }
    picked
}

/// An optional date: a "Definir data" button when empty, otherwise a picker plus clear button.
pub fn opt_date(ui: &mut Ui, salt: impl AsIdSalt, value: &mut Option<NaiveDate>) {
    ui.push_id(salt, |ui| {
        ui.horizontal(|ui| match value {
            Some(date) => {
                date_picker(ui, date);
                if ui.small_button(icons::X).on_hover_text("Limpar data").clicked() {
                    *value = None;
                }
            }
            None => {
                if ui.button(format!("{} Definir data", icons::CALENDAR_BLANK)).clicked() {
                    *value = Some(today());
                }
            }
        });
    });
}

/// An optional time of day, edited as hour/minute drag values.
pub fn opt_time(ui: &mut Ui, salt: impl AsIdSalt, value: &mut Option<NaiveTime>) {
    ui.push_id(salt, |ui| {
        ui.horizontal(|ui| match value {
            Some(time) => {
                let mut h = time.hour();
                let mut m = time.minute();
                ui.add(egui::DragValue::new(&mut h).range(0..=23).custom_formatter(|v, _| format!("{v:02}")))
                    .on_hover_text("Hora");
                ui.label(":");
                ui.add(
                    egui::DragValue::new(&mut m)
                        .range(0..=59)
                        .speed(0.3)
                        .custom_formatter(|v, _| format!("{v:02}")),
                )
                .on_hover_text("Minutos");
                *time = NaiveTime::from_hms_opt(h, m, 0).unwrap_or(*time);
                if ui.small_button(icons::X).on_hover_text("Limpar horário").clicked() {
                    *value = None;
                }
            }
            None => {
                if ui.button(format!("{} Definir horário", icons::CLOCK)).clicked() {
                    *value = NaiveTime::from_hms_opt(10, 0, 0);
                }
            }
        });
    });
}

pub fn tri_state(ui: &mut Ui, value: &mut TriState) {
    ui.horizontal(|ui| {
        ui.selectable_value(value, TriState::Unknown, "Não sei");
        ui.selectable_value(value, TriState::Yes, "Sim");
        ui.selectable_value(value, TriState::No, "Não");
    });
}

pub fn reply_status(ui: &mut Ui, value: &mut ReplyStatus) {
    ui.horizontal(|ui| {
        for status in [ReplyStatus::Awaiting, ReplyStatus::Replied, ReplyStatus::NoReply] {
            ui.selectable_value(value, status, status.label());
        }
    });
}

pub fn notes(ui: &mut Ui, text: &mut String, hint: &str) {
    ui.add(
        egui::TextEdit::multiline(text)
            .hint_text(hint)
            .desired_rows(3)
            .desired_width(f32::INFINITY),
    );
}

/// Narrowest a single-line text field is drawn, so it never shrinks to a sliver.
pub const MIN_FIELD_WIDTH: f32 = 160.0;

pub fn text_field(ui: &mut Ui, text: &mut String, hint: &str) -> egui::Response {
    let width = ui.available_width().max(MIN_FIELD_WIDTH);
    ui.add(egui::TextEdit::singleline(text).hint_text(hint).desired_width(width))
}

/// Shows `edit` as a masked field. egui keeps an unmasked undo history of
/// every text field, so for passwords it is wiped after each frame.
pub fn password_field(ui: &mut Ui, edit: egui::TextEdit<'_>) -> egui::Response {
    let mut out = edit.password(true).show(ui);
    out.state.clear_undoer();
    out.state.store(ui.ctx(), out.response.id);
    out.response.response
}

/// A subtle inset frame for a meeting or visit inside a card.
pub fn inner_frame(ui: &Ui) -> egui::Frame {
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(egui::CornerRadius::same(12))
        .inner_margin(egui::Margin::same(12))
}

/// Two-column form grid with consistent spacing.
pub fn form_grid(ui: &mut Ui, id: impl AsIdSalt, add: impl FnOnce(&mut Ui)) {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([12.0, 8.0])
        .min_col_width(130.0)
        .show(ui, add);
}

/// A label for the left column of a form grid, wrapped so long ones don't
/// push the fields out of the panel.
pub fn field_label(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(text).wrap_mode(egui::TextWrapMode::Wrap));
}

/// Button for saving an invite file; needs a date to be set.
pub fn invite_button(ui: &mut Ui, has_date: bool) -> bool {
    ui.add_enabled(has_date, egui::Button::new(format!("{} Convite…", icons::PAPER_PLANE_TILT)).small())
        .on_hover_text("Salva um .ics com o aluno como convidado, para importar no Outlook ou no Google Agenda e enviar")
        .on_disabled_hover_text("Defina uma data primeiro")
        .clicked()
}

/// "S5 · seg, 26 out" style label for a date (S = semana letiva).
pub fn week_label(settings: &Settings, date: NaiveDate) -> String {
    match settings.teaching_week(date) {
        Some((_, week)) => format!("S{week} · {}", pt::weekday_day_month(date)),
        None => pt::weekday_day_month(date),
    }
}

/// Editable list of meetings: date, time, online (Meet, Teams, Zoom, RNP) or
/// room, repetition and notes. `first_label` names the first entry (e.g.
/// "Reunião do semestre"); later entries become follow-ups. Returns the UID
/// of a meeting whose invite button was clicked.
pub fn meetings_editor(
    ui: &mut Ui,
    salt: impl AsIdSalt,
    meetings: &mut Vec<Meeting>,
    first_label: Option<&str>,
    settings: &Settings,
) -> Option<String> {
    let mut invite = None;
    ui.push_id(salt, |ui| {
        let mut remove = None;
        for (i, m) in meetings.iter_mut().enumerate() {
            let title = match (first_label, i) {
                (Some(label), 0) => label.to_string(),
                (Some(_), n) => format!("Retorno {n}"),
                (None, n) => format!("Reunião {}", n + 1),
            };
            ui.push_id(m.id, |ui| {
                inner_frame(ui).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(theme::semibold(title));
                        if let Some(r) = &m.repeat {
                            theme::chip(ui, every_label(r.every_weeks), theme::ACCENT);
                        } else if m.held {
                            theme::chip(ui, "realizada", theme::GREEN);
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button(icons::TRASH).on_hover_text("Remover reunião").clicked() {
                                remove = Some(i);
                            }
                            if invite_button(ui, m.date.is_some()) {
                                invite = Some(m.uid());
                            }
                        });
                    });
                    ui.horizontal_wrapped(|ui| {
                        opt_date(ui, "date", &mut m.date);
                        opt_time(ui, "time", &mut m.time);
                        if m.repeat.is_none() {
                            ui.checkbox(&mut m.held, "Realizada");
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.checkbox(&mut m.online, format!("{} On-line", icons::DESKTOP));
                        if m.online {
                            for platform in Platform::ALL {
                                ui.selectable_value(&mut m.platform, platform, platform.label())
                                    .on_hover_text(platform.long_label());
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        if m.online {
                            ui.label(RichText::new(icons::LINK).color(theme::muted(ui)));
                            let hint = match m.platform {
                                Platform::Teams => "Link da reunião (opcional; senão, o das Configurações)",
                                Platform::GoogleMeet => "https://meet.google.com/…",
                                Platform::Zoom => "https://…zoom.us/j/…",
                                Platform::Rnp => "https://conferenciaweb.rnp.br/…",
                            };
                            text_field(ui, &mut m.link, hint);
                        } else {
                            ui.label(RichText::new(icons::MAP_PIN).color(theme::muted(ui)));
                            text_field(ui, &mut m.location, "Sala / local");
                        }
                    });
                    repeat_editor(ui, m, settings);
                    notes(ui, &mut m.notes, "Anotações da reunião…");
                });
            });
        }
        if let Some(i) = remove {
            meetings.remove(i);
        }
        let add_label = match (first_label, meetings.is_empty()) {
            (Some(label), true) => format!("{} Adicionar {}", icons::PLUS, label.to_lowercase()),
            (Some(_), false) => format!("{} Adicionar retorno", icons::PLUS),
            (None, _) => format!("{} Adicionar reunião", icons::PLUS),
        };
        if ui.button(add_label).clicked() {
            meetings.push(Meeting::default());
        }
    });
    invite
}

fn every_label(weeks: u8) -> String {
    match weeks {
        0 | 1 => format!("{} semanal", icons::REPEAT),
        2 => format!("{} quinzenal", icons::REPEAT),
        n => format!("{} a cada {n} semanas", icons::REPEAT),
    }
}

/// The "Repete" controls and, for a repeating meeting, its list of weeks.
fn repeat_editor(ui: &mut Ui, m: &mut Meeting, settings: &Settings) {
    let teaching_end = m
        .date
        .and_then(|d| settings.teaching_end(settings.semester_of(d)));
    ui.horizontal(|ui| {
        let mut on = m.repeat.is_some();
        if ui.checkbox(&mut on, format!("{} Repete", icons::REPEAT)).changed() {
            m.repeat = on.then(Repeat::default);
        }
        let Some(r) = &mut m.repeat else { return };
        egui::ComboBox::from_id_salt("every")
            .selected_text(match r.every_weeks {
                0 | 1 => "toda semana".to_string(),
                n => format!("a cada {n} semanas"),
            })
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut r.every_weeks, 1, "toda semana");
                for n in 2..=4u8 {
                    ui.selectable_value(&mut r.every_weeks, n, format!("a cada {n} semanas"));
                }
            });
    });
    let date = m.date;
    if let Some(r) = &mut m.repeat {
        ui.horizontal_wrapped(|ui| {
            ui.label("até");
            match r.until {
                None => {
                    let text = match teaching_end {
                        Some(end) => format!("o fim das aulas ({})", fmt_short(end)),
                        None => "o fim das aulas".into(),
                    };
                    ui.label(RichText::new(text).strong());
                    if ui.small_button("Alterar…").on_hover_text("Terminar em outra data").clicked() {
                        r.until = Some(teaching_end.or(date).unwrap_or_else(today));
                    }
                }
                Some(_) => {
                    opt_date(ui, "until", &mut r.until);
                    ui.label(
                        RichText::new(format!("({} = fim das aulas)", icons::X))
                            .small()
                            .color(theme::muted(ui)),
                    );
                }
            }
        });
    }
    if m.repeat.is_none() {
        return;
    }
    if m.date.is_none() {
        ui.label(
            RichText::new("Defina a primeira data para ver as semanas.")
                .small()
                .color(theme::muted(ui)),
        );
        return;
    }
    if m.repeat_end(settings).is_none() {
        ui.colored_label(
            theme::fg(ui, theme::AMBER),
            format!(
                "{} Defina as datas do semestre em Configurações (ou uma data final) – só a primeira semana é planejada.",
                icons::WARNING
            ),
        );
    }

    let weeks = m.occurrences(settings);
    let cancelled = weeks.iter().filter(|o| o.cancelled).count();
    let held = weeks.iter().filter(|o| o.held).count();
    let mut header = format!("Semanas ({}", weeks.len());
    if held > 0 {
        header.push_str(&format!(", {held} realizadas"));
    }
    if cancelled > 0 {
        header.push_str(&format!(", {cancelled} canceladas"));
    }
    header.push(')');
    let mut changed = Vec::new();
    egui::CollapsingHeader::new(header)
        .id_salt("weeks")
        .show(ui, |ui| {
            egui::Grid::new("weeks-grid")
                .num_columns(5)
                .striped(true)
                .spacing([10.0, 4.0])
                .show(ui, |ui| {
                    for h in ["Prevista", "Realizada", "Cancelar", "Remarcada para", "Notas"] {
                        ui.label(RichText::new(h).small().color(theme::muted(ui)));
                    }
                    ui.end_row();
                    for occ in weeks {
                        let mut edited = occ.clone();
                        let mut planned = RichText::new(week_label(settings, occ.planned));
                        if occ.cancelled {
                            planned = planned.strikethrough().color(theme::muted(ui));
                        }
                        ui.label(planned);
                        ui.push_id((occ.planned, "held"), |ui| ui.checkbox(&mut edited.held, ""));
                        ui.push_id((occ.planned, "cancel"), |ui| ui.checkbox(&mut edited.cancelled, ""));
                        ui.push_id((occ.planned, "move"), |ui| {
                            ui.horizontal(|ui| {
                                if edited.moved_to.is_none() && edited.time.is_none() {
                                    if ui.small_button("Remarcar…").clicked() {
                                        edited.moved_to = Some(occ.planned);
                                        edited.time = m.time;
                                    }
                                } else {
                                    opt_date(ui, "date", &mut edited.moved_to);
                                    opt_time(ui, "time", &mut edited.time);
                                }
                            });
                        });
                        ui.push_id((occ.planned, "notes"), |ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut edited.notes)
                                    .hint_text("Notas")
                                    .desired_width(140.0),
                            );
                        });
                        ui.end_row();
                        if edited != occ {
                            changed.push(edited);
                        }
                    }
                });
        });
    for occ in changed {
        m.set_occurrence(occ);
    }
}
