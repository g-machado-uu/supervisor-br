//! Iniciação científica section: the work plan, the grant (PIBIC, PIBITI, …),
//! the reports and the seminar, and meetings.

use egui::{RichText, Ui};

use crate::model::{IcModality, IcRecord, Settings};
use crate::ui::steps::{self, combo};
use crate::ui::widgets::Share;
use crate::ui::{icons, theme, widgets};

/// Returns the meeting to send, if asked.
pub fn show(ui: &mut Ui, ic: &mut IcRecord, settings: &Settings) -> Option<Share> {
    let today = widgets::today();
    widgets::form_grid(ui, "ic-grid", |ui| {
        ui.label("Plano de trabalho");
        widgets::text_field(ui, &mut ic.title, "Título do plano de trabalho");
        ui.end_row();
        ui.label("Projeto");
        widgets::text_field(ui, &mut ic.project, "Projeto de pesquisa do orientador");
        ui.end_row();
        ui.label("Modalidade");
        let before = ic.modality;
        combo(ui, "ic-modality", &mut ic.modality, &IcModality::ALL, |m| m.label());
        if ic.modality != before {
            // Voluntary IC has no grant; the others usually do.
            if ic.modality == IcModality::Voluntary {
                ic.grant.agency.clear();
            } else if before == IcModality::Voluntary && !ic.grant.active() {
                ic.grant.agency = "CNPq".into();
                if ic.grant.months == 0 {
                    ic.grant.months = 12;
                }
            }
            if matches!(ic.grant.modality.as_str(), "" | "PIBIC" | "PIBITI" | "PIBIC-Af" | "PIBIC-EM") {
                ic.grant.modality = match ic.modality {
                    IcModality::Pibic | IcModality::Pibiti | IcModality::PibicAf | IcModality::PibicEm => {
                        ic.modality.short().into()
                    }
                    _ => String::new(),
                };
            }
        }
        ui.end_row();
    });

    ui.add_space(10.0);
    theme::section(ui, "Bolsa", |ui| {
        steps::grant_editor(ui, "ic-grant", &mut ic.grant, steps::GrantOf::Ic);
    });

    ui.add_space(10.0);
    theme::section(ui, "Relatórios e seminário", |ui| reports(ui, ic, today));

    ui.add_space(10.0);
    theme::section(ui, "Reuniões de orientação", |ui| {
        widgets::meetings_editor(ui, "ic-meetings", &mut ic.meetings, None, settings)
    })
    .flatten()
}

/// The reports and the seminar, suggested from the grant, and deadlines added by hand.
fn reports(ui: &mut Ui, ic: &mut IcRecord, today: chrono::NaiveDate) {
    ui.horizontal_wrapped(|ui| {
        if ui
            .add_enabled(
                ic.grant.start.is_some(),
                egui::Button::new(format!("{} Preencher prazos sugeridos", icons::CALENDAR_CHECK)).small(),
            )
            .on_hover_text(
                "Relatório parcial na metade da vigência e relatório final no último dia. \
                 Datas já preenchidas são mantidas.",
            )
            .on_disabled_hover_text("Defina o início da vigência da bolsa primeiro")
            .clicked()
        {
            ic.suggest();
        }
    });
    ui.add(
        egui::Label::new(
            RichText::new(
                "Ciclo usual: 12 meses de vigência, relatório parcial por volta do 6º mês, relatório final \
                 e apresentação no seminário anual de IC (CNPq RN-017/2006). Confira o edital da sua instituição.",
            )
            .small()
            .color(theme::muted(ui)),
        )
        .wrap_mode(egui::TextWrapMode::Wrap),
    );
    let all = ic.steps();
    let standard: Vec<_> = all.into_iter().filter(|s| !s.key.starts_with("custom-")).collect();
    let mut edits = Vec::new();
    steps::steps_grid(ui, "ic-steps", &standard, today, |_, _| {}, |key, m| edits.push((key.to_string(), m)));
    for (key, m) in edits {
        if let Some(slot) = ic.milestone_mut(&key) {
            *slot = m;
        }
    }
    steps::custom_steps(ui, &mut ic.custom_steps, today);
}
