//! Settings: academic calendar, courses, disciplines and their items,
//! postgraduate deadlines, invitations, privacy, security, key file and about.

use std::collections::HashMap;

use egui::{RichText, Ui};
use zeroize::Zeroizing;

use crate::model::{CourseworkTemplate, Level, LevelRule, Module, Programme, Semester, Settings, default_level_rules};
use crate::storage;
use crate::ui::{icons, theme, widgets};

#[derive(Default)]
pub struct PasswordForm {
    pub current: Zeroizing<String>,
    pub new: Zeroizing<String>,
    pub confirm: Zeroizing<String>,
    pub message: Option<(bool, String)>,
}

#[derive(Default)]
pub struct KeyFileForm {
    pub password: Zeroizing<String>,
    pub message: Option<(bool, String)>,
}

/// What the Security and Key file cards show and edit.
pub struct Security<'a> {
    pub data_path: &'a str,
    pub key_file_on: bool,
    pub weak_password: bool,
    pub password: &'a mut PasswordForm,
    pub key_file: &'a mut KeyFileForm,
}

pub enum SettingsAction {
    None,
    ChangePassword,
    AddKeyFile,
    RemoveKeyFile,
    ExportIcs,
}

/// Choices for locking after a while without use (minutes; 0 = never).
const AUTO_LOCK_CHOICES: [u32; 7] = [2, 5, 10, 15, 30, 60, 0];

fn auto_lock_label(minutes: u32) -> String {
    match minutes {
        0 => "Nunca".into(),
        m => format!("Após {m} minutos sem uso"),
    }
}

/// `in_use` counts the TCC students on each course (by id).
pub fn show(
    ui: &mut Ui,
    settings: &mut Settings,
    in_use: &HashMap<String, usize>,
    security: Security<'_>,
) -> SettingsAction {
    let mut action = SettingsAction::None;
    let Security { data_path, key_file_on, weak_password, password: pw, key_file: kf } = security;
    theme::page_header(ui, "Configurações", "Calendário acadêmico, cursos, prazos, convites, privacidade, segurança e versão", |_| {});
    egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
        ui.set_max_width(920.0);
        ui.spacing_mut().item_spacing.y = 14.0;
        theme::card(ui, "Ano letivo", |ui| {
            widgets::form_grid(ui, "year-grid", |ui| {
                ui.label("Ano letivo");
                ui.add(egui::TextEdit::singleline(&mut settings.academic_year).desired_width(120.0));
                ui.end_row();
                for (sem, label) in [(Semester::S1, "1º semestre"), (Semester::S2, "2º semestre")] {
                    let (start, end) = match sem {
                        Semester::S2 => (&mut settings.sem2_start, &mut settings.sem2_teaching_end),
                        _ => (&mut settings.sem1_start, &mut settings.sem1_teaching_end),
                    };
                    ui.label(format!("{label} – início das aulas"));
                    widgets::opt_date(ui, (label, "start"), start);
                    ui.end_row();
                    ui.label(format!("{label} – fim das aulas"));
                    ui.horizontal(|ui| {
                        if end.is_some() {
                            widgets::opt_date(ui, (label, "end"), end);
                        } else if let Some(start) = *start {
                            let default = Settings::default_teaching_end(start);
                            ui.label(format!("{} (sexta da semana {})", widgets::fmt_date(Some(default)), crate::model::TEACHING_WEEKS));
                            if ui.small_button("Alterar…").clicked() {
                                *end = Some(default);
                            }
                        } else {
                            ui.label(RichText::new("defina o início primeiro").color(theme::muted(ui)));
                        }
                    });
                    ui.end_row();
                }
                ui.label("Antecedência no painel");
                ui.add(egui::DragValue::new(&mut settings.upcoming_days).range(1..=120).suffix(" dias"));
                ui.end_row();
                ui.label("Aparência");
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut settings.dark_mode, false, format!("{} Claro", icons::SUN));
                    ui.selectable_value(&mut settings.dark_mode, true, format!("{} Escuro", icons::MOON));
                });
                ui.end_row();
            });
            ui.label(
                RichText::new("Entregas definidas por semana letiva vencem no início daquela semana (segunda-feira, se a semana 1 começa numa segunda). Uma data fixa tem prioridade. Reuniões recorrentes vão até o fim das aulas.")
                    .small()
                    .color(theme::muted(ui)),
            );
        });

        ui.add_space(8.0);
        theme::card(ui, "Cursos de graduação (TCC)", |ui| {
            courses_editor(ui, &mut settings.programmes, in_use);
        });

        ui.add_space(8.0);
        theme::card(ui, "Disciplinas e entregas", |ui| {
            ui.label(
                RichText::new("Cada disciplina (ex.: TCC I, TCC II) vale para um ou mais cursos, com as entregas que os alunos fazem nela.")
                    .small()
                    .color(theme::muted(ui)),
            );
            let starts = (settings.sem1_start, settings.sem2_start);
            let programmes = settings.programmes.clone();
            let mut remove_module = None;
            for (mi, module) in settings.modules.iter_mut().enumerate() {
                ui.push_id(module.id, |ui| {
                    let header = format!("{} – {} ({})", module.code, module.name, module.semester.label());
                    egui::CollapsingHeader::new(RichText::new(header).strong())
                        .id_salt("mod")
                        .show(ui, |ui| {
                            let start = match module.semester {
                                Semester::S1 | Semester::FullYear => starts.0,
                                Semester::S2 => starts.1,
                            };
                            if module_editor(ui, module, &programmes, start) {
                                remove_module = Some(mi);
                            }
                        });
                });
            }
            if let Some(i) = remove_module {
                settings.modules.remove(i);
            }
            if ui.button(format!("{} Adicionar disciplina", icons::PLUS)).clicked() {
                settings.modules.push(Module::default());
            }
        });

        ui.add_space(8.0);
        theme::card(ui, "Prazos da pós-graduação", |ui| {
            level_rules_editor(ui, &mut settings.level_rules);
        });

        ui.add_space(8.0);
        theme::card(ui, "Convites", |ui| {
            widgets::form_grid(ui, "invite-grid", |ui| {
                ui.label("Seu nome");
                widgets::text_field(ui, &mut settings.my_name, "Aparece para os alunos como organizador");
                ui.end_row();
                ui.label("Seu e-mail");
                ui.vertical(|ui| {
                    widgets::text_field(ui, &mut settings.my_email, "nome@universidade.edu.br");
                    let email = settings.my_email.trim();
                    if !email.is_empty() && !crate::ics::valid_email(email) {
                        ui.colored_label(theme::fg(ui, theme::AMBER), format!("{} Isto não parece um endereço de e-mail", icons::WARNING));
                    }
                });
                ui.end_row();
                ui.label("Link do Teams (opcional)");
                ui.vertical(|ui| {
                    widgets::text_field(ui, &mut settings.teams_link, "Link fixo de reunião do Teams");
                    let link = settings.teams_link.trim();
                    if !link.is_empty() && !crate::ics::valid_link(link) {
                        ui.colored_label(theme::fg(ui, theme::AMBER), format!("{} Cole o link completo, começando com https://", icons::WARNING));
                    }
                });
                ui.end_row();
            });
            ui.label(
                RichText::new("“Convite…” em uma reunião, visita ou defesa salva um .ics com um evento: importe no Outlook (Adicionar calendário › Carregar de arquivo) ou no Google Agenda, abra o evento e envie. Em reuniões on-line, o link da reunião vai junto; para o Teams sem link, ative “Reunião do Teams” no Outlook antes de enviar.")
                    .small()
                    .color(theme::muted(ui)),
            );
            ui.label(
                RichText::new("Estes dados ficam no arquivo criptografado. Os arquivos de convite não são criptografados e contêm o seu e-mail e o do aluno.")
                    .small()
                    .color(theme::muted(ui)),
            );
        });

        ui.add_space(8.0);
        theme::card(ui, "Agenda", |ui| {
            if ui.button(format!("{} Exportar agenda (.ics)…", icons::EXPORT)).clicked() {
                action = SettingsAction::ExportIcs;
            }
            ui.label(
                RichText::new("Exporta reuniões, visitas, defesas e prazos para o Outlook / Google Agenda. O arquivo exportado não é criptografado.")
                    .small()
                    .color(theme::muted(ui)),
            );
        });

        ui.add_space(8.0);
        theme::card(ui, "Privacidade", |ui| {
            ui.checkbox(&mut settings.hide_enrollment, "Ocultar a matrícula nas listas e nos detalhes do aluno");
            ui.add_enabled_ui(settings.hide_enrollment, |ui| {
                ui.checkbox(&mut settings.reveal_needs_password, "Pedir a senha para revelar a matrícula");
            });
            ui.add(
                egui::Label::new(
                    RichText::new(
                        "Em muitas instituições a matrícula é o próprio CPF. Com esta opção, ela aparece como \
                         •••.•••.•••-•• até você clicar em “Revelar” nos detalhes do aluno. A matrícula nunca vai \
                         para os arquivos de agenda e convites.",
                    )
                    .small()
                    .color(theme::muted(ui)),
                )
                .wrap_mode(egui::TextWrapMode::Wrap),
            );
        });

        ui.add_space(8.0);
        theme::card(ui, "Segurança", |ui| {
            ui.add(egui::Label::new(format!("Arquivo de dados: {data_path}")).wrap_mode(egui::TextWrapMode::Wrap));
            ui.label(
                RichText::new("Criptografado com Argon2id + XChaCha20-Poly1305. Mantém um backup do arquivo como estava quando você o desbloqueou ou clicou em Salvar pela última vez (.bak).")
                    .small()
                    .color(theme::muted(ui)),
            );
            ui.add_space(6.0);
            widgets::form_grid(ui, "lock-grid", |ui| {
                ui.label("Bloquear automaticamente");
                egui::ComboBox::from_id_salt("auto-lock")
                    .selected_text(auto_lock_label(settings.auto_lock_minutes))
                    .show_ui(ui, |ui| {
                        for minutes in AUTO_LOCK_CHOICES {
                            ui.selectable_value(&mut settings.auto_lock_minutes, minutes, auto_lock_label(minutes));
                        }
                    });
                ui.end_row();
            });
            ui.label(
                RichText::new("Bloquear salva as alterações e fecha os dados; a sua senha os abre de novo.")
                    .small()
                    .color(theme::muted(ui)),
            );
            ui.add_space(6.0);
            ui.label(RichText::new("Alterar senha").strong());
            if weak_password {
                ui.colored_label(
                    theme::fg(ui, theme::AMBER),
                    format!("{} A sua senha é mais fácil de adivinhar do que o Supervisor permite hoje. Escolha uma nova.", icons::WARNING),
                );
            }
            widgets::form_grid(ui, "pw-grid", |ui| {
                for (label, field) in [
                    ("Senha atual", &mut pw.current),
                    ("Nova senha", &mut pw.new),
                    ("Confirme a nova senha", &mut pw.confirm),
                ] {
                    ui.label(label);
                    widgets::password_field(ui, egui::TextEdit::singleline(&mut **field).desired_width(240.0));
                    ui.end_row();
                }
            });
            ui.label(
                RichText::new(format!(
                    "Pelo menos {} caracteres – uma frase curta que você vá lembrar funciona bem.",
                    storage::MIN_PASSWORD_LEN
                ))
                .small()
                .color(theme::muted(ui)),
            );
            if ui.button("Alterar senha").clicked() {
                action = SettingsAction::ChangePassword;
            }
            if let Some((ok, msg)) = &pw.message {
                ui.colored_label(theme::fg(ui, if *ok { theme::GREEN } else { theme::RED }), msg);
            }
        });

        ui.add_space(8.0);
        theme::card(ui, "Arquivo-chave (opcional)", |ui| {
            if key_file_on {
                ui.colored_label(
                    theme::fg(ui, theme::GREEN),
                    format!("{} Ativado – este arquivo de dados só abre com a sua senha e o arquivo-chave deste computador.", icons::KEY),
                );
            } else {
                ui.label("Desativado – só a sua senha abre este arquivo de dados.");
            }
            ui.label(
                RichText::new(
                    "O arquivo-chave é um arquivo pequeno guardado em cada computador que você usa, nunca junto ao arquivo de dados. \
                     Com ele, uma cópia do arquivo de dados é inútil para quem não tiver também o arquivo-chave, \
                     mesmo que adivinhe a sua senha. Use se o arquivo de dados estiver numa pasta sincronizada ou compartilhada.",
                )
                .small()
                .color(theme::muted(ui)),
            );
            ui.label(
                RichText::new(
                    "Você o copia uma vez para cada computador e guarda um backup, por exemplo num pendrive ou num \
                     gerenciador de senhas. Se todas as cópias se perderem, os dados não poderão ser abertos.",
                )
                .small()
                .color(theme::muted(ui)),
            );
            if let Some(path) = storage::key_file_path() {
                ui.label(
                    RichText::new(format!("Local do arquivo-chave neste computador: {}", path.display()))
                        .small()
                        .color(theme::muted(ui)),
                );
            }
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label("Sua senha");
                widgets::password_field(ui, egui::TextEdit::singleline(&mut *kf.password).desired_width(200.0));
                if key_file_on {
                    if ui.button("Remover arquivo-chave").clicked() {
                        action = SettingsAction::RemoveKeyFile;
                    }
                } else if ui.button(format!("{}  Adicionar arquivo-chave…", icons::KEY)).clicked() {
                    action = SettingsAction::AddKeyFile;
                }
            });
            if let Some((ok, msg)) = &kf.message {
                ui.colored_label(theme::fg(ui, if *ok { theme::GREEN } else { theme::RED }), msg);
            }
        });

        ui.add_space(8.0);
        theme::card(ui, "Sobre", |ui| {
            ui.horizontal(|ui| {
                theme::logo(ui, 44.0);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(theme::serif("Supervisor BR", 21.0));
                    ui.label(RichText::new(format!("Versão {}", env!("CARGO_PKG_VERSION"))).color(theme::muted(ui)));
                });
            });
            ui.add_space(4.0);
            ui.add(egui::Label::new("Acompanha alunos de TCC, iniciação científica, estágio, tutoria e pós-graduação (lato e stricto sensu), com bolsas e prazos.").wrap_mode(egui::TextWrapMode::Wrap));
            ui.label(RichText::new("Versão brasileira do Supervisor original.").small().color(theme::muted(ui)));
            ui.hyperlink_to(format!("{} github.com/g-machado-uu/supervisor-br", icons::ARROW_RIGHT), "https://github.com/g-machado-uu/supervisor-br");
            ui.label(
                RichText::new("Feito com egui. Fontes: Inter e DM Serif Display (SIL Open Font License). Ícones: Phosphor (MIT).")
                    .small()
                    .color(theme::muted(ui)),
            );
        });
    });
    action
}

/// Returns true if the module should be removed.
fn module_editor(
    ui: &mut Ui,
    module: &mut Module,
    programmes: &[Programme],
    semester_start: Option<chrono::NaiveDate>,
) -> bool {
    let mut remove = false;
    widgets::form_grid(ui, "module-grid", |ui| {
        ui.label("Código");
        ui.add_sized([200.0, 28.0], egui::TextEdit::singleline(&mut module.code));
        ui.end_row();
        ui.label("Nome");
        widgets::text_field(ui, &mut module.name, "Nome da disciplina");
        ui.end_row();
        ui.label("Semestre");
        ui.horizontal(|ui| {
            for sem in Semester::ALL {
                ui.selectable_value(&mut module.semester, sem, sem.label());
            }
        });
        ui.end_row();
        ui.label("Cursos");
        ui.horizontal_wrapped(|ui| {
            for p in programmes {
                let mut on = module.applies_to.contains(&p.id);
                if ui
                    .checkbox(&mut on, &p.short)
                    .on_hover_text(&p.name)
                    .changed()
                {
                    if on {
                        module.applies_to.push(p.id.clone());
                    } else {
                        module.applies_to.retain(|id| *id != p.id);
                    }
                }
            }
        });
        ui.end_row();
    });

    ui.label(RichText::new("Entregas").strong());
    if module.courseworks.is_empty() {
        ui.label(RichText::new("Nenhuma ainda.").italics().color(theme::muted(ui)));
    }
    let mut remove_cw = None;
    egui::Grid::new("cw-template-grid")
        .num_columns(5)
        .spacing([10.0, 6.0])
        .show(ui, |ui| {
            if !module.courseworks.is_empty() {
                for h in ["Título", "Semana letiva", "Data fixa", "Prazo resultante", ""] {
                    ui.label(RichText::new(h).small().color(theme::muted(ui)));
                }
                ui.end_row();
            }
            for (i, cw) in module.courseworks.iter_mut().enumerate() {
                ui.push_id(cw.id, |ui| {
                    ui.add_sized([200.0, 28.0], egui::TextEdit::singleline(&mut cw.title));
                });
                ui.push_id((cw.id, "week"), |ui| {
                    ui.horizontal(|ui| {
                        let mut has_week = cw.week.is_some();
                        if ui.checkbox(&mut has_week, "").changed() {
                            cw.week = has_week.then_some(1);
                        }
                        if let Some(w) = &mut cw.week {
                            ui.add(egui::DragValue::new(w).range(1..=30).prefix("Semana "));
                        }
                    });
                });
                widgets::opt_date(ui, (cw.id, "deadline"), &mut cw.deadline);
                ui.label(widgets::fmt_date(cw.resolve_deadline(semester_start)));
                if ui.small_button(icons::TRASH).on_hover_text("Remover entrega").clicked() {
                    remove_cw = Some(i);
                }
                ui.end_row();
            }
        });
    if let Some(i) = remove_cw {
        module.courseworks.remove(i);
    }
    ui.horizontal(|ui| {
        if ui.button(format!("{} Adicionar entrega", icons::PLUS)).clicked() {
            module.courseworks.push(CourseworkTemplate::default());
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .small_button(RichText::new("Remover disciplina").color(theme::fg(ui, theme::RED)))
                .on_hover_text("Clique duas vezes para remover esta disciplina")
                .double_clicked()
            {
                remove = true;
            }
        });
    });
    remove
}

/// Courses whose TCC students are supervised; each list name becomes a
/// sidebar list. Courses with students can't be removed.
fn courses_editor(ui: &mut Ui, programmes: &mut Vec<Programme>, in_use: &HashMap<String, usize>) {
    ui.label(
        RichText::new("Cursos com o mesmo nome de lista aparecem juntos na barra lateral. Adicione as disciplinas e entregas abaixo.")
            .small()
            .color(theme::muted(ui)),
    );
    let mut remove = None;
    egui::Grid::new("courses-grid")
        .num_columns(5)
        .spacing([10.0, 6.0])
        .show(ui, |ui| {
            for h in ["Nome", "Sigla", "Lista", "Alunos", ""] {
                ui.label(RichText::new(h).small().color(theme::muted(ui)));
            }
            ui.end_row();
            for (i, p) in programmes.iter_mut().enumerate() {
                ui.push_id(&p.id, |ui| {
                    ui.add_sized([220.0, 28.0], egui::TextEdit::singleline(&mut p.name));
                });
                ui.push_id((&p.id, "short"), |ui| {
                    ui.add_sized([100.0, 28.0], egui::TextEdit::singleline(&mut p.short));
                });
                ui.push_id((&p.id, "list"), |ui| {
                    ui.add_sized([100.0, 28.0], egui::TextEdit::singleline(&mut p.list));
                });
                let used = in_use.get(&p.id).copied().unwrap_or(0);
                ui.label(used.to_string());
                let button = ui.add_enabled(used == 0, egui::Button::new(icons::TRASH).small());
                if button
                    .on_hover_text("Remover curso")
                    .on_disabled_hover_text("Há alunos neste curso – mude-os de curso primeiro")
                    .clicked()
                {
                    remove = Some(i);
                }
                ui.end_row();
            }
        });
    if let Some(i) = remove {
        programmes.remove(i);
    }
    if ui.button(format!("{} Adicionar curso", icons::PLUS)).clicked() {
        programmes.push(Programme::default());
    }
}

/// Months from enrolment to each step, per level; 0 = doesn't apply.
fn level_rules_editor(ui: &mut Ui, rules: &mut Vec<LevelRule>) {
    // Older files, or a level added later: fill in the defaults.
    for level in Level::ALL {
        if !rules.iter().any(|r| r.level == level) {
            rules.push(LevelRule::default_for(level));
        }
    }
    ui.add(
        egui::Label::new(
            RichText::new(
                "Meses a partir do ingresso para cada etapa (0 = não se aplica). Usados em “Preencher prazos \
                 sugeridos” no cadastro do aluno. Cada programa define os seus no regimento; os valores \
                 padrão são apenas um ponto de partida comum.",
            )
            .small()
            .color(theme::muted(ui)),
        )
        .wrap_mode(egui::TextWrapMode::Wrap),
    );
    egui::ScrollArea::horizontal().id_salt("rules-scroll").show(ui, |ui| {
        egui::Grid::new("level-rules")
            .num_columns(5)
            .striped(true)
            .spacing([14.0, 6.0])
            .show(ui, |ui| {
                for h in ["Nível", "Créditos", "Proficiência", "Qualificação", "Defesa / final"] {
                    ui.label(RichText::new(h).small().color(theme::muted(ui)));
                }
                ui.end_row();
                for level in Level::ALL {
                    let Some(r) = rules.iter_mut().find(|r| r.level == level) else { continue };
                    ui.label(level.label());
                    for (i, value) in [
                        &mut r.credits_months,
                        &mut r.proficiency_months,
                        &mut r.qualification_months,
                        &mut r.final_months,
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        ui.push_id((level.label(), i), |ui| {
                            ui.add(egui::DragValue::new(value).range(0..=96).suffix(" meses"));
                        });
                    }
                    ui.end_row();
                }
            });
    });
    if ui.small_button("Restaurar valores padrão").clicked() {
        *rules = default_level_rules();
    }
    ui.add(
        egui::Label::new(
            RichText::new(
                "Bolsas: mestrado até 24 meses e doutorado até 48 (CAPES, Portaria 76/2010, art. 10; CNPq e a \
                 maioria das FAPs); IC com vigência de 12 meses, renovável (CNPq RN-017/2006). Prorrogações: até \
                 180 dias por parto, adoção ou guarda (Lei 14.925/2024).",
            )
            .small()
            .color(theme::muted(ui)),
        )
        .wrap_mode(egui::TextWrapMode::Wrap),
    );
}
