//! Unlock screen (and first-run password creation).

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

use anyhow::Result;
use egui::{Align, Key, Layout, RichText};
use zeroize::Zeroizing;

use crate::model::Database;
use crate::storage::{self, KeyFile, Vault};
use crate::ui::{icons, theme, widgets};

/// What the lock screen hands over once the data is open.
pub struct Unlocked {
    pub vault: Vault,
    pub db: Database,
    /// The password is weaker than new passwords may be; suggest changing it.
    pub weak_password: bool,
}

/// Whether the data file needs a key file, and whether this computer has the right one.
enum KeyState {
    NotNeeded,
    Ready(KeyFile),
    Missing([u8; 32]),
    Wrong([u8; 32]),
}

impl KeyState {
    fn of(path: &Path) -> Self {
        let Ok(Some(id)) = storage::required_key_id(path) else {
            return Self::NotNeeded;
        };
        match storage::local_key_file() {
            Ok(Some(key)) if key.id() == id => Self::Ready(key),
            Ok(Some(_)) => Self::Wrong(id),
            _ => Self::Missing(id),
        }
    }
}

pub struct LockScreen {
    pub path: PathBuf,
    password: Zeroizing<String>,
    confirm: Zeroizing<String>,
    error: Option<String>,
    key: KeyState,
    weak_password: bool,
    pending: Option<Receiver<Result<(Vault, Database)>>>,
}

impl LockScreen {
    pub fn new(path: PathBuf) -> Self {
        Self {
            key: KeyState::of(&path),
            path,
            password: Zeroizing::new(String::new()),
            confirm: Zeroizing::new(String::new()),
            error: None,
            weak_password: false,
            pending: None,
        }
    }

    fn first_run(&self) -> bool {
        !self.path.exists()
    }

    fn submit(&mut self, ctx: &egui::Context) {
        let creating = self.first_run();
        if creating {
            if let Some(problem) = storage::password_problem(&self.password) {
                self.error = Some(problem);
                return;
            }
            if *self.password != *self.confirm {
                self.error = Some("As senhas não coincidem.".into());
                return;
            }
        } else if self.password.is_empty() {
            return;
        }
        // The file may have changed (e.g. synced) since this screen opened.
        self.key = KeyState::of(&self.path);
        let key_file = match &self.key {
            KeyState::NotNeeded => None,
            KeyState::Ready(key) => Some(key.clone()),
            KeyState::Missing(_) | KeyState::Wrong(_) => return,
        };
        self.weak_password = !creating && storage::password_problem(&self.password).is_some();
        self.error = None;
        let (tx, rx) = channel();
        let path = self.path.clone();
        let password = std::mem::replace(&mut self.password, Zeroizing::new(String::new()));
        self.confirm = Zeroizing::new(String::new());
        let ctx = ctx.clone();
        // Key derivation is deliberately slow; keep the UI responsive.
        std::thread::spawn(move || {
            let result = if creating {
                let db = Database::default();
                Vault::create(&path, &password, &db).map(|v| (v, db))
            } else {
                Vault::open(&path, &password, key_file.as_ref())
            };
            let _ = tx.send(result);
            ctx.request_repaint();
        });
        self.pending = Some(rx);
    }

    /// Draws the screen; returns the unlocked data once ready.
    pub fn ui(&mut self, ui: &mut egui::Ui) -> Option<Unlocked> {
        if let Some(rx) = &self.pending
            && let Ok(result) = rx.try_recv()
        {
            self.pending = None;
            match result {
                Ok((vault, db)) => {
                    storage::remember_path(&self.path);
                    return Some(Unlocked {
                        vault,
                        db,
                        weak_password: self.weak_password,
                    });
                }
                Err(e) => self.error = Some(e.to_string()),
            }
        }

        let first_run = self.first_run();
        let busy = self.pending.is_some();
        let needs_key = matches!(self.key, KeyState::Missing(_) | KeyState::Wrong(_));
        let width = ui.available_width();
        if width > 900.0 {
            egui::Panel::left("lock-brand")
                .exact_size((width * 0.42).min(560.0))
                .frame(egui::Frame::new().fill(theme::sidebar_fill(ui)).inner_margin(egui::Margin::same(48)))
                .show_separator_line(false)
                .resizable(false)
                .show(ui, |ui| {
                    ui.add_space((ui.available_height() * 0.22).max(10.0));
                    brand(ui, Align::Min);
                    ui.add_space(28.0);
                    for (icon, title, text) in [
                        (icons::BOOK_OPEN_TEXT, "TCC e iniciação científica", "Reuniões, entregas, relatórios e defesas"),
                        (icons::GRADUATION_CAP, "Pós-graduação", "Lato e stricto sensu, com prazos de qualificação e defesa"),
                        (icons::COINS, "Bolsas", "CNPq, CAPES, FAPs e outras, com vigência e prorrogações"),
                        (icons::LOCK_SIMPLE, "Criptografado", "Protegido por senha, com arquivo-chave opcional"),
                    ] {
                        ui.horizontal(|ui| {
                            theme::icon_badge(ui, icon, theme::ACCENT, 38.0);
                            ui.vertical(|ui| {
                                ui.spacing_mut().item_spacing.y = 1.0;
                                ui.label(theme::semibold(title));
                                ui.label(RichText::new(text).small().color(theme::muted(ui)));
                            });
                        });
                        ui.add_space(8.0);
                    }
                });
        }
        egui::CentralPanel::default().show(ui, |ui| {
            ui.with_layout(Layout::top_down(Align::Center), |ui| {
                ui.add_space((ui.available_height() * 0.2).max(20.0));
                if width <= 900.0 {
                    brand(ui, Align::Center);
                    ui.add_space(18.0);
                }

                ui.scope(|ui| {
                    ui.set_max_width(400.0);
                    theme::card_frame(ui).inner_margin(egui::Margin::same(28)).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.vertical_centered(|ui| {
                            theme::icon_badge(ui, icons::LOCK_SIMPLE, theme::ACCENT, 48.0);
                            ui.add_space(4.0);
                            if first_run {
                                ui.label(theme::serif("Crie uma senha", 24.0));
                                ui.label(
                                    RichText::new(
                                        "Seus dados serão criptografados com esta senha.\n\
                                         Não há como recuperá-la se você esquecer.",
                                    )
                                    .color(theme::fg(ui, theme::AMBER)),
                                );
                                ui.label(
                                    RichText::new(format!(
                                        "Pelo menos {} caracteres – uma frase curta que você vá lembrar funciona bem.",
                                        storage::MIN_PASSWORD_LEN
                                    ))
                                    .small()
                                    .color(theme::muted(ui)),
                                );
                            } else {
                                ui.label(theme::serif("Bem-vindo(a) de volta", 24.0));
                                ui.label(RichText::new("Digite a sua senha para desbloquear os dados.").color(theme::muted(ui)));
                            }
                            if let KeyState::Missing(id) | KeyState::Wrong(id) = self.key {
                                ui.add_space(8.0);
                                let text = if matches!(self.key, KeyState::Missing(_)) {
                                    "Este arquivo de dados também precisa do seu arquivo-chave, que ainda não está neste computador. \
                                     Importe-o da sua cópia de backup (por exemplo, de um pendrive)."
                                } else {
                                    "O arquivo-chave deste computador pertence a outro arquivo de dados. \
                                     Importe o correto da sua cópia de backup."
                                };
                                ui.label(RichText::new(text).color(theme::fg(ui, theme::AMBER)));
                                if ui
                                    .add_enabled(!busy, egui::Button::new(format!("{}  Importar arquivo-chave…", icons::KEY)))
                                    .clicked()
                                    && let Some(src) = rfd::FileDialog::new()
                                        .set_title("Escolha o seu arquivo-chave do Supervisor")
                                        .pick_file()
                                {
                                    match storage::import_key_file(&src, id) {
                                        Ok(key) => {
                                            self.key = KeyState::Ready(key);
                                            self.error = None;
                                        }
                                        Err(e) => self.error = Some(format!("{e:#}")),
                                    }
                                }
                            }
                            ui.add_space(10.0);

                            let pw = ui
                                .add_enabled_ui(!busy, |ui| {
                                    widgets::password_field(
                                        ui,
                                        egui::TextEdit::singleline(&mut *self.password)
                                            .hint_text("Senha")
                                            .desired_width(300.0)
                                            .margin(egui::vec2(10.0, 8.0)),
                                    )
                                })
                                .inner;
                            if !busy && self.error.is_none() && !pw.has_focus() && self.password.is_empty() {
                                pw.request_focus();
                            }
                            let mut enter = pw.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                            if first_run {
                                let c = ui
                                    .add_enabled_ui(!busy, |ui| {
                                        widgets::password_field(
                                            ui,
                                            egui::TextEdit::singleline(&mut *self.confirm)
                                                .hint_text("Confirme a senha")
                                                .desired_width(300.0)
                                                .margin(egui::vec2(10.0, 8.0)),
                                        )
                                    })
                                    .inner;
                                if enter {
                                    c.request_focus();
                                    enter = false;
                                }
                                enter |= c.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
                            }

                            ui.add_space(8.0);
                            let label = if first_run { "Criar e desbloquear" } else { "Desbloquear" };
                            let button = theme::primary_button(label).min_size(egui::vec2(300.0, 38.0));
                            if busy {
                                ui.horizontal(|ui| {
                                    ui.spinner();
                                    ui.label("Desbloqueando…");
                                });
                            } else if ui.add_enabled(!needs_key, button).clicked() || (enter && !needs_key) {
                                self.submit(ui.ctx());
                            }
                            if let Some(err) = &self.error {
                                ui.colored_label(theme::fg(ui, theme::RED), err);
                            }
                        });
                    });
                });

                ui.add_space(12.0);
                ui.label(
                    RichText::new(format!("Arquivo de dados: {}", self.path.display()))
                        .small()
                        .color(theme::muted(ui)),
                );
                if matches!(self.key, KeyState::Ready(_)) {
                    ui.label(
                        RichText::new(format!("{} Arquivo-chave encontrado neste computador", icons::KEY))
                            .small()
                            .color(theme::muted(ui)),
                    );
                }
                ui.horizontal(|ui| {
                    ui.add_space((ui.available_width() - 330.0).max(0.0) / 2.0);
                    if ui.add_enabled(!busy, egui::Button::new("Abrir outro arquivo…").small()).clicked()
                        && let Some(p) = rfd::FileDialog::new()
                            .add_filter("Dados do Supervisor", &["svt"])
                            .pick_file()
                    {
                        *self = Self::new(p);
                    }
                    if ui.add_enabled(!busy, egui::Button::new("Criar arquivo novo em…").small()).clicked()
                        && let Some(p) = rfd::FileDialog::new()
                            .add_filter("Dados do Supervisor", &["svt"])
                            .set_file_name("data.svt")
                            .save_file()
                    {
                        if p.exists() {
                            self.error = Some("Esse arquivo já existe – use “Abrir outro arquivo…”.".into());
                        } else {
                            *self = Self::new(p);
                        }
                    }
                });
            });
        });
        None
    }
}

fn brand(ui: &mut egui::Ui, align: Align) {
    ui.with_layout(Layout::top_down(align), |ui| {
        theme::logo(ui, 64.0);
        ui.add_space(6.0);
        ui.label(theme::serif("Supervisor BR", 40.0));
        ui.label(RichText::new("TCC · Iniciação científica · Pós-graduação").color(theme::muted(ui)));
        ui.label(RichText::new(format!("Versão {}", env!("CARGO_PKG_VERSION"))).color(theme::muted(ui)));

    });
}
