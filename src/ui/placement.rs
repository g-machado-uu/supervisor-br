//! Estágio supervisionado section: the company, the internship agreement
//! (TCE), the report and the two visits.

use egui::{RichText, Ui};

use uuid::Uuid;

use crate::model::{PlacementRecord, ReplyStatus, VisitMode, visit_uid};
use crate::ui::{icons, steps, theme, widgets};

/// Returns the UID of a visit to save an invite for.
pub fn show(ui: &mut Ui, p: &mut PlacementRecord, student_id: Uuid) -> Option<String> {
    let mut invite = None;
    let today = widgets::today();
    widgets::form_grid(ui, "placement-grid", |ui| {
        ui.label("Empresa / concedente");
        widgets::text_field(ui, &mut p.company, "Nome da empresa ou instituição");
        ui.end_row();
        ui.label("Tipo");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut p.mandatory, true, "Obrigatório");
            ui.selectable_value(&mut p.mandatory, false, "Não obrigatório");
        });
        ui.end_row();
        ui.label("E-mail na empresa");
        widgets::text_field(ui, &mut p.professional_email, "nome@empresa.com.br");
        ui.end_row();
        ui.label("Supervisor(a) na empresa");
        widgets::text_field(ui, &mut p.line_manager_name, "Nome");
        ui.end_row();
        ui.label("E-mail do supervisor");
        widgets::text_field(ui, &mut p.line_manager_email, "supervisor@empresa.com.br");
        ui.end_row();
        ui.label("Início do TCE")
            .on_hover_text("Termo de Compromisso de Estágio (Lei 11.788/2008)");
        widgets::opt_date(ui, "tce-start", &mut p.tce_start);
        ui.end_row();
        ui.label("Fim do TCE");
        widgets::opt_date(ui, "tce-end", &mut p.tce_end);
        ui.end_row();
        ui.label("Prazo do relatório");
        widgets::opt_date(ui, ("report", "due"), &mut p.report.deadline);
        ui.end_row();
        ui.label("Relatório entregue");
        ui.horizontal(|ui| {
            let color = steps::state_color(ui, &p.report, crate::model::Lead::weeks(2), today);
            if ui.checkbox(&mut p.report.done, "").changed() && p.report.done && p.report.done_on.is_none() {
                p.report.done_on = Some(today);
            }
            if p.report.done {
                widgets::opt_date(ui, ("report", "done-on"), &mut p.report.done_on);
                ui.label(RichText::new("entregue").color(color));
            }
        });
        ui.end_row();
    });

    for (i, v) in p.visits.iter_mut().enumerate() {
        ui.add_space(6.0);
        ui.push_id(("visit", i), |ui| {
            widgets::inner_frame(ui).show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(theme::semibold(format!("Visita {}", i + 1)));
                    if v.completed {
                        theme::chip(ui, "realizada", theme::GREEN);
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::invite_button(ui, v.date.is_some()) {
                            invite = Some(visit_uid(student_id, i));
                        }
                    });
                });
                widgets::form_grid(ui, "visit-grid", |ui| {
                    ui.label("Modo");
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut v.mode, VisitMode::Online, format!("{} On-line", icons::DESKTOP));
                        ui.selectable_value(&mut v.mode, VisitMode::FaceToFace, format!("{} Presencial", icons::BUILDINGS));
                    });
                    ui.end_row();

                    ui.label("Pedido enviado");
                    widgets::opt_date(ui, "request", &mut v.request_sent);
                    ui.end_row();

                    ui.label("Resposta do aluno");
                    widgets::reply_status(ui, &mut v.response);
                    ui.end_row();

                    if v.response == ReplyStatus::Replied {
                        ui.label("Respondeu em");
                        widgets::opt_date(ui, "responded", &mut v.responded_on);
                        ui.end_row();
                    }

                    ui.label("Data");
                    widgets::opt_date(ui, "date", &mut v.date);
                    ui.end_row();
                    ui.label("Horário");
                    widgets::opt_time(ui, "time", &mut v.time);
                    ui.end_row();

                    widgets::field_label(ui, "Supervisor(a) participa");
                    widgets::tri_state(ui, &mut v.line_manager_attending);
                    ui.end_row();

                    ui.label("Realizada");
                    ui.checkbox(&mut v.completed, "Visita feita");
                    ui.end_row();
                });
                widgets::notes(ui, &mut v.notes, "Anotações da visita…");
            });
        });
    }
    invite
}
