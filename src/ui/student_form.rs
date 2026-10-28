//! Detail panel: common details plus a collapsible section per role, whose
//! chips at the top jump to them. The enrolment number (often the CPF) stays
//! hidden until revealed, and everything that names the student can be
//! hidden ("Ocultar nomes", or the eye on the details).

use egui::{Align, Layout, RichText, Ui};

use crate::model::{
    AdviseeRecord, IcRecord, Level, PlacementRecord, PosRecord, Settings, Student, TccRecord, is_cpf, masked,
};
use crate::ui::widgets::Share;
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
    /// Icon and colour of its chip and section.
    fn style(self) -> (&'static str, egui::Color32) {
        match self {
            Role::Tcc => (icons::BOOK_OPEN_TEXT, theme::ACCENT),
            Role::Ic => (icons::FLASK, theme::TEAL),
            Role::Pos => (icons::GRADUATION_CAP, theme::PLUM),
            Role::Placement => (icons::BUILDINGS, theme::AMBER),
            Role::Advisee => (icons::CHAT_CIRCLE_TEXT, theme::GREEN),
        }
    }

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
    /// Send the meeting, visit or defence: an invite file or Google Agenda.
    Share(Share),
    /// Show the enrolment number (asking for the password if set to).
    Reveal,
    /// Hide it again.
    Hide,
}

/// What of a student's private details is on screen; the eye button
/// switches `details`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Shown {
    /// Name, enrolment number, email and grade; off in "Ocultar nomes"
    /// mode, or once hidden for this student.
    pub details: bool,
    /// The enrolment number, hidden by Settings › Privacidade until revealed.
    pub number: bool,
}

/// Dots for a hidden value, a dash for an empty one.
pub fn hidden_value(ui: &mut Ui, text: &str) {
    if text.trim().is_empty() {
        ui.label(RichText::new("–").color(theme::muted(ui)));
    } else {
        ui.label(RichText::new("•".repeat(8)).monospace().color(theme::muted(ui)));
    }
}

pub fn show(ui: &mut Ui, s: &mut Student, settings: &Settings, duplicate: bool, shown: &mut Shown) -> FormAction {
    let mut action = FormAction::None;
    let private = !shown.details;
    let number_hidden = settings.hide_enrollment && !s.enrollment.trim().is_empty();
    let hidden = private || (number_hidden && !shown.number);
    ui.horizontal(|ui| {
        theme::avatar(ui, if private { "" } else { &s.name }, 52.0);
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 2.0;
            ui.set_max_width((ui.available_width() - 30.0).max(120.0));
            let name = if private {
                "•".repeat(10)
            } else if s.name.is_empty() {
                "Novo aluno".into()
            } else {
                s.name.clone()
            };
            ui.add(egui::Label::new(theme::serif(name, 24.0)).truncate());
            let number = if hidden { masked(&s.enrollment) } else { s.enrollment.clone() };
            let sub: Vec<&str> = [number.as_str(), s.course.as_str()]
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
    // Clicking a role's chip opens its section and scrolls to it.
    let mut jump = None;
    ui.horizontal_wrapped(|ui| {
        ui.add_space(60.0);
        let mut role_chip = |ui: &mut Ui, role: Role, text: String| {
            let (icon, color) = role.style();
            if theme::chip_button(ui, format!("{icon} {text}"), color)
                .on_hover_text(format!("Ir para {}", role.label()))
                .clicked()
            {
                jump = Some(role);
            }
        };
        if let Some(f) = &s.tcc {
            role_chip(ui, Role::Tcc, format!("TCC · {}", settings.programme_short(&f.programme)));
        }
        if let Some(f) = &s.ic {
            role_chip(ui, Role::Ic, format!("IC · {}", f.modality.short()));
        }
        if let Some(p) = &s.pos {
            let ext = if p.external { " · coorientação" } else { "" };
            role_chip(ui, Role::Pos, format!("{}{ext}", p.level.short()));
        }
        if s.placement.is_some() {
            role_chip(ui, Role::Placement, "Estágio".into());
        }
        if s.advisee.is_some() {
            role_chip(ui, Role::Advisee, "Tutoria".into());
        }
        if s.archived {
            theme::chip(ui, format!("{} arquivado", icons::ARCHIVE), theme::GREY);
        }
    });
    ui.add_space(10.0);

    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        theme::card(ui, "", |ui| {
            let eye = |ui: &mut Ui| {
                if theme::eye_button(ui, shown.details, "os dados do aluno") {
                    shown.details = !shown.details;
                }
            };
            theme::section_with(ui, "Dados do aluno", eye, |ui| {
                widgets::form_grid(ui, "details-grid", |ui| {
                    ui.label("Matrícula");
                    ui.vertical(|ui| {
                        if private {
                            hidden_value(ui, &s.enrollment);
                        } else if hidden {
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
                                if number_hidden && ui.button(format!("{} Ocultar", icons::EYE_SLASH)).clicked() {
                                    action = FormAction::Hide;
                                }
                            });
                        }
                        if is_cpf(&s.enrollment) && !private {
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
                    if private {
                        hidden_value(ui, &s.name);
                    } else {
                        widgets::text_field(ui, &mut s.name, "Nome completo");
                    }
                    ui.end_row();
                    ui.label("E-mail");
                    if private {
                        hidden_value(ui, &s.email);
                    } else {
                        widgets::text_field(ui, &mut s.email, "E-mail institucional");
                    }
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
            role_section(ui, Role::Tcc, jump, &mut action, |ui| {
                if let Some(share) = tcc::show(ui, f, settings, id, private) {
                    invite = Some(share);
                }
            });
        }
        if let Some(f) = &mut s.ic {
            role_section(ui, Role::Ic, jump, &mut action, |ui| {
                if let Some(share) = ic::show(ui, f, settings) {
                    invite = Some(share);
                }
            });
        }
        if let Some(p) = &mut s.pos {
            role_section(ui, Role::Pos, jump, &mut action, |ui| {
                if let Some(share) = pos::show(ui, p, settings, id) {
                    invite = Some(share);
                }
            });
        }
        if let Some(p) = &mut s.placement {
            role_section(ui, Role::Placement, jump, &mut action, |ui| {
                if let Some(share) = placement::show(ui, p, id) {
                    invite = Some(share);
                }
            });
        }
        if let Some(a) = &mut s.advisee {
            role_section(ui, Role::Advisee, jump, &mut action, |ui| {
                if let Some(share) = advisee::show(ui, a, settings) {
                    invite = Some(share);
                }
            });
        }
        if let Some(share) = invite {
            action = FormAction::Share(share);
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

/// `jump`: the role whose chip was clicked, to open and scroll to.
fn role_section(ui: &mut Ui, role: Role, jump: Option<Role>, action: &mut FormAction, add: impl FnOnce(&mut Ui)) {
    let (icon, color) = role.style();
    ui.add_space(10.0);
    let card = theme::card_frame(ui).show(ui, |ui| {
        ui.set_width(ui.available_width());
        let title = theme::semibold(format!("{icon}  {}", role.label()))
            .size(15.0)
            .color(theme::fg(ui, color));
        egui::CollapsingHeader::new(title)
            .id_salt(role.label())
            .default_open(true)
            .open((jump == Some(role)).then_some(true))
            .show(ui, |ui| {
                // Sections of different roles share names ("Bolsa", "Reuniões…").
                ui.push_id(role.label(), add);
                ui.add_space(4.0);
                if ui
                    .small_button(RichText::new(format!("Remover registro de {}…", role.label())).color(theme::muted(ui)))
                    .clicked()
                {
                    *action = FormAction::RemoveRole(role);
                }
            });
    });
    if jump == Some(role) {
        ui.scroll_to_rect(card.response.rect, Some(Align::TOP));
    }
}
