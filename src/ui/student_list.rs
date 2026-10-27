//! Category list views with status chips.

use chrono::NaiveDate;
use egui::{Color32, RichText, Sense, Ui};
use egui_extras::{Column, TableBuilder};
use uuid::Uuid;

use crate::model::{
    Database, Grant, Meeting, PosGroup, ReplyStatus, SemesterContact, Settings, Step, Student, TccRecord, Visit,
    masked,
};
use crate::pt;
use crate::ui::{icons, theme, widgets};

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Category {
    /// TCC students on the courses shown in this sidebar list.
    Tcc(String),
    Ic,
    Pos(PosGroup),
    Placement,
    Advisee,
    All,
}

impl Category {
    pub fn title(&self) -> String {
        match self {
            Category::Tcc(list) if list == "TCC" => "Alunos de TCC".into(),
            Category::Tcc(list) => format!("Alunos de TCC – {list}"),
            Category::Ic => "Iniciação científica".into(),
            Category::Pos(group) => group.label().to_string(),
            Category::Placement => "Estágio".into(),
            Category::Advisee => "Tutoria".into(),
            Category::All => "Todos os alunos".into(),
        }
    }

    pub fn includes(&self, s: &Student, settings: &Settings) -> bool {
        match self {
            Category::Tcc(list) => s.tcc.as_ref().is_some_and(|f| {
                settings
                    .programme(&f.programme)
                    .is_some_and(|p| p.list_name() == list)
            }),
            Category::Ic => s.ic.is_some(),
            Category::Pos(group) => s.pos.as_ref().is_some_and(|p| p.group() == *group),
            Category::Placement => s.placement.is_some(),
            Category::Advisee => s.advisee.is_some(),
            Category::All => true,
        }
    }

    pub fn count(&self, db: &Database) -> usize {
        db.students
            .iter()
            .filter(|s| !s.archived && self.includes(s, &db.settings))
            .count()
    }
}

pub struct ListState<'a> {
    pub search: &'a mut String,
    pub show_archived: &'a mut bool,
    pub selected: &'a mut Option<Uuid>,
}

/// Returns true when "Adicionar aluno" was clicked.
pub fn show(ui: &mut Ui, db: &Database, cat: &Category, st: ListState<'_>) -> bool {
    let mut add = false;
    let count = cat.count(db);
    let subtitle = match count {
        0 => "Nenhum aluno ativo".to_string(),
        n => pt::count(n, "aluno ativo", "alunos ativos"),
    };
    theme::page_header(ui, &cat.title(), &subtitle, |ui| {
        if ui
            .add(theme::primary_button(format!("{}  Adicionar aluno", icons::PLUS)))
            .on_hover_text("Ctrl+N")
            .clicked()
        {
            add = true;
        }
    });
    ui.horizontal_wrapped(|ui| {
        egui::Frame::new()
            .fill(ui.visuals().window_fill)
            .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
            .corner_radius(egui::CornerRadius::same(10))
            .inner_margin(egui::Margin::symmetric(10, 4))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(icons::MAGNIFYING_GLASS).size(16.0).color(theme::muted(ui)));
                    ui.add(
                        egui::TextEdit::singleline(st.search)
                            .hint_text("Buscar nome, matrícula, e-mail, curso, empresa, título…")
                            .frame(egui::Frame::NONE)
                            .background_color(egui::Color32::TRANSPARENT)
                            .desired_width(360.0),
                    );
                });
            });
        ui.add_space(8.0);
        ui.checkbox(st.show_archived, "Mostrar arquivados");
    });
    ui.add_space(12.0);

    let mut rows: Vec<&Student> = db
        .students
        .iter()
        .filter(|s| {
            (*st.show_archived || !s.archived)
                && cat.includes(s, &db.settings)
                && s.matches(st.search)
        })
        .collect();
    rows.sort_by_cached_key(|s| s.name.to_lowercase());

    if rows.is_empty() {
        theme::card(ui, "", |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(30.0);
                ui.label(RichText::new(icons::STUDENT).size(40.0).color(theme::muted(ui)));
                let text = if st.search.is_empty() {
                    "Nenhum aluno aqui ainda."
                } else {
                    "Nenhum aluno corresponde à busca."
                };
                ui.label(RichText::new(text).color(theme::muted(ui)).size(16.0));
                ui.add_space(30.0);
            });
        });
        return add;
    }

    let today = widgets::today();
    // Show which course when a list holds several.
    let mixed = match cat {
        Category::Tcc(list) => db.settings.programmes.iter().filter(|p| p.list_name() == list).count() > 1,
        _ => false,
    };
    let headers: &[&str] = match cat {
        Category::Tcc(_) => &["Nome", "Matrícula", "Curso", "Contato inicial", "Reuniões", "Entregas"],
        Category::Ic => &["Nome", "Matrícula", "Modalidade", "Bolsa", "Reuniões", "Próximo prazo"],
        Category::Pos(_) => &["Nome", "Matrícula", "Nível", "Bolsa", "Reuniões", "Próximo prazo"],
        Category::Placement => &["Nome", "Matrícula", "Empresa", "Visita 1", "Visita 2"],
        Category::Advisee => &["Nome", "Matrícula", "Curso", "1º semestre", "2º semestre"],
        Category::All => &["Nome", "Matrícula", "E-mail", "Curso", "Vínculos"],
    };

    theme::card_frame(ui)
        .inner_margin(egui::Margin { left: 12, right: 12, top: 8, bottom: 4 })
        .show(ui, |ui| {
            egui::ScrollArea::horizontal().show(ui, |ui| table(ui, db, cat, &rows, mixed, headers, st.selected, today));
        });
    add
}

#[allow(clippy::too_many_arguments)]
fn table(
    ui: &mut Ui,
    db: &Database,
    cat: &Category,
    rows: &[&Student],
    mixed: bool,
    headers: &[&str],
    selected: &mut Option<Uuid>,
    today: NaiveDate,
) {
    ui.spacing_mut().item_spacing.x = 6.0;
    let mut table = TableBuilder::new(ui)
        .striped(false)
        .sense(Sense::click())
        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
        .column(Column::initial(210.0).at_least(150.0).resizable(true).clip(true))
        .column(Column::initial(130.0).at_least(110.0).resizable(true).clip(true));
    for _ in 2..headers.len() - 1 {
        table = table.column(Column::initial(150.0).at_least(110.0).resizable(true).clip(true));
    }
    let hide = db.settings.hide_enrollment;
    table
        .column(Column::remainder().at_least(150.0).clip(true))
        .header(32.0, |mut header| {
            for h in headers {
                header.col(|ui| {
                    ui.label(
                        theme::semibold(h.to_uppercase())
                            .size(11.0)
                            .extra_letter_spacing(0.6)
                            .color(theme::muted(ui)),
                    );
                });
            }
        })
        .body(|body| {
            body.rows(46.0, rows.len(), |mut row| {
                let s = rows[row.index()];
                row.set_selected(*selected == Some(s.id));
                row.col(|ui| {
                    theme::avatar(ui, &s.name, 30.0);
                    let text = if s.name.is_empty() { "(sem nome)" } else { &s.name };
                    let mut rt = theme::semibold(text);
                    if s.archived {
                        rt = rt.color(theme::muted(ui));
                    }
                    ui.add(egui::Label::new(rt).truncate());
                    if s.archived {
                        ui.label(RichText::new(icons::ARCHIVE).color(theme::muted(ui)))
                            .on_hover_text("Arquivado");
                    }
                });
                row.col(|ui| {
                    let text = if hide { masked(&s.enrollment) } else { s.enrollment.clone() };
                    ui.label(RichText::new(text).color(theme::muted(ui)));
                });
                match cat {
                    Category::Tcc(_) => {
                        let f = s.tcc.as_ref().expect("filtered");
                        row.col(|ui| {
                            ui.add(egui::Label::new(&s.course).truncate());
                            if mixed {
                                theme::chip(ui, db.settings.programme_short(&f.programme), theme::ACCENT);
                            }
                        });
                        row.col(|ui| {
                            if f.initial_contact {
                                theme::chip(ui, format!("{} feito", icons::CHECK), theme::GREEN);
                            } else {
                                theme::chip(ui, format!("{} ainda não", icons::HOURGLASS), theme::AMBER);
                            }
                        });
                        row.col(|ui| meetings_chip(ui, &f.meetings, &db.settings, today));
                        row.col(|ui| coursework_chip(ui, f, &db.settings, today));
                    }
                    Category::Ic => {
                        let f = s.ic.as_ref().expect("filtered");
                        row.col(|ui| {
                            ui.label(f.modality.short());
                        });
                        row.col(|ui| grant_chip(ui, &f.grant, today));
                        row.col(|ui| meetings_chip(ui, &f.meetings, &db.settings, today));
                        row.col(|ui| next_deadline_chip(ui, f.next_step(), today));
                    }
                    Category::Pos(_) => {
                        let p = s.pos.as_ref().expect("filtered");
                        row.col(|ui| {
                            let text = match p.year_of_study(today) {
                                Some(year) => format!("{} · {year}º ano", p.level.short()),
                                None => p.level.short().to_string(),
                            };
                            ui.add(egui::Label::new(text).truncate());
                            if p.external {
                                ui.label(RichText::new(icons::USERS).color(theme::muted(ui)))
                                    .on_hover_text("Coorientação");
                            }
                        });
                        row.col(|ui| grant_chip(ui, &p.grant, today));
                        row.col(|ui| meetings_chip(ui, &p.meetings, &db.settings, today));
                        row.col(|ui| {
                            if p.status.active() {
                                next_deadline_chip(ui, p.next_step(), today);
                            } else {
                                theme::chip(ui, p.status.label(), theme::GREY);
                            }
                        });
                    }
                    Category::Placement => {
                        let p = s.placement.as_ref().expect("filtered");
                        row.col(|ui| {
                            ui.add(egui::Label::new(&p.company).truncate());
                        });
                        for v in &p.visits {
                            row.col(|ui| {
                                let (text, color) = visit_status(v);
                                theme::chip(ui, text, color);
                            });
                        }
                    }
                    Category::Advisee => {
                        let a = s.advisee.as_ref().expect("filtered");
                        row.col(|ui| {
                            ui.add(egui::Label::new(&s.course).truncate());
                        });
                        for c in [&a.s1, &a.s2] {
                            row.col(|ui| {
                                let (text, color) = semester_status(c, &db.settings, today);
                                theme::chip(ui, text, color);
                            });
                        }
                    }
                    Category::All => {
                        row.col(|ui| {
                            ui.add(egui::Label::new(&s.email).truncate());
                        });
                        row.col(|ui| {
                            ui.add(egui::Label::new(&s.course).truncate());
                        });
                        row.col(|ui| {
                            if s.tcc.is_some() {
                                theme::chip(ui, "TCC", theme::ACCENT);
                            }
                            if s.ic.is_some() {
                                theme::chip(ui, "IC", theme::TEAL);
                            }
                            if let Some(p) = &s.pos {
                                theme::chip(ui, p.level.short(), theme::PLUM);
                            }
                            if s.placement.is_some() {
                                theme::chip(ui, "Estágio", theme::AMBER);
                            }
                            if s.advisee.is_some() {
                                theme::chip(ui, "Tutoria", theme::GREEN);
                            }
                        });
                    }
                }
                if row.response().clicked() {
                    *selected = Some(s.id);
                }
            });
        });
}

/// Next planned meeting on or after `today` and how many have been held,
/// counting each week of repeating meetings.
fn meeting_summary(meetings: &[Meeting], settings: &Settings, today: NaiveDate) -> (Option<NaiveDate>, usize) {
    let weeks: Vec<_> = meetings
        .iter()
        .flat_map(|m| m.occurrences(settings))
        .filter(|o| !o.cancelled)
        .collect();
    let next = weeks
        .iter()
        .filter(|o| !o.held && o.date() >= today)
        .map(|o| o.date())
        .min();
    (next, weeks.iter().filter(|o| o.held).count())
}

fn meetings_chip(ui: &mut Ui, meetings: &[Meeting], settings: &Settings, today: NaiveDate) {
    let (next, held) = meeting_summary(meetings, settings, today);
    match next {
        Some(d) => theme::chip(ui, format!("próx. {}", widgets::fmt_short(d)), theme::ACCENT),
        None => theme::chip(ui, "nenhuma marcada", theme::GREY),
    };
    ui.label(RichText::new(pt::count(held, "realizada", "realizadas")).small().color(theme::muted(ui)));
}

fn coursework_chip(ui: &mut Ui, f: &TccRecord, settings: &Settings, today: NaiveDate) {
    let mut total = 0;
    let mut done = 0;
    let mut overdue = 0;
    for module in settings.modules_for(&f.programme) {
        for cw in &module.courseworks {
            total += 1;
            let status = f.coursework.get(&cw.id);
            if status.is_some_and(|s| s.submitted) {
                done += 1;
            } else if settings.effective_deadline(module, cw, status).is_some_and(|d| d < today) {
                overdue += 1;
            }
        }
    }
    if total == 0 {
        theme::chip(ui, "sem entregas", theme::GREY);
        return;
    }
    let color = if overdue > 0 {
        theme::RED
    } else if done == total {
        theme::GREEN
    } else {
        theme::ACCENT
    };
    theme::chip(ui, format!("{done}/{total} entregues"), color);
    if overdue > 0 {
        theme::chip(ui, format!("{overdue} atrasada(s)"), theme::RED);
    }
}

/// The agency, and the end of the grant when it is near.
fn grant_chip(ui: &mut Ui, g: &Grant, today: NaiveDate) {
    if !g.active() {
        theme::chip(ui, "sem bolsa", theme::GREY);
        return;
    }
    match g.end() {
        Some(end) if end < today => theme::chip(ui, format!("{} · encerrada", g.agency_label()), theme::GREY),
        Some(end) if (end - today).num_days() <= 90 => {
            theme::chip(ui, format!("{} · até {}", g.agency_label(), widgets::fmt_short(end)), theme::AMBER)
        }
        Some(end) => theme::chip(
            ui,
            format!("{} · até {}/{}", g.agency_label(), pt::month_short(end), end.format("%y")),
            theme::GREEN,
        ),
        None => theme::chip(ui, g.agency_label(), theme::GREEN),
    };
}

/// The earliest deadline not yet met: red when overdue, gold once its
/// reminder date has passed.
fn next_deadline_chip(ui: &mut Ui, step: Option<Step>, today: NaiveDate) {
    let Some(step) = step else {
        theme::chip(ui, "sem prazos definidos", theme::GREY);
        return;
    };
    let due = step.milestone.deadline.expect("next_step has a deadline");
    let color = if due < today {
        theme::RED
    } else if step.milestone.reminder(step.lead).is_some_and(|r| r <= today) {
        theme::AMBER
    } else {
        theme::ACCENT
    };
    theme::chip(ui, format!("{} · {}", step.label, pt::day_month_year(due)), color)
        .on_hover_text(&step.label);
}

pub fn visit_status(v: &Visit) -> (String, Color32) {
    if v.completed {
        return (format!("{} realizada", icons::CHECK), theme::GREEN);
    }
    if let Some(d) = v.date {
        return (format!("marcada {}", widgets::fmt_short(d)), theme::ACCENT);
    }
    match (v.request_sent, v.response) {
        (None, _) => ("não solicitada".into(), theme::GREY),
        (Some(_), ReplyStatus::Replied) => ("respondeu – marcar".into(), theme::ACCENT),
        (Some(_), ReplyStatus::Awaiting) => ("aguardando resposta".into(), theme::AMBER),
        (Some(_), ReplyStatus::NoReply) => ("sem resposta".into(), theme::RED),
    }
}

pub fn semester_status(c: &SemesterContact, settings: &Settings, today: NaiveDate) -> (String, Color32) {
    let (next, held) = meeting_summary(&c.meetings, settings, today);
    if let Some(d) = next {
        return (format!("reunião {}", widgets::fmt_short(d)), theme::ACCENT);
    }
    if held > 0 {
        return (format!("{} reuniu-se ({held})", icons::CHECK), theme::GREEN);
    }
    match (c.contacted_on, c.reply) {
        (None, _) => ("sem contato".into(), theme::GREY),
        (Some(_), ReplyStatus::Replied) => ("respondeu".into(), theme::ACCENT),
        (Some(_), ReplyStatus::Awaiting) => ("aguardando resposta".into(), theme::AMBER),
        (Some(_), ReplyStatus::NoReply) => ("sem resposta".into(), theme::RED),
    }
}
