//! Pós-graduação section of the student detail panel: the project, the
//! grant, the deadlines of the course (suggested from the rules of the level
//! in Settings, or added by hand) with their reminders, the defence and
//! meetings.

use chrono::Months;
use egui::{RichText, Ui};
use uuid::Uuid;

use crate::model::{DefenseOutcome, ExtensionReason, Level, Milestone, PosRecord, PosStatus, Settings};
use crate::pt;
use crate::ui::steps::{self, combo};
use crate::ui::{icons, theme, widgets};

/// Returns the UID of a meeting (or the defence) to save an invite for.
pub fn show(ui: &mut Ui, p: &mut PosRecord, settings: &Settings, student_id: Uuid) -> Option<String> {
    let mut invite = None;
    let today = widgets::today();
    details(ui, p);

    ui.add_space(10.0);
    theme::card_title(ui, "Bolsa");
    steps::grant_editor(ui, "pos-grant", &mut p.grant, Some(p.level));

    ui.add_space(10.0);
    theme::card_title(ui, "Datas e prazos");
    let rule = settings.rule(p.level);
    widgets::form_grid(ui, "pos-dates", |ui| {
        ui.label("Ingresso");
        widgets::opt_date(ui, "start", &mut p.start);
        ui.end_row();
        if let Some(year) = p.year_of_study(today) {
            ui.label("Ano do curso");
            ui.label(format!("{year}º ano"));
            ui.end_row();
        }
        ui.label("Prorrogação do prazo");
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut p.extension_months).range(0..=24).suffix(" meses"));
            combo(ui, "pos-ext-reason", &mut p.extension_reason, &ExtensionReason::ALL, |r| r.label());
        });
        ui.end_row();
    });
    let hint = p.extension_reason.hint();
    if !hint.is_empty() {
        ui.add(egui::Label::new(RichText::new(hint).small().color(theme::muted(ui))).wrap_mode(egui::TextWrapMode::Wrap));
    }

    let mut standard = !p.skip_standard;
    ui.horizontal_wrapped(|ui| {
        if ui
            .checkbox(&mut standard, "Etapas padrão do curso")
            .on_hover_text("Créditos, proficiência, qualificação, relatórios anuais, envio à banca e defesa")
            .changed()
        {
            p.skip_standard = !standard;
        }
        if !p.skip_standard
            && ui
                .add_enabled(p.start.is_some(), egui::Button::new(format!("{} Preencher prazos sugeridos", icons::CALENDAR_CHECK)).small())
                .on_hover_text(
                    "Calcula os prazos vazios a partir do ingresso, com as regras do nível em \
                     Configurações › Prazos da pós-graduação. Datas já preenchidas são mantidas.",
                )
                .on_disabled_hover_text("Defina a data de ingresso primeiro")
                .clicked()
        {
            p.suggest(&rule);
        }
    });
    let hint = if p.skip_standard {
        "Adicione os prazos do programa do aluno e escolha quando ser lembrado."
    } else {
        "Os prazos sugeridos seguem as regras do nível; confira o regimento do programa e ajuste o que for preciso."
    };
    ui.add(egui::Label::new(RichText::new(hint).small().color(theme::muted(ui))).wrap_mode(egui::TextWrapMode::Wrap));

    // The deadline with the extension, when it differs from the one set.
    if let Some(with_ext) = p.rule_deadline(&rule).filter(|_| p.extension_months > 0)
        && p.final_deadline.deadline != Some(with_ext)
        && !p.skip_standard
    {
        ui.horizontal_wrapped(|ui| {
            ui.colored_label(
                theme::fg(ui, theme::AMBER),
                format!("{} Com a prorrogação, o prazo final passa a {}", icons::HOURGLASS, pt::date(with_ext)),
            );
            if ui.small_button("Usar esta data").clicked() {
                p.final_deadline.deadline = Some(with_ext);
            }
        });
    }

    let all = p.steps();
    let mut remove_report = None;
    let mut add_report = false;
    if !p.skip_standard {
        let standard: Vec<_> = all
            .iter()
            .filter(|s| s.key != "final-version" && !s.key.starts_with("custom-"))
            .collect();
        let mut edits = Vec::new();
        let mut placed_add = false;
        egui::Grid::new("pos-steps")
            .num_columns(3)
            .striped(true)
            .spacing([10.0, 6.0])
            .min_col_width(110.0)
            .show(ui, |ui| {
                for h in ["Etapa", "Prazo", "Concluída"] {
                    ui.label(RichText::new(h).small().color(theme::muted(ui)));
                }
                ui.end_row();
                for step in &standard {
                    if !placed_add && (step.key == "submission" || step.key == "final") {
                        placed_add = true;
                        if ui.small_button(format!("{} Relatório anual", icons::PLUS)).clicked() {
                            add_report = true;
                        }
                        ui.end_row();
                    }
                    let annual = step.key.strip_prefix("annual").and_then(|n| n.parse::<usize>().ok());
                    ui.horizontal(|ui| {
                        ui.set_max_width(230.0);
                        steps::step_label(ui, step, today);
                        if let Some(n) = annual
                            && ui
                                .push_id(&step.key, |ui| ui.small_button(icons::TRASH))
                                .inner
                                .on_hover_text("Remover este relatório")
                                .clicked()
                        {
                            remove_report = Some(n - 1);
                        }
                    });
                    let mut edited = step.milestone.clone();
                    steps::milestone_cells(ui, &step.key, &mut edited, today);
                    if edited != step.milestone {
                        edits.push((step.key.clone(), edited));
                    }
                    ui.end_row();
                }
            });
        for (key, m) in edits {
            if let Some(slot) = p.milestone_mut(&key) {
                *slot = m;
            }
        }
    }
    if add_report {
        let last = p.annual_reports.iter().rev().find_map(|r| r.deadline).or(p.start);
        p.annual_reports.push(Milestone {
            deadline: last.and_then(|d| d.checked_add_months(Months::new(12))),
            ..Default::default()
        });
    }
    if let Some(i) = remove_report {
        p.annual_reports.remove(i);
    }
    steps::custom_steps(ui, &mut p.custom_steps, today);

    ui.add_space(10.0);
    let title = match p.level {
        Level::PosDoutorado => "Conclusão",
        _ => "Defesa e titulação",
    };
    theme::card_title(ui, title);
    widgets::form_grid(ui, "pos-defense", |ui| {
        if p.level != Level::PosDoutorado {
            ui.label("Data da defesa");
            widgets::opt_date(ui, "defense", &mut p.defense);
            ui.end_row();
            ui.label("Horário");
            ui.horizontal(|ui| {
                widgets::opt_time(ui, "defense-time", &mut p.defense_time);
                if widgets::invite_button(ui, p.defense.is_some()) {
                    invite = Some(format!("pos-defense-{student_id}"));
                }
            });
            ui.end_row();
            ui.label("Local");
            widgets::text_field(ui, &mut p.defense_location, "Sala, auditório ou link");
            ui.end_row();
            ui.label("Banca");
            widgets::text_field(ui, &mut p.committee, "Membros da banca examinadora");
            ui.end_row();
            ui.label("Resultado");
            combo(ui, "outcome", &mut p.outcome, &DefenseOutcome::ALL, |o| o.label());
            ui.end_row();
        }
        let corrections = matches!(p.outcome, DefenseOutcome::ApprovedWithChanges | DefenseOutcome::Resubmit);
        if let Some(step) = all
            .iter()
            .find(|s| s.key == "final-version")
            .filter(|s| corrections || s.milestone.deadline.is_some())
        {
            ui.horizontal(|ui| {
                ui.set_max_width(140.0);
                steps::step_label(ui, step, today);
            });
            ui.horizontal(|ui| {
                let mut edited = step.milestone.clone();
                steps::milestone_cells(ui, &step.key, &mut edited, today);
                if edited != step.milestone {
                    p.final_version = edited;
                }
            });
            ui.end_row();
        }
        ui.label(if p.level == Level::PosDoutorado { "Concluído em" } else { "Titulação em" });
        widgets::opt_date(ui, "awarded", &mut p.awarded_on);
        ui.end_row();
    });

    ui.add_space(10.0);
    let meetings = match p.level {
        Level::PosDoutorado => "Reuniões de supervisão",
        _ => "Reuniões de orientação",
    };
    theme::card_title(ui, meetings);
    if let Some(uid) = widgets::meetings_editor(ui, "pos-meetings", &mut p.meetings, None, settings) {
        invite = Some(uid);
    }
    invite
}

fn details(ui: &mut Ui, p: &mut PosRecord) {
    widgets::form_grid(ui, "pos-grid", |ui| {
        ui.label("Título");
        let hint = match p.level {
            Level::PosDoutorado => "Projeto de pós-doutorado",
            _ => "Título do trabalho",
        };
        widgets::text_field(ui, &mut p.title, hint);
        ui.end_row();

        ui.label("Nível");
        ui.vertical(|ui| {
            combo(ui, "level", &mut p.level, &Level::ALL, |l| l.label());
            ui.label(RichText::new(p.level.kind()).small().color(theme::muted(ui)));
        });
        ui.end_row();

        ui.label("Programa");
        widgets::text_field(ui, &mut p.program, "Programa de pós-graduação (PPG)");
        ui.end_row();

        ui.label("Coorientação");
        ui.checkbox(&mut p.external, "Aluno de outro programa ou instituição")
            .on_hover_text("Você coorienta; os prazos seguem as regras do programa de origem");
        ui.end_row();

        if p.external {
            ui.label("Instituição");
            widgets::text_field(ui, &mut p.institution, "Onde o aluno está matriculado");
            ui.end_row();
        }

        ui.label("Situação");
        combo(ui, "status", &mut p.status, &PosStatus::ALL, |s| s.label());
        ui.end_row();

        ui.label(if p.external { "Orientador(a)" } else { "Coorientadores" });
        widgets::text_field(ui, &mut p.co_supervisors, "Nomes");
        ui.end_row();
    });
}
