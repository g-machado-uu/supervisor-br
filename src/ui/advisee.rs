//! Tutoria acadêmica section: contact / reply / meetings for each semester.

use egui::Ui;

use crate::model::{AdviseeRecord, ReplyStatus, SemesterContact, Settings};
use crate::ui::widgets::Share;
use crate::ui::{theme, widgets};

/// Returns the meeting to send, if asked.
pub fn show(ui: &mut Ui, a: &mut AdviseeRecord, settings: &Settings) -> Option<Share> {
    let s1 = semester(ui, "1º semestre", &mut a.s1, settings);
    ui.add_space(6.0);
    let s2 = semester(ui, "2º semestre", &mut a.s2, settings);
    s1.or(s2)
}

fn semester(
    ui: &mut Ui,
    title: &str,
    c: &mut SemesterContact,
    settings: &Settings,
) -> Option<Share> {
    ui.push_id(title, |ui| {
        egui::Frame::group(ui.style())
            .corner_radius(egui::CornerRadius::same(12))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(theme::semibold(title).size(15.0));
                widgets::form_grid(ui, "sem-grid", |ui| {
                    ui.label("Entrei em contato");
                    widgets::opt_date(ui, "contacted", &mut c.contacted_on);
                    ui.end_row();

                    ui.label("Resposta");
                    widgets::reply_status(ui, &mut c.reply);
                    ui.end_row();

                    if c.reply == ReplyStatus::Replied {
                        ui.label("Respondeu em");
                        widgets::opt_date(ui, "replied", &mut c.replied_on);
                        ui.end_row();
                    }
                });
                widgets::meetings_editor(
                    ui,
                    "meetings",
                    &mut c.meetings,
                    Some("Reunião do semestre"),
                    settings,
                )
            })
            .inner
    })
    .inner
}
