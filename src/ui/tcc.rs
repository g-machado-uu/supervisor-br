//! TCC section of the student detail panel: the course, the items handed in
//! for each discipline (TCC I, TCC II, …), the defence and meetings.

use egui::{RichText, Ui};
use uuid::Uuid;

use crate::model::{Settings, TccRecord};
use crate::ui::{theme, widgets};

/// Returns the UID of a meeting (or the defence) to save an invite for.
pub fn show(ui: &mut Ui, tcc: &mut TccRecord, settings: &Settings, student_id: Uuid) -> Option<String> {
    let mut invite = None;
    widgets::form_grid(ui, "tcc-grid", |ui| {
        ui.label("Curso");
        let current = settings.programme(&tcc.programme).map_or_else(
            || format!("{} (removido)", tcc.programme),
            |p| p.name.clone(),
        );
        egui::ComboBox::from_id_salt("tcc-course")
            .selected_text(current)
            .width(260.0)
            .show_ui(ui, |ui| {
                for p in &settings.programmes {
                    ui.selectable_value(&mut tcc.programme, p.id.clone(), &p.name);
                }
            });
        ui.end_row();

        ui.label("Título do TCC");
        widgets::text_field(ui, &mut tcc.title, "Título do trabalho");
        ui.end_row();

        ui.label("Coorientador(a)");
        widgets::text_field(ui, &mut tcc.co_supervisor, "Opcional");
        ui.end_row();

        ui.label("Contato inicial");
        ui.horizontal(|ui| {
            ui.checkbox(&mut tcc.initial_contact, "Feito");
            if tcc.initial_contact {
                widgets::opt_date(ui, "initial-contact", &mut tcc.initial_contact_on);
            }
        });
        ui.end_row();
    });

    ui.add_space(10.0);
    theme::card_title(ui, "Entregas");
    let today = widgets::today();
    let mut any_module = false;
    for module in settings.modules_for(&tcc.programme) {
        any_module = true;
        ui.label(
            theme::semibold(format!("{} – {} · {}", module.code, module.name, module.semester.label()))
                .color(theme::fg(ui, theme::ACCENT)),
        );
        if module.courseworks.is_empty() {
            ui.label(
                RichText::new("Nenhuma entrega definida – adicione em Configurações › Disciplinas.")
                    .italics()
                    .color(theme::muted(ui)),
            );
            continue;
        }
        egui::Grid::new(("cw-grid", module.id))
            .num_columns(4)
            .striped(true)
            .spacing([10.0, 6.0])
            .show(ui, |ui| {
                for h in ["Item", "Prazo", "Prorrogação", "Entregue"] {
                    ui.label(RichText::new(h).small().color(theme::muted(ui)));
                }
                ui.end_row();
                for cw in &module.courseworks {
                    // Edit a copy so merely viewing doesn't create (and save) empty entries.
                    let original = tcc.coursework.get(&cw.id).cloned().unwrap_or_default();
                    let mut edited = original.clone();
                    let status = &mut edited;
                    ui.label(&cw.title);
                    match settings.effective_deadline(module, cw, Some(status)) {
                        Some(due) => {
                            let color = if status.submitted {
                                theme::fg(ui, theme::GREEN)
                            } else if due < today {
                                theme::fg(ui, theme::RED)
                            } else {
                                ui.visuals().text_color()
                            };
                            ui.colored_label(color, widgets::fmt_date(Some(due)));
                        }
                        None => {
                            ui.label(RichText::new("a definir").color(theme::muted(ui)));
                        }
                    }
                    widgets::opt_date(ui, ("ext", cw.id), &mut status.deadline_override);
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut status.submitted, "").changed()
                            && status.submitted
                            && status.submitted_on.is_none()
                        {
                            status.submitted_on = Some(today);
                        }
                        if status.submitted {
                            widgets::opt_date(ui, ("sub", cw.id), &mut status.submitted_on);
                        }
                    });
                    ui.end_row();
                    if edited != original {
                        tcc.coursework.insert(cw.id, edited);
                    }
                }
            });
    }
    if !any_module {
        ui.label(
            RichText::new("Nenhuma disciplina ligada a este curso – adicione em Configurações › Disciplinas.")
                .italics()
                .color(theme::muted(ui)),
        );
    }

    ui.add_space(10.0);
    theme::card_title(ui, "Defesa");
    widgets::form_grid(ui, "tcc-defense", |ui| {
        ui.label("Data");
        widgets::opt_date(ui, "defense", &mut tcc.defense);
        ui.end_row();
        ui.label("Horário");
        ui.horizontal(|ui| {
            widgets::opt_time(ui, "defense-time", &mut tcc.defense_time);
            if widgets::invite_button(ui, tcc.defense.is_some()) {
                invite = Some(format!("tcc-defense-{student_id}"));
            }
        });
        ui.end_row();
        ui.label("Local");
        widgets::text_field(ui, &mut tcc.defense_location, "Sala, auditório ou link");
        ui.end_row();
        ui.label("Banca");
        widgets::text_field(ui, &mut tcc.committee, "Membros da banca");
        ui.end_row();
        ui.label("Nota final");
        widgets::text_field(ui, &mut tcc.grade, "Ex.: 9,5");
        ui.end_row();
    });

    ui.add_space(10.0);
    theme::card_title(ui, "Reuniões de orientação");
    if let Some(uid) = widgets::meetings_editor(ui, "tcc-meetings", &mut tcc.meetings, None, settings) {
        invite = Some(uid);
    }
    invite
}
