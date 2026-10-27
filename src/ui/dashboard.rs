//! Home screen: counts, upcoming items, things needing attention, and a side
//! column with the month, the teaching week, TCC hand-ins and grants.

use std::collections::BTreeMap;

use chrono::{Datelike, Duration, Local, Months, NaiveDate, Timelike};
use egui::{Align2, Color32, CornerRadius, FontId, RichText, Sense, Ui};
use uuid::Uuid;

use crate::model::{self, Attention, Database, Event, EventKind, PosGroup, Severity};
use crate::pt;
use crate::ui::student_list::Category;
use crate::ui::{icons, theme, widgets};

pub enum DashAction {
    None,
    Open(Uuid),
    Goto(Category),
    ExportIcs,
    /// Show this month in the calendar.
    Month(NaiveDate),
}

const GAP: f32 = 16.0;

/// `events` and `attention` are the lists derived from `db`, built once per change.
pub fn show(ui: &mut Ui, db: &Database, events: &[Event], attention: &[Attention]) -> DashAction {
    let mut action = DashAction::None;
    let today = widgets::today();

    let first_name = db.settings.my_name.split_whitespace().next().unwrap_or("");
    let greeting = match Local::now().hour() {
        5..=11 => "Bom dia",
        12..=17 => "Boa tarde",
        _ => "Boa noite",
    };
    let title = if first_name.is_empty() {
        greeting.to_string()
    } else {
        format!("{greeting}, {first_name}")
    };
    let mut subtitle = format!("{} · Ano letivo {}", pt::long(today), db.settings.academic_year);
    if let Some((sem, week)) = db.settings.teaching_week(today) {
        subtitle.push_str(&format!(" · {}, semana {week}", sem.label()));
    }
    theme::page_header(ui, &title, &subtitle, |ui| {
        if ui
            .add(theme::primary_button(format!("{}  Exportar agenda", icons::EXPORT)))
            .on_hover_text("Reuniões, visitas e prazos em um arquivo .ics. Atenção: o arquivo exportado não é criptografado.")
            .clicked()
        {
            action = DashAction::ExportIcs;
        }
    });

    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        // Keep the cards clear of the scroll bar.
        ui.set_max_width(ui.available_width() - 12.0);
        tiles(ui, db, events, today, &mut action);
        ui.add_space(GAP);

        let width = ui.available_width();
        let side = if width > 900.0 { (width * 0.36).clamp(300.0, 380.0) } else { 0.0 };
        let main = width - side - if side > 0.0 { GAP } else { 0.0 };
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            ui.allocate_ui(egui::vec2(main, 0.0), |ui| {
                ui.set_width(main);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = GAP;
                    upcoming(ui, db, events, today, &mut action);
                    needs_attention(ui, attention, &mut action);
                    if side == 0.0 {
                        side_column(ui, db, events, today, &mut action);
                    }
                });
            });
            if side > 0.0 {
                ui.allocate_ui(egui::vec2(side, 0.0), |ui| {
                    ui.set_width(side);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = GAP;
                        side_column(ui, db, events, today, &mut action);
                    });
                });
            }
        });
        ui.add_space(12.0);
    });
    action
}

fn side_column(ui: &mut Ui, db: &Database, events: &[Event], today: NaiveDate, action: &mut DashAction) {
    mini_month(ui, events, today, action);
    teaching(ui, db, today);
    coursework(ui, db, today);
    grants(ui, db, today);
}

/// One tile per list, as many per row as fit, stretched to fill the row.
fn tiles(ui: &mut Ui, db: &Database, events: &[Event], today: NaiveDate, action: &mut DashAction) {
    let lists = db.settings.programme_lists().into_iter().map(Category::Tcc);
    // TCC, IC, mestrado and doutorado always; the other lists once they have students.
    let pos = PosGroup::ALL
        .into_iter()
        .filter(|g| g.always_shown() || Category::Pos(*g).count(db) > 0)
        .map(Category::Pos);
    let others = [Category::Placement, Category::Advisee].into_iter().filter(|c| c.count(db) > 0);
    let tiles: Vec<Category> = lists
        .chain([Category::Ic])
        .chain(pos)
        .chain(others)
        .collect();
    let width = ui.available_width();
    let fit = ((width + GAP) / (190.0 + GAP)).floor().clamp(1.0, tiles.len() as f32) as usize;
    // Balance the rows, e.g. 3 + 3 rather than 5 + 1.
    let per_row = tiles.len().div_ceil(tiles.len().div_ceil(fit));
    let tile_w = (width - GAP * (per_row - 1) as f32) / per_row as f32;
    let week_end = today + Duration::days(7);
    for (r, row) in tiles.chunks(per_row).enumerate() {
        if r > 0 {
            ui.add_space(GAP);
        }
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = GAP;
            for cat in row {
                let (icon, color, caption) = match cat {
                    Category::Tcc(list) if list == "TCC" => (icons::BOOK_OPEN_TEXT, theme::ACCENT, "Alunos de TCC".into()),
                    Category::Tcc(list) => (icons::BOOK_OPEN_TEXT, theme::ACCENT, format!("TCC – {list}")),
                    Category::Ic => (icons::FLASK, theme::TEAL, "Iniciação científica".into()),
                    Category::Pos(group) => (icons::GRADUATION_CAP, theme::PLUM, group.label().to_string()),
                    Category::Placement => (icons::BUILDINGS, theme::AMBER, "Estágio".into()),
                    Category::Advisee => (icons::CHAT_CIRCLE_TEXT, theme::GREEN, "Tutoria".into()),
                    Category::All => (icons::USERS_THREE, theme::GREY, "Todos os alunos".into()),
                };
                let this_week = events
                    .iter()
                    .filter(|e| {
                        !e.cancelled
                            && !matches!(e.kind, EventKind::Deadline | EventKind::Reminder)
                            && (today..week_end).contains(&e.date)
                            && db.student(e.student_id).is_some_and(|s| {
                                !s.archived
                                    && cat.includes(s, &db.settings)
                                    && match cat {
                                        Category::Placement => e.kind == EventKind::Visit,
                                        Category::Advisee => e.kind == EventKind::AdviseeMeeting,
                                        Category::Pos(_) => e.kind == EventKind::PosMeeting,
                                        Category::Ic => e.kind == EventKind::IcMeeting,
                                        _ => e.kind == EventKind::TccMeeting,
                                    }
                            })
                    })
                    .count();
                let note = match this_week {
                    0 => "nada nos próximos 7 dias".to_string(),
                    1 => "1 nos próximos 7 dias".to_string(),
                    n => format!("{n} nos próximos 7 dias"),
                };
                let resp = theme::card_frame(ui)
                    .inner_margin(egui::Margin::symmetric(16, 14))
                    .show(ui, |ui| {
                        ui.set_width(tile_w - 34.0);
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                theme::icon_badge(ui, icon, color, 40.0);
                                ui.vertical(|ui| {
                                    ui.spacing_mut().item_spacing.y = 0.0;
                                    ui.add(egui::Label::new(RichText::new(caption).color(theme::muted(ui))).truncate());
                                    ui.label(theme::semibold(cat.count(db).to_string()).size(26.0));
                                });
                            });
                            ui.add_space(2.0);
                            ui.add(egui::Label::new(RichText::new(note).small().color(theme::muted(ui))).truncate());
                        });
                    })
                    .response
                    .interact(Sense::click())
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                if resp.hovered() {
                    ui.painter().rect_stroke(
                        resp.rect,
                        CornerRadius::same(14),
                        egui::Stroke::new(1.0, theme::fg(ui, color).gamma_multiply(0.6)),
                        egui::StrokeKind::Inside,
                    );
                }
                if resp.clicked() {
                    *action = DashAction::Goto(cat.clone());
                }
            }
        });
    }
}

/// A rounded date block: weekday, day number and month, tinted by kind.
fn date_badge(ui: &mut Ui, date: NaiveDate, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(48.0, 52.0), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(10), theme::soft(ui, color));
    let c = theme::fg(ui, color);
    painter.text(
        rect.center_top() + egui::vec2(0.0, 10.0),
        Align2::CENTER_CENTER,
        pt::weekday_short(date).to_uppercase(),
        FontId::proportional(10.0),
        c,
    );
    painter.text(
        rect.center() + egui::vec2(0.0, 1.0),
        Align2::CENTER_CENTER,
        date.day().to_string(),
        FontId::new(19.0, egui::FontFamily::Name("semibold".into())),
        c,
    );
    painter.text(
        rect.center_bottom() - egui::vec2(0.0, 10.0),
        Align2::CENTER_CENTER,
        pt::month_short(date).to_uppercase(),
        FontId::proportional(10.0),
        c,
    );
}

fn upcoming(ui: &mut Ui, db: &Database, events: &[Event], today: NaiveDate, action: &mut DashAction) {
    let days = db.settings.upcoming_days;
    let items = model::upcoming(events, today, days);
    theme::card(ui, &format!("Próximos {days} dias"), |ui| {
        if items.is_empty() {
            empty(ui, icons::CALENDAR_CHECK, "Nada agendado.");
        }
        for (i, e) in items.iter().enumerate() {
            if i > 0 {
                ui.separator();
            }
            let (icon, color) = theme::kind_style(e.kind);
            let row = ui
                .horizontal(|ui| {
                    date_badge(ui, e.date, color);
                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 3.0;
                        ui.add_space(4.0);
                        ui.add(egui::Label::new(theme::semibold(&e.title)).truncate());
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing.x = 12.0;
                            let muted = theme::muted(ui);
                            let when = match e.time {
                                Some(t) => format!("{} {}", icons::CLOCK, t.format("%H:%M")),
                                None => format!("{} dia todo", icons::CLOCK),
                            };
                            ui.label(RichText::new(when).small().color(muted));
                            ui.label(RichText::new(format!("{icon} {}", kind_name(e.kind))).small().color(theme::fg(ui, color)));
                            if e.online {
                                ui.label(RichText::new(format!("{} {}", icons::DESKTOP, e.platform.label())).small().color(muted));
                            } else if !e.location.is_empty() {
                                ui.label(RichText::new(format!("{} {}", icons::MAP_PIN, e.location)).small().color(muted));
                            }
                        });
                    });
                })
                .response
                .interact(Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text("Abrir aluno");
            if row.clicked() {
                *action = DashAction::Open(e.student_id);
            }
        }
    });
}

fn kind_name(kind: EventKind) -> &'static str {
    match kind {
        EventKind::TccMeeting => "TCC",
        EventKind::IcMeeting => "Iniciação científica",
        EventKind::Visit => "Visita de estágio",
        EventKind::AdviseeMeeting => "Tutoria",
        EventKind::PosMeeting => "Pós-graduação",
        EventKind::Deadline => "Prazo",
        EventKind::Reminder => "Lembrete",
    }
}

fn empty(ui: &mut Ui, icon: &str, text: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(8.0);
        ui.label(RichText::new(icon).size(26.0).color(theme::muted(ui)));
        ui.label(RichText::new(text).color(theme::muted(ui)));
        ui.add_space(8.0);
    });
}

fn needs_attention(ui: &mut Ui, items: &[Attention], action: &mut DashAction) {
    theme::card(ui, "Precisa de atenção", |ui| {
        if items.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(8.0);
                ui.label(RichText::new(icons::CHECK_CIRCLE).size(26.0).color(theme::fg(ui, theme::GREEN)));
                ui.label(RichText::new("Tudo em dia").color(theme::fg(ui, theme::GREEN)));
                ui.add_space(8.0);
            });
        }
        for a in items {
            let (icon, color, tag) = match a.severity {
                Severity::Overdue => (icons::WARNING_CIRCLE, theme::RED, "atrasado"),
                Severity::Pending => (icons::HOURGLASS, theme::AMBER, "pendente"),
            };
            let row = egui::Frame::new()
                .fill(ui.visuals().faint_bg_color)
                .corner_radius(CornerRadius::same(10))
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(icon).size(17.0).color(theme::fg(ui, color)));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            theme::chip(ui, tag, color);
                            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                                ui.add(egui::Label::new(&a.message).truncate());
                            });
                        });
                    });
                })
                .response
                .interact(Sense::click())
                .on_hover_cursor(egui::CursorIcon::PointingHand);
            if row.clicked() {
                *action = DashAction::Open(a.student_id);
            }
        }
    });
}

/// This month, with a dot per kind of item on each day.
fn mini_month(ui: &mut Ui, events: &[Event], today: NaiveDate, action: &mut DashAction) {
    let month = today.with_day(1).unwrap_or(today);
    let next = month + Months::new(1);
    let mut kinds: BTreeMap<NaiveDate, Vec<EventKind>> = BTreeMap::new();
    for e in events.iter().filter(|e| !e.cancelled && e.date >= month && e.date < next) {
        let k = kinds.entry(e.date).or_default();
        if !k.contains(&e.kind) {
            k.push(e.kind);
        }
    }
    theme::card(ui, "", |ui| {
        ui.horizontal(|ui| {
            ui.label(theme::serif(pt::month_year(month), 20.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::Button::new(RichText::new(icons::ARROW_RIGHT).size(15.0)).frame(false))
                    .on_hover_text("Abrir na agenda")
                    .clicked()
                {
                    *action = DashAction::Month(month);
                }
            });
        });
        ui.add_space(4.0);
        let cell = (ui.available_width() / 7.0).floor();
        let muted = theme::muted(ui);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for d in pt::WEEKDAY_INITIALS {
                let (rect, _) = ui.allocate_exact_size(egui::vec2(cell, 18.0), Sense::hover());
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, d, FontId::proportional(11.0), muted);
            }
        });
        let lead = month.weekday().num_days_from_monday() as i64;
        let days = (next - month).num_days();
        let rows = (lead + days + 6) / 7;
        let ink = ui.visuals().text_color();
        for r in 0..rows {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for c in 0..7 {
                    let (rect, resp) = ui.allocate_exact_size(egui::vec2(cell, 34.0), Sense::click());
                    let n = r * 7 + c - lead;
                    if !(0..days).contains(&n) {
                        continue;
                    }
                    let date = month + Duration::days(n);
                    let painter = ui.painter();
                    let centre = rect.center() - egui::vec2(0.0, 4.0);
                    let is_today = date == today;
                    if is_today {
                        painter.circle_filled(centre, 12.0, theme::ACCENT);
                    } else if resp.hovered() {
                        painter.circle_filled(centre, 12.0, ui.visuals().widgets.hovered.bg_fill);
                    }
                    let color = if is_today {
                        Color32::WHITE
                    } else if c >= 5 {
                        muted
                    } else {
                        ink
                    };
                    painter.text(centre, Align2::CENTER_CENTER, date.day().to_string(), FontId::proportional(12.5), color);
                    if let Some(k) = kinds.get(&date) {
                        let w = 6.0 * k.len() as f32;
                        for (i, kind) in k.iter().enumerate() {
                            let x = rect.center().x - w / 2.0 + 3.0 + 6.0 * i as f32;
                            painter.circle_filled(egui::pos2(x, rect.bottom() - 3.0), 2.0, theme::fg(ui, theme::kind_style(*kind).1));
                        }
                    }
                    if resp.on_hover_cursor(egui::CursorIcon::PointingHand).clicked() {
                        *action = DashAction::Month(month);
                    }
                }
            });
        }
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 12.0;
            for (kind, label) in [
                (EventKind::TccMeeting, "TCC"),
                (EventKind::IcMeeting, "IC"),
                (EventKind::PosMeeting, "Pós"),
                (EventKind::Visit, "Estágio"),
                (EventKind::AdviseeMeeting, "Tutoria"),
                (EventKind::Deadline, "Prazo"),
                (EventKind::Reminder, "Lembrete"),
            ] {
                let color = theme::fg(ui, theme::kind_style(kind).1);
                // Keep each dot with its name when the legend wraps.
                ui.add(egui::Label::new(RichText::new(format!("● {label}")).small().color(color)).wrap_mode(egui::TextWrapMode::Extend));
            }
        });
    });
}

/// Where we are in the teaching semester.
fn teaching(ui: &mut Ui, db: &Database, today: NaiveDate) {
    let s = &db.settings;
    theme::card(ui, "Semestre letivo", |ui| {
        let sem = s.semester_of(today);
        let (Some(start), Some(end)) = (s.semester_start(sem), s.teaching_end(sem)) else {
            ui.add(egui::Label::new(RichText::new("Defina as datas do semestre em Configurações para ver as semanas letivas.").color(theme::muted(ui))).wrap_mode(egui::TextWrapMode::Wrap));
            return;
        };
        let total = ((end - start).num_days() / 7 + 1).max(1);
        let label = sem.label();
        let (headline, fraction, note) = if today < start {
            let days = (start - today).num_days();
            (format!("O {label} começa em breve"), 0.0, format!("A semana 1 começa {} · em {days} dias", pt::weekday_day_month(start)))
        } else if today > end {
            (format!("As aulas do {label} terminaram"), 1.0, format!("Aulas encerradas em {}", pt::weekday_day_month(end)))
        } else {
            let week = (today - start).num_days() / 7 + 1;
            (
                format!("Semana {week} de {total}"),
                week as f32 / total as f32,
                format!("{label} · aulas até {}", pt::weekday_day_month(end)),
            )
        };
        ui.label(theme::semibold(headline).size(18.0));
        ui.add_space(2.0);
        theme::progress(ui, fraction, theme::ACCENT, 8.0);
        ui.add_space(2.0);
        ui.label(RichText::new(note).small().color(theme::muted(ui)));
    });
}

/// Handed in, upcoming and overdue TCC items across current students.
fn coursework(ui: &mut Ui, db: &Database, today: NaiveDate) {
    let (mut done, mut due, mut overdue) = (0usize, 0usize, 0usize);
    for f in db.students.iter().filter(|s| !s.archived).filter_map(|s| s.tcc.as_ref()) {
        for module in db.settings.modules_for(&f.programme) {
            for cw in &module.courseworks {
                let status = f.coursework.get(&cw.id);
                if status.is_some_and(|s| s.submitted) {
                    done += 1;
                } else if db.settings.effective_deadline(module, cw, status).is_some_and(|d| d < today) {
                    overdue += 1;
                } else {
                    due += 1;
                }
            }
        }
    }
    theme::card(ui, "Entregas de TCC", |ui| {
        let total = done + due + overdue;
        if total == 0 {
            ui.add(egui::Label::new(RichText::new("Nenhuma entrega com prazo para os alunos de TCC.").color(theme::muted(ui))).wrap_mode(egui::TextWrapMode::Wrap));
            return;
        }
        ui.horizontal(|ui| {
            let pct = format!("{}%", (done * 100 + total / 2) / total);
            theme::ring(
                ui,
                104.0,
                &[(done as f32, theme::GREEN), (due as f32, theme::ACCENT), (overdue as f32, theme::RED)],
                &pct,
                "entregues",
            );
            ui.add_space(8.0);
            ui.vertical(|ui| {
                ui.add_space(10.0);
                for (n, label, color) in [
                    (done, "Entregues", theme::GREEN),
                    (due, "A entregar", theme::ACCENT),
                    (overdue, "Atrasadas", theme::RED),
                ] {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("●").color(theme::fg(ui, color)));
                        ui.label(label);
                        ui.label(theme::semibold(n.to_string()));
                    });
                }
            });
        });
    });
}

/// Students with a grant, by agency, and how many grants end within 90 days.
fn grants(ui: &mut Ui, db: &Database, today: NaiveDate) {
    let mut by_agency: BTreeMap<String, usize> = BTreeMap::new();
    let mut ending = 0;
    let active_pos = |s: &&crate::model::Student| s.pos.as_ref().is_none_or(|p| p.status.active());
    for s in db.students.iter().filter(|s| !s.archived).filter(active_pos) {
        let grants = s.ic.iter().map(|i| &i.grant).chain(s.pos.iter().map(|p| &p.grant));
        for g in grants.filter(|g| g.active() && g.end().is_none_or(|end| end >= today)) {
            *by_agency.entry(g.agency_label()).or_default() += 1;
            if g.end().is_some_and(|end| (end - today).num_days() <= 90) {
                ending += 1;
            }
        }
    }
    theme::card(ui, "Bolsas", |ui| {
        if by_agency.is_empty() {
            ui.add(
                egui::Label::new(RichText::new("Nenhum bolsista ativo.").color(theme::muted(ui)))
                    .wrap_mode(egui::TextWrapMode::Wrap),
            );
            return;
        }
        let total: usize = by_agency.values().sum();
        ui.horizontal(|ui| {
            theme::icon_badge(ui, icons::COINS, theme::GREEN, 36.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(theme::semibold(pt::count(total, "bolsista", "bolsistas")).size(17.0));
                let note = match ending {
                    0 => "nenhuma bolsa termina nos próximos 90 dias".to_string(),
                    1 => "1 bolsa termina nos próximos 90 dias".to_string(),
                    n => format!("{n} bolsas terminam nos próximos 90 dias"),
                };
                let color = if ending > 0 { theme::fg(ui, theme::AMBER) } else { theme::muted(ui) };
                ui.add(egui::Label::new(RichText::new(note).small().color(color)).wrap_mode(egui::TextWrapMode::Wrap));
            });
        });
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            for (agency, n) in &by_agency {
                theme::chip(ui, format!("{agency} · {n}"), theme::GREEN);
            }
        });
    });
}
