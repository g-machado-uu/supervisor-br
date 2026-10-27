//! Detail panel: common details plus a collapsible section per role. The
//! enrolment number (often the CPF) stays hidden until revealed.

use egui::{Align, Layout, RichText, Ui};

use crate::model::{
    AdviseeRecord, IcRecord, Level, PlacementRecord, PosRecord, Settings, Student, TccRecord, is_cpf, masked,
};
use crate::ui::{advisee, ic, icons, placement, pos, tcc, theme, widgets};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Tcc,
    Ic,
    Pos,
    Placement,
    Advisee,
}

impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Role::Tcc => "TCC",
            Role::Ic => "Iniciação científica",
            Role::Pos => "Pós-graduação",
            Role::Placement => "Estágio supervisionado",
            Role::Advisee => "Tutoria acadêmica",
        }
    }
}

pub enum FormAction {
    None,
    Close,
    Delete,
    RemoveRole(Role),
    /// Save an invite file for the meeting, visit or defence with this UID.
    Invite(String),
    /// Show the enrolment number (asking for the password if set to).
    Reveal,
    /// Hide it again.
    Hide,
}

/// `revealed`: the enrolment number of this student is shown.
pub fn show(ui: &mut Ui, s: &mut Student, settings: &Settings, duplicate: bool, revealed: bool) -> FormAction {
    let mut action = FormAction::None;
    let hidden = settings.hide_enrollment && !revealed && !s.enrollment.trim().is_empty();
    ui.horizontal(|ui| {
        theme::avatar(ui, &s.name, 52.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.set_max_width((ui.available_width() - 30.0).max(120.0));
            let name = if s.name.is_empty() { "Novo aluno" } else { &s.name };
            ui.add(egui::Label::new(theme::serif(name, 24.0)).truncate());
            let shown = if hidden { masked(&s.enrollment) } else { s.enrollment.clone() };
            let sub: Vec<&str> = [shown.as_str(), s.course.as_str()]
                .into_iter()
                .filter(|t| !t.trim().is_empty())
                .collect();
            if !sub.is_empty() {
                ui.add(egui::Label::new(RichText::new(sub.join(" · ")).color(theme::muted(ui))).truncate());
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            if ui
                .add(egui::Button::new(RichText::new(icons::X).size(15.0)).frame(false))
                .on_hover_text("Fechar")
                .clicked()
            {
                action = FormAction::Close;
            }
        });
    });
    ui.horizontal_wrapped(|ui| {
        ui.add_space(60.0);
        if let Some(f) = &s.tcc {
            theme::chip(ui, format!("{} TCC · {}", icons::BOOK_OPEN_TEXT, settings.programme_short(&f.programme)), theme::ACCENT);
        }
        if let Some(f) = &s.ic {
            theme::chip(ui, format!("{} IC · {}", icons::FLASK, f.modality.short()), theme::TEAL);
        }
        if let Some(p) = &s.pos {
            let ext = if p.external { " · coorientação" } else { "" };
            theme::chip(ui, format!("{} {}{ext}", icons::GRADUATION_CAP, p.level.short()), theme::PLUM);
        }
        if s.placement.is_some() {
            theme::chip(ui, format!("{} Estágio", icons::BUILDINGS), theme::AMBER);
        }
        if s.advisee.is_some() {
            theme::chip(ui, format!("{} Tutoria", icons::CHAT_CIRCLE_TEXT), theme::GREEN);
        }
        if s.archived {
            theme::chip(ui, format!("{} arquivado", icons::ARCHIVE), theme::GREY);
        }
    });
    ui.add_space(10.0);

    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        theme::card(ui, "Dados do aluno", |ui| {
            widgets::form_grid(ui, "details-grid", |ui| {
                ui.label("Matrícula");
                ui.vertical(|ui| {
                    if hidden {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(masked(&s.enrollment)).monospace());
                            let hover = if settings.reveal_needs_password {
                                "Mostrar a matrícula (pede a senha)"
                            } else {
                                "Mostrar a matrícula"
                            };
                            if ui.button(format!("{} Revelar", icons::EYE)).on_hover_text(hover).clicked() {
                                action = FormAction::Reveal;
                            }
                        });
                    } else {
                        ui.horizontal(|ui| {
                            let width = (ui.available_width() - 110.0).max(widgets::MIN_FIELD_WIDTH);
                            ui.add(
                                egui::TextEdit::singleline(&mut s.enrollment)
                                    .hint_text("Matrícula ou CPF")
                                    .desired_width(width),
                            );
                            if settings.hide_enrollment
                                && !s.enrollment.trim().is_empty()
                                && ui.button(format!("{} Ocultar", icons::EYE_SLASH)).clicked()
                            {
                                action = FormAction::Hide;
                            }
                        });
                    }
                    if is_cpf(&s.enrollment) {
                        ui.label(
                            RichText::new(format!("{} É um CPF – evite compartilhar", icons::KEY))
                                .small()
                                .color(theme::muted(ui)),
                        );
                    }
                    if duplicate {
                        ui.colored_label(
                            theme::fg(ui, theme::AMBER),
                            format!("{} Outro aluno tem esta matrícula", icons::WARNING),
                        );
                    }
                });
                ui.end_row();
                ui.label("Nome");
                widgets::text_field(ui, &mut s.name, "Nome completo");
                ui.end_row();
                ui.label("E-mail");
                widgets::text_field(ui, &mut s.email, "E-mail institucional");
                ui.end_row();
                ui.label("Curso");
                widgets::text_field(ui, &mut s.course, "Ex.: Engenharia Mecânica");
                ui.end_row();
                ui.label("Arquivado");
                ui.checkbox(&mut s.archived, "Ocultar das listas e do painel");
                ui.end_row();
            });
            widgets::notes(ui, &mut s.notes, "Anotações gerais…");
        });

        if s.tcc.is_none() || s.ic.is_none() || s.pos.is_none() || s.placement.is_none() || s.advisee.is_none() {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Adicionar vínculo").color(theme::muted(ui)));
                if s.tcc.is_none() && ui.button(format!("{} TCC", icons::PLUS)).clicked() {
                    s.tcc = Some(TccRecord {
                        programme: settings.programmes.first().map(|p| p.id.clone()).unwrap_or_default(),
                        ..Default::default()
                    });
                }
                if s.ic.is_none() && ui.button(format!("{} Iniciação científica", icons::PLUS)).clicked() {
                    s.ic = Some(IcRecord::new());
                }
                if s.pos.is_none() && ui.button(format!("{} Pós-graduação", icons::PLUS)).clicked() {
                    s.pos = Some(PosRecord::new(Level::MestradoAcademico));
                }
                if s.placement.is_none() && ui.button(format!("{} Estágio", icons::PLUS)).clicked() {
                    s.placement = Some(PlacementRecord::default());
                }
                if s.advisee.is_none() && ui.button(format!("{} Tutoria", icons::PLUS)).clicked() {
                    s.advisee = Some(AdviseeRecord::default());
                }
            });
        }

        // At most one invite button is clicked per frame.
        let mut invite = None;
        let id = s.id;
        if let Some(f) = &mut s.tcc {
            role_section(ui, Role::Tcc, &mut action, |ui| {
                if let Some(uid) = tcc::show(ui, f, settings, id) {
                    invite = Some(uid);
                }
            });
        }
        if let Some(f) = &mut s.ic {
            role_section(ui, Role::Ic, &mut action, |ui| {
                if let Some(uid) = ic::show(ui, f, settings) {
                    invite = Some(uid);
                }
            });
        }
        if let Some(p) = &mut s.pos {
            role_section(ui, Role::Pos, &mut action, |ui| {
                if let Some(uid) = pos::show(ui, p, settings, id) {
                    invite = Some(uid);
                }
            });
        }
        if let Some(p) = &mut s.placement {
            role_section(ui, Role::Placement, &mut action, |ui| {
                if let Some(uid) = placement::show(ui, p, id) {
                    invite = Some(uid);
                }
            });
        }
        if let Some(a) = &mut s.advisee {
            role_section(ui, Role::Advisee, &mut action, |ui| {
                if let Some(uid) = advisee::show(ui, a, settings) {
                    invite = Some(uid);
                }
            });
        }
        if let Some(uid) = invite {
            action = FormAction::Invite(uid);
        }

        ui.add_space(16.0);
        if ui
            .button(RichText::new(format!("{} Excluir aluno…", icons::TRASH)).color(theme::fg(ui, theme::RED)))
            .clicked()
        {
            action = FormAction::Delete;
        }
        ui.add_space(8.0);
    });
    action
}

fn role_section(ui: &mut Ui, role: Role, action: &mut FormAction, add: impl FnOnce(&mut Ui)) {
    let (icon, color) = match role {
        Role::Tcc => (icons::BOOK_OPEN_TEXT, theme::ACCENT),
        Role::Ic => (icons::FLASK, theme::TEAL),
        Role::Pos => (icons::GRADUATION_CAP, theme::PLUM),
        Role::Placement => (icons::BUILDINGS, theme::AMBER),
        Role::Advisee => (icons::CHAT_CIRCLE_TEXT, theme::GREEN),
    };
    ui.add_space(10.0);
    theme::card(ui, "", |ui| {
        let title = theme::semibold(format!("{icon}  {}", role.label()))
            .size(15.0)
            .color(theme::fg(ui, color));
        egui::CollapsingHeader::new(title)
            .id_salt(role.label())
            .default_open(true)
            .show(ui, |ui| {
                add(ui);
                ui.add_space(4.0);
                if ui
                    .small_button(RichText::new(format!("Remover registro de {}…", role.label())).color(theme::muted(ui)))
                    .clicked()
                {
                    *action = FormAction::RemoveRole(role);
                }
            });
    });
}
