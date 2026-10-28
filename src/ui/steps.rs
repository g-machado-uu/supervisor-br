//! Shared pieces of the research sections (IC and pós-graduação): deadline
//! rows with their reminders, deadlines added by hand, and the grant (bolsa).

use chrono::{Datelike, NaiveDate};
use egui::{RichText, Ui};

use crate::model::{
    AGENCIES, AgencyKind, CustomStep, ExtensionReason, Grant, LeadUnit, Level, Milestone, Step, agency_by_id,
    typical_grant_months,
};
use crate::pt;
use crate::ui::{icons, theme, widgets};

pub fn combo<T: Copy + PartialEq>(
    ui: &mut Ui,
    salt: &str,
    value: &mut T,
    all: &[T],
    label: impl Fn(T) -> &'static str,
) {
    egui::ComboBox::from_id_salt(salt)
        .selected_text(label(*value))
        .width(220.0)
        .show_ui(ui, |ui| {
            for &option in all {
                ui.selectable_value(value, option, label(option));
            }
        });
}

/// Green when done, red when overdue, gold once the reminder has passed.
pub fn state_color(ui: &Ui, m: &Milestone, lead: crate::model::Lead, today: NaiveDate) -> egui::Color32 {
    match m.deadline {
        _ if m.done => theme::fg(ui, theme::GREEN),
        Some(due) if due < today => theme::fg(ui, theme::RED),
        Some(_) if m.reminder(lead).is_some_and(|r| r <= today) => theme::fg(ui, theme::AMBER),
        _ => ui.visuals().text_color(),
    }
}

fn reminder_note(ui: &mut Ui, m: &Milestone, lead: crate::model::Lead) {
    if let Some(r) = m.reminder(lead).filter(|_| !m.done) {
        ui.label(
            RichText::new(format!("{} lembrete em {}", icons::BELL, pt::day_month_year(r)))
                .small()
                .color(theme::muted(ui)),
        );
    }
}

/// The step's name, coloured by state, with its reminder date underneath.
pub fn step_label(ui: &mut Ui, step: &Step, today: NaiveDate) {
    let color = state_color(ui, &step.milestone, step.lead, today);
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.add(egui::Label::new(RichText::new(&step.label).color(color)).wrap_mode(egui::TextWrapMode::Wrap));
        reminder_note(ui, &step.milestone, step.lead);
    });
}

/// Deadline and completed cells of a milestone row.
pub fn milestone_cells(ui: &mut Ui, key: &str, m: &mut Milestone, today: NaiveDate) {
    widgets::opt_date(ui, (key, "due"), &mut m.deadline);
    ui.horizontal(|ui| {
        let done = ui.push_id((key, "done"), |ui| ui.checkbox(&mut m.done, "")).inner;
        if done.changed() && m.done && m.done_on.is_none() {
            m.done_on = Some(today);
        }
        if m.done {
            widgets::opt_date(ui, (key, "done-on"), &mut m.done_on);
        }
    });
}

/// A grid of standard steps. `edit` gets each changed milestone by key;
/// `extra` may add rows (or buttons) before a step, by its key.
pub fn steps_grid(
    ui: &mut Ui,
    salt: &str,
    steps: &[Step],
    today: NaiveDate,
    mut extra: impl FnMut(&mut Ui, &Step),
    mut edit: impl FnMut(&str, Milestone),
) {
    egui::Grid::new(salt)
        .num_columns(3)
        .striped(true)
        .spacing([10.0, 6.0])
        .min_col_width(110.0)
        .show(ui, |ui| {
            for h in ["Etapa", "Prazo", "Concluída"] {
                ui.label(RichText::new(h).small().color(theme::muted(ui)));
            }
            ui.end_row();
            for step in steps {
                extra(ui, step);
                ui.horizontal(|ui| {
                    ui.set_max_width(220.0);
                    step_label(ui, step, today);
                });
                let mut edited = step.milestone.clone();
                milestone_cells(ui, &step.key, &mut edited, today);
                if edited != step.milestone {
                    edit(&step.key, edited);
                }
                ui.end_row();
            }
        });
}

/// Deadlines added by hand, each with its own reminder.
pub fn custom_steps(ui: &mut Ui, steps: &mut Vec<CustomStep>, today: NaiveDate) {
    let mut remove = None;
    for (i, c) in steps.iter_mut().enumerate() {
        ui.add_space(4.0);
        ui.push_id(c.id, |ui| {
            widgets::inner_frame(ui).show(ui, |ui| {
                ui.set_width(ui.available_width());
                let color = state_color(ui, &c.milestone, c.remind, today);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button(icons::TRASH).on_hover_text("Remover este prazo").clicked() {
                            remove = Some(i);
                        }
                        ui.add(
                            egui::TextEdit::singleline(&mut c.title)
                                .hint_text("Ex.: Seminário de acompanhamento")
                                .text_color(color)
                                .desired_width(ui.available_width()),
                        );
                    });
                });
                widgets::form_grid(ui, "custom-grid", |ui| {
                    ui.label("Prazo");
                    widgets::opt_date(ui, "due", &mut c.milestone.deadline);
                    ui.end_row();
                    ui.label("Concluído");
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut c.milestone.done, "").changed()
                            && c.milestone.done
                            && c.milestone.done_on.is_none()
                        {
                            c.milestone.done_on = Some(today);
                        }
                        if c.milestone.done {
                            widgets::opt_date(ui, "done-on", &mut c.milestone.done_on);
                        }
                    });
                    ui.end_row();
                    ui.label("Lembrar");
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut c.remind.amount).range(0..=365))
                            .on_hover_text("0 = sem lembrete");
                        egui::ComboBox::from_id_salt("unit")
                            .selected_text(c.remind.unit.label())
                            .width(90.0)
                            .show_ui(ui, |ui| {
                                for unit in LeadUnit::ALL {
                                    ui.selectable_value(&mut c.remind.unit, unit, unit.label());
                                }
                            });
                        ui.label("antes");
                    });
                    ui.end_row();
                });
                reminder_note(ui, &c.milestone, c.remind);
            });
        });
    }
    if let Some(i) = remove {
        steps.remove(i);
    }
    if ui.small_button(format!("{} Adicionar prazo", icons::PLUS)).clicked() {
        steps.push(CustomStep::default());
    }
}

fn agency_text(id: &str) -> String {
    match agency_by_id(id) {
        None if id.is_empty() => "Sem bolsa".into(),
        None => id.into(),
        Some(a) if a.kind == AgencyKind::State => format!("{} ({})", a.id, a.uf),
        Some(a) => a.id.into(),
    }
}

/// Whose grant the editor shows.
#[derive(Clone, Copy)]
pub enum GrantOf {
    /// Iniciação científica: the grant has its own start.
    Ic,
    /// A postgraduate student: the level, the enrolment (where a grant
    /// without its own start begins) and the final deadline of the course.
    Pos {
        level: Level,
        start: Option<NaiveDate>,
        final_deadline: Option<NaiveDate>,
    },
}

/// Whole months of a grant from `start` to its last day `end`.
fn months_between(start: NaiveDate, end: NaiveDate) -> i64 {
    // A grant of whole months ends the day before its start's day of the month.
    let after = end.succ_opt().unwrap_or(end);
    let mut months = i64::from(after.year() - start.year()) * 12 + i64::from(after.month()) - i64::from(start.month());
    if after.day() < start.day() {
        months -= 1;
    }
    months.max(0)
}

/// The grant: agency (federal, the 27 FAPs or other), modality, process
/// number, start, length and any extension, and its end: worked out, typed
/// in, or (postgraduate) linked to the final deadline of the course.
pub fn grant_editor(ui: &mut Ui, salt: &str, g: &mut Grant, of: GrantOf) {
    let (level, course_start, final_deadline) = match of {
        GrantOf::Ic => (None, None, None),
        GrantOf::Pos { level, start, final_deadline } => (Some(level), start, final_deadline),
    };
    let typical = typical_grant_months(level);
    ui.push_id(salt, |ui| {
        widgets::form_grid(ui, "grant-grid", |ui| {
            ui.label("Bolsa");
            ui.vertical(|ui| {
                let before = g.agency.clone();
                egui::ComboBox::from_id_salt("agency")
                    .selected_text(agency_text(&g.agency))
                    .width(220.0)
                    .height(360.0)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut g.agency, String::new(), "Sem bolsa");
                        for (kind, title) in [
                            (AgencyKind::Federal, "FEDERAIS"),
                            (AgencyKind::State, "ESTADUAIS (FAPs)"),
                            (AgencyKind::Other, "OUTRAS"),
                        ] {
                            ui.add_space(4.0);
                            ui.label(theme::semibold(title).size(10.5).color(theme::muted(ui)));
                            for a in AGENCIES.iter().filter(|a| a.kind == kind) {
                                ui.selectable_value(&mut g.agency, a.id.to_string(), agency_text(a.id))
                                    .on_hover_text(a.name);
                            }
                        }
                    });
                if g.agency != before && g.active() && g.months == 0 {
                    g.months = typical;
                }
                if let Some(a) = agency_by_id(&g.agency) {
                    small_wrapped(ui, a.name);
                }
            });
            ui.end_row();
            if !g.active() {
                return;
            }
            if g.agency == "Outra" {
                ui.label("Fonte");
                widgets::text_field(ui, &mut g.agency_other, "Nome da fonte da bolsa");
                ui.end_row();
            }
            ui.label("Modalidade");
            let hint = if level.is_none() {
                "Ex.: PIBIC, PIBITI, IC institucional"
            } else {
                "Ex.: Demanda Social, PROEX, GM, GD, PDJ"
            };
            widgets::text_field(ui, &mut g.modality, hint);
            ui.end_row();
            ui.label("Nº do processo");
            widgets::text_field(ui, &mut g.process, "Opcional");
            ui.end_row();
            ui.label("Início da vigência");
            match (g.start, level) {
                // Postgraduate grants start at enrolment unless set otherwise.
                (None, Some(_)) => {
                    ui.horizontal_wrapped(|ui| {
                        let text = match course_start {
                            Some(start) => format!("No ingresso ({})", widgets::fmt_date(Some(start))),
                            None => "No ingresso (defina-o em Datas e prazos)".into(),
                        };
                        ui.label(text);
                        if ui
                            .small_button("Alterar…")
                            .on_hover_text("A bolsa começa em outro dia")
                            .clicked()
                        {
                            g.start = Some(course_start.unwrap_or_else(widgets::today));
                        }
                    });
                }
                _ => widgets::opt_date(ui, "start", &mut g.start),
            }
            ui.end_row();
            if level.is_some() {
                widgets::field_label(ui, "Vínculo ao curso");
                ui.vertical(|ui| {
                    ui.checkbox(&mut g.until_final, "Vigência até o prazo final de conclusão")
                        .on_hover_text(
                            "A bolsa termina no prazo final do curso (defesa), com as prorrogações dele, \
                             em vez de durar um número fixo de meses.",
                        );
                    if g.until_final && final_deadline.is_none() {
                        ui.colored_label(
                            theme::fg(ui, theme::AMBER),
                            format!("{} Defina o prazo final em Datas e prazos; até lá vale a duração abaixo.", icons::WARNING),
                        );
                    }
                });
                ui.end_row();
            }
            let linked = g.until_final && final_deadline.is_some() && level.is_some();
            ui.label("Duração");
            if let (true, Some(end)) = (linked, final_deadline) {
                let start = g.start.or(course_start);
                let text = match start {
                    Some(start) => format!("até o prazo final ({})", pt::count(months_between(start, end) as usize, "mês", "meses")),
                    None => "até o prazo final".into(),
                };
                ui.label(text);
            } else {
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut g.months).range(0..=72).suffix(" meses"));
                    if g.months != typical
                        && ui
                            .small_button(format!("Usual: {typical} meses"))
                            .on_hover_text(
                                "IC: 12 meses, renovável (CNPq RN-017/2006). Mestrado: até 24 meses; \
                                 doutorado: até 48 (CAPES Portaria 76/2010, CNPq e a maioria das FAPs).",
                            )
                            .clicked()
                    {
                        g.months = typical;
                    }
                });
            }
            ui.end_row();
            if !linked {
                ui.label("Prorrogação");
                ui.vertical(|ui| {
                    ui.horizontal(|ui| {
                        ui.add(egui::DragValue::new(&mut g.extension_months).range(0..=24).suffix(" meses"));
                        combo(ui, "reason", &mut g.extension_reason, &ExtensionReason::ALL, |r| r.label());
                    });
                    let hint = g.extension_reason.hint();
                    if !hint.is_empty() {
                        small_wrapped(ui, hint);
                    }
                });
                ui.end_row();
            }
            ui.label("Fim da vigência");
            if let (true, Some(end)) = (linked, final_deadline) {
                ui.label(theme::semibold(pt::date(end)))
                    .on_hover_text("O prazo final de conclusão do curso");
                ui.end_row();
                return;
            }
            let calculated = g.calculated_end(course_start);
            ui.vertical(|ui| match g.end_override {
                None => {
                    ui.horizontal_wrapped(|ui| {
                        match calculated {
                            Some(end) => ui.label(theme::semibold(pt::date(end))),
                            None => ui.label(RichText::new("defina o início e a duração").color(theme::muted(ui))),
                        }
                        .on_hover_text("Início + duração + prorrogação");
                        if ui
                            .small_button(format!("{} Alterar…", icons::PENCIL_SIMPLE))
                            .on_hover_text("Digite o fim da vigência quando ele for diferente do calculado")
                            .clicked()
                        {
                            g.end_override = Some(calculated.unwrap_or_else(widgets::today));
                        }
                    });
                }
                Some(_) => {
                    widgets::opt_date(ui, "end", &mut g.end_override);
                    if let Some(end) = calculated.filter(|end| g.end_override != Some(*end)) {
                        ui.horizontal_wrapped(|ui| {
                            small_wrapped(ui, &format!("Calculado: {}", pt::date(end)));
                            if ui.small_button("Usar esta data").clicked() {
                                g.end_override = None;
                            }
                        });
                    }
                }
            });
            ui.end_row();
        });
    });
}

fn small_wrapped(ui: &mut Ui, text: &str) {
    ui.add(egui::Label::new(RichText::new(text).small().color(theme::muted(ui))).wrap_mode(egui::TextWrapMode::Wrap));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grant_lengths_in_months() {
        let d = |y, m, day| NaiveDate::from_ymd_opt(y, m, day).unwrap();
        assert_eq!(months_between(d(2026, 3, 1), d(2028, 2, 29)), 24);
        assert_eq!(months_between(d(2026, 3, 15), d(2027, 3, 14)), 12);
        assert_eq!(months_between(d(2026, 3, 15), d(2027, 3, 10)), 11);
        assert_eq!(months_between(d(2026, 3, 1), d(2026, 2, 1)), 0);
    }
}
