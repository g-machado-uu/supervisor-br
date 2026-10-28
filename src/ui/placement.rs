//! Estágio supervisionado section: the company, the internship agreement
//! (TCE), the report and the visits, which can be added, removed and folded.

use egui::collapsing_header::CollapsingState;
use egui::{RichText, Ui};

use uuid::Uuid;

use crate::model::{PlacementRecord, ReplyStatus, Visit, VisitMode, visit_uid};
use crate::ui::widgets::Share;
use crate::ui::{icons, steps, student_list, theme, widgets};

/// Returns the visit to send, if asked.
pub fn show(ui: &mut Ui, p: &mut PlacementRecord, student_id: Uuid) -> Option<Share> {
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

    ui.add_space(6.0);
    let mut remove = None;
    for (i, v) in p.visits.iter_mut().enumerate() {
        ui.add_space(6.0);
        ui.push_id(("visit", i), |ui| {
            widgets::inner_frame(ui).show(ui, |ui| {
                ui.set_width(ui.available_width());
                // Each visit folds away; those already made start folded.
                let id = ui.make_persistent_id("visit-open");
                let mut state = CollapsingState::load_with_default_open(ui.ctx(), id, !v.completed);
                let open = state.is_open();
                ui.horizontal(|ui| {
                    let caret = if open { icons::CARET_DOWN } else { icons::CARET_RIGHT };
                    if ui
                        .add(egui::Label::new(theme::semibold(format!("{caret} Visita {}", i + 1))).sense(egui::Sense::click()))
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .on_hover_text(if open { "Recolher" } else { "Mostrar" })
                        .clicked()
                    {
                        state.toggle(ui);
                    }
                    if open {
                        if v.completed {
                            theme::chip(ui, "realizada", theme::GREEN);
                        }
                    } else {
                        let (text, color) = student_list::visit_status(v);
                        theme::chip(ui, text, color);
                        ui.label(RichText::new(v.mode.label()).small().color(theme::muted(ui)));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.small_button(icons::TRASH).on_hover_text("Remover esta visita").clicked() {
                            remove = Some(i);
                        }
                    });
                });
                state.show_body_unindented(ui, |ui| {
                    if let Some(share) = visit(ui, v, || visit_uid(student_id, i)) {
                        invite = Some(share);
                    }
                });
            });
        });
    }
    if let Some(i) = remove {
        p.visits.remove(i);
    }
    ui.add_space(4.0);
    if ui
        .button(format!("{} Adicionar visita", icons::PLUS))
        .on_hover_text("O número de visitas depende do curso e do estágio")
        .clicked()
    {
        p.add_visit();
    }
    invite
}

/// The request, the reply, the day and how it went. Returns the visit
/// (`uid`) to send, if asked.
fn visit(ui: &mut Ui, v: &mut Visit, uid: impl Fn() -> String) -> Option<Share> {
    let mut invite = None;
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
        ui.label("Convidar");
        ui.horizontal_wrapped(|ui| {
            invite = widgets::share_buttons(ui, v.date.is_some(), &uid);
        });
        ui.end_row();

        widgets::field_label(ui, "Supervisor(a) participa");
        widgets::tri_state(ui, &mut v.line_manager_attending);
        ui.end_row();

        ui.label("Realizada");
        ui.checkbox(&mut v.completed, "Visita feita");
        ui.end_row();
    });
    widgets::notes(ui, &mut v.notes, "Anotações da visita…");
    invite
}
