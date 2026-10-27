//! Shared pieces of the research sections (IC and pós-graduação): deadline
//! rows with their reminders, deadlines added by hand, and the grant (bolsa).

use chrono::NaiveDate;
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

/// The grant: agency (federal, the 27 FAPs or other), modality, process
/// number, start, length and any extension. `level` is `None` for IC.
pub fn grant_editor(ui: &mut Ui, salt: &str, g: &mut Grant, level: Option<Level>) {
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
                    ui.add(
                        egui::Label::new(RichText::new(a.name).small().color(theme::muted(ui)))
                            .wrap_mode(egui::TextWrapMode::Wrap),
                    );
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
            widgets::opt_date(ui, "start", &mut g.start);
            ui.end_row();
            ui.label("Duração");
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
            ui.end_row();
            ui.label("Prorrogação");
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::DragValue::new(&mut g.extension_months).range(0..=24).suffix(" meses"));
                    combo(ui, "reason", &mut g.extension_reason, &ExtensionReason::ALL, |r| r.label());
                });
                let hint = g.extension_reason.hint();
                if !hint.is_empty() {
                    ui.add(
                        egui::Label::new(RichText::new(hint).small().color(theme::muted(ui)))
                            .wrap_mode(egui::TextWrapMode::Wrap),
                    );
                }
            });
            ui.end_row();
            ui.label("Fim da vigência");
            match g.end() {
                Some(end) => ui.label(theme::semibold(pt::date(end))),
                None => ui.label(RichText::new("defina o início e a duração").color(theme::muted(ui))),
            };
            ui.end_row();
        });
    });
}
