//! Application shell: lock state, navigation, autosave and dialogs.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::{Duration, Instant};

use chrono::NaiveDate;
use egui::{Id, RichText, Ui};
use uuid::Uuid;
use zeroize::Zeroizing;

use crate::ics;
use crate::model::{
    AdviseeRecord, Attention, CalItem, Database, Event, IcRecord, Level, MeetingChange, PlacementRecord, PosGroup,
    PosRecord, Student, TccRecord,
};
use crate::pt;
use crate::storage::{self, KeyFile, Vault, VaultError, Written};
use crate::ui::calendar::{self, CalAction, CalendarState};
use crate::ui::dashboard::{self, DashAction};
use crate::ui::lock::{LockScreen, Unlocked};
use crate::ui::settings::{self, KeyFileForm, PasswordForm, SettingsAction};
use crate::ui::student_form::{self, FormAction, Role};
use crate::ui::student_list::{self, Category, ListState};
use crate::ui::{icons, theme, widgets};

const AUTOSAVE_DELAY: f64 = 1.0;

pub struct SupervisorApp {
    screen: Screen,
}

enum Screen {
    Locked(LockScreen),
    Unlocked(Box<Session>),
}

impl SupervisorApp {
    /// `data_file` overrides the last-used data file (e.g. from the command line).
    pub fn new(cc: &eframe::CreationContext<'_>, data_file: Option<std::path::PathBuf>) -> Self {
        theme::install(&cc.egui_ctx);
        // The saved preference is encrypted; follow the OS until unlocked.
        cc.egui_ctx.set_theme(egui::ThemePreference::System);
        Self {
            screen: Screen::Locked(LockScreen::new(data_file.unwrap_or_else(storage::last_used_path))),
        }
    }
}

impl SupervisorApp {
    /// Drop the session, which wipes the key, and egui's copies of what was
    /// on screen, such as the undo history every text field keeps.
    fn lock(&mut self, ctx: &egui::Context) {
        let Screen::Unlocked(s) = &self.screen else { return };
        let path = s.vault.path().to_path_buf();
        self.screen = Screen::Locked(LockScreen::new(path));
        ctx.data_mut(|d| d.clear());
        ctx.forget_all_images();
    }
}

impl eframe::App for SupervisorApp {
    // Also runs while the window is hidden, so idle locking works then too.
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let Screen::Unlocked(s) = &mut self.screen else { return };
        let now = ctx.input(|i| i.time);
        s.poll_save(now, false);
        if ctx.input(|i| i.viewport().close_requested()) {
            if s.discard {
                s.leaving = Some(Leave::Quit);
            } else {
                s.detect_changes(now);
                s.leave(ctx, Leave::Quit);
            }
            if s.ready_to_leave().is_none() {
                // Not saved yet: stay open, and close once saved or say why not.
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            }
        }
        s.check_idle(ctx);
        if s.ready_to_leave() == Some(Leave::Lock) {
            self.lock(ctx);
        }
    }

    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        match &mut self.screen {
            Screen::Locked(lock) => {
                if let Some(unlocked) = lock.ui(ui) {
                    self.screen = Screen::Unlocked(Box::new(Session::new(unlocked)));
                }
            }
            Screen::Unlocked(session) => match session.ui(ui) {
                Some(Leave::Lock) => self.lock(ui.ctx()),
                Some(Leave::Quit) => ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close),
                None => {}
            },
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
enum View {
    Dashboard,
    Calendar,
    List(Category),
    Settings,
}

enum SaveStatus {
    Saved,
    Pending,
    Error(String),
    /// The file was written elsewhere since it was unlocked; saving waits for the user.
    Conflict,
}

/// Lock or quit, once nothing is left unsaved.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Leave {
    Lock,
    Quit,
}

/// Adding a key file: it's already in place on this computer; the data file
/// is re-keyed only after a backup copy has been saved.
struct KeyFileSetup {
    key: KeyFile,
    path: PathBuf,
    password: Zeroizing<String>,
    backup: Option<PathBuf>,
    error: Option<String>,
}

enum Confirm {
    DeleteStudent(Uuid),
    RemoveRole(Uuid, Role),
}

/// Asking for the password before showing an enrolment number.
struct RevealPrompt {
    student: Uuid,
    password: Zeroizing<String>,
    error: Option<String>,
}

struct NewStudent {
    student: Student,
    tcc: bool,
    programme: String,
    ic: bool,
    pos: bool,
    level: Level,
    placement: bool,
    company: String,
    professional_email: String,
    advisee: bool,
}

impl NewStudent {
    fn for_category(cat: Option<&Category>, settings: &crate::model::Settings) -> Self {
        let first = settings
            .programmes
            .first()
            .map(|p| p.id.clone())
            .unwrap_or_default();
        let mut n = Self {
            student: Student::default(),
            tcc: false,
            programme: first,
            ic: false,
            pos: false,
            level: Level::MestradoAcademico,
            placement: false,
            company: String::new(),
            professional_email: String::new(),
            advisee: false,
        };
        match cat {
            Some(Category::Tcc(list)) => {
                n.tcc = true;
                if let Some(p) = settings.programmes.iter().find(|p| p.list_name() == list) {
                    n.programme = p.id.clone();
                }
            }
            Some(Category::Ic) => n.ic = true,
            Some(Category::Pos(group)) => {
                n.pos = true;
                n.level = group.level();
            }
            Some(Category::Placement) => n.placement = true,
            Some(Category::Advisee) => n.advisee = true,
            _ => {}
        }
        n
    }

    fn build(self) -> Student {
        let mut s = self.student;
        if self.tcc {
            s.tcc = Some(TccRecord {
                programme: self.programme,
                ..Default::default()
            });
        }
        if self.ic {
            s.ic = Some(IcRecord::new());
        }
        if self.pos {
            s.pos = Some(PosRecord::new(self.level));
        }
        if self.placement {
            s.placement = Some(PlacementRecord {
                company: self.company,
                professional_email: self.professional_email,
                ..Default::default()
            });
        }
        if self.advisee {
            s.advisee = Some(AdviseeRecord::default());
        }
        s
    }
}

/// Lists the Dashboard and Calendar draw on every frame, rebuilt only when
/// the data or the date changes.
pub(crate) struct Derived {
    print: [u8; 32],
    today: NaiveDate,
    pub events: Rc<[Event]>,
    pub attention: Rc<[Attention]>,
}

impl Derived {
    /// The lists for `db`, whose fingerprint is `print`, from `cache` if
    /// that is still current.
    pub(crate) fn of<'c>(cache: &'c mut Option<Self>, db: &Database, print: [u8; 32], today: NaiveDate) -> &'c Self {
        let stale = cache.as_ref().is_none_or(|d| d.print != print || d.today != today);
        if stale {
            *cache = Some(Self {
                print,
                today,
                events: db.events().into(),
                attention: db.attention(today).into(),
            });
        }
        cache.as_ref().expect("just filled")
    }
}

struct Session {
    vault: Vault,
    db: Database,
    view: View,
    selected: Option<Uuid>,
    search: String,
    show_archived: bool,
    dirty_since: Option<f64>,
    status: SaveStatus,
    new_student: Option<NewStudent>,
    confirm: Option<Confirm>,
    password_form: PasswordForm,
    key_form: KeyFileForm,
    key_setup: Option<KeyFileSetup>,
    calendar: CalendarState,
    toast: Option<(String, f64)>,
    applied_dark: Option<bool>,
    /// Fingerprint of `db` when last checked, to spot edits.
    db_print: [u8; 32],
    /// Events and attention items for `db_print` and today.
    derived: Option<Derived>,
    /// A save running in the background, and the `db_print` it started from.
    saving: Option<(Receiver<anyhow::Result<Written>>, [u8; 32])>,
    /// Save was clicked: save without waiting for a pause in typing, then
    /// make what is saved the restore point.
    save_requested: bool,
    last_active: Instant,
    leaving: Option<Leave>,
    /// Lock or quit even though the latest changes aren't saved.
    discard: bool,
    weak_password: bool,
    /// Shown once the session is on screen.
    notice: Option<String>,
    /// The student whose enrolment number is shown, until another is opened.
    revealed: Option<Uuid>,
    reveal_prompt: Option<RevealPrompt>,
}

impl Session {
    fn new(unlocked: Unlocked) -> Self {
        let Unlocked { vault, db, weak_password } = unlocked;
        Self {
            db_print: storage::fingerprint(&db),
            derived: None,
            saving: None,
            save_requested: false,
            vault,
            db,
            view: View::Dashboard,
            selected: None,
            search: String::new(),
            show_archived: false,
            dirty_since: None,
            status: SaveStatus::Saved,
            new_student: None,
            confirm: None,
            password_form: PasswordForm::default(),
            key_form: KeyFileForm::default(),
            key_setup: None,
            calendar: CalendarState::default(),
            toast: None,
            applied_dark: None,
            last_active: Instant::now(),
            leaving: None,
            discard: false,
            weak_password,
            notice: weak_password.then(|| {
                "A sua senha é mais fácil de adivinhar do que o Supervisor permite hoje – altere-a em Configurações › Segurança."
                    .to_string()
            }),
            revealed: None,
            reveal_prompt: None,
        }
    }

    /// Start saving in the background, unless a save is already running.
    /// Serialising and encrypting happen here; only ciphertext goes to the
    /// worker, which writes it to disk. `poll_save` picks up the result.
    fn save(&mut self, ctx: &egui::Context) {
        if self.saving.is_some() {
            return;
        }
        let job = self.vault.seal(&self.db).map(|bytes| (self.vault.writer(), bytes));
        let (tx, rx) = channel();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(job.and_then(|(writer, bytes)| writer.write(&bytes)));
            ctx.request_repaint();
        });
        self.saving = Some((rx, self.db_print));
    }

    /// Take the result of the background save if it has finished, or with
    /// `wait`, once it does.
    fn poll_save(&mut self, now: f64, wait: bool) {
        let Some((rx, started_from)) = &self.saving else { return };
        let received = if wait {
            rx.recv().map_err(|_| TryRecvError::Disconnected)
        } else {
            rx.try_recv()
        };
        let result = match received {
            Ok(result) => result,
            Err(TryRecvError::Empty) => return,
            Err(TryRecvError::Disconnected) => Err(anyhow::anyhow!("o salvamento parou inesperadamente")),
        };
        let started_from = *started_from;
        self.saving = None;
        match result {
            Ok(written) => {
                self.vault.saved(written);
                // Edits made while it was saving stay pending for the next save.
                if self.db_print == started_from {
                    self.saved(Ok(()));
                    if self.save_requested {
                        self.mark_restore_point(now);
                    }
                }
            }
            Err(e) => {
                self.saved(Err(e));
                match self.status {
                    // Back off before retrying.
                    SaveStatus::Error(_) => self.dirty_since = Some(now + 4.0),
                    // The conflict dialog takes over.
                    SaveStatus::Conflict => self.save_requested = false,
                    _ => {}
                }
            }
        }
    }

    /// Save straight away (the Save button or Ctrl+S), and make what is saved
    /// the restore point: the next save keeps it as the `.bak`.
    fn save_now(&mut self, ctx: &egui::Context, now: f64) {
        if matches!(self.status, SaveStatus::Conflict) {
            return;
        }
        if self.dirty_since.is_none() && self.saving.is_none() {
            self.mark_restore_point(now);
        } else {
            self.save_requested = true;
            if self.dirty_since.is_some() {
                self.save(ctx);
            }
        }
    }

    fn mark_restore_point(&mut self, now: f64) {
        self.save_requested = false;
        self.vault.mark_restore_point();
        self.toast = Some(("Salvo".into(), now));
    }

    /// Wait for a background save, before writing the file from this thread.
    fn wait_for_save(&mut self, ui: &Ui) {
        self.poll_save(ui.input(|i| i.time), true);
    }

    fn saved(&mut self, result: anyhow::Result<()>) {
        match result {
            Ok(()) => {
                self.status = SaveStatus::Saved;
                self.dirty_since = None;
            }
            // Written elsewhere: ask before replacing it.
            Err(e) if storage::vault_error(&e) == Some(VaultError::ChangedOnDisk) => {
                self.status = SaveStatus::Conflict;
            }
            Err(e) => {
                // Keep it dirty so the next autosave retries.
                self.status = SaveStatus::Error(format!("{e:#}"));
            }
        }
    }

    /// Mark the data unsaved if anything changed since the last check.
    fn detect_changes(&mut self, now: f64) {
        let print = storage::fingerprint(&self.db);
        if print != self.db_print {
            self.db_print = print;
            self.dirty_since = Some(now);
            if !matches!(self.status, SaveStatus::Conflict) {
                self.status = SaveStatus::Pending;
            }
        }
    }

    /// Ask to lock or quit. It happens once nothing is left unsaved; if
    /// saving fails, a dialog says so and offers to go ahead anyway.
    fn leave(&mut self, ctx: &egui::Context, leave: Leave) {
        if self.leaving.is_none() || leave == Leave::Quit {
            self.leaving = Some(leave);
        }
        if self.dirty_since.is_some() && !matches!(self.status, SaveStatus::Conflict) {
            self.save(ctx);
        }
    }

    fn ready_to_leave(&self) -> Option<Leave> {
        self.leaving
            .filter(|_| (self.dirty_since.is_none() || self.discard) && self.saving.is_none())
    }

    /// Lock after the chosen time without input. Uses the wall clock, as
    /// egui's time stands still while the window is hidden.
    fn check_idle(&mut self, ctx: &egui::Context) {
        let minutes = self.db.settings.auto_lock_minutes;
        if minutes == 0 || self.leaving.is_some() {
            return;
        }
        let limit = Duration::from_secs(u64::from(minutes) * 60);
        match limit.checked_sub(self.last_active.elapsed()) {
            Some(left) if !left.is_zero() => ctx.request_repaint_after(left),
            _ => self.leave(ctx, Leave::Lock),
        }
    }

    /// Events and attention items as of the last change check, which runs
    /// on every frame that can edit.
    fn derived(&mut self) -> &Derived {
        Derived::of(&mut self.derived, &self.db, self.db_print, widgets::today())
    }

    fn notify(&mut self, ui: &Ui, msg: impl Into<String>) {
        self.toast = Some((msg.into(), ui.input(|i| i.time)));
    }

    /// Draws the unlocked app. Returns what to do once nothing is left unsaved.
    fn ui(&mut self, ui: &mut Ui) -> Option<Leave> {
        let now = ui.input(|i| i.time);
        let mut lock = false;
        let mut save_now = false;
        if ui.input(|i| !i.events.is_empty()) {
            self.last_active = Instant::now();
        }
        if let Some(msg) = self.notice.take() {
            self.notify(ui, msg);
        }
        // A revealed enrolment number is hidden again once another student is opened.
        if self.revealed.is_some() && self.revealed != self.selected {
            self.revealed = None;
        }

        if ui.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::L))) {
            lock = true;
        }
        if ui.input_mut(|i| i.consume_shortcut(&egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::S))) {
            save_now = true;
        }

        self.sidebar(ui, &mut lock, &mut save_now);
        if self.view != View::Settings && self.selected.is_some() {
            self.detail_panel(ui);
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::central_panel(ui.style())
                    .inner_margin(egui::Margin { left: 28, right: 28, top: 22, bottom: 12 }),
            )
            .show(ui, |ui| self.central(ui));
        self.toast(ui);
        self.dialogs(ui);

        // Every edit needs a click, key press or drag; frames from just moving
        // the mouse or a blinking cursor skip the (whole-database) check.
        let may_edit = ui.input(|i| {
            i.pointer.any_down()
                || i.events.iter().any(|e| {
                    !matches!(e, egui::Event::PointerMoved(_) | egui::Event::MouseMoved(_))
                })
        });
        if may_edit {
            self.detect_changes(now);
        }
        if self.applied_dark != Some(self.db.settings.dark_mode) {
            self.applied_dark = Some(self.db.settings.dark_mode);
            theme::apply(ui.ctx(), self.db.settings.dark_mode);
        }
        if lock {
            self.leave(ui.ctx(), Leave::Lock);
        } else if save_now {
            self.save_now(ui.ctx(), now);
        }
        if let Some(since) = self.dirty_since
            && !matches!(self.status, SaveStatus::Conflict)
            && self.saving.is_none()
        {
            // Locking, quitting and the Save button don't wait for a pause in
            // typing, only for the back-off after a failed save.
            let wait = match self.status {
                SaveStatus::Pending if self.leaving.is_some() || self.save_requested => 0.0,
                _ => AUTOSAVE_DELAY - (now - since),
            };
            if wait <= 0.0 {
                self.save(ui.ctx());
            } else {
                ui.ctx().request_repaint_after(Duration::from_secs_f64(wait));
            }
        }
        self.ready_to_leave()
    }

    /// The latest message, floating at the bottom of the window for a few seconds.
    fn toast(&mut self, ui: &mut Ui) {
        let now = ui.input(|i| i.time);
        let Some((msg, at)) = &self.toast else { return };
        if now - at >= 5.0 {
            self.toast = None;
            return;
        }
        ui.ctx().request_repaint_after(Duration::from_millis(500));
        egui::Area::new(Id::unique("toast"))
            .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(115.0, -24.0))
            .order(egui::Order::Foreground)
            .interactable(false)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style())
                    .corner_radius(egui::CornerRadius::same(12))
                    .inner_margin(egui::Margin::symmetric(16, 10))
                    .show(ui, |ui| {
                        ui.set_max_width(560.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(icons::BELL).size(16.0).color(theme::fg(ui, theme::ACCENT)));
                            ui.label(msg);
                        });
                    });
            });
    }

    fn sidebar(&mut self, ui: &mut Ui, lock: &mut bool, save_now: &mut bool) {
        let frame = egui::Frame::new()
            .fill(theme::sidebar_fill(ui))
            .inner_margin(egui::Margin { left: 14, right: 14, top: 18, bottom: 14 });
        egui::Panel::left("nav")
            .exact_size(theme::SIDEBAR_WIDTH)
            .frame(frame)
            .show_separator_line(false)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    theme::logo(ui, 42.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(theme::serif("Supervisor BR", 21.0));
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&self.db.settings.academic_year).small().color(theme::muted(ui)));
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label(RichText::new(theme::VERSION).small().color(theme::muted(ui)));
                            });
                        });
                    });
                });
                ui.add_space(12.0);

                // The buttons at the bottom get their own strip, so a long
                // list of lists scrolls instead of running under them.
                egui::Panel::bottom("nav-bottom")
                    .frame(egui::Frame::NONE)
                    .show_separator_line(false)
                    .show(ui, |ui| self.sidebar_bottom(ui, lock, save_now));

                ui.spacing_mut().item_spacing.y = 3.0;
                let section = |ui: &mut Ui, text: &str| {
                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        ui.add_space(14.0);
                        ui.label(
                            theme::semibold(text)
                                .size(10.5)
                                .extra_letter_spacing(1.0)
                                .color(theme::muted(ui)),
                        );
                    });
                    ui.add_space(2.0);
                };
                let lists = self.db.settings.programme_lists();
                let mut nav = |ui: &mut Ui, view: View, icon: &str, label: &str, count: Option<usize>| {
                    if theme::nav_item(ui, icon, label, count, self.view == view).clicked() {
                        self.view = view;
                    }
                };
                egui::ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                    nav(ui, View::Dashboard, icons::HOUSE, "Painel", None);
                    nav(ui, View::Calendar, icons::CALENDAR_DOTS, "Agenda", None);
                    section(ui, "GRADUAÇÃO");
                    for list in lists {
                        let cat = Category::Tcc(list.clone());
                        let count = cat.count(&self.db);
                        let label = if list == "TCC" { "TCC".to_string() } else { format!("TCC – {list}") };
                        nav(ui, View::List(cat), icons::BOOK_OPEN_TEXT, &label, Some(count));
                    }
                    nav(ui, View::List(Category::Ic), icons::FLASK, "Iniciação científica", Some(Category::Ic.count(&self.db)));
                    nav(ui, View::List(Category::Placement), icons::BUILDINGS, "Estágio", Some(Category::Placement.count(&self.db)));
                    nav(ui, View::List(Category::Advisee), icons::CHAT_CIRCLE_TEXT, "Tutoria", Some(Category::Advisee.count(&self.db)));
                    section(ui, "PÓS-GRADUAÇÃO");
                    for group in PosGroup::ALL {
                        let cat = Category::Pos(group);
                        let count = cat.count(&self.db);
                        nav(ui, View::List(cat), icons::GRADUATION_CAP, group.label(), Some(count));
                    }
                    section(ui, "GERAL");
                    nav(ui, View::List(Category::All), icons::USERS_THREE, "Todos os alunos", Some(Category::All.count(&self.db)));
                });
            });
    }

    /// Settings, Lock, the light/dark switch and the save status.
    fn sidebar_bottom(&mut self, ui: &mut Ui, lock: &mut bool, save_now: &mut bool) {
        ui.add_space(6.0);
        if theme::nav_item(ui, icons::GEAR_SIX, "Configurações", None, self.view == View::Settings).clicked() {
            self.view = View::Settings;
        }
        ui.spacing_mut().item_spacing.y = 6.0;
        // Lock and the theme switch on the left, the save status on the
        // right; it moves to its own line when the three don't fit.
        ui.horizontal_wrapped(|ui| {
            if ui
                .add(egui::Button::new(format!("{}  Bloquear", icons::LOCK_SIMPLE)).corner_radius(egui::CornerRadius::same(10)))
                .on_hover_text("Bloquear (Ctrl+L)")
                .clicked()
            {
                *lock = true;
            }
            let dark = &mut self.db.settings.dark_mode;
            if ui
                .add(
                    egui::Button::new(RichText::new(if *dark { icons::SUN } else { icons::MOON }).size(16.0))
                        .corner_radius(egui::CornerRadius::same(10)),
                )
                .on_hover_text("Alternar claro/escuro")
                .clicked()
            {
                *dark = !*dark;
            }
            // The save status doubles as the Save button.
            {
                let (text, color, hover) = match &self.status {
                    SaveStatus::Conflict => {
                        ui.label(RichText::new(format!("{} Não salvo", icons::WARNING)).small().color(theme::fg(ui, theme::RED)))
                            .on_hover_text("O arquivo de dados foi alterado em outro lugar");
                        return;
                    }
                    SaveStatus::Saved => (
                        format!("{} Salvo", icons::FLOPPY_DISK),
                        theme::fg(ui, theme::GREEN),
                        "Todas as alterações estão salvas e criptografadas.\nClique para salvar agora e manter esta versão como backup (Ctrl+S).".to_string(),
                    ),
                    SaveStatus::Pending => ("Salvando…".to_string(), theme::muted(ui), "Clique para salvar agora (Ctrl+S)".to_string()),
                    SaveStatus::Error(e) => (
                        format!("{} Falha ao salvar", icons::WARNING),
                        theme::fg(ui, theme::RED),
                        format!("{e}\nClique para tentar de novo."),
                    ),
                };
                let button = egui::Button::new(RichText::new(text).small().color(color))
                    .frame_when_inactive(false)
                    .corner_radius(egui::CornerRadius::same(10));
                if ui.add(button).on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(hover).clicked() {
                    *save_now = true;
                }
            }
        });
    }

    fn detail_panel(&mut self, ui: &mut Ui) {
        let Some(id) = self.selected else { return };
        let dup = self
            .db
            .student(id)
            .is_some_and(|s| self.db.enrollment_taken(&s.enrollment, Some(id)));
        let revealed = self.revealed == Some(id);
        let Database { settings, students, .. } = &mut self.db;
        let Some(student) = students.iter_mut().find(|s| s.id == id) else {
            self.selected = None;
            return;
        };
        let mut action = FormAction::None;
        let frame = egui::Frame::side_top_panel(ui.style())
            .fill(ui.visuals().window_fill)
            .inner_margin(egui::Margin { left: 20, right: 14, top: 20, bottom: 8 });
        egui::Panel::right("detail")
            .resizable(true)
            .default_size(540.0)
            .min_size(460.0)
            .frame(frame)
            .show(ui, |ui| {
                action = student_form::show(ui, student, settings, dup, revealed);
            });
        match action {
            FormAction::None => {}
            FormAction::Close => self.selected = None,
            FormAction::Delete => self.confirm = Some(Confirm::DeleteStudent(id)),
            FormAction::RemoveRole(role) => self.confirm = Some(Confirm::RemoveRole(id, role)),
            FormAction::Invite(uid) => self.save_invite(ui, id, &uid),
            FormAction::Reveal if self.db.settings.reveal_needs_password => {
                self.reveal_prompt = Some(RevealPrompt {
                    student: id,
                    password: Zeroizing::new(String::new()),
                    error: None,
                });
            }
            FormAction::Reveal => self.revealed = Some(id),
            FormAction::Hide => self.revealed = None,
        }
    }

    fn central(&mut self, ui: &mut Ui) {
        match self.view.clone() {
            View::Dashboard => {
                let d = self.derived();
                let (events, attention) = (Rc::clone(&d.events), Rc::clone(&d.attention));
                match dashboard::show(ui, &self.db, &events, &attention) {
                    DashAction::None => {}
                    DashAction::Open(id) => self.selected = Some(id),
                    DashAction::Goto(cat) => self.view = View::List(cat),
                    DashAction::ExportIcs => self.export_ics(ui),
                    DashAction::Month(month) => {
                        self.calendar.layout = calendar::Layout::Month;
                        self.calendar.month = month;
                        self.view = View::Calendar;
                    }
                }
            }
            View::Calendar => {
                let events = Rc::clone(&self.derived().events);
                let mut guests = self.db.settings.export_with_guests;
                let action = calendar::show(ui, &self.db, &events, &mut self.calendar, &mut guests);
                self.db.settings.export_with_guests = guests;
                match action {
                    CalAction::None => {}
                    CalAction::Open(id) => self.selected = Some(id),
                    CalAction::Export(uids) => self.export_selected(ui, &uids),
                    CalAction::Change(event, change) => {
                        if self.db.change_meeting(&event, change) {
                            let done = match change {
                                MeetingChange::Held(true) => "Marcada como realizada".to_string(),
                                MeetingChange::Held(false) => "Marcada como não realizada".to_string(),
                                MeetingChange::Postpone(date, _) => format!("Remarcada para {}", pt::weekday_day_month(date)),
                            };
                            self.notify(ui, done);
                        }
                    }
                }
            }
            View::List(cat) => {
                let add = student_list::show(
                    ui,
                    &self.db,
                    &cat,
                    ListState {
                        search: &mut self.search,
                        show_archived: &mut self.show_archived,
                        selected: &mut self.selected,
                    },
                );
                if add
                    || ui.input_mut(|i| {
                        i.consume_shortcut(&egui::KeyboardShortcut::new(
                            egui::Modifiers::COMMAND,
                            egui::Key::N,
                        ))
                    })
                {
                    self.new_student =
                        Some(NewStudent::for_category(Some(&cat), &self.db.settings));
                }
            }
            View::Settings => {
                let path = self.vault.path().display().to_string();
                let mut in_use = std::collections::HashMap::new();
                for f in self.db.students.iter().filter_map(|s| s.tcc.as_ref()) {
                    *in_use.entry(f.programme.clone()).or_insert(0) += 1;
                }
                let security = settings::Security {
                    data_path: &path,
                    key_file_on: self.vault.uses_key_file(),
                    weak_password: self.weak_password,
                    password: &mut self.password_form,
                    key_file: &mut self.key_form,
                };
                match settings::show(ui, &mut self.db.settings, &in_use, security) {
                    SettingsAction::None => {}
                    SettingsAction::ExportIcs => self.export_ics(ui),
                    SettingsAction::ChangePassword => self.change_password(ui),
                    SettingsAction::AddKeyFile => self.start_key_file(),
                    SettingsAction::RemoveKeyFile => self.remove_key_file(ui),
                }
            }
        }
    }

    /// Ask where to save, then write an .ics of `items`.
    fn write_ics(&mut self, ui: &Ui, items: &[CalItem], invite: bool, file_name: &str) -> bool {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("iCalendar", &["ics"])
            .set_file_name(file_name)
            .save_file()
        else {
            return false;
        };
        let text = ics::calendar(
            items,
            &self.db.settings,
            invite,
            &jiff::tz::TimeZone::system(),
        );
        match std::fs::write(&path, text) {
            Ok(()) => true,
            Err(e) => {
                self.notify(ui, format!("Falha ao salvar {}: {e}", path.display()));
                false
            }
        }
    }

    /// Invites need your address as the organiser; send to Settings if missing.
    fn require_email(&mut self, ui: &Ui) -> bool {
        if ics::valid_email(&self.db.settings.my_email) {
            return true;
        }
        self.view = View::Settings;
        self.notify(ui, "Adicione o seu e-mail em Configurações › Convites primeiro");
        false
    }

    fn export_ics(&mut self, ui: &Ui) {
        let items = self.db.cal_items(|_| true, true);
        if self.write_ics(ui, &items, false, "agenda.ics") {
            self.notify(ui, format!("{} itens exportados", items.len()));
        }
    }

    fn export_selected(&mut self, ui: &Ui, uids: &std::collections::HashSet<String>) {
        let invite = self.db.settings.export_with_guests;
        if invite && !self.require_email(ui) {
            return;
        }
        let items = self
            .db
            .cal_items(|e| uids.contains(&e.uid), self.calendar.as_series);
        let name = if invite {
            "convites.ics"
        } else {
            "agenda-selecao.ics"
        };
        if self.write_ics(ui, &items, invite, name) {
            let mut msg = format!("{} itens exportados", items.len());
            if invite {
                // Deadlines and reminders never have guests; anything else without one lacks an email.
                let missing = items
                    .iter()
                    .filter(|i| {
                        !matches!(i.event.kind, crate::model::EventKind::Deadline | crate::model::EventKind::Reminder)
                            && i.attendees.is_empty()
                    })
                    .count();
                if missing > 0 {
                    msg.push_str(&format!(" – {missing} sem e-mail do aluno; adicione-o para convidar"));
                } else {
                    msg.push_str(" – importe no Outlook ou Google Agenda, abra cada evento e envie");
                }
            }
            self.notify(ui, msg);
        }
    }

    fn save_invite(&mut self, ui: &Ui, student_id: Uuid, uid: &str) {
        if !self.require_email(ui) {
            return;
        }
        let Some(invite) = self.db.invite(student_id, uid) else {
            return;
        };
        if invite.attendees.is_empty() {
            self.notify(ui, "Este aluno ainda não tem e-mail");
            return;
        }
        let name: String = format!(
            "{} {}",
            invite.event.date.format("%Y-%m-%d"),
            invite.event.title
        )
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || " -–_().".contains(c) {
                c
            } else {
                '_'
            }
        })
        .collect();
        let series = if invite.series.is_some() {
            " (série recorrente)"
        } else {
            ""
        };
        let no_link = invite.event.online && ics::meeting_link(&invite.event, &self.db.settings).is_none();
        let hint = if no_link && invite.event.platform == crate::model::Platform::Teams {
            "importe no Outlook, abra o evento, ative “Reunião do Teams” e envie"
        } else if no_link {
            "importe no Outlook ou Google Agenda, abra o evento, adicione o link da reunião e envie"
        } else {
            "importe no Outlook ou Google Agenda, abra o evento e envie"
        };
        if self.write_ics(ui, &[invite], true, &format!("{name}.ics")) {
            self.notify(ui, format!("Convite salvo{series} – {hint}"));
        }
    }

    fn change_password(&mut self, ui: &Ui) {
        self.wait_for_save(ui);
        let pw = std::mem::take(&mut self.password_form);
        let result = if let Some(problem) = storage::password_problem(&pw.new) {
            Err(problem)
        } else if *pw.new != *pw.confirm {
            Err("As novas senhas não coincidem.".to_string())
        } else {
            let result = self.vault.change_password(&pw.current, &pw.new, &self.db);
            self.resealed(result, "Não foi possível alterar a senha")
        };
        if result.is_ok() {
            self.weak_password = false;
        }
        self.password_form.message = Some(match result {
            Ok(()) => (true, "Senha alterada.".into()),
            Err(e) => (false, e),
        });
    }

    /// Re-keying saves the data too. Failures are put in plain words.
    fn resealed(&mut self, result: anyhow::Result<()>, failed: &str) -> Result<(), String> {
        let Err(e) = result else {
            self.saved(Ok(()));
            return Ok(());
        };
        Err(match storage::vault_error(&e) {
            Some(VaultError::WrongPassword) => "A senha atual está incorreta.".into(),
            Some(VaultError::ChangedOnDisk) => {
                self.status = SaveStatus::Conflict;
                format!("{failed}: {e}")
            }
            _ => format!("{failed}: {e:#}"),
        })
    }

    /// Adding a key file, step 1: check the password and put the key file in
    /// place on this computer. The data file is re-keyed only after a backup
    /// copy has been saved (see `dialogs`).
    fn start_key_file(&mut self) {
        let form = std::mem::take(&mut self.key_form);
        let result = (|| {
            let path = storage::key_file_path().ok_or("Este computador não tem pasta de configurações.")?;
            if storage::same_folder(self.vault.path(), &path) {
                return Err(format!(
                    "O arquivo de dados está na pasta onde ficaria o arquivo-chave ({}). Mova o arquivo de dados primeiro.",
                    path.parent().unwrap_or(&path).display()
                ));
            }
            match self.vault.check_password(&form.password) {
                Ok(true) => {}
                Ok(false) => return Err("A senha atual está incorreta.".into()),
                Err(e) => return Err(format!("{e:#}")),
            }
            // One key file per computer: reuse it if there already is one.
            let key = match storage::local_key_file() {
                Ok(Some(key)) => key,
                Ok(None) => {
                    let key = KeyFile::generate();
                    storage::install_key_file(&key).map_err(|e| format!("Não foi possível salvar o arquivo-chave: {e:#}"))?;
                    key
                }
                Err(e) => return Err(format!("{e:#}")),
            };
            Ok(KeyFileSetup {
                key,
                path,
                password: form.password,
                backup: None,
                error: None,
            })
        })();
        match result {
            Ok(setup) => self.key_setup = Some(setup),
            Err(e) => self.key_form.message = Some((false, e)),
        }
    }

    fn finish_key_file(&mut self, ui: &Ui) {
        self.wait_for_save(ui);
        let Some(setup) = self.key_setup.take() else { return };
        let result = self.vault.set_key_file(&setup.password, Some(setup.key), &self.db);
        self.key_form.message = Some(match self.resealed(result, "Não foi possível ativar o arquivo-chave") {
            Ok(()) => (
                true,
                "Arquivo-chave ativado. Em outro computador, o Supervisor vai pedir a sua cópia de backup na primeira vez.".into(),
            ),
            Err(e) => (false, e),
        });
    }

    fn remove_key_file(&mut self, ui: &Ui) {
        self.wait_for_save(ui);
        let form = std::mem::take(&mut self.key_form);
        let result = self.vault.set_key_file(&form.password, None, &self.db);
        self.key_form.message = Some(match self.resealed(result, "Não foi possível remover o arquivo-chave") {
            Ok(()) => (
                true,
                "Arquivo-chave removido – só a sua senha volta a abrir este arquivo de dados. \
                 O arquivo-chave continua neste computador, caso outro arquivo de dados o use."
                    .into(),
            ),
            Err(e) => (false, e),
        });
    }

    fn dialogs(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let without_saving = if self.leaving == Some(Leave::Quit) {
            "Sair sem salvar"
        } else {
            "Bloquear sem salvar"
        };

        if matches!(self.status, SaveStatus::Conflict) {
            let backup = format!("{}.bak", self.vault.path().file_name().unwrap_or_default().to_string_lossy());
            let mut keep_mine = false;
            let mut discard = false;
            egui::Modal::new(Id::unique("conflict")).frame(theme::modal_frame(ui)).show(&ctx, |ui| {
                ui.set_width(460.0);
                ui.horizontal(|ui| {
                    theme::icon_badge(ui, icons::WARNING, theme::AMBER, 40.0);
                    ui.label(theme::serif("Alterado em outro lugar", 22.0));
                });
                ui.add_space(4.0);
                ui.label(
                    "Depois que você o desbloqueou, o arquivo de dados foi alterado por outra cópia do Supervisor – \
                     provavelmente em outro computador que compartilha o arquivo, ou numa segunda janela.",
                );
                ui.add_space(4.0);
                ui.label(format!(
                    "Salvar a sua versão substitui aquela. Ela fica guardada como {backup} na mesma pasta, então nada se perde de vez."
                ));
                ui.add_space(12.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(theme::primary_button("Salvar a minha versão")).clicked() {
                        keep_mine = true;
                    }
                    if ui
                        .add(egui::Button::new(without_saving).min_size(egui::vec2(0.0, 34.0)))
                        .on_hover_text("As suas alterações desde o último salvamento serão perdidas. Desbloqueie de novo para ver a outra versão.")
                        .clicked()
                    {
                        discard = true;
                    }
                });
            });
            if keep_mine {
                self.wait_for_save(ui);
                let result = self.vault.overwrite(&self.db);
                self.saved(result);
            } else if discard {
                self.discard = true;
                self.leaving.get_or_insert(Leave::Lock);
            }
        }

        if let (Some(leave), SaveStatus::Error(err)) = (self.leaving, &self.status)
            && self.dirty_since.is_some()
        {
            let err = err.clone();
            let mut retry = false;
            let mut stay = false;
            let mut discard = false;
            let resp = egui::Modal::new(Id::unique("not-saved")).frame(theme::modal_frame(ui)).show(&ctx, |ui| {
                ui.set_width(460.0);
                ui.horizontal(|ui| {
                    theme::icon_badge(ui, icons::WARNING, theme::RED, 40.0);
                    ui.label(theme::serif("Ainda não salvo", 22.0));
                });
                ui.add_space(4.0);
                let what = if leave == Leave::Quit { "fechou" } else { "bloqueou" };
                ui.label(format!("O Supervisor não conseguiu salvar as últimas alterações, por isso não {what}:"));
                ui.label(RichText::new(err).small().color(theme::muted(ui)));
                ui.add_space(4.0);
                ui.label("Ele continua tentando. Se o arquivo de dados está numa unidade de rede ou num pendrive, verifique se está conectado.");
                ui.add_space(12.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(theme::primary_button("Tentar de novo")).clicked() {
                        retry = true;
                    }
                    if ui.add(egui::Button::new("Continuar trabalhando").min_size(egui::vec2(0.0, 34.0))).clicked() {
                        stay = true;
                    }
                    if ui.add(theme::danger_button(without_saving)).clicked() {
                        discard = true;
                    }
                });
            });
            if retry {
                self.save(&ctx);
            } else if discard {
                self.discard = true;
            } else if stay || resp.should_close() {
                self.leaving = None;
            }
        }

        if let Some(setup) = &mut self.key_setup {
            let mut turn_on = false;
            let mut cancel = false;
            let data_file = self.vault.path().to_path_buf();
            let resp = egui::Modal::new(Id::unique("key-file-setup")).frame(theme::modal_frame(ui)).show(&ctx, |ui| {
                ui.set_width(500.0);
                ui.horizontal(|ui| {
                    theme::icon_badge(ui, icons::KEY, theme::ACCENT, 40.0);
                    ui.label(theme::serif("Faça backup do arquivo-chave", 24.0));
                });
                ui.add_space(6.0);
                ui.label("O seu arquivo-chave está salvo neste computador em:");
                ui.label(RichText::new(setup.path.display().to_string()).small().color(theme::muted(ui)));
                ui.add_space(6.0);
                ui.label(
                    "Salve uma cópia de backup antes de ativá-lo. Guarde-a num lugar seguro e longe do arquivo de dados: \
                     um pendrive, ou como anexo num gerenciador de senhas como KeePassXC ou Bitwarden. \
                     Você também vai precisar dela para usar o Supervisor em outro computador.",
                );
                ui.add_space(4.0);
                ui.colored_label(
                    theme::fg(ui, theme::AMBER),
                    "Sem o arquivo-chave, os dados não podem ser abertos, mesmo com a senha certa.",
                );
                ui.add_space(10.0);
                if ui.button(format!("{} Salvar cópia de backup…", icons::EXPORT)).clicked()
                    && let Some(dest) = rfd::FileDialog::new()
                        .set_title("Salvar uma cópia de backup do arquivo-chave")
                        .set_file_name("supervisor-br-chave.bin")
                        .save_file()
                {
                    match storage::save_key_backup(&setup.key, &dest, &data_file) {
                        Ok(()) => {
                            setup.backup = Some(dest);
                            setup.error = None;
                        }
                        Err(e) => setup.error = Some(format!("{e:#}")),
                    }
                }
                if let Some(dest) = &setup.backup {
                    ui.colored_label(
                        theme::fg(ui, theme::GREEN),
                        format!("{} Backup salvo em {}", icons::CHECK_CIRCLE, dest.display()),
                    );
                }
                if let Some(e) = &setup.error {
                    ui.colored_label(theme::fg(ui, theme::RED), e);
                }
                ui.add_space(12.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(setup.backup.is_some(), theme::primary_button("Ativar arquivo-chave"))
                        .on_disabled_hover_text("Salve uma cópia de backup primeiro")
                        .clicked()
                    {
                        turn_on = true;
                    }
                    if ui.add(egui::Button::new("Cancelar").min_size(egui::vec2(0.0, 34.0))).clicked() {
                        cancel = true;
                    }
                });
            });
            if turn_on {
                self.finish_key_file(ui);
            } else if cancel || resp.should_close() {
                self.key_setup = None;
            }
        }

        if let Some(n) = &mut self.new_student {
            let mut create = false;
            let mut cancel = false;
            let dup = self.db.enrollment_taken(&n.student.enrollment, None);
            let resp = egui::Modal::new(Id::unique("new-student")).frame(theme::modal_frame(ui)).show(&ctx, |ui| {
                ui.set_width(500.0);
                ui.horizontal(|ui| {
                    theme::icon_badge(ui, icons::STUDENT, theme::ACCENT, 40.0);
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing.y = 0.0;
                        ui.label(theme::serif("Adicionar aluno", 24.0));
                        ui.label(RichText::new("Os detalhes podem ser completados depois no painel lateral.").small().color(theme::muted(ui)));
                    });
                });
                ui.add_space(10.0);
                crate::ui::widgets::form_grid(ui, "new-grid", |ui| {
                    let s = &mut n.student;
                    ui.label("Matrícula");
                    ui.vertical(|ui| {
                        crate::ui::widgets::text_field(ui, &mut s.enrollment, "Matrícula ou CPF");
                        if dup {
                            ui.colored_label(theme::fg(ui, theme::AMBER), format!("{} Já existe", icons::WARNING));
                        }
                    });
                    ui.end_row();
                    ui.label("Nome");
                    crate::ui::widgets::text_field(ui, &mut s.name, "Nome completo");
                    ui.end_row();
                    ui.label("E-mail");
                    crate::ui::widgets::text_field(ui, &mut s.email, "E-mail institucional");
                    ui.end_row();
                    ui.label("Curso");
                    crate::ui::widgets::text_field(ui, &mut s.course, "Curso");
                    ui.end_row();
                });
                ui.add_space(8.0);
                theme::card_title(ui, "Vínculos");
                ui.label(RichText::new("GRADUAÇÃO").small().color(theme::muted(ui)));
                ui.checkbox(&mut n.tcc, "Aluno de TCC");
                if n.tcc && self.db.settings.programmes.len() > 1 {
                    ui.indent("tcc-course", |ui| {
                        for p in &self.db.settings.programmes {
                            ui.radio_value(&mut n.programme, p.id.clone(), &p.name);
                        }
                    });
                }
                ui.checkbox(&mut n.ic, "Iniciação científica (IC)");
                ui.checkbox(&mut n.placement, "Estágio supervisionado");
                if n.placement {
                    ui.indent("placement", |ui| {
                        crate::ui::widgets::text_field(ui, &mut n.company, "Empresa / concedente");
                        crate::ui::widgets::text_field(ui, &mut n.professional_email, "E-mail na empresa");
                    });
                }
                ui.checkbox(&mut n.advisee, "Tutoria acadêmica");
                ui.add_space(4.0);
                ui.label(RichText::new("PÓS-GRADUAÇÃO").small().color(theme::muted(ui)));
                ui.checkbox(&mut n.pos, "Aluno de pós-graduação");
                if n.pos {
                    ui.indent("pos-level", |ui| {
                        ui.horizontal_wrapped(|ui| {
                            for level in Level::ALL {
                                ui.radio_value(&mut n.level, level, level.label());
                            }
                        });
                    });
                }
                ui.add_space(12.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let valid = !n.student.name.trim().is_empty() || !n.student.enrollment.trim().is_empty();
                    if ui.add_enabled(valid, theme::primary_button("Criar")).clicked() {
                        create = true;
                    }
                    if ui.add(egui::Button::new("Cancelar").min_size(egui::vec2(0.0, 34.0))).clicked() {
                        cancel = true;
                    }
                });
            });
            if create {
                let student = self.new_student.take().expect("open").build();
                self.selected = Some(student.id);
                self.db.students.push(student);
            } else if cancel || resp.should_close() {
                self.new_student = None;
            }
        }

        if let Some(prompt) = &mut self.reveal_prompt {
            let name = self.db.student(prompt.student).map(|s| s.display_name()).unwrap_or_default();
            let mut submit = false;
            let mut cancel = false;
            let resp = egui::Modal::new(Id::unique("reveal")).frame(theme::modal_frame(ui)).show(&ctx, |ui| {
                ui.set_width(420.0);
                ui.horizontal(|ui| {
                    theme::icon_badge(ui, icons::EYE, theme::ACCENT, 40.0);
                    ui.label(theme::serif("Revelar matrícula", 22.0));
                });
                ui.add_space(4.0);
                ui.add(
                    egui::Label::new(format!("Digite a sua senha para ver a matrícula de {name}."))
                        .wrap_mode(egui::TextWrapMode::Wrap),
                );
                ui.add_space(6.0);
                let field = widgets::password_field(
                    ui,
                    egui::TextEdit::singleline(&mut *prompt.password)
                        .hint_text("Senha")
                        .desired_width(f32::INFINITY),
                );
                if !field.has_focus() && prompt.password.is_empty() && prompt.error.is_none() {
                    field.request_focus();
                }
                if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    submit = true;
                }
                if let Some(e) = &prompt.error {
                    ui.colored_label(theme::fg(ui, theme::RED), e);
                }
                ui.add_space(12.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(theme::primary_button("Revelar")).clicked() {
                        submit = true;
                    }
                    if ui.add(egui::Button::new("Cancelar").min_size(egui::vec2(0.0, 34.0))).clicked() {
                        cancel = true;
                    }
                });
            });
            if submit && !prompt.password.is_empty() {
                match self.vault.check_password(&prompt.password) {
                    Ok(true) => {
                        self.revealed = Some(prompt.student);
                        self.reveal_prompt = None;
                    }
                    Ok(false) => {
                        prompt.error = Some("Senha incorreta.".into());
                        prompt.password = Zeroizing::new(String::new());
                    }
                    Err(e) => prompt.error = Some(format!("{e:#}")),
                }
            } else if cancel || resp.should_close() {
                self.reveal_prompt = None;
            }
        }

        if let Some(confirm) = &self.confirm {
            let (title, body) = match confirm {
                Confirm::DeleteStudent(id) => (
                    "Excluir aluno?".to_string(),
                    format!(
                        "Excluir definitivamente {} e todos os seus registros?",
                        self.db.student(*id).map(|s| s.display_name()).unwrap_or_default()
                    ),
                ),
                Confirm::RemoveRole(_, role) => (
                    format!("Remover registro de {}?", role.label()),
                    "Todas as datas e anotações desta seção serão perdidas.".to_string(),
                ),
            };
            let mut yes = false;
            let mut no = false;
            let resp = egui::Modal::new(Id::unique("confirm")).frame(theme::modal_frame(ui)).show(&ctx, |ui| {
                ui.set_width(380.0);
                ui.horizontal(|ui| {
                    theme::icon_badge(ui, icons::TRASH, theme::RED, 40.0);
                    ui.label(theme::serif(title, 22.0));
                });
                ui.add_space(4.0);
                ui.label(body);
                ui.add_space(12.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(theme::danger_button("Excluir")).clicked() {
                        yes = true;
                    }
                    if ui.add(egui::Button::new("Cancelar").min_size(egui::vec2(0.0, 34.0))).clicked() {
                        no = true;
                    }
                });
            });
            if yes {
                match self.confirm.take().expect("open") {
                    Confirm::DeleteStudent(id) => {
                        self.db.students.retain(|s| s.id != id);
                        self.selected = None;
                    }
                    Confirm::RemoveRole(id, role) => {
                        if let Some(s) = self.db.student_mut(id) {
                            match role {
                                Role::Tcc => s.tcc = None,
                                Role::Ic => s.ic = None,
                                Role::Pos => s.pos = None,
                                Role::Placement => s.placement = None,
                                Role::Advisee => s.advisee = None,
                            }
                        }
                    }
                }
            } else if no || resp.should_close() {
                self.confirm = None;
            }
        }
    }
}

#[cfg(test)]
mod ui_tests {
    //! Drives the real UI headlessly. Set `SUPERVISOR_SHOTS=<dir>` to also
    //! write screenshots of each screen.
    use super::*;
    use crate::model::*;
    use chrono::{Datelike, Duration as Days, NaiveTime};
    use egui::accesskit::Role as AkRole;
    use egui_kittest::Harness;
    use egui_kittest::kittest::{NodeT, Queryable};

    const PASSWORD: &str = "senha-de-teste-123";
    /// A made-up CPF with valid check digits, used in examples.
    const CPF: &str = "529.982.247-25";

    fn session<'s>(h: &'s mut Harness<'_, SupervisorApp>) -> &'s mut Session {
        match &mut h.state_mut().screen {
            Screen::Unlocked(s) => s,
            Screen::Locked(_) => panic!("still locked"),
        }
    }

    fn shot(h: &mut Harness<'_, SupervisorApp>, name: &str) {
        if let Ok(dir) = std::env::var("SUPERVISOR_SHOTS") {
            h.run_steps(3);
            let img = h.render().expect("render");
            img.save(std::path::Path::new(&dir).join(format!("{name}.png"))).unwrap();
        }
    }

    /// Runs frames until `done`, giving work in the background (unlocking,
    /// saving) time to finish.
    fn wait_until(h: &mut Harness<'_, SupervisorApp>, what: &str, done: impl Fn(&SupervisorApp) -> bool) {
        for _ in 0..600 {
            h.run_steps(1);
            if done(h.state()) {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        panic!("timed out waiting for: {what}");
    }

    fn wait_unlocked(h: &mut Harness<'_, SupervisorApp>) {
        wait_until(h, "unlocked", |app| matches!(app.screen, Screen::Unlocked(_)));
    }

    fn locked(app: &SupervisorApp) -> bool {
        matches!(app.screen, Screen::Locked(_))
    }

    /// Unlocked with every change saved and no save running.
    fn all_saved(app: &SupervisorApp) -> bool {
        matches!(&app.screen, Screen::Unlocked(s) if s.saving.is_none() && s.dirty_since.is_none())
    }

    fn status_is(app: &SupervisorApp, want: fn(&SaveStatus) -> bool) -> bool {
        matches!(&app.screen, Screen::Unlocked(s) if s.saving.is_none() && want(&s.status))
    }

    /// Fictional students for the screenshots: names, e-mails and companies
    /// are made up, and the institution is "exemplo.edu.br".
    fn demo_students(today: chrono::NaiveDate) -> Vec<Student> {
        let t = |h| NaiveTime::from_hms_opt(h, 0, 0);
        let meeting = |days: i64, held: bool, notes: &str| Meeting {
            date: Some(today + Days::days(days)),
            time: t(14),
            held,
            notes: notes.into(),
            ..Default::default()
        };
        let due = |days: i64| Milestone {
            deadline: Some(today + Days::days(days)),
            ..Default::default()
        };
        let mut doctorate = PosRecord {
            title: "Compósitos autorreparáveis para pás de turbinas eólicas".into(),
            program: "PPG em Engenharia Mecânica".into(),
            co_supervisors: "Profa. Coorientadora Exemplo".into(),
            start: Some(today - Days::days(400)),
            grant: Grant {
                agency: "CAPES".into(),
                modality: "Demanda Social".into(),
                ..Default::default()
            },
            meetings: vec![Meeting {
                online: true,
                platform: Platform::GoogleMeet,
                link: "https://meet.google.com/abc-defg-hij".into(),
                repeat: Some(Repeat {
                    every_weeks: 2,
                    ..Default::default()
                }),
                ..meeting(2, false, "")
            }],
            ..PosRecord::new(Level::DoutoradoAcademico)
        };
        doctorate.suggest(&LevelRule::default_for(doctorate.level));
        let report = &mut doctorate.annual_reports[0];
        report.done = true;
        report.done_on = report.deadline.map(|d| d - Days::days(5));
        let mut masters = PosRecord {
            title: "Gêmeos digitais para manutenção preditiva".into(),
            program: "PPG em Engenharia de Produção".into(),
            start: Some(today - Days::days(300)),
            grant: Grant {
                agency: "FACEPE".into(),
                ..Default::default()
            },
            ..PosRecord::new(Level::MestradoAcademico)
        };
        masters.suggest(&LevelRule::default_for(masters.level));
        let mut ic = IcRecord {
            title: "Análise de vibrações em estruturas leves".into(),
            project: "Monitoramento estrutural com sensores de baixo custo".into(),
            ..IcRecord::new()
        };
        ic.grant.start = Some(today - Days::days(150));
        ic.suggest();
        vec![
            Student {
                enrollment: CPF.into(),
                name: "Beatriz Almeida".into(),
                email: "beatriz.almeida@exemplo.edu.br".into(),
                course: "Engenharia Mecânica".into(),
                tcc: Some(TccRecord {
                    title: "Braço de drone com otimização topológica".into(),
                    initial_contact: true,
                    initial_contact_on: Some(today - Days::days(30)),
                    meetings: vec![
                        meeting(-14, true, "Escopo definido."),
                        Meeting {
                            location: "Sala 201 – Bloco B".into(),
                            repeat: Some(Repeat {
                                exceptions: vec![Occurrence {
                                    planned: today + Days::days(10),
                                    cancelled: true,
                                    ..Default::default()
                                }],
                                ..Default::default()
                            }),
                            ..meeting(3, false, "")
                        },
                    ],
                    ..Default::default()
                }),
                advisee: Some(AdviseeRecord {
                    s1: SemesterContact {
                        contacted_on: Some(today - Days::days(20)),
                        reply: ReplyStatus::Replied,
                        replied_on: Some(today - Days::days(18)),
                        meetings: vec![
                            meeting(-10, true, "Adaptação tranquila."),
                            Meeting {
                                online: true,
                                ..meeting(5, false, "Pediu uma reunião de retorno")
                            },
                        ],
                    },
                    ..Default::default()
                }),
                ..Default::default()
            },
            Student {
                enrollment: "2022104587".into(),
                name: "Lucas Ferreira".into(),
                course: "Engenharia de Produção".into(),
                tcc: Some(TccRecord::default()),
                ..Default::default()
            },
            Student {
                enrollment: "2023100912".into(),
                name: "Júlia Ribeiro".into(),
                email: "julia.ribeiro@exemplo.edu.br".into(),
                course: "Engenharia Mecânica".into(),
                ic: Some(ic),
                ..Default::default()
            },
            Student {
                enrollment: "2021107733".into(),
                name: "Rafael Souza".into(),
                course: "Engenharia Mecânica".into(),
                placement: Some(PlacementRecord {
                    company: "Metalúrgica Exemplo Ltda.".into(),
                    professional_email: "rafael.souza@empresa.com.br".into(),
                    line_manager_name: "Supervisora Exemplo".into(),
                    tce_start: Some(today - Days::days(60)),
                    tce_end: Some(today + Days::days(120)),
                    report: due(90),
                    visits: [
                        Visit {
                            request_sent: Some(today - Days::days(7)),
                            response: ReplyStatus::Replied,
                            date: Some(today + Days::days(6)),
                            time: t(11),
                            ..Default::default()
                        },
                        Visit {
                            mode: VisitMode::FaceToFace,
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                }),
                ..Default::default()
            },
            Student {
                enrollment: "2024101208".into(),
                name: "Camila Santos".into(),
                course: "Engenharia Elétrica".into(),
                advisee: Some(AdviseeRecord {
                    s1: SemesterContact {
                        contacted_on: Some(today - Days::days(21)),
                        reply: ReplyStatus::NoReply,
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                ..Default::default()
            },
            Student {
                enrollment: "20250031".into(),
                name: "Mariana Costa".into(),
                email: "mariana.costa@exemplo.edu.br".into(),
                course: "Doutorado em Engenharia Mecânica".into(),
                pos: Some(doctorate),
                ..Default::default()
            },
            Student {
                enrollment: "20250117".into(),
                name: "Pedro Oliveira".into(),
                email: "pedro.oliveira@exemplo.edu.br".into(),
                course: "Mestrado em Engenharia de Produção".into(),
                pos: Some(masters),
                ..Default::default()
            },
        ]
    }

    /// Opens the app on `path`, which doesn't exist yet, and creates the password.
    fn create_and_unlock(path: &std::path::Path, shots: bool) -> Harness<'static, SupervisorApp> {
        let p = path.to_path_buf();
        let mut h = Harness::builder()
            .with_size(egui::vec2(1360.0, 860.0))
            .build_eframe(move |cc| SupervisorApp::new(cc, Some(p)));
        h.run_steps(2);
        if shots {
            shot(&mut h, "01_criar_senha");
        }

        // First run: create the password. A short one is refused first.
        let (start, rest) = PASSWORD.split_at(4);
        for text in [start, rest] {
            let fields = h.get_all_by_role(AkRole::PasswordInput).collect::<Vec<_>>();
            assert_eq!(fields.len(), 2);
            for field in &fields {
                field.focus();
                field.type_text(text);
            }
            h.run_steps(1);
            h.get_by_label("Criar e desbloquear").click();
            h.run_steps(2);
            if text == start {
                assert!(h.query_by_label_contains("pelo menos 12 caracteres").is_some());
                assert!(!path.exists());
            }
        }
        wait_unlocked(&mut h);
        h.run_steps(2);
        assert!(path.exists());
        h
    }

    /// Unlocked, with the demo students and the semester dates set.
    fn demo(path: &std::path::Path) -> Harness<'static, SupervisorApp> {
        let mut h = create_and_unlock(path, false);
        let today = crate::ui::widgets::today();
        {
            let s = session(&mut h);
            s.db.settings.sem2_start = Some(today - Days::days(30));
            s.db.settings.my_name = "Prof. Exemplo".into();
            s.db.students.extend(demo_students(today));
        }
        h.key_press(egui::Key::F20);
        h.run_steps(2);
        h
    }

    fn id_of(h: &mut Harness<'_, SupervisorApp>, name: &str) -> Uuid {
        session(h).db.students.iter().find(|s| s.name == name).unwrap().id
    }

    #[test]
    fn end_to_end() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.svt");
        let mut h = create_and_unlock(&path, true);

        // Add a student through the dialog.
        h.get_by_label("Estágio").click();
        h.run_steps(2);
        h.get_by_label(&format!("{}  Adicionar aluno", icons::PLUS)).click();
        h.run_steps(2);
        shot(&mut h, "02_adicionar_aluno");
        let inputs = h.get_all_by_role(AkRole::TextInput).collect::<Vec<_>>();
        // The list's search box, then the dialog's matrícula, name, email,
        // course, company and company email.
        assert_eq!(inputs.len(), 7);
        inputs[1].focus();
        inputs[1].type_text("2026000123");
        inputs[2].focus();
        inputs[2].type_text("Aluno Novo");
        h.run_steps(1);
        h.get_by_label("Criar").click();
        h.run_steps(2);
        let created = session(&mut h)
            .db
            .students
            .iter()
            .find(|s| s.name == "Aluno Novo")
            .cloned()
            .expect("student created");
        assert!(created.placement.is_some(), "Estágio preset from the view");

        // Autosave after the debounce: the encrypted file must contain the student.
        for _ in 0..90 {
            h.run_steps(1);
        }
        wait_until(&mut h, "autosave", all_saved);
        let saved = storage::read_database(&path, PASSWORD, None).unwrap();
        assert!(saved.students.iter().any(|s| s.name == "Aluno Novo"));

        // Seed richer data and capture each screen.
        let today = crate::ui::widgets::today();
        {
            let s = session(&mut h);
            s.db.settings.sem2_start = Some(today - Days::days(30));
            s.db.settings.my_name = "Prof. Exemplo".into();
            s.db.students.retain(|s| s.name != "Aluno Novo");
            s.db.students.extend(demo_students(today));
            s.selected = None;
            s.view = View::Dashboard;
        }
        h.run_steps(2);
        assert!(h.query_by_label_contains("sem resposta").is_some(), "dashboard attention list");
        assert!(h.query_by_label_contains("Bom").or(h.query_by_label_contains("Boa")).is_some(), "greeting");
        shot(&mut h, "03_painel");

        for (view, name) in [
            (View::Calendar, "04_agenda"),
            (View::List(Category::Tcc("TCC".into())), "05_tcc"),
            (View::List(Category::Ic), "06_ic"),
            (View::List(Category::Placement), "07_estagio"),
            (View::List(Category::Advisee), "08_tutoria"),
            (View::List(Category::Pos(PosGroup::Doutorado)), "09_doutorado"),
            (View::Settings, "10_configuracoes"),
        ] {
            session(&mut h).view = view;
            h.run_steps(2);
            shot(&mut h, name);
        }
        // Enrolment numbers are hidden in the lists.
        session(&mut h).view = View::List(Category::All);
        h.run_steps(2);
        assert!(h.query_by_label(CPF).is_none(), "CPF not shown in the list");
        assert!(h.query_by_label("•••.•••.•••-••").is_some());

        // Month grid: clicking an item selects it for export.
        {
            let s = session(&mut h);
            s.view = View::Calendar;
            s.calendar.layout = crate::ui::calendar::Layout::Month;
            s.calendar.month = (today + Days::days(3)).with_day(1).unwrap();
        }
        h.run_steps(3);
        shot(&mut h, "04_agenda_mes");
        h.get_all_by_label_contains("Beatriz Almeida").next().expect("meeting in month grid").click();
        h.run_steps(2);
        assert_eq!(session(&mut h).calendar.selected.len(), 1);
        assert!(session(&mut h).db.settings.export_with_guests, "guests on by default");

        // Detail panels.
        let beatriz = id_of(&mut h, "Beatriz Almeida");
        {
            let s = session(&mut h);
            s.view = View::List(Category::Tcc("TCC".into()));
            s.selected = Some(beatriz);
        }
        h.run_steps(2);
        shot(&mut h, "11_detalhe_tcc");
        for (name, view, file) in [
            ("Rafael Souza", View::List(Category::Placement), "12_detalhe_estagio"),
            ("Júlia Ribeiro", View::List(Category::Ic), "13_detalhe_ic"),
            ("Mariana Costa", View::List(Category::Pos(PosGroup::Doutorado)), "14_detalhe_doutorado"),
        ] {
            let id = id_of(&mut h, name);
            let s = session(&mut h);
            s.view = view;
            s.selected = Some(id);
            h.run_steps(2);
            shot(&mut h, file);
        }
        // The whole doctorate panel, for the README.
        h.set_size(egui::vec2(1360.0, 2300.0));
        h.run_steps(3);
        shot(&mut h, "15_detalhe_doutorado_completo");
        h.set_size(egui::vec2(1360.0, 860.0));
        h.run_steps(2);

        // Dark mode.
        {
            let s = session(&mut h);
            s.db.settings.dark_mode = true;
            s.view = View::Dashboard;
            s.selected = None;
        }
        h.run_steps(3);
        assert!(h.ctx.global_style().visuals.dark_mode);
        shot(&mut h, "16_painel_escuro");
        {
            let s = session(&mut h);
            s.view = View::List(Category::Pos(PosGroup::Mestrado));
            s.selected = Some(s.db.students.iter().find(|s| s.name == "Pedro Oliveira").unwrap().id);
        }
        h.run_steps(3);
        shot(&mut h, "17_mestrado_escuro");

        // Lock: back to the unlock screen, and the right password reopens the data.
        h.get_by_label(&format!("{}  Bloquear", icons::LOCK_SIMPLE)).click();
        wait_until(&mut h, "locked", locked);
        shot(&mut h, "18_desbloquear");
        let (_, saved) = Vault::open(&path, PASSWORD, None).unwrap();
        assert!(saved.students.iter().any(|s| s.name == "Rafael Souza"));
        assert!(saved.settings.dark_mode);
    }

    /// Every label, value and description the screen exposes.
    fn all_text(h: &Harness<'_, SupervisorApp>) -> String {
        h.root()
            .children_recursive()
            .flat_map(|n| [n.accesskit_node().label(), n.accesskit_node().value()])
            .flatten()
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn enrollment_is_hidden_until_revealed() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = demo(&dir.path().join("data.svt"));
        let beatriz = id_of(&mut h, "Beatriz Almeida");
        let pedro = id_of(&mut h, "Pedro Oliveira");
        {
            let s = session(&mut h);
            s.view = View::List(Category::Tcc("TCC".into()));
            s.selected = Some(beatriz);
        }
        h.run_steps(2);
        assert!(!all_text(&h).contains(CPF), "the CPF is nowhere on screen");
        assert!(!all_text(&h).contains("52998224725"));
        shot(&mut h, "19_matricula_oculta");

        // Without the password option, one click reveals it.
        h.get_by_label(&format!("{} Revelar", icons::EYE)).click();
        h.run_steps(2);
        assert!(all_text(&h).contains(CPF));
        assert!(h.query_by_label_contains("É um CPF").is_some());
        // Opening another student hides it again.
        session(&mut h).selected = Some(pedro);
        h.run_steps(2);
        session(&mut h).selected = Some(beatriz);
        h.run_steps(2);
        assert!(!all_text(&h).contains(CPF), "hidden again after switching students");

        // With the option on, revealing asks for the password.
        session(&mut h).db.settings.reveal_needs_password = true;
        h.run_steps(1);
        h.get_by_label(&format!("{} Revelar", icons::EYE)).click();
        h.run_steps(2);
        assert!(h.query_by_label("Revelar matrícula").is_some());
        shot(&mut h, "20_revelar_com_senha");
        let field = h.get_by_role(AkRole::PasswordInput);
        field.focus();
        field.type_text("senha-errada-000");
        h.run_steps(1);
        h.get_by_label("Revelar").click();
        h.run_steps(2);
        assert!(h.query_by_label("Senha incorreta.").is_some());
        assert!(!all_text(&h).contains(CPF));
        let field = h.get_by_role(AkRole::PasswordInput);
        field.focus();
        field.type_text(PASSWORD);
        h.run_steps(1);
        h.get_by_label("Revelar").click();
        h.run_steps(2);
        assert!(h.query_by_label("Revelar matrícula").is_none(), "dialog closed");
        assert!(all_text(&h).contains(CPF));
        h.get_by_label(&format!("{} Ocultar", icons::EYE_SLASH)).click();
        h.run_steps(2);
        assert!(!all_text(&h).contains(CPF));

        // Turning the option off shows numbers everywhere.
        session(&mut h).db.settings.hide_enrollment = false;
        session(&mut h).view = View::List(Category::All);
        h.run_steps(2);
        assert!(all_text(&h).contains(CPF));
    }

    /// Widgets a person clicks or types into.
    fn interactive(role: AkRole) -> bool {
        matches!(
            role,
            AkRole::Button
                | AkRole::TextInput
                | AkRole::MultilineTextInput
                | AkRole::PasswordInput
                | AkRole::CheckBox
                | AkRole::RadioButton
                | AkRole::ComboBox
                | AkRole::SpinButton
                | AkRole::Slider
                | AkRole::Link
        )
    }

    /// Checks the screen for single-line text fields too narrow to use, and
    /// for widgets drawn on top of each other. `skip_left` leaves out a
    /// strip on the left (a sidebar that scrolls at small sizes).
    fn check_layout(h: &Harness<'_, SupervisorApp>, screen: &str, skip_left: f32) -> usize {
        let size = h.ctx.content_rect();
        // What egui itself counts as visible of a widget: its rect cut by
        // the panel or scroll area it is in.
        let visible = |n: &egui_kittest::Node<'_>| {
            let (node_id, _) = n.accesskit_node().locate();
            // SAFETY: accesskit ids are egui ids, which are never zero.
            let id = unsafe { egui::Id::from_high_entropy_bits(node_id.0) };
            h.ctx.read_response(id).map_or(egui::Rect::NOTHING, |r| r.interact_rect)
        };
        let nodes: Vec<(AkRole, String, egui::Rect)> = h
            .root()
            .children_recursive()
            .filter(|n| interactive(n.accesskit_node().role()) || n.accesskit_node().role() == AkRole::Label)
            .filter(|n| n.rect() == visible(n))
            .map(|n| {
                let label = n.accesskit_node().label().or(n.accesskit_node().value()).unwrap_or_default();
                (n.accesskit_node().role(), label, n.rect())
            })
            // Drawn and fully inside the window (not scrolled or clipped out of view).
            .filter(|(_, _, r)| r.width() > 0.5 && r.height() > 0.5 && size.contains_rect(*r) && r.min.x >= skip_left)
            .collect();
        let checked = nodes.iter().filter(|(role, _, _)| interactive(*role)).count();
        assert!(checked >= 1, "{screen}: no widgets visible – is the check looking at the screen?");
        for (role, label, r) in &nodes {
            if matches!(role, AkRole::TextInput | AkRole::PasswordInput) {
                assert!(
                    r.width() >= 80.0 && r.height() >= 18.0,
                    "{screen}: text field {label:?} is too small: {:.0}×{:.0}",
                    r.width(),
                    r.height()
                );
            }
        }
        for (i, (ra, la, a)) in nodes.iter().enumerate() {
            for (rb, lb, b) in &nodes[i + 1..] {
                if !(interactive(*ra) || interactive(*rb)) {
                    continue;
                }
                let overlap = a.intersect(*b);
                let nested = a.contains_rect(*b) || b.contains_rect(*a);
                if overlap.width() > 2.0 && overlap.height() > 2.0 && !nested {
                    panic!("{screen}: {ra:?} {la:?} {a:?} overlaps {rb:?} {lb:?} {b:?}");
                }
            }
        }
        checked
    }

    #[test]
    fn no_tiny_fields_or_overlapping_widgets() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = demo(&dir.path().join("data.svt"));
        let students: Vec<(String, View)> = vec![
            ("Beatriz Almeida".into(), View::List(Category::Tcc("TCC".into()))),
            ("Júlia Ribeiro".into(), View::List(Category::Ic)),
            ("Rafael Souza".into(), View::List(Category::Placement)),
            ("Camila Santos".into(), View::List(Category::Advisee)),
            ("Mariana Costa".into(), View::List(Category::Pos(PosGroup::Doutorado))),
            ("Pedro Oliveira".into(), View::List(Category::Pos(PosGroup::Mestrado))),
        ];
        let mut checked = 0;
        // Tall enough to see each whole panel, then the smallest window allowed.
        for (size, skip_left) in [(egui::vec2(1360.0, 2600.0), 0.0), (egui::vec2(960.0, 600.0), theme::SIDEBAR_WIDTH + 4.0)] {
            h.set_size(size);
            for dark in [false, true] {
                session(&mut h).db.settings.dark_mode = dark;
                for view in [
                    View::Dashboard,
                    View::Calendar,
                    View::Settings,
                    View::List(Category::All),
                    View::List(Category::Pos(PosGroup::LatoSensu)),
                ] {
                    let s = session(&mut h);
                    s.selected = None;
                    s.view = view.clone();
                    h.run_steps(3);
                    checked += check_layout(&h, &format!("{size:?} {:?}", std::mem::discriminant(&view)), skip_left);
                }
                {
                    let s = session(&mut h);
                    s.view = View::Calendar;
                    s.calendar.layout = crate::ui::calendar::Layout::Month;
                }
                h.run_steps(3);
                checked += check_layout(&h, "month", skip_left);
                session(&mut h).calendar.layout = crate::ui::calendar::Layout::List;
                for (name, view) in &students {
                    let id = id_of(&mut h, name);
                    let s = session(&mut h);
                    s.view = view.clone();
                    s.selected = Some(id);
                    h.run_steps(3);
                    checked += check_layout(&h, &format!("{size:?} detalhe de {name}"), skip_left);
                }
            }
        }
        assert!(checked > 1000, "only {checked} widgets checked in all");
        // The add-student dialog with every role ticked.
        h.set_size(egui::vec2(1360.0, 860.0));
        let s = session(&mut h);
        s.selected = None;
        s.view = View::Dashboard;
        let mut n = NewStudent::for_category(None, &s.db.settings);
        (n.tcc, n.ic, n.pos, n.placement, n.advisee) = (true, true, true, true, true);
        s.new_student = Some(n);
        h.run_steps(3);
        shot(&mut h, "21_adicionar_aluno_vinculos");
        let modal = h.get_by_label("Adicionar aluno").rect();
        assert!(modal.width() > 0.0);
    }

    #[test]
    fn date_picker_in_portuguese() {
        let dir = tempfile::tempdir().unwrap();
        let mut h = demo(&dir.path().join("data.svt"));
        let mariana = id_of(&mut h, "Mariana Costa");
        {
            let s = session(&mut h);
            s.view = View::List(Category::Pos(PosGroup::Doutorado));
            s.selected = Some(mariana);
        }
        h.set_size(egui::vec2(1360.0, 2300.0));
        h.run_steps(3);
        let start = session(&mut h).db.student(mariana).unwrap().pos.as_ref().unwrap().start.unwrap();
        // The enrolment date (the grant, shown above it, starts the same day).
        h.get_all_by_label(&format!("{}  {}", crate::pt::date(start), icons::CALENDAR_BLANK)).last().unwrap().click();
        h.run_steps(2);
        assert!(h.query_by_label(&crate::pt::month_year(start)).is_some(), "month in Portuguese");
        shot(&mut h, "22_seletor_de_data");
        // Every month name fits between the arrows (a year and a month
        // forward each time, so all twelve show up).
        let first = start.with_day(1).unwrap();
        for i in 0..12 {
            let month = first + chrono::Months::new(13 * i);
            let prev = h.get_by_label(icons::CARET_LEFT).rect();
            let label = h.get_by_label(&crate::pt::month_year(month)).rect();
            let next = h.get_by_label(icons::CARET_RIGHT).rect();
            let name = crate::pt::month_year(month);
            let text = h.ctx.fonts_mut(|f| f.layout_no_wrap(name.clone(), theme::semibold_font(14.0), egui::Color32::WHITE).size().x);
            assert!(
                label.min.x >= prev.max.x && label.max.x <= next.min.x && text <= label.width(),
                "{name} ({text:.0}px) doesn't fit between the arrows ({:.0}px)",
                label.width()
            );
            // One- and two-digit days take the same room, so the columns
            // stay under the weekday initials.
            let widths: Vec<f32> = (0..28)
                .map(|d| h.get_by_label(&crate::pt::date(month + chrono::Duration::days(d))).rect().width())
                .collect();
            assert!(widths.iter().all(|w| (w - widths[0]).abs() < 0.5), "day widths differ: {widths:?}");
            h.get_by_label("»").click();
            h.run_steps(1);
            h.get_by_label(icons::CARET_RIGHT).click();
            h.run_steps(2);
        }
        for _ in 0..13 {
            h.get_by_label("«").click();
            h.run_steps(1);
        }
        h.run_steps(1);
        assert!(h.query_by_label(&crate::pt::month_year(start)).is_some(), "back to the start");
        // Next month, then the 10th.
        h.get_by_label(icons::CARET_RIGHT).click();
        h.run_steps(1);
        let target = (start.with_day(1).unwrap() + chrono::Months::new(1)).with_day(10).unwrap();
        h.get_by_label(&crate::pt::date(target)).click();
        h.run_steps(2);
        let p = session(&mut h).db.student(mariana).unwrap().pos.clone().unwrap();
        assert_eq!(p.start, Some(target));
        assert!(h.query_by_label(&crate::pt::month_year(start)).is_none(), "picker closed");
    }

    fn unlock(h: &mut Harness<'_, SupervisorApp>) {
        let field = h.get_by_role(AkRole::PasswordInput);
        field.focus();
        field.type_text(PASSWORD);
        h.run_steps(1);
        h.get_by_label("Desbloquear").click();
        wait_unlocked(h);
        h.run_steps(2);
    }

    #[test]
    fn lock_waits_for_save_and_locks_when_idle() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.svt");
        let mut h = create_and_unlock(&path, false);

        // Saving fails (a folder is in the backup's way): Lock must not drop the edit.
        let bak = dir.path().join("data.svt.bak");
        std::fs::create_dir_all(bak.join("blocker")).unwrap();
        session(&mut h).db.settings.my_name = "Edição não salva".into();
        h.run_steps(1);
        h.get_by_label(&format!("{}  Bloquear", icons::LOCK_SIMPLE)).click();
        wait_until(&mut h, "save failed", |app| status_is(app, |s| matches!(s, SaveStatus::Error(_))));
        h.run_steps(2);
        assert!(matches!(h.state().screen, Screen::Unlocked(_)), "stays unlocked when saving fails");
        assert!(h.query_by_label("Ainda não salvo").is_some());

        // Once saving works again, Try again saves and locks.
        std::fs::remove_dir_all(&bak).unwrap();
        h.get_by_label("Tentar de novo").click();
        wait_until(&mut h, "locked", locked);
        assert_eq!(storage::read_database(&path, PASSWORD, None).unwrap().settings.my_name, "Edição não salva");

        // Idle for longer than the auto-lock time: locks by itself.
        unlock(&mut h);
        assert_eq!(session(&mut h).db.settings.auto_lock_minutes, 10);
        if let Some(earlier) = Instant::now().checked_sub(Duration::from_secs(11 * 60)) {
            session(&mut h).last_active = earlier;
            h.run_steps(1);
            assert!(matches!(h.state().screen, Screen::Locked(_)), "auto-locked");
        }
    }

    #[test]
    fn changed_elsewhere_asks_before_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.svt");
        let mut h = create_and_unlock(&path, false);

        // Another computer saves the file (simulated by writing other bytes).
        std::fs::write(&path, b"someone else's version").unwrap();
        session(&mut h).db.settings.my_name = "Minha".into();
        // Edits are only looked for on frames with input, as typing would give.
        h.key_press(egui::Key::F20);
        for _ in 0..90 {
            h.run_steps(1);
        }
        wait_until(&mut h, "conflict", |app| status_is(app, |s| matches!(s, SaveStatus::Conflict)));
        h.run_steps(2);
        assert_eq!(std::fs::read(&path).unwrap(), b"someone else's version", "not overwritten");
        assert!(h.query_by_label("Alterado em outro lugar").is_some());

        h.get_by_label("Salvar a minha versão").click();
        h.run_steps(2);
        assert_eq!(storage::read_database(&path, PASSWORD, None).unwrap().settings.my_name, "Minha");
        assert_eq!(std::fs::read(dir.path().join("data.svt.bak")).unwrap(), b"someone else's version");
    }

    #[test]
    fn edit_made_while_saving_is_saved_afterwards() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.svt");
        let mut h = create_and_unlock(&path, false);
        let ctx = h.ctx.clone();
        let now = ctx.input(|i| i.time);
        let s = session(&mut h);
        s.db.settings.my_name = "Primeira".into();
        s.detect_changes(now);
        s.save(&ctx);
        // Edited while that save is still running.
        s.db.settings.my_name = "Segunda".into();
        s.detect_changes(now);
        s.poll_save(now, true);
        assert_eq!(storage::read_database(&path, PASSWORD, None).unwrap().settings.my_name, "Primeira");
        assert!(matches!(s.status, SaveStatus::Pending), "the newer edit isn't saved yet");
        assert!(s.dirty_since.is_some());

        wait_until(&mut h, "second save", all_saved);
        assert_eq!(storage::read_database(&path, PASSWORD, None).unwrap().settings.my_name, "Segunda");
    }

    #[test]
    fn calendar_list_draws_only_weeks_in_view() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.svt");
        let mut h = create_and_unlock(&path, false);
        let first = chrono::NaiveDate::from_ymd_opt(2026, 1, 5).unwrap();
        let last = first + Days::weeks(59);
        {
            let s = session(&mut h);
            s.db.students.push(Student {
                name: "Aluno Semanal".into(),
                tcc: Some(TccRecord {
                    meetings: vec![Meeting {
                        date: Some(first),
                        time: NaiveTime::from_hms_opt(10, 0, 0),
                        repeat: Some(Repeat {
                            until: Some(last),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }],
                    ..Default::default()
                }),
                ..Default::default()
            });
            s.view = View::Calendar;
            s.calendar.range = crate::ui::calendar::Range::All;
        }
        h.key_press(egui::Key::F20);
        h.run_steps(3);
        let week = |monday: chrono::NaiveDate| {
            format!("{} – {}", crate::pt::day_month(monday), crate::pt::day_month_year(monday + Days::days(6)))
        };
        assert!(h.query_by_label(&week(first)).is_some(), "first week drawn");
        assert!(h.query_by_label(&week(last)).is_none(), "weeks out of view not drawn");

        // Selecting covers every week, drawn or not.
        h.get_by_label("Selecionar todos visíveis").click();
        h.run_steps(2);
        assert_eq!(session(&mut h).calendar.selected.len(), 60);

        // Scrolled to the end, the last week is drawn.
        h.event(egui::Event::PointerMoved(egui::pos2(700.0, 500.0)));
        for _ in 0..80 {
            h.event(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -400.0),
                phase: egui::TouchPhase::Move,
                source: egui::MouseWheelSource::Unknown,
                modifiers: egui::Modifiers::NONE,
            });
            h.run_steps(1);
        }
        h.run_steps(5);
        assert!(h.query_by_label(&week(last)).is_some(), "last week drawn once in view");
        assert!(h.query_by_label(&week(first)).is_none());
    }

    #[test]
    fn save_button_saves_now_and_keeps_that_version_as_the_backup() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.svt");
        let bak = dir.path().join("data.svt.bak");
        let mut h = create_and_unlock(&path, false);
        let name_in = |p: &std::path::Path| storage::read_database(p, PASSWORD, None).unwrap().settings.my_name;
        let edit = |h: &mut Harness<'_, SupervisorApp>, name: &str| {
            session(h).db.settings.my_name = name.into();
            h.key_press(egui::Key::F20);
        };

        // Ctrl+S saves without waiting for a pause in typing: with no frames
        // run after the key's own, an autosave can't have happened.
        session(&mut h).db.settings.my_name = "Ponto de restauração".into();
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::S);
        h.step();
        for _ in 0..100 {
            if name_in(&path) == "Ponto de restauração" {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(name_in(&path), "Ponto de restauração");
        wait_until(&mut h, "saved", all_saved);
        assert_eq!(name_in(&path), "Ponto de restauração");
        assert_eq!(name_in(&bak), "", "the first save keeps the file as unlocked");

        // The next save keeps the version saved by hand as the backup.
        edit(&mut h, "Depois");
        for _ in 0..90 {
            h.run_steps(1);
        }
        wait_until(&mut h, "autosaved", all_saved);
        assert_eq!(name_in(&path), "Depois");
        assert_eq!(name_in(&bak), "Ponto de restauração");

        // With nothing left to save, clicking Salvo does the same.
        h.get_by_label(&format!("{} Salvo", icons::FLOPPY_DISK)).click();
        h.run_steps(2);
        assert!(h.query_by_label("Salvo").is_some(), "confirmed");
        edit(&mut h, "Mais recente");
        for _ in 0..90 {
            h.run_steps(1);
        }
        wait_until(&mut h, "autosaved", all_saved);
        assert_eq!(name_in(&bak), "Depois");
    }

    #[test]
    fn calendar_marks_past_meetings_held_and_postpones_them() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.svt");
        let mut h = create_and_unlock(&path, false);
        let today = crate::ui::widgets::today();
        let student = |name: &str, days: i64, hour: u32| Student {
            name: name.into(),
            tcc: Some(TccRecord {
                meetings: vec![Meeting {
                    date: Some(today + Days::days(days)),
                    time: NaiveTime::from_hms_opt(hour, 0, 0),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        {
            let s = session(&mut h);
            s.db.students.extend([
                student("Aluno Passado", -2, 10),
                student("Aluno Atrasado", -1, 9),
                student("Aluno Futuro", 3, 10),
            ]);
            s.view = View::Calendar;
            s.calendar.range = crate::ui::calendar::Range::All;
        }
        h.key_press(egui::Key::F20);
        h.run_steps(3);
        let meeting = |h: &mut Harness<'_, SupervisorApp>, name: &str| {
            let s = session(h);
            s.db.students.iter().find(|s| s.name == name).unwrap().tcc.as_ref().unwrap().meetings[0].clone()
        };

        // Only meetings whose time has come offer Realizada and Remarcar.
        let held = format!("{} Realizada", icons::CHECK);
        assert_eq!(h.get_all_by_label(&held).count(), 2);
        shot(&mut h, "23_agenda_reunioes_passadas");
        h.get_all_by_label(&held).next().unwrap().click();
        h.run_steps(2);
        assert!(meeting(&mut h, "Aluno Passado").held);
        assert!(h.query_by_label("Marcada como realizada").is_some());
        assert_eq!(h.get_all_by_label(&held).count(), 1);

        // Postpone, picking the day in the date picker inside the dialog.
        h.get_by_label("Remarcar…").click();
        h.run_steps(2);
        assert!(h.query_by_label("Remarcar reunião").is_some());
        let suggested = today - Days::days(1) + Days::days(7);
        h.get_by_label(&format!("{}  {}", crate::pt::date(suggested), icons::CALENDAR_BLANK)).click();
        h.run_steps(2);
        shot(&mut h, "24_dialogo_remarcar");
        let next_day = suggested + Days::days(1);
        if next_day.month() != suggested.month() {
            h.get_by_label(icons::CARET_RIGHT).click();
            h.run_steps(1);
        }
        h.get_by_label(&crate::pt::date(next_day)).click();
        h.run_steps(1);
        h.get_by_label("Remarcar").click();
        h.run_steps(2);
        let late = meeting(&mut h, "Aluno Atrasado");
        assert_eq!(late.date, Some(next_day));
        assert_eq!(late.time, NaiveTime::from_hms_opt(9, 0, 0));
        assert!(h.query_by_label("Remarcar reunião").is_none());

        // Month grid: right-click a past meeting to undo "realizada".
        {
            let s = session(&mut h);
            s.calendar.layout = crate::ui::calendar::Layout::Month;
            s.calendar.month = (today - Days::days(2)).with_day(1).unwrap();
        }
        h.run_steps(2);
        h.get_by_label_contains("Aluno Passado").click_secondary();
        h.run_steps(2);
        shot(&mut h, "25_menu_reuniao_mes");
        h.get_by_label(&format!("{} Marcar como não realizada", icons::X)).click();
        h.run_steps(2);
        assert!(!meeting(&mut h, "Aluno Passado").held);
    }
}
