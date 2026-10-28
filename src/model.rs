//! Data model: students, their roles (TCC, iniciação científica, estágio,
//! tutoria, pós-graduação), grants (bolsas) and settings.

use std::collections::{HashMap, HashSet};

use chrono::{Datelike, Duration, Months, NaiveDate, NaiveTime};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::pt;

pub const DB_VERSION: u32 = 1;

/// How long before a grant (bolsa) ends to flag it on the dashboard.
const GRANT_WARNING_DAYS: i64 = 90;
/// ...and for how long after it ended.
const GRANT_ENDED_DAYS: i64 = 30;
/// How long before an internship agreement (TCE) ends to flag it.
const TCE_WARNING_DAYS: i64 = 30;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Semester {
    #[default]
    S1,
    S2,
    FullYear,
}

impl Semester {
    pub const ALL: [Semester; 3] = [Semester::S1, Semester::S2, Semester::FullYear];

    pub fn label(self) -> &'static str {
        match self {
            Semester::S1 => "1º semestre",
            Semester::S2 => "2º semestre",
            Semester::FullYear => "Anual",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TriState {
    #[default]
    Unknown,
    Yes,
    No,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ReplyStatus {
    /// Not contacted yet, or contacted and waiting.
    #[default]
    Awaiting,
    Replied,
    /// The student ignored the message.
    NoReply,
}

impl ReplyStatus {
    pub fn label(self) -> &'static str {
        match self {
            ReplyStatus::Awaiting => "Aguardando resposta",
            ReplyStatus::Replied => "Respondeu",
            ReplyStatus::NoReply => "Sem resposta",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum VisitMode {
    #[default]
    Online,
    FaceToFace,
}

impl VisitMode {
    pub fn label(self) -> &'static str {
        match self {
            VisitMode::Online => "On-line",
            VisitMode::FaceToFace => "Presencial",
        }
    }
}

/// Service used for an online meeting.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Platform {
    #[default]
    GoogleMeet,
    Teams,
    Zoom,
    /// Conferência Web of RNP, used by many Brazilian universities.
    Rnp,
}

impl Platform {
    pub const ALL: [Platform; 4] = [Platform::GoogleMeet, Platform::Teams, Platform::Zoom, Platform::Rnp];

    pub fn label(self) -> &'static str {
        match self {
            Platform::GoogleMeet => "Google Meet",
            Platform::Teams => "Teams",
            Platform::Zoom => "Zoom",
            Platform::Rnp => "RNP",
        }
    }

    /// Full name, for calendar files.
    pub fn long_label(self) -> &'static str {
        match self {
            Platform::GoogleMeet => "Google Meet",
            Platform::Teams => "Microsoft Teams",
            Platform::Zoom => "Zoom",
            Platform::Rnp => "Conferência Web RNP",
        }
    }
}

// ---------------------------------------------------------------------------
// Settings, courses and disciplines
// ---------------------------------------------------------------------------

/// Something TCC students hand in (an "entrega"), set per discipline.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct CourseworkTemplate {
    pub id: Uuid,
    pub title: String,
    /// Teaching week the item is due (relative to the semester start).
    pub week: Option<u8>,
    /// An explicit deadline; takes precedence over `week`.
    pub deadline: Option<NaiveDate>,
}

impl Default for CourseworkTemplate {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            title: "Nova entrega".into(),
            week: None,
            deadline: None,
        }
    }
}

impl CourseworkTemplate {
    /// The explicit date, or else the start of teaching `week` counted from
    /// the semester's week-1 start date.
    pub fn resolve_deadline(&self, semester_start: Option<NaiveDate>) -> Option<NaiveDate> {
        self.deadline.or_else(|| {
            let week = self.week?;
            Some(semester_start? + Duration::weeks(i64::from(week.max(1)) - 1))
        })
    }

    fn new(title: &str, week: Option<u8>) -> Self {
        Self {
            title: title.into(),
            week,
            ..Default::default()
        }
    }
}

/// A discipline (e.g. TCC I) with the items its students hand in.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct Module {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub semester: Semester,
    /// `Programme::id`s this discipline applies to.
    pub applies_to: Vec<String>,
    pub courseworks: Vec<CourseworkTemplate>,
}

impl Default for Module {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            code: "NOVA".into(),
            name: "Nova disciplina".into(),
            semester: Semester::S1,
            applies_to: Vec::new(),
            courseworks: Vec::new(),
        }
    }
}

/// An undergraduate course whose TCC students are supervised.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct Programme {
    /// Key stored on students and disciplines.
    pub id: String,
    pub name: String,
    pub short: String,
    /// Sidebar list it is shown in; courses sharing a list appear together.
    pub list: String,
}

impl Default for Programme {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            name: "Novo curso".into(),
            short: "Novo".into(),
            list: "TCC".into(),
        }
    }
}

impl Programme {
    /// The sidebar list, "TCC" when left blank.
    pub fn list_name(&self) -> &str {
        match self.list.trim() {
            "" => "TCC",
            list => list,
        }
    }
}

/// One generic course to start from; rename it in Settings.
pub fn default_programmes() -> Vec<Programme> {
    vec![Programme {
        id: "graduacao".into(),
        name: "Bacharelado".into(),
        short: "Grad.".into(),
        list: "TCC".into(),
    }]
}

/// Teaching weeks per semester when no end of teaching is set (a semester
/// of 100 dias letivos is about 18 weeks).
pub const TEACHING_WEEKS: i64 = 18;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct Settings {
    pub academic_year: String,
    pub sem1_start: Option<NaiveDate>,
    pub sem2_start: Option<NaiveDate>,
    /// Last day of teaching; defaults to Friday of week 18.
    pub sem1_teaching_end: Option<NaiveDate>,
    pub sem2_teaching_end: Option<NaiveDate>,
    pub programmes: Vec<Programme>,
    pub modules: Vec<Module>,
    pub dark_mode: bool,
    pub upcoming_days: u32,
    /// Organiser of invite files. Kept in the encrypted file like everything else.
    pub my_name: String,
    pub my_email: String,
    /// Optional reusable Teams meeting link, put into invites for Teams meetings.
    pub teams_link: String,
    /// Calendar exports include you as organiser and the students as guests.
    pub export_with_guests: bool,
    /// Lock after this many minutes without input; 0 = never.
    pub auto_lock_minutes: u32,
    /// Show enrolment numbers (often the CPF) as dots until revealed.
    pub hide_enrollment: bool,
    /// Revealing an enrolment number asks for the password again.
    pub reveal_needs_password: bool,
    /// Deadlines of each postgraduate level, in months from enrolment.
    pub level_rules: Vec<LevelRule>,
    /// Calendar weeks start on Sunday (usual in Brazil) rather than Monday.
    pub week_starts_sunday: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            academic_year: "2026".into(),
            sem1_start: None,
            sem2_start: None,
            sem1_teaching_end: None,
            sem2_teaching_end: None,
            programmes: default_programmes(),
            modules: default_modules(),
            dark_mode: false,
            upcoming_days: 14,
            my_name: String::new(),
            my_email: String::new(),
            teams_link: String::new(),
            export_with_guests: true,
            auto_lock_minutes: 10,
            hide_enrollment: true,
            reveal_needs_password: false,
            level_rules: default_level_rules(),
            week_starts_sunday: true,
        }
    }
}

/// TCC I and TCC II, without dates: a starting point to edit in Settings.
pub fn default_modules() -> Vec<Module> {
    let module = |code: &str, name: &str, semester, cws| Module {
        code: code.into(),
        name: name.into(),
        semester,
        applies_to: vec!["graduacao".into()],
        courseworks: cws,
        ..Default::default()
    };
    vec![
        module(
            "TCC I",
            "Trabalho de Conclusão de Curso I",
            Semester::S1,
            vec![
                CourseworkTemplate::new("Projeto de TCC", None),
                CourseworkTemplate::new("Apresentação do projeto", None),
            ],
        ),
        module(
            "TCC II",
            "Trabalho de Conclusão de Curso II",
            Semester::S2,
            vec![
                CourseworkTemplate::new("Versão para a banca", None),
                CourseworkTemplate::new("Versão final corrigida", None),
            ],
        ),
    ]
}

impl Settings {
    pub fn semester_start(&self, semester: Semester) -> Option<NaiveDate> {
        match semester {
            Semester::S1 | Semester::FullYear => self.sem1_start,
            Semester::S2 => self.sem2_start,
        }
    }

    /// Friday of the last teaching week, counted from the week-1 start date.
    pub fn default_teaching_end(start: NaiveDate) -> NaiveDate {
        let monday = start - Duration::days(i64::from(start.weekday().num_days_from_monday()));
        monday + Duration::weeks(TEACHING_WEEKS - 1) + Duration::days(4)
    }

    /// Last day of teaching: the date set in Settings, or Friday of week 18.
    pub fn teaching_end(&self, semester: Semester) -> Option<NaiveDate> {
        let end = match semester {
            Semester::S1 | Semester::FullYear => self.sem1_teaching_end,
            Semester::S2 => self.sem2_teaching_end,
        };
        end.or_else(|| self.semester_start(semester).map(Self::default_teaching_end))
    }

    /// The semester a date belongs to (semester 2 once it has started).
    pub fn semester_of(&self, date: NaiveDate) -> Semester {
        if self.sem2_start.is_some_and(|s| date >= s) {
            Semester::S2
        } else {
            Semester::S1
        }
    }

    /// First day of the calendar week `date` is in (Sunday, or Monday).
    pub fn week_start(&self, date: NaiveDate) -> NaiveDate {
        let back = if self.week_starts_sunday {
            date.weekday().num_days_from_sunday()
        } else {
            date.weekday().num_days_from_monday()
        };
        date - Duration::days(i64::from(back))
    }

    /// Teaching week number of `date`, if it falls within teaching.
    pub fn teaching_week(&self, date: NaiveDate) -> Option<(Semester, i64)> {
        [Semester::S1, Semester::S2].into_iter().find_map(|sem| {
            let start = self.semester_start(sem)?;
            let end = self.teaching_end(sem)?;
            (start..=end)
                .contains(&date)
                .then(|| (sem, (date - start).num_days() / 7 + 1))
        })
    }

    /// Deadline from the template alone (no per-student extension).
    pub fn template_deadline(&self, module: &Module, cw: &CourseworkTemplate) -> Option<NaiveDate> {
        cw.resolve_deadline(self.semester_start(module.semester))
    }

    /// Deadline for one student, honouring a per-student extension.
    pub fn effective_deadline(
        &self,
        module: &Module,
        cw: &CourseworkTemplate,
        status: Option<&CwStatus>,
    ) -> Option<NaiveDate> {
        status
            .and_then(|s| s.deadline_override)
            .or_else(|| self.template_deadline(module, cw))
    }

    pub fn modules_for<'a>(&'a self, programme: &'a str) -> impl Iterator<Item = &'a Module> {
        self.modules
            .iter()
            .filter(move |m| m.applies_to.iter().any(|p| p == programme))
    }

    pub fn programme(&self, id: &str) -> Option<&Programme> {
        self.programmes.iter().find(|p| p.id == id)
    }

    /// Short label of a course, for chips and event titles.
    pub fn programme_short(&self, id: &str) -> String {
        self.programme(id)
            .map_or_else(|| id.to_string(), |p| p.short.clone())
    }

    /// Sidebar lists of TCC students, in the order courses are defined.
    pub fn programme_lists(&self) -> Vec<String> {
        let mut lists: Vec<String> = Vec::new();
        for p in &self.programmes {
            if !lists.iter().any(|l| l == p.list_name()) {
                lists.push(p.list_name().to_string());
            }
        }
        lists
    }

    /// The deadlines in months for a postgraduate level.
    pub fn rule(&self, level: Level) -> LevelRule {
        self.level_rules
            .iter()
            .find(|r| r.level == level)
            .cloned()
            .unwrap_or_else(|| LevelRule::default_for(level))
    }
}

// ---------------------------------------------------------------------------
// Meetings
// ---------------------------------------------------------------------------

/// Upper bound on the weeks generated for one repeating meeting.
pub const MAX_OCCURRENCES: usize = 60;

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct Meeting {
    pub id: Uuid,
    /// The date, or the first date of a repeating meeting.
    pub date: Option<NaiveDate>,
    pub time: Option<NaiveTime>,
    /// For one-off meetings; repeating ones track this per week.
    pub held: bool,
    pub online: bool,
    pub platform: Platform,
    /// Meeting link; Teams meetings without one use the link in Settings.
    pub link: String,
    /// Room or place for in-person meetings.
    pub location: String,
    pub repeat: Option<Repeat>,
    pub notes: String,
    /// UID of the calendar event it was imported from, if any.
    pub source: String,
}

impl Default for Meeting {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            date: None,
            time: None,
            held: false,
            online: false,
            platform: Platform::GoogleMeet,
            link: String::new(),
            location: String::new(),
            repeat: None,
            notes: String::new(),
            source: String::new(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct Repeat {
    /// 1 = weekly, 2 = fortnightly, …
    pub every_weeks: u8,
    /// Last possible date; `None` means the end of teaching of the semester
    /// the meeting starts in.
    pub until: Option<NaiveDate>,
    /// Weeks that differ from the plan, keyed by their planned date.
    pub exceptions: Vec<Occurrence>,
}

impl Default for Repeat {
    fn default() -> Self {
        Self {
            every_weeks: 1,
            until: None,
            exceptions: Vec::new(),
        }
    }
}

/// One week of a meeting: moved, cancelled, held or with its own notes.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct Occurrence {
    pub planned: NaiveDate,
    pub cancelled: bool,
    pub moved_to: Option<NaiveDate>,
    /// Replaces the meeting's usual time for this week.
    pub time: Option<NaiveTime>,
    pub held: bool,
    pub notes: String,
}

impl Occurrence {
    fn planned(date: NaiveDate) -> Self {
        Self {
            planned: date,
            ..Default::default()
        }
    }

    pub fn date(&self) -> NaiveDate {
        self.moved_to.unwrap_or(self.planned)
    }

    /// Moved or retimed (not just held or annotated).
    pub fn rescheduled(&self) -> bool {
        !self.cancelled && (self.moved_to.is_some_and(|d| d != self.planned) || self.time.is_some())
    }

    fn is_plain(&self) -> bool {
        *self == Self::planned(self.planned)
    }
}

impl Meeting {
    /// Calendar UID; shared by the full export and invite files.
    pub fn uid(&self) -> String {
        format!("meeting-{}", self.id)
    }

    /// UID of one week of a repeating meeting, when exported on its own.
    pub fn occurrence_uid(&self, planned: NaiveDate) -> String {
        format!("meeting-{}-{}", self.id, planned.format("%Y%m%d"))
    }

    /// Where it happens: empty for online meetings, which say so themselves.
    pub fn place(&self) -> String {
        if self.online {
            String::new()
        } else {
            self.location.trim().into()
        }
    }

    /// Last date a repeating meeting can fall on.
    pub fn repeat_end(&self, settings: &Settings) -> Option<NaiveDate> {
        let repeat = self.repeat.as_ref()?;
        let start = self.date?;
        repeat
            .until
            .or_else(|| settings.teaching_end(settings.semester_of(start)))
    }

    /// Every week of a repeating meeting with its changes, or the single
    /// meeting itself. Without a known end only the first week is listed.
    pub fn occurrences(&self, settings: &Settings) -> Vec<Occurrence> {
        let Some(start) = self.date else {
            return Vec::new();
        };
        let Some(repeat) = &self.repeat else {
            return vec![Occurrence {
                held: self.held,
                ..Occurrence::planned(start)
            }];
        };
        let end = self.repeat_end(settings).unwrap_or(start);
        let step = Duration::weeks(i64::from(repeat.every_weeks.max(1)));
        let mut out = Vec::new();
        let mut date = start;
        while date <= end && out.len() < MAX_OCCURRENCES {
            out.push(
                repeat
                    .exceptions
                    .iter()
                    .find(|o| o.planned == date)
                    .cloned()
                    .unwrap_or_else(|| Occurrence::planned(date)),
            );
            date += step;
        }
        if out.is_empty() {
            out.push(Occurrence::planned(start));
        }
        out
    }

    /// The week planned for `planned`, with any changes made to it.
    pub fn occurrence(&self, planned: NaiveDate) -> Occurrence {
        self.repeat
            .as_ref()
            .and_then(|r| r.exceptions.iter().find(|o| o.planned == planned))
            .cloned()
            .unwrap_or_else(|| Occurrence::planned(planned))
    }

    /// Store a week's changes; a week back to plain is forgotten.
    pub fn set_occurrence(&mut self, occurrence: Occurrence) {
        if let Some(repeat) = &mut self.repeat {
            repeat
                .exceptions
                .retain(|o| o.planned != occurrence.planned);
            if !occurrence.is_plain() {
                repeat.exceptions.push(occurrence);
                repeat.exceptions.sort_by_key(|o| o.planned);
            }
        }
    }
}

pub fn visit_uid(student_id: Uuid, index: usize) -> String {
    format!("visit{}-{student_id}", index + 1)
}

// ---------------------------------------------------------------------------
// TCC
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct CwStatus {
    pub submitted: bool,
    pub submitted_on: Option<NaiveDate>,
    pub deadline_override: Option<NaiveDate>,
    pub notes: String,
}

/// An undergraduate final project (Trabalho de Conclusão de Curso).
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct TccRecord {
    /// `Programme::id`.
    pub programme: String,
    pub title: String,
    /// Co-supervisor (coorientador), if any.
    pub co_supervisor: String,
    pub initial_contact: bool,
    pub initial_contact_on: Option<NaiveDate>,
    pub meetings: Vec<Meeting>,
    /// Keyed by `CourseworkTemplate::id`.
    pub coursework: HashMap<Uuid, CwStatus>,
    /// The presentation to the committee (banca).
    pub defense: Option<NaiveDate>,
    pub defense_time: Option<NaiveTime>,
    pub defense_location: String,
    /// Members of the committee.
    pub committee: String,
    pub grade: String,
}

impl Default for TccRecord {
    fn default() -> Self {
        Self {
            programme: "graduacao".into(),
            title: String::new(),
            co_supervisor: String::new(),
            initial_contact: false,
            initial_contact_on: None,
            meetings: Vec::new(),
            coursework: HashMap::new(),
            defense: None,
            defense_time: None,
            defense_location: String::new(),
            committee: String::new(),
            grade: String::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Estágio supervisionado (internship) and tutoria (personal tutees)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct Visit {
    pub mode: VisitMode,
    pub request_sent: Option<NaiveDate>,
    pub response: ReplyStatus,
    pub responded_on: Option<NaiveDate>,
    pub date: Option<NaiveDate>,
    pub time: Option<NaiveTime>,
    pub line_manager_attending: TriState,
    pub completed: bool,
    pub notes: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct PlacementRecord {
    /// The company or institution offering the internship (concedente).
    pub company: String,
    pub professional_email: String,
    /// Supervisor at the company.
    pub line_manager_name: String,
    pub line_manager_email: String,
    /// Required by the course (estágio obrigatório) or optional.
    pub mandatory: bool,
    /// Term of the internship agreement (Termo de Compromisso de Estágio).
    pub tce_start: Option<NaiveDate>,
    pub tce_end: Option<NaiveDate>,
    /// The internship report (relatório de estágio).
    pub report: Milestone,
    /// Visits to the company, as many as the course asks for (two to start
    /// with; older files always have two).
    pub visits: Vec<Visit>,
}

impl Default for PlacementRecord {
    fn default() -> Self {
        Self {
            company: String::new(),
            professional_email: String::new(),
            line_manager_name: String::new(),
            line_manager_email: String::new(),
            mandatory: true,
            tce_start: None,
            tce_end: None,
            report: Milestone::default(),
            visits: vec![
                Visit::default(),
                Visit {
                    mode: VisitMode::FaceToFace,
                    ..Default::default()
                },
            ],
        }
    }
}

impl PlacementRecord {
    /// A new visit: in person when the last one was online, and vice versa.
    pub fn add_visit(&mut self) {
        let mode = match self.visits.last().map(|v| v.mode) {
            Some(VisitMode::Online) => VisitMode::FaceToFace,
            _ => VisitMode::Online,
        };
        self.visits.push(Visit {
            mode,
            ..Default::default()
        });
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct SemesterContact {
    pub contacted_on: Option<NaiveDate>,
    pub reply: ReplyStatus,
    pub replied_on: Option<NaiveDate>,
    /// First entry is the semester meeting; the rest are follow-ons.
    pub meetings: Vec<Meeting>,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct AdviseeRecord {
    pub s1: SemesterContact,
    pub s2: SemesterContact,
}

// ---------------------------------------------------------------------------
// Deadlines and reminders
// ---------------------------------------------------------------------------

/// A deadline, typed in or suggested, and when it was met.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct Milestone {
    pub deadline: Option<NaiveDate>,
    pub done: bool,
    pub done_on: Option<NaiveDate>,
}

impl Milestone {
    /// The reminder date, `lead` before the deadline.
    pub fn reminder(&self, lead: Lead) -> Option<NaiveDate> {
        lead.before(self.deadline?)
    }

    fn due(date: Option<NaiveDate>) -> Self {
        Self {
            deadline: date,
            ..Default::default()
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LeadUnit {
    Days,
    #[default]
    Weeks,
    Months,
}

impl LeadUnit {
    pub const ALL: [LeadUnit; 3] = [LeadUnit::Days, LeadUnit::Weeks, LeadUnit::Months];

    pub fn label(self) -> &'static str {
        match self {
            LeadUnit::Days => "dias",
            LeadUnit::Weeks => "semanas",
            LeadUnit::Months => "meses",
        }
    }
}

/// How long before a deadline its reminder falls; 0 means no reminder.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(default)]
pub struct Lead {
    pub amount: u32,
    pub unit: LeadUnit,
}

impl Default for Lead {
    fn default() -> Self {
        Self {
            amount: 2,
            unit: LeadUnit::Weeks,
        }
    }
}

impl Lead {
    pub fn months(amount: u32) -> Self {
        Self {
            amount,
            unit: LeadUnit::Months,
        }
    }

    pub fn weeks(amount: u32) -> Self {
        Self {
            amount,
            unit: LeadUnit::Weeks,
        }
    }

    pub fn before(self, date: NaiveDate) -> Option<NaiveDate> {
        match (self.amount, self.unit) {
            (0, _) => None,
            (n, LeadUnit::Days) => date.checked_sub_signed(Duration::days(i64::from(n))),
            (n, LeadUnit::Weeks) => date.checked_sub_signed(Duration::weeks(i64::from(n))),
            (n, LeadUnit::Months) => date.checked_sub_months(Months::new(n)),
        }
    }
}

/// A deadline added by hand (e.g. from the programme's own rules), with its own reminder.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct CustomStep {
    pub id: Uuid,
    pub title: String,
    pub milestone: Milestone,
    pub remind: Lead,
}

impl Default for CustomStep {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            title: String::new(),
            milestone: Milestone::default(),
            remind: Lead::default(),
        }
    }
}

/// One dated step of a research project, for events, reminders and editing
/// (change a copy, then write it back with `milestone_mut`).
pub struct Step {
    /// Stable part of the event UID.
    pub key: String,
    /// E.g. "Exame de qualificação".
    pub label: String,
    pub milestone: Milestone,
    /// How long before the deadline the reminder falls.
    pub lead: Lead,
}

fn step(key: &str, label: &str, milestone: &Milestone, lead: Lead) -> Step {
    Step {
        key: key.into(),
        label: label.into(),
        milestone: milestone.clone(),
        lead,
    }
}

fn custom_steps(steps: &[CustomStep]) -> impl Iterator<Item = Step> + '_ {
    steps.iter().map(|c| Step {
        key: format!("custom-{}", c.id),
        label: match c.title.trim() {
            "" => "Prazo".into(),
            title => title.into(),
        },
        milestone: c.milestone.clone(),
        lead: c.remind,
    })
}

fn custom_milestone<'a>(steps: &'a mut [CustomStep], key: &str) -> Option<&'a mut Milestone> {
    steps
        .iter_mut()
        .find(|c| format!("custom-{}", c.id) == key)
        .map(|c| &mut c.milestone)
}

fn add_months(date: NaiveDate, months: u32) -> Option<NaiveDate> {
    date.checked_add_months(Months::new(months))
}

// ---------------------------------------------------------------------------
// Funding agencies and grants (bolsas)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AgencyKind {
    Federal,
    /// A state research foundation (FAP).
    State,
    Other,
}

/// A body that funds scholarships, listed in the grant editor.
pub struct Agency {
    /// Stored on grants.
    pub id: &'static str,
    pub name: &'static str,
    pub kind: AgencyKind,
    /// State (UF) of a FAP.
    pub uf: &'static str,
}

const fn agency(id: &'static str, name: &'static str, kind: AgencyKind, uf: &'static str) -> Agency {
    Agency { id, name, kind, uf }
}

/// Federal agencies, the 27 state foundations (FAPs, one per state, as
/// listed by CONFAP) and other sources. All of them fund both iniciação
/// científica and postgraduate scholarships.
pub const AGENCIES: &[Agency] = &[
    agency("CNPq", "Conselho Nacional de Desenvolvimento Científico e Tecnológico", AgencyKind::Federal, ""),
    agency("CAPES", "Coordenação de Aperfeiçoamento de Pessoal de Nível Superior", AgencyKind::Federal, ""),
    agency("FINEP", "Financiadora de Estudos e Projetos", AgencyKind::Federal, ""),
    agency("ANP", "Agência Nacional do Petróleo – PRH-ANP", AgencyKind::Federal, ""),
    agency("MEC", "Ministério da Educação (PET, residências)", AgencyKind::Federal, ""),
    agency("MS", "Ministério da Saúde (residências)", AgencyKind::Federal, ""),
    agency("FAPAC", "Fundação de Amparo à Pesquisa do Acre", AgencyKind::State, "AC"),
    agency("FAPEAL", "Fundação de Amparo à Pesquisa do Estado de Alagoas", AgencyKind::State, "AL"),
    agency("FAPEAP", "Fundação de Amparo à Pesquisa do Amapá", AgencyKind::State, "AP"),
    agency("FAPEAM", "Fundação de Amparo à Pesquisa do Estado do Amazonas", AgencyKind::State, "AM"),
    agency("FAPESB", "Fundação de Amparo à Pesquisa do Estado da Bahia", AgencyKind::State, "BA"),
    agency("FUNCAP", "Fundação Cearense de Apoio ao Desenvolvimento Científico e Tecnológico", AgencyKind::State, "CE"),
    agency("FAPDF", "Fundação de Apoio à Pesquisa do Distrito Federal", AgencyKind::State, "DF"),
    agency("FAPES", "Fundação de Amparo à Pesquisa e Inovação do Espírito Santo", AgencyKind::State, "ES"),
    agency("FAPEG", "Fundação de Amparo à Pesquisa do Estado de Goiás", AgencyKind::State, "GO"),
    agency("FAPEMA", "Fundação de Amparo à Pesquisa e ao Desenvolvimento Científico do Maranhão", AgencyKind::State, "MA"),
    agency("FAPEMAT", "Fundação de Amparo à Pesquisa do Estado de Mato Grosso", AgencyKind::State, "MT"),
    agency("FUNDECT", "Fundação de Apoio ao Desenvolvimento do Ensino, Ciência e Tecnologia de MS", AgencyKind::State, "MS"),
    agency("FAPEMIG", "Fundação de Amparo à Pesquisa do Estado de Minas Gerais", AgencyKind::State, "MG"),
    agency("FAPESPA", "Fundação Amazônia de Amparo a Estudos e Pesquisas do Pará", AgencyKind::State, "PA"),
    agency("FAPESQ", "Fundação de Apoio à Pesquisa do Estado da Paraíba", AgencyKind::State, "PB"),
    agency("Araucária", "Fundação Araucária (Paraná)", AgencyKind::State, "PR"),
    agency("FACEPE", "Fundação de Amparo à Ciência e Tecnologia do Estado de Pernambuco", AgencyKind::State, "PE"),
    agency("FAPEPI", "Fundação de Amparo à Pesquisa do Estado do Piauí", AgencyKind::State, "PI"),
    agency("FAPERJ", "Fundação Carlos Chagas Filho de Amparo à Pesquisa do Estado do Rio de Janeiro", AgencyKind::State, "RJ"),
    agency("FAPERN", "Fundação de Apoio à Pesquisa do Estado do Rio Grande do Norte", AgencyKind::State, "RN"),
    agency("FAPERGS", "Fundação de Amparo à Pesquisa do Estado do Rio Grande do Sul", AgencyKind::State, "RS"),
    agency("FAPERO", "Fundação de Amparo à Pesquisa de Rondônia", AgencyKind::State, "RO"),
    agency("FAPERR", "Fundação de Amparo à Pesquisa do Estado de Roraima", AgencyKind::State, "RR"),
    agency("FAPESC", "Fundação de Amparo à Pesquisa e Inovação do Estado de Santa Catarina", AgencyKind::State, "SC"),
    agency("FAPESP", "Fundação de Amparo à Pesquisa do Estado de São Paulo", AgencyKind::State, "SP"),
    agency("FAPITEC", "Fundação de Apoio à Pesquisa e à Inovação Tecnológica de Sergipe", AgencyKind::State, "SE"),
    agency("FAPT", "Fundação de Amparo à Pesquisa do Tocantins", AgencyKind::State, "TO"),
    agency("Institucional", "Bolsa da própria instituição (IES)", AgencyKind::Other, ""),
    agency("Empresa", "Empresa ou convênio", AgencyKind::Other, ""),
    agency("Outra", "Outra fonte", AgencyKind::Other, ""),
];

pub fn agency_by_id(id: &str) -> Option<&'static Agency> {
    AGENCIES.iter().find(|a| a.id == id)
}

/// Usual length of a grant in months: 12 for iniciação científica (renewable),
/// 24 for a master's and 48 for a doctorate (CAPES Portaria 76/2010 art. 10,
/// CNPq RN-017/2006, and most FAPs). Always editable.
pub fn typical_grant_months(level: Option<Level>) -> u32 {
    match level {
        None => 12,
        Some(Level::MestradoAcademico | Level::MestradoProfissional) => 24,
        Some(Level::DoutoradoAcademico | Level::DoutoradoProfissional) => 48,
        Some(Level::Residencia) => 24,
        Some(Level::Especializacao | Level::PosDoutorado) => 12,
    }
}

/// Why a grant or a course deadline was extended.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ExtensionReason {
    #[default]
    None,
    /// Birth, adoption or guardianship (Lei 13.536/2017, alterada pela Lei 14.925/2024).
    Parental,
    Health,
    /// Decided by the programme's board (colegiado).
    Board,
    Other,
}

impl ExtensionReason {
    pub const ALL: [ExtensionReason; 5] = [
        ExtensionReason::None,
        ExtensionReason::Parental,
        ExtensionReason::Health,
        ExtensionReason::Board,
        ExtensionReason::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ExtensionReason::None => "—",
            ExtensionReason::Parental => "Licença parental (Lei 14.925/2024)",
            ExtensionReason::Health => "Saúde",
            ExtensionReason::Board => "Decisão do colegiado",
            ExtensionReason::Other => "Outro motivo",
        }
    }

    /// A hint on how long the extension may be.
    pub fn hint(self) -> &'static str {
        match self {
            ExtensionReason::Parental => {
                "A Lei 14.925/2024 permite prorrogar bolsas por até 180 dias (6 meses) em caso de parto, adoção ou guarda."
            }
            ExtensionReason::Health => {
                "Na CAPES (Portaria 76/2010), a suspensão por doença grave, parto ou aleitamento é de até 6 meses e não conta no prazo da bolsa."
            }
            _ => "",
        }
    }
}

/// A scholarship: who pays, which programme, and for how long.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct Grant {
    /// `Agency::id`; empty when there is no grant.
    pub agency: String,
    /// Name of the source when `agency` is "Outra".
    pub agency_other: String,
    /// E.g. "Demanda Social", "PROEX", "GM", "PIBIC".
    pub modality: String,
    /// Process number at the agency.
    pub process: String,
    pub start: Option<NaiveDate>,
    /// Planned length in months; 0 = not set.
    pub months: u32,
    /// Months added by an extension (prorrogação).
    pub extension_months: u32,
    pub extension_reason: ExtensionReason,
    /// The last day, typed in when it differs from the calculated one.
    pub end_override: Option<NaiveDate>,
    /// Postgraduate grants: it lasts until the final deadline of the course
    /// (the defence), whatever its length in months.
    pub until_final: bool,
}

impl Grant {
    pub fn active(&self) -> bool {
        !self.agency.is_empty()
    }

    /// Start + months + extension, minus a day. Without a start of its own
    /// the grant starts with the course (`course_start`, the enrolment).
    pub fn calculated_end(&self, course_start: Option<NaiveDate>) -> Option<NaiveDate> {
        if !self.active() || self.months == 0 {
            return None;
        }
        add_months(self.start.or(course_start)?, self.months + self.extension_months)?.pred_opt()
    }

    /// Last day of the grant: the one typed in, or the calculated one.
    pub fn end_from(&self, course_start: Option<NaiveDate>) -> Option<NaiveDate> {
        if !self.active() {
            return None;
        }
        self.end_override.or_else(|| self.calculated_end(course_start))
    }

    /// "CAPES", or the name typed in for another source.
    pub fn agency_label(&self) -> String {
        match self.agency.as_str() {
            "Outra" if !self.agency_other.trim().is_empty() => self.agency_other.trim().into(),
            "" => "Sem bolsa".into(),
            id => id.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Iniciação científica
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum IcModality {
    #[default]
    Pibic,
    Pibiti,
    /// Ações afirmativas.
    PibicAf,
    /// Ensino médio.
    PibicEm,
    /// Without a grant (PIVIC and similar).
    Voluntary,
    Institutional,
    Other,
}

impl IcModality {
    pub const ALL: [IcModality; 7] = [
        IcModality::Pibic,
        IcModality::Pibiti,
        IcModality::PibicAf,
        IcModality::PibicEm,
        IcModality::Voluntary,
        IcModality::Institutional,
        IcModality::Other,
    ];

    pub fn label(self) -> &'static str {
        match self {
            IcModality::Pibic => "PIBIC",
            IcModality::Pibiti => "PIBITI",
            IcModality::PibicAf => "PIBIC-Af (ações afirmativas)",
            IcModality::PibicEm => "PIBIC-EM (ensino médio)",
            IcModality::Voluntary => "Voluntária (PIVIC)",
            IcModality::Institutional => "Programa institucional",
            IcModality::Other => "Outra",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            IcModality::PibicAf => "PIBIC-Af",
            IcModality::PibicEm => "PIBIC-EM",
            IcModality::Voluntary => "Voluntária",
            IcModality::Institutional => "Institucional",
            other => other.label(),
        }
    }
}

/// An undergraduate research student (iniciação científica). The usual
/// cycle is a 12-month grant with a partial report around month 6, a final
/// report at the end and a talk or poster at the institution's annual
/// seminar (CNPq RN-017/2006).
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct IcRecord {
    /// Title of the work plan (plano de trabalho).
    pub title: String,
    /// The supervisor's research project it belongs to.
    pub project: String,
    pub modality: IcModality,
    pub grant: Grant,
    pub partial_report: Milestone,
    pub final_report: Milestone,
    pub seminar: Milestone,
    pub custom_steps: Vec<CustomStep>,
    pub meetings: Vec<Meeting>,
}

impl IcRecord {
    pub fn new() -> Self {
        Self {
            grant: Grant {
                agency: "CNPq".into(),
                modality: "PIBIC".into(),
                months: 12,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub fn steps(&self) -> Vec<Step> {
        let mut steps = vec![
            step("partial", "Relatório parcial", &self.partial_report, Lead::weeks(3)),
            step("final", "Relatório final", &self.final_report, Lead::months(1)),
            step("seminar", "Apresentação no seminário de IC", &self.seminar, Lead::months(1)),
        ];
        steps.extend(custom_steps(&self.custom_steps));
        steps
    }

    /// Last day of the grant, if any. An IC grant has its own start.
    pub fn grant_end(&self) -> Option<NaiveDate> {
        self.grant.end_from(None)
    }

    pub fn milestone_mut(&mut self, key: &str) -> Option<&mut Milestone> {
        match key {
            "partial" => Some(&mut self.partial_report),
            "final" => Some(&mut self.final_report),
            "seminar" => Some(&mut self.seminar),
            _ => custom_milestone(&mut self.custom_steps, key),
        }
    }

    /// Fills the empty report deadlines from the grant: the partial report
    /// half-way, the final report on its last day.
    pub fn suggest(&mut self) {
        let Some(start) = self.grant.start else { return };
        let months = match self.grant.months {
            0 => 12,
            n => n,
        } + self.grant.extension_months;
        let end = self
            .grant
            .end_override
            .or_else(|| add_months(start, months).and_then(|d| d.pred_opt()));
        if self.partial_report.deadline.is_none() {
            self.partial_report.deadline = match self.grant.end_override {
                // A typed-in end: half-way between the two days.
                Some(end) if end > start => Some(start + (end - start) / 2),
                _ => add_months(start, months / 2),
            };
        }
        if self.final_report.deadline.is_none() {
            self.final_report.deadline = end;
        }
    }

    pub fn next_step(&self) -> Option<Step> {
        next_step(self.steps())
    }
}

fn next_step(steps: Vec<Step>) -> Option<Step> {
    steps
        .into_iter()
        .filter(|s| !s.milestone.done && s.milestone.deadline.is_some())
        .min_by_key(|s| s.milestone.deadline)
}

// ---------------------------------------------------------------------------
// Pós-graduação
// ---------------------------------------------------------------------------

/// Postgraduate levels: lato sensu (specialisation, residency) and stricto
/// sensu (academic and professional master's and doctorates), plus
/// postdoctoral researchers you supervise.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub enum Level {
    Especializacao,
    Residencia,
    #[default]
    MestradoAcademico,
    MestradoProfissional,
    DoutoradoAcademico,
    DoutoradoProfissional,
    PosDoutorado,
}

impl Level {
    pub const ALL: [Level; 7] = [
        Level::Especializacao,
        Level::Residencia,
        Level::MestradoAcademico,
        Level::MestradoProfissional,
        Level::DoutoradoAcademico,
        Level::DoutoradoProfissional,
        Level::PosDoutorado,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Level::Especializacao => "Especialização / MBA",
            Level::Residencia => "Residência",
            Level::MestradoAcademico => "Mestrado acadêmico",
            Level::MestradoProfissional => "Mestrado profissional",
            Level::DoutoradoAcademico => "Doutorado acadêmico",
            Level::DoutoradoProfissional => "Doutorado profissional",
            Level::PosDoutorado => "Pós-doutorado",
        }
    }

    /// For chips and event titles.
    pub fn short(self) -> &'static str {
        match self {
            Level::Especializacao => "Especialização",
            Level::Residencia => "Residência",
            Level::MestradoAcademico => "Mestrado",
            Level::MestradoProfissional => "Mestrado prof.",
            Level::DoutoradoAcademico => "Doutorado",
            Level::DoutoradoProfissional => "Doutorado prof.",
            Level::PosDoutorado => "Pós-doc",
        }
    }

    pub fn lato_sensu(self) -> bool {
        matches!(self, Level::Especializacao | Level::Residencia)
    }

    /// "Lato sensu" or "Stricto sensu" (postdocs are neither).
    pub fn kind(self) -> &'static str {
        match self {
            Level::Especializacao | Level::Residencia => "Lato sensu",
            Level::PosDoutorado => "Pós-doutorado",
            _ => "Stricto sensu",
        }
    }

    pub fn group(self) -> PosGroup {
        match self {
            Level::Especializacao | Level::Residencia => PosGroup::LatoSensu,
            Level::MestradoAcademico => PosGroup::Mestrado,
            Level::MestradoProfissional => PosGroup::MestradoProfissional,
            Level::DoutoradoAcademico | Level::DoutoradoProfissional => PosGroup::Doutorado,
            Level::PosDoutorado => PosGroup::PosDoutorado,
        }
    }

    /// The work that ends the course.
    pub fn work(self) -> &'static str {
        match self {
            Level::Especializacao | Level::Residencia => "trabalho de conclusão",
            Level::MestradoAcademico => "dissertação",
            Level::MestradoProfissional => "trabalho de conclusão",
            Level::DoutoradoAcademico | Level::DoutoradoProfissional => "tese",
            Level::PosDoutorado => "relatório final",
        }
    }

    /// Label of the final deadline.
    pub fn final_label(self) -> String {
        match self {
            Level::PosDoutorado => "Relatório final".into(),
            Level::Especializacao | Level::Residencia => "Entrega do trabalho de conclusão".into(),
            _ => format!("Prazo para a defesa da {}", self.work()),
        }
    }
}

/// The postgraduate lists in the sidebar.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum PosGroup {
    LatoSensu,
    Mestrado,
    MestradoProfissional,
    Doutorado,
    PosDoutorado,
}

impl PosGroup {
    pub const ALL: [PosGroup; 5] = [
        PosGroup::LatoSensu,
        PosGroup::Mestrado,
        PosGroup::MestradoProfissional,
        PosGroup::Doutorado,
        PosGroup::PosDoutorado,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PosGroup::LatoSensu => "Lato sensu",
            PosGroup::Mestrado => "Mestrado",
            PosGroup::MestradoProfissional => "Mestrado profissional",
            PosGroup::Doutorado => "Doutorado",
            PosGroup::PosDoutorado => "Pós-doutorado",
        }
    }

    /// The level a new student of this list starts with.
    pub fn level(self) -> Level {
        match self {
            PosGroup::LatoSensu => Level::Especializacao,
            PosGroup::Mestrado => Level::MestradoAcademico,
            PosGroup::MestradoProfissional => Level::MestradoProfissional,
            PosGroup::Doutorado => Level::DoutoradoAcademico,
            PosGroup::PosDoutorado => Level::PosDoutorado,
        }
    }

    /// Shown in the sidebar and dashboard even when empty.
    pub fn always_shown(self) -> bool {
        matches!(self, PosGroup::Mestrado | PosGroup::Doutorado)
    }
}

/// Deadlines of a level, in months from enrolment; 0 = doesn't apply.
/// Each programme's regulations (regimento) set its own, so the defaults
/// are only a common starting point and can be changed in Settings.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct LevelRule {
    pub level: Level,
    /// Complete the course credits (integralização dos créditos).
    pub credits_months: u32,
    /// Foreign-language proficiency exam.
    pub proficiency_months: u32,
    /// Qualifying exam (exame de qualificação).
    pub qualification_months: u32,
    /// The defence, or the final work for lato sensu and postdocs.
    pub final_months: u32,
}

impl Default for LevelRule {
    fn default() -> Self {
        Self::default_for(Level::MestradoAcademico)
    }
}

impl LevelRule {
    pub fn default_for(level: Level) -> Self {
        let (credits, proficiency, qualification, final_months) = match level {
            Level::Especializacao => (12, 0, 0, 18),
            Level::Residencia => (0, 0, 0, 24),
            Level::MestradoAcademico => (12, 12, 12, 24),
            Level::MestradoProfissional => (12, 12, 15, 24),
            Level::DoutoradoAcademico => (24, 24, 24, 48),
            Level::DoutoradoProfissional => (24, 24, 30, 48),
            Level::PosDoutorado => (0, 0, 0, 12),
        };
        Self {
            level,
            credits_months: credits,
            proficiency_months: proficiency,
            qualification_months: qualification,
            final_months,
        }
    }
}

pub fn default_level_rules() -> Vec<LevelRule> {
    Level::ALL.into_iter().map(LevelRule::default_for).collect()
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum PosStatus {
    #[default]
    Active,
    /// Enrolment suspended (trancamento) or on leave.
    Suspended,
    Writing,
    AwaitingDefense,
    Graduated,
    Withdrawn,
}

impl PosStatus {
    pub const ALL: [PosStatus; 6] = [
        PosStatus::Active,
        PosStatus::Suspended,
        PosStatus::Writing,
        PosStatus::AwaitingDefense,
        PosStatus::Graduated,
        PosStatus::Withdrawn,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PosStatus::Active => "Cursando",
            PosStatus::Suspended => "Trancado / afastado",
            PosStatus::Writing => "Em redação",
            PosStatus::AwaitingDefense => "Aguardando defesa",
            PosStatus::Graduated => "Titulado(a)",
            PosStatus::Withdrawn => "Desligado(a)",
        }
    }

    /// Still working towards the degree, so deadlines and grants matter.
    pub fn active(self) -> bool {
        !matches!(self, PosStatus::Graduated | PosStatus::Withdrawn)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum DefenseOutcome {
    #[default]
    Pending,
    Approved,
    ApprovedWithChanges,
    Resubmit,
    Failed,
}

impl DefenseOutcome {
    pub const ALL: [DefenseOutcome; 5] = [
        DefenseOutcome::Pending,
        DefenseOutcome::Approved,
        DefenseOutcome::ApprovedWithChanges,
        DefenseOutcome::Resubmit,
        DefenseOutcome::Failed,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DefenseOutcome::Pending => "Ainda não definido",
            DefenseOutcome::Approved => "Aprovado(a)",
            DefenseOutcome::ApprovedWithChanges => "Aprovado(a) com correções",
            DefenseOutcome::Resubmit => "Reformulação (nova defesa)",
            DefenseOutcome::Failed => "Reprovado(a)",
        }
    }
}

/// A postgraduate student, or a postdoc you supervise.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct PosRecord {
    pub title: String,
    /// Postgraduate programme (PPG).
    pub program: String,
    pub co_supervisors: String,
    pub status: PosStatus,
    pub level: Level,
    /// Enrolled in another programme or institution; you co-supervise.
    pub external: bool,
    /// Their institution, for external students.
    pub institution: String,
    /// Enrolment (ingresso).
    pub start: Option<NaiveDate>,
    /// Months added to the course deadlines by an extension.
    pub extension_months: u32,
    pub extension_reason: ExtensionReason,
    pub grant: Grant,
    pub credits: Milestone,
    pub proficiency: Milestone,
    pub qualification: Milestone,
    /// Reports to the programme or the funding agency.
    pub annual_reports: Vec<Milestone>,
    /// Sending the work to the committee (depósito / envio à banca).
    pub submission: Milestone,
    /// The deadline for the defence (or the final work).
    pub final_deadline: Milestone,
    pub defense: Option<NaiveDate>,
    pub defense_time: Option<NaiveTime>,
    pub defense_location: String,
    pub committee: String,
    pub outcome: DefenseOutcome,
    /// Handing in the corrected final version after the defence.
    pub final_version: Milestone,
    pub awarded_on: Option<NaiveDate>,
    /// Leave out the standard steps (credits, qualifying exam, …), e.g. for
    /// external students whose programme has other rules.
    pub skip_standard: bool,
    pub custom_steps: Vec<CustomStep>,
    pub meetings: Vec<Meeting>,
}

impl PosRecord {
    pub fn new(level: Level) -> Self {
        Self {
            level,
            ..Default::default()
        }
    }

    /// Every step with a deadline, in the order of the course, then those
    /// added by hand.
    pub fn steps(&self) -> Vec<Step> {
        let mut steps = Vec::new();
        if !self.skip_standard {
            let stricto = !self.level.lato_sensu() && self.level != Level::PosDoutorado;
            if self.level != Level::PosDoutorado && self.level != Level::Residencia {
                steps.push(step("credits", "Integralização dos créditos", &self.credits, Lead::months(2)));
            }
            if stricto {
                steps.push(step("proficiency", "Proficiência em língua estrangeira", &self.proficiency, Lead::months(2)));
                steps.push(step("qualification", "Exame de qualificação", &self.qualification, Lead::months(3)));
            }
            for (i, report) in self.annual_reports.iter().enumerate() {
                let label = match report.deadline {
                    Some(d) => format!("Relatório anual {}", d.year()),
                    None => format!("Relatório anual {}", i + 1),
                };
                steps.push(step(&format!("annual{}", i + 1), &label, report, Lead::months(1)));
            }
            if self.level != Level::PosDoutorado {
                let send = format!("Envio da {} à banca", self.level.work());
                steps.push(step("submission", &send, &self.submission, Lead::months(1)));
            }
            steps.push(step("final", &self.level.final_label(), &self.final_deadline, Lead::months(4)));
        }
        steps.push(step("final-version", "Entrega da versão final", &self.final_version, Lead::weeks(2)));
        steps.extend(custom_steps(&self.custom_steps));
        steps
    }

    /// Last day of the grant, if any. Without a start of its own it starts
    /// at enrolment; linked to the final deadline, it ends then.
    pub fn grant_end(&self) -> Option<NaiveDate> {
        if !self.grant.active() {
            return None;
        }
        match self.final_deadline.deadline {
            Some(deadline) if self.grant.until_final => Some(deadline),
            _ => self.grant.end_from(self.start),
        }
    }

    /// Which postgraduate list the student is shown in.
    pub fn group(&self) -> PosGroup {
        self.level.group()
    }

    /// Year of the course (1-based) on `date`, counted from enrolment.
    pub fn year_of_study(&self, date: NaiveDate) -> Option<u32> {
        let start = self.start?;
        let mut year = 1;
        while start.checked_add_months(Months::new(12 * year)).is_some_and(|d| d <= date) {
            year += 1;
        }
        (date >= start).then_some(year)
    }

    /// The milestone a `Step::key` refers to.
    pub fn milestone_mut(&mut self, key: &str) -> Option<&mut Milestone> {
        match key {
            "credits" => Some(&mut self.credits),
            "proficiency" => Some(&mut self.proficiency),
            "qualification" => Some(&mut self.qualification),
            "submission" => Some(&mut self.submission),
            "final" => Some(&mut self.final_deadline),
            "final-version" => Some(&mut self.final_version),
            _ if key.starts_with("custom-") => custom_milestone(&mut self.custom_steps, key),
            _ => {
                let n: usize = key.strip_prefix("annual")?.parse().ok()?;
                self.annual_reports.get_mut(n.checked_sub(1)?)
            }
        }
    }

    /// The final deadline the rules give: enrolment + months of the level
    /// + any extension.
    pub fn rule_deadline(&self, rule: &LevelRule) -> Option<NaiveDate> {
        (rule.final_months > 0)
            .then(|| add_months(self.start?, rule.final_months + self.extension_months))
            .flatten()
    }

    /// Fills the empty deadlines from the enrolment date and the rules of
    /// the level; dates already set are kept. Annual reports are added for
    /// each full year before the final deadline.
    pub fn suggest(&mut self, rule: &LevelRule) {
        let Some(start) = self.start else { return };
        let at = |months: u32| (months > 0).then(|| add_months(start, months)).flatten();
        for (m, months) in [
            (&mut self.credits, rule.credits_months),
            (&mut self.proficiency, rule.proficiency_months),
            (&mut self.qualification, rule.qualification_months),
        ] {
            if m.deadline.is_none() {
                m.deadline = at(months);
            }
        }
        if self.final_deadline.deadline.is_none() {
            self.final_deadline.deadline = self.rule_deadline(rule);
        }
        if self.submission.deadline.is_none() && self.level != Level::PosDoutorado {
            self.submission.deadline = self.final_deadline.deadline.and_then(|d| d.checked_sub_months(Months::new(1)));
        }
        if self.annual_reports.is_empty()
            && let Some(end) = self.final_deadline.deadline
        {
            let mut year = 1;
            while let Some(date) = add_months(start, 12 * year).filter(|d| *d < end) {
                self.annual_reports.push(Milestone::due(Some(date)));
                year += 1;
            }
        }
        if self.grant.active() && self.grant.months == 0 {
            self.grant.months = typical_grant_months(Some(self.level));
        }
    }

    /// The earliest deadline not yet met.
    pub fn next_step(&self) -> Option<Step> {
        next_step(self.steps())
    }
}

// ---------------------------------------------------------------------------
// Students
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct Student {
    pub id: Uuid,
    /// Enrolment number (matrícula); some institutions use the CPF.
    pub enrollment: String,
    pub name: String,
    pub email: String,
    pub course: String,
    pub archived: bool,
    pub notes: String,
    pub tcc: Option<TccRecord>,
    pub ic: Option<IcRecord>,
    pub placement: Option<PlacementRecord>,
    pub advisee: Option<AdviseeRecord>,
    pub pos: Option<PosRecord>,
}

impl Default for Student {
    fn default() -> Self {
        Self {
            id: Uuid::new_v4(),
            enrollment: String::new(),
            name: String::new(),
            email: String::new(),
            course: String::new(),
            archived: false,
            notes: String::new(),
            tcc: None,
            ic: None,
            placement: None,
            advisee: None,
            pos: None,
        }
    }
}

impl Student {
    /// Every meeting across the TCC, IC, tutoria and postgraduate roles.
    pub fn meetings(&self) -> impl Iterator<Item = &Meeting> {
        let tcc = self.tcc.iter().flat_map(|f| &f.meetings);
        let ic = self.ic.iter().flat_map(|f| &f.meetings);
        let advisee = self
            .advisee
            .iter()
            .flat_map(|a| a.s1.meetings.iter().chain(&a.s2.meetings));
        let pos = self.pos.iter().flat_map(|p| &p.meetings);
        tcc.chain(ic).chain(advisee).chain(pos)
    }

    pub fn meetings_mut(&mut self) -> impl Iterator<Item = &mut Meeting> {
        let tcc = self.tcc.iter_mut().flat_map(|f| &mut f.meetings);
        let ic = self.ic.iter_mut().flat_map(|f| &mut f.meetings);
        let advisee = self
            .advisee
            .iter_mut()
            .flat_map(|a| a.s1.meetings.iter_mut().chain(&mut a.s2.meetings));
        let pos = self.pos.iter_mut().flat_map(|p| &mut p.meetings);
        tcc.chain(ic).chain(advisee).chain(pos)
    }

    /// The name, for titles, messages and calendar files. The enrolment
    /// number is left out, as it may be a CPF.
    pub fn display_name(&self) -> String {
        match self.name.trim() {
            "" => "(aluno sem nome)".into(),
            name => name.into(),
        }
    }

    pub fn matches(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        let q = query.to_lowercase();
        let digits: String = query.chars().filter(char::is_ascii_digit).collect();
        let company = self.placement.as_ref().map(|p| p.company.as_str());
        let pos = self.pos.as_ref().map(|p| p.title.as_str());
        let ic = self.ic.as_ref().map(|p| p.title.as_str());
        let tcc = self.tcc.as_ref().map(|p| p.title.as_str());
        // A CPF matches with or without its dots and dash.
        let enrollment_digits: String = self.enrollment.chars().filter(char::is_ascii_digit).collect();
        (digits.len() >= 3 && enrollment_digits.contains(&digits))
            || [
                Some(self.name.as_str()),
                Some(self.enrollment.as_str()),
                Some(self.email.as_str()),
                Some(self.course.as_str()),
                company,
                pos,
                ic,
                tcc,
            ]
            .into_iter()
            .flatten()
            .any(|field| field.to_lowercase().contains(&q))
    }
}

/// Whether `text` is a valid CPF (11 digits with both check digits right),
/// with or without dots and dash.
pub fn is_cpf(text: &str) -> bool {
    let digits: Vec<u32> = text.chars().filter_map(|c| c.to_digit(10)).collect();
    let only_digits_and_marks = text.trim().chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-');
    if digits.len() != 11 || !only_digits_and_marks || digits.iter().all(|d| *d == digits[0]) {
        return false;
    }
    let check = |n: usize| {
        let sum: u32 = digits[..n].iter().enumerate().map(|(i, d)| d * (n as u32 + 1 - i as u32)).sum();
        (sum * 10 % 11) % 10
    };
    check(9) == digits[9] && check(10) == digits[10]
}

/// An enrolment number hidden behind dots: shaped like a CPF when it is
/// one, otherwise one dot per character (at least 6, at most 12).
pub fn masked(enrollment: &str) -> String {
    let text = enrollment.trim();
    if text.is_empty() {
        String::new()
    } else if is_cpf(text) {
        "•••.•••.•••-••".into()
    } else {
        "•".repeat(text.chars().count().clamp(6, 12))
    }
}

// ---------------------------------------------------------------------------
// Database and derived views
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(default)]
pub struct Database {
    pub version: u32,
    pub settings: Settings,
    pub students: Vec<Student>,
    /// Imported calendar events that aren't linked to a student ("Outros").
    pub entries: Vec<CalEntry>,
    /// UIDs of imported calendar events the user chose not to import.
    pub ignored_imports: Vec<String>,
}

impl Default for Database {
    fn default() -> Self {
        Self {
            version: DB_VERSION,
            settings: Settings::default(),
            students: Vec::new(),
            entries: Vec::new(),
            ignored_imports: Vec::new(),
        }
    }
}

/// An event from an imported calendar, kept in the calendar without a
/// student. Its date, time, place and repetition work like a meeting's.
#[derive(Serialize, Deserialize, Clone, PartialEq, Debug, Default)]
#[serde(default)]
pub struct CalEntry {
    pub title: String,
    pub meeting: Meeting,
}

/// Which list of meetings an imported event joins.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MeetingRole {
    Tcc,
    Ic,
    Pos,
    /// The semester is the one the meeting falls in.
    Advisee,
}

impl MeetingRole {
    pub fn label(self) -> &'static str {
        match self {
            MeetingRole::Tcc => "Orientação de TCC",
            MeetingRole::Ic => "Orientação de IC",
            MeetingRole::Pos => "Orientação na pós",
            MeetingRole::Advisee => "Tutoria",
        }
    }

    /// The roles of `s` that have meetings.
    pub fn of(s: &Student) -> Vec<MeetingRole> {
        let mut out = Vec::new();
        if s.tcc.is_some() {
            out.push(MeetingRole::Tcc);
        }
        if s.ic.is_some() {
            out.push(MeetingRole::Ic);
        }
        if s.pos.is_some() {
            out.push(MeetingRole::Pos);
        }
        if s.advisee.is_some() {
            out.push(MeetingRole::Advisee);
        }
        out
    }
}

/// An event read from another calendar, ready to become a meeting.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedEvent {
    pub uid: String,
    pub title: String,
    /// Its date, time, place and repetition; `source` is the UID.
    pub meeting: Meeting,
    /// Email addresses of the guests, to suggest a student.
    pub guests: Vec<String>,
    /// It repeats in a way the app can't follow; only its first date is kept.
    pub repeat_dropped: bool,
}

/// Why an event of an imported calendar may not be wanted.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Known {
    /// Exported from the app, so it is here already.
    FromSupervisor,
    Imported,
    Skipped,
}

impl Known {
    pub fn label(self) -> &'static str {
        match self {
            Known::FromSupervisor => "do Supervisor",
            Known::Imported => "importado antes",
            Known::Skipped => "ignorado antes",
        }
    }

    /// Importing it again would show it twice.
    pub fn duplicate(self) -> bool {
        self != Known::Skipped
    }
}

/// What to do with one imported event.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ImportTarget {
    Ignore,
    /// Keep it in the calendar without a student.
    Calendar,
    Student(Uuid, MeetingRole),
}

/// What the calendar's edit dialog can change in an item.
#[derive(Clone, Debug, PartialEq)]
pub struct EventDetails {
    /// Only imported events have a title of their own.
    pub title: Option<String>,
    pub date: NaiveDate,
    pub time: Option<NaiveTime>,
    /// `None` for items without a place (visits).
    pub place: Option<Place>,
}

/// Where a meeting happens.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Place {
    /// `None` for a defence, which only has a location.
    pub online: Option<bool>,
    pub platform: Platform,
    pub link: String,
    pub location: String,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EventKind {
    /// TCC meetings and the TCC defence.
    TccMeeting,
    IcMeeting,
    Visit,
    AdviseeMeeting,
    /// Postgraduate meetings and the defence.
    PosMeeting,
    Deadline,
    /// Advance notice of a deadline.
    Reminder,
    /// An imported event that isn't linked to a student.
    Other,
}

/// A dated item derived from the records, used for the dashboard, the
/// calendar and .ics export. Repeating meetings give one event per week.
#[derive(Clone, Debug)]
pub struct Event {
    pub uid: String,
    /// Nil for imported events without a student.
    pub student_id: Uuid,
    pub date: NaiveDate,
    pub time: Option<NaiveTime>,
    pub kind: EventKind,
    pub title: String,
    pub done: bool,
    /// An online meeting or visit.
    pub online: bool,
    /// Service of an online item.
    pub platform: Platform,
    /// The meeting's own link, if it has one.
    pub link: String,
    /// Room or company for in-person items; empty otherwise.
    pub location: String,
    /// For a week of a repeating meeting: the series UID and its planned date.
    pub series: Option<(String, NaiveDate)>,
    pub cancelled: bool,
}

impl Event {
    /// The title without the student's name, for "Ocultar nomes". Events
    /// without a student (from an imported calendar) are just "Evento".
    pub fn anonymous_title(&self) -> &str {
        if self.student_id.is_nil() {
            return "Evento";
        }
        self.title.split_once(" – ").map_or(&self.title, |(title, _)| title)
    }

    /// A week of a repeating meeting that was moved to another day.
    pub fn moved(&self) -> bool {
        self.series
            .as_ref()
            .is_some_and(|(_, planned)| *planned != self.date)
    }

    /// The id of the meeting this is (a week of), if it is a meeting.
    pub fn meeting_id(&self) -> Option<Uuid> {
        let rest = self.uid.strip_prefix("meeting-")?;
        Uuid::parse_str(rest.get(..36)?).ok()
    }

    /// Can be moved, edited and deleted from the calendar: meetings, visits,
    /// defences and imported events, but not deadlines and their reminders,
    /// which come from the course's dates.
    pub fn editable(&self) -> bool {
        !matches!(self.kind, EventKind::Deadline | EventKind::Reminder)
    }

    /// Which placement visit (from 0) this is.
    fn visit_index(&self) -> Option<usize> {
        let rest = self.uid.strip_prefix("visit")?;
        let n: usize = rest.split('-').next()?.parse().ok()?;
        n.checked_sub(1)
    }

    fn is_tcc_defense(&self) -> bool {
        self.uid.starts_with("tcc-defense-")
    }

    fn is_pos_defense(&self) -> bool {
        self.uid.starts_with("pos-defense-")
    }

    /// A dated item that isn't a meeting (deadlines, defences, …).
    fn dated(uid: String, student_id: Uuid, date: NaiveDate, kind: EventKind, title: String, done: bool) -> Self {
        Self {
            uid,
            student_id,
            date,
            time: None,
            kind,
            title,
            done,
            online: false,
            platform: Platform::default(),
            link: String::new(),
            location: String::new(),
            series: None,
            cancelled: false,
        }
    }
}

/// A change the calendar can make to a meeting, or to one week of a
/// repeating meeting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MeetingChange {
    Held(bool),
    /// Move it to this day and time.
    Postpone(NaiveDate, Option<NaiveTime>),
    /// Cancel a week of a repeating meeting, or bring it back.
    Cancelled(bool),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Attendee {
    pub name: String,
    pub email: String,
}

/// How a repeating meeting recurs, for an .ics series.
#[derive(Clone, Debug)]
pub struct SeriesRule {
    pub every_weeks: u8,
    pub until: NaiveDate,
    /// Cancelled and rescheduled weeks.
    pub exceptions: Vec<Occurrence>,
}

/// One entry of an .ics file: a single event or a whole repeating series,
/// with the people to invite.
#[derive(Clone, Debug)]
pub struct CalItem {
    /// For a series, its first week with the usual time.
    pub event: Event,
    pub series: Option<SeriesRule>,
    pub attendees: Vec<Attendee>,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Severity {
    Overdue,
    Pending,
}

#[derive(Clone, Debug)]
pub struct Attention {
    pub student_id: Uuid,
    pub severity: Severity,
    pub message: String,
    /// The message without the student's name.
    pub detail: String,
}

/// One event per week of `m` (only the meeting itself if it doesn't repeat).
fn push_meeting(
    out: &mut Vec<Event>,
    settings: &Settings,
    student_id: Uuid,
    m: &Meeting,
    kind: EventKind,
    title: String,
) {
    for occ in m.occurrences(settings) {
        let (uid, series) = match m.repeat {
            Some(_) => (m.occurrence_uid(occ.planned), Some((m.uid(), occ.planned))),
            None => (m.uid(), None),
        };
        out.push(Event {
            uid,
            student_id,
            date: occ.date(),
            time: occ.time.or(m.time),
            kind,
            title: title.clone(),
            done: occ.held,
            online: m.online,
            platform: m.platform,
            link: m.link.trim().into(),
            location: m.place(),
            series,
            cancelled: occ.cancelled,
        });
    }
}

/// Move a meeting, or the week of a repeating one planned for `planned`,
/// to `date` at `time`. A week back on its planned day and usual time is
/// no longer marked as moved.
fn reschedule(m: &mut Meeting, planned: Option<NaiveDate>, date: NaiveDate, time: Option<NaiveTime>) {
    match planned {
        None => {
            m.date = Some(date);
            m.time = time;
        }
        Some(planned) => {
            let usual = m.time;
            let mut week = m.occurrence(planned);
            week.moved_to = (date != planned).then_some(date);
            week.time = time.filter(|t| Some(*t) != usual);
            m.set_occurrence(week);
        }
    }
}

/// A deadline and its reminder.
fn push_step(out: &mut Vec<Event>, s: &Student, prefix: &str, what: &str, step: &Step, name: &str) {
    let Some(due) = step.milestone.deadline else { return };
    let label = step.label.to_lowercase();
    out.push(Event::dated(
        format!("{prefix}-{}-{}", step.key, s.id),
        s.id,
        due,
        EventKind::Deadline,
        format!("{what}: {label} – {name}"),
        step.milestone.done,
    ));
    if let Some(remind) = step.milestone.reminder(step.lead) {
        out.push(Event::dated(
            format!("{prefix}-{}-reminder-{}", step.key, s.id),
            s.id,
            remind,
            EventKind::Reminder,
            format!("Lembrete: {what}: {label} em {} – {name}", pt::numeric(due)),
            step.milestone.done,
        ));
    }
}

/// Things to chase about the deadlines of a project.
fn step_attention(steps: Vec<Step>, today: NaiveDate, mut push: impl FnMut(Severity, String)) {
    for step in steps {
        let m = step.milestone;
        let Some(due) = m.deadline.filter(|_| !m.done) else {
            continue;
        };
        if due < today {
            push(Severity::Overdue, format!("{}: prazo vencido em {}", step.label, pt::numeric(due)));
        } else if m.reminder(step.lead).is_some_and(|r| r <= today) {
            push(Severity::Pending, format!("{}: prazo em {}", step.label, pt::numeric(due)));
        }
    }
}

/// A grant ending soon, or that ended in the last month. `end` is its
/// effective last day.
fn grant_attention(grant: &Grant, end: Option<NaiveDate>, today: NaiveDate, mut push: impl FnMut(Severity, String)) {
    let Some(end) = end else { return };
    let who = grant.agency_label();
    if end < today && today - end <= Duration::days(GRANT_ENDED_DAYS) {
        push(Severity::Pending, format!("bolsa {who} terminou em {}", pt::numeric(end)));
    } else if end >= today && end <= today + Duration::days(GRANT_WARNING_DAYS) {
        push(Severity::Pending, format!("bolsa {who} termina em {}", pt::numeric(end)));
    }
}

/// Items of `events` in the next `days` days that are still to happen.
pub fn upcoming(events: &[Event], today: NaiveDate, days: u32) -> Vec<&Event> {
    let end = today + Duration::days(i64::from(days));
    events
        .iter()
        .filter(|e| !e.done && !e.cancelled && e.date >= today && e.date <= end)
        .collect()
}

/// The words of `text`, in lower case, to look for hints in a title.
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

impl Database {
    pub fn student(&self, id: Uuid) -> Option<&Student> {
        self.students.iter().find(|s| s.id == id)
    }

    pub fn student_mut(&mut self, id: Uuid) -> Option<&mut Student> {
        self.students.iter_mut().find(|s| s.id == id)
    }

    /// Apply `change` to the meeting behind `event`, or to that week of a
    /// repeating meeting. Returns false if `event` isn't a meeting.
    pub fn change_meeting(&mut self, event: &Event, change: MeetingChange) -> bool {
        let Some(id) = event.meeting_id() else { return false };
        let Some(m) = self.meeting_mut(event.student_id, id) else {
            return false;
        };
        let planned = event.series.as_ref().map(|(_, planned)| *planned);
        match change {
            MeetingChange::Held(held) => match planned {
                None => m.held = held,
                Some(planned) => {
                    let mut week = m.occurrence(planned);
                    week.held = held;
                    m.set_occurrence(week);
                }
            },
            MeetingChange::Postpone(date, time) => reschedule(m, planned, date, time),
            MeetingChange::Cancelled(cancelled) => {
                let Some(planned) = planned else { return false };
                let mut week = m.occurrence(planned);
                week.cancelled = cancelled;
                m.set_occurrence(week);
            }
        }
        true
    }

    /// The meetings of a student or, for the nil id, of the imported events.
    fn meetings_of(&self, student_id: Uuid) -> Box<dyn Iterator<Item = &Meeting> + '_> {
        if student_id.is_nil() {
            Box::new(self.entries.iter().map(|e| &e.meeting))
        } else {
            match self.student(student_id) {
                Some(s) => Box::new(s.meetings()),
                None => Box::new(std::iter::empty()),
            }
        }
    }

    fn meeting_mut(&mut self, student_id: Uuid, id: Uuid) -> Option<&mut Meeting> {
        if student_id.is_nil() {
            return self.entries.iter_mut().map(|e| &mut e.meeting).find(|m| m.id == id);
        }
        self.student_mut(student_id)?.meetings_mut().find(|m| m.id == id)
    }

    /// What the calendar's edit dialog shows for `e`, if it can be edited.
    pub fn event_details(&self, e: &Event) -> Option<EventDetails> {
        if !e.editable() {
            return None;
        }
        if let Some(id) = e.meeting_id() {
            let m = self.meetings_of(e.student_id).find(|m| m.id == id)?;
            let title = self
                .entries
                .iter()
                .find(|x| e.student_id.is_nil() && x.meeting.id == id)
                .map(|x| x.title.clone());
            return Some(EventDetails {
                title,
                date: e.date,
                time: e.time,
                place: Some(Place {
                    online: Some(m.online),
                    platform: m.platform,
                    link: m.link.clone(),
                    location: m.location.clone(),
                }),
            });
        }
        let s = self.student(e.student_id)?;
        let defense = |date: Option<NaiveDate>, time, location: &str| {
            Some(EventDetails {
                title: None,
                date: date?,
                time,
                place: Some(Place {
                    online: None,
                    location: location.into(),
                    ..Default::default()
                }),
            })
        };
        if let Some(i) = e.visit_index() {
            let v = s.placement.as_ref()?.visits.get(i)?;
            return Some(EventDetails {
                title: None,
                date: v.date?,
                time: v.time,
                place: None,
            });
        }
        if e.is_tcc_defense() {
            let t = s.tcc.as_ref()?;
            return defense(t.defense, t.defense_time, &t.defense_location);
        }
        if e.is_pos_defense() {
            let p = s.pos.as_ref()?;
            return defense(p.defense, p.defense_time, &p.defense_location);
        }
        None
    }

    /// Store what was changed in the edit dialog. For a week of a repeating
    /// meeting the day and time change that week only, and the place every
    /// week. Returns false if `e` can't be edited.
    pub fn edit_event(&mut self, e: &Event, d: &EventDetails) -> bool {
        if !e.editable() {
            return false;
        }
        if let Some(id) = e.meeting_id() {
            if let Some(title) = &d.title
                && let Some(x) = self.entries.iter_mut().find(|x| e.student_id.is_nil() && x.meeting.id == id)
            {
                x.title = title.trim().into();
            }
            let Some(m) = self.meeting_mut(e.student_id, id) else {
                return false;
            };
            if let Some(place) = &d.place {
                m.online = place.online.unwrap_or(m.online);
                m.platform = place.platform;
                m.link = place.link.trim().into();
                m.location = place.location.trim().into();
            }
            let planned = e.series.as_ref().map(|(_, planned)| *planned);
            reschedule(m, planned, d.date, d.time);
            return true;
        }
        let location = d.place.as_ref().map(|p| p.location.trim().to_string());
        let Some(s) = self.student_mut(e.student_id) else {
            return false;
        };
        if let Some(i) = e.visit_index() {
            let Some(v) = s.placement.as_mut().and_then(|p| p.visits.get_mut(i)) else {
                return false;
            };
            v.date = Some(d.date);
            v.time = d.time;
            return true;
        }
        if e.is_tcc_defense()
            && let Some(t) = &mut s.tcc
        {
            t.defense = Some(d.date);
            t.defense_time = d.time;
            if let Some(location) = location {
                t.defense_location = location;
            }
            return true;
        }
        if e.is_pos_defense()
            && let Some(p) = &mut s.pos
        {
            p.defense = Some(d.date);
            p.defense_time = d.time;
            if let Some(location) = location {
                p.defense_location = location;
            }
            return true;
        }
        false
    }

    /// Move `e` to another day, keeping its time.
    pub fn move_event(&mut self, e: &Event, date: NaiveDate) -> bool {
        let Some(details) = self.event_details(e) else {
            return false;
        };
        self.edit_event(e, &EventDetails { date, ..details })
    }

    /// Delete `e`: a meeting is removed (a week of a repeating one is
    /// cancelled, unless `whole_series`), a visit or a defence loses its
    /// date. Returns false if `e` can't be deleted.
    pub fn delete_event(&mut self, e: &Event, whole_series: bool) -> bool {
        if !e.editable() {
            return false;
        }
        if let Some(id) = e.meeting_id() {
            if let (Some((_, planned)), false) = (&e.series, whole_series) {
                let Some(m) = self.meeting_mut(e.student_id, id) else {
                    return false;
                };
                let mut week = m.occurrence(*planned);
                week.cancelled = true;
                m.set_occurrence(week);
                return true;
            }
            return self.remove_meeting(e.student_id, id);
        }
        let Some(s) = self.student_mut(e.student_id) else {
            return false;
        };
        if let Some(i) = e.visit_index() {
            let Some(v) = s.placement.as_mut().and_then(|p| p.visits.get_mut(i)) else {
                return false;
            };
            v.date = None;
            v.time = None;
            return true;
        }
        if e.is_tcc_defense()
            && let Some(t) = &mut s.tcc
        {
            t.defense = None;
            t.defense_time = None;
            return true;
        }
        if e.is_pos_defense()
            && let Some(p) = &mut s.pos
        {
            p.defense = None;
            p.defense_time = None;
            return true;
        }
        false
    }

    fn remove_meeting(&mut self, student_id: Uuid, id: Uuid) -> bool {
        let keep = |m: &Meeting| m.id != id;
        if student_id.is_nil() {
            let before = self.entries.len();
            self.entries.retain(|e| keep(&e.meeting));
            return self.entries.len() != before;
        }
        let Some(s) = self.student_mut(student_id) else {
            return false;
        };
        let before = s.meetings().count();
        if let Some(t) = &mut s.tcc {
            t.meetings.retain(keep);
        }
        if let Some(ic) = &mut s.ic {
            ic.meetings.retain(keep);
        }
        if let Some(a) = &mut s.advisee {
            a.s1.meetings.retain(keep);
            a.s2.meetings.retain(keep);
        }
        if let Some(p) = &mut s.pos {
            p.meetings.retain(keep);
        }
        s.meetings().count() != before
    }

    /// Why an imported event may not be wanted: it came from this app (or
    /// the original Supervisor), or was imported or skipped before.
    pub fn known_import(&self, uid: &str) -> Option<Known> {
        if uid.ends_with("@supervisor-br.local") || uid.ends_with("@supervisor.local") {
            Some(Known::FromSupervisor)
        } else if self
            .students
            .iter()
            .flat_map(Student::meetings)
            .chain(self.entries.iter().map(|e| &e.meeting))
            .any(|m| m.source == uid)
        {
            Some(Known::Imported)
        } else if self.ignored_imports.iter().any(|u| u == uid) {
            Some(Known::Skipped)
        } else {
            None
        }
    }

    /// The student an imported event is probably with: one of the guests,
    /// or named in the title. The role follows words in the title.
    pub fn suggest_student(&self, ev: &ImportedEvent) -> Option<(Uuid, MeetingRole)> {
        let candidates = || {
            self.students
                .iter()
                .filter(|s| !s.archived && !MeetingRole::of(s).is_empty())
        };
        let title = ev.title.to_lowercase();
        let s = candidates()
            .find(|s| {
                let email = s.email.trim();
                !email.is_empty() && ev.guests.iter().any(|g| g.trim().eq_ignore_ascii_case(email))
            })
            .or_else(|| {
                candidates().find(|s| {
                    let name = s.name.trim().to_lowercase();
                    name.chars().count() >= 4 && title.contains(&name)
                })
            })?;
        let roles = MeetingRole::of(s);
        let title_words = words(&title);
        // Short hints ("ic", "pós") are whole words; longer ones may start a word.
        let hinted = |role, hints: &[&str]| {
            roles.contains(&role)
                && title_words
                    .iter()
                    .any(|w| hints.iter().any(|h| w == h || (h.chars().count() >= 5 && w.starts_with(h))))
        };
        let role = if hinted(MeetingRole::Advisee, &["tutoria", "tutor"]) {
            MeetingRole::Advisee
        } else if hinted(MeetingRole::Tcc, &["tcc"]) {
            MeetingRole::Tcc
        } else if hinted(MeetingRole::Ic, &["ic", "iniciação", "iniciacao", "pibic", "pibiti", "pivic"]) {
            MeetingRole::Ic
        } else if hinted(
            MeetingRole::Pos,
            &["mestrado", "doutorado", "dissertação", "dissertacao", "tese", "pós", "pos", "posdoc", "qualificação", "qualificacao"],
        ) {
            MeetingRole::Pos
        } else {
            roles[0]
        };
        Some((s.id, role))
    }

    /// Add the imported events as chosen; the skipped ones are remembered.
    /// Returns how many were added.
    pub fn import(&mut self, choices: Vec<(ImportedEvent, ImportTarget)>) -> usize {
        let mut added = 0;
        for (ev, target) in choices {
            let uid = ev.uid.clone();
            let mut meeting = ev.meeting;
            meeting.source = uid.clone();
            let semester = meeting.date.map(|d| self.settings.semester_of(d));
            let list = match target {
                ImportTarget::Ignore => {
                    if !self.ignored_imports.contains(&uid) {
                        self.ignored_imports.push(uid);
                    }
                    continue;
                }
                ImportTarget::Calendar => {
                    self.entries.push(CalEntry { title: ev.title, meeting });
                    added += 1;
                    self.ignored_imports.retain(|u| *u != uid);
                    continue;
                }
                ImportTarget::Student(id, role) => self.student_mut(id).and_then(|s| match role {
                    MeetingRole::Tcc => s.tcc.as_mut().map(|t| &mut t.meetings),
                    MeetingRole::Ic => s.ic.as_mut().map(|ic| &mut ic.meetings),
                    MeetingRole::Pos => s.pos.as_mut().map(|p| &mut p.meetings),
                    MeetingRole::Advisee => s.advisee.as_mut().map(|a| match semester {
                        Some(Semester::S2) => &mut a.s2.meetings,
                        _ => &mut a.s1.meetings,
                    }),
                }),
            };
            if let Some(list) = list {
                list.push(meeting);
                added += 1;
                self.ignored_imports.retain(|u| *u != uid);
            }
        }
        added
    }

    pub fn enrollment_taken(&self, enrollment: &str, except: Option<Uuid>) -> bool {
        let b = enrollment.trim();
        !b.is_empty()
            && self
                .students
                .iter()
                .any(|s| Some(s.id) != except && s.enrollment.trim().eq_ignore_ascii_case(b))
    }

    /// Every dated item across all active (non-archived) students and the
    /// imported events, including cancelled weeks of repeating meetings.
    pub fn events(&self) -> Vec<Event> {
        let mut out = Vec::new();
        for s in self.students.iter().filter(|s| !s.archived) {
            self.student_events(s, &mut out);
        }
        self.entry_events(&mut out);
        out.sort_by_key(|e| (e.date, e.time));
        out
    }

    fn entry_events(&self, out: &mut Vec<Event>) {
        for x in &self.entries {
            push_meeting(out, &self.settings, Uuid::nil(), &x.meeting, EventKind::Other, x.title.clone());
        }
    }

    fn student_events(&self, s: &Student, out: &mut Vec<Event>) {
        let settings = &self.settings;
        let name = s.display_name();
        if let Some(tcc) = &s.tcc {
            for m in &tcc.meetings {
                push_meeting(out, settings, s.id, m, EventKind::TccMeeting, format!("Orientação de TCC – {name}"));
            }
            if let Some(date) = tcc.defense {
                out.push(Event {
                    time: tcc.defense_time,
                    location: tcc.defense_location.trim().into(),
                    ..Event::dated(
                        format!("tcc-defense-{}", s.id),
                        s.id,
                        date,
                        EventKind::TccMeeting,
                        format!("Defesa de TCC – {name}"),
                        !tcc.grade.trim().is_empty(),
                    )
                });
            }
            for module in settings.modules_for(&tcc.programme) {
                for cw in &module.courseworks {
                    let status = tcc.coursework.get(&cw.id);
                    if let Some(date) = settings.effective_deadline(module, cw, status) {
                        out.push(Event::dated(
                            format!("cw-{}-{}", s.id, cw.id),
                            s.id,
                            date,
                            EventKind::Deadline,
                            format!("{}: {} – {name}", module.code, cw.title),
                            status.is_some_and(|st| st.submitted),
                        ));
                    }
                }
            }
        }
        if let Some(ic) = &s.ic {
            for m in &ic.meetings {
                push_meeting(out, settings, s.id, m, EventKind::IcMeeting, format!("Orientação de IC – {name}"));
            }
            for step in ic.steps() {
                push_step(out, s, "ic", "IC", &step, &name);
            }
            if let Some(end) = ic.grant_end() {
                out.push(Event::dated(
                    format!("ic-grant-end-{}", s.id),
                    s.id,
                    end,
                    EventKind::Deadline,
                    format!("Fim da bolsa {} (IC) – {name}", ic.grant.agency_label()),
                    false,
                ));
            }
        }
        if let Some(p) = &s.placement {
            for (i, v) in p.visits.iter().enumerate() {
                if let Some(date) = v.date {
                    let mut title = format!("Visita de estágio {} ({}) – {name}", i + 1, v.mode.label().to_lowercase());
                    if !p.company.trim().is_empty() {
                        title.push_str(&format!(" @ {}", p.company.trim()));
                    }
                    out.push(Event {
                        time: v.time,
                        online: v.mode == VisitMode::Online,
                        platform: Platform::GoogleMeet,
                        location: match v.mode {
                            VisitMode::Online => String::new(),
                            VisitMode::FaceToFace => p.company.trim().into(),
                        },
                        ..Event::dated(visit_uid(s.id, i), s.id, date, EventKind::Visit, title, v.completed)
                    });
                }
            }
            if let Some(due) = p.report.deadline {
                out.push(Event::dated(
                    format!("placement-report-{}", s.id),
                    s.id,
                    due,
                    EventKind::Deadline,
                    format!("Estágio: relatório – {name}"),
                    p.report.done,
                ));
            }
        }
        if let Some(p) = &s.pos {
            self.pos_events(s, p, &name, out);
        }
        if let Some(a) = &s.advisee {
            for (sem, contact) in [("1º sem.", &a.s1), ("2º sem.", &a.s2)] {
                for (i, m) in contact.meetings.iter().enumerate() {
                    let label = if i == 0 { "reunião" } else { "retorno" };
                    push_meeting(
                        out,
                        settings,
                        s.id,
                        m,
                        EventKind::AdviseeMeeting,
                        format!("Tutoria {sem} ({label}) – {name}"),
                    );
                }
            }
        }
    }

    fn pos_events(&self, s: &Student, p: &PosRecord, name: &str, out: &mut Vec<Event>) {
        let level = p.level.short();
        for m in &p.meetings {
            push_meeting(out, &self.settings, s.id, m, EventKind::PosMeeting, format!("Orientação ({level}) – {name}"));
        }
        if let Some(date) = p.defense {
            out.push(Event {
                time: p.defense_time,
                location: p.defense_location.trim().into(),
                ..Event::dated(
                    format!("pos-defense-{}", s.id),
                    s.id,
                    date,
                    EventKind::PosMeeting,
                    format!("Defesa ({level}) – {name}"),
                    p.outcome != DefenseOutcome::Pending,
                )
            });
        }
        if !p.status.active() {
            return;
        }
        for step in p.steps() {
            push_step(out, s, "pos", level, &step, name);
        }
        if let Some(end) = p.grant_end() {
            out.push(Event::dated(
                format!("pos-grant-end-{}", s.id),
                s.id,
                end,
                EventKind::Deadline,
                format!("Fim da bolsa {} ({level}) – {name}", p.grant.agency_label()),
                false,
            ));
        }
    }

    /// The people to invite to one of a student's events.
    fn attendees(s: &Student, event: &Event) -> Vec<Attendee> {
        let mut attendees: Vec<Attendee> = Vec::new();
        let mut add = |name: &str, email: &str| {
            let email = email.trim();
            if !email.is_empty()
                && !attendees
                    .iter()
                    .any(|a| a.email.eq_ignore_ascii_case(email))
            {
                attendees.push(Attendee {
                    name: name.trim().into(),
                    email: email.into(),
                });
            }
        };
        match (event.kind, &s.placement) {
            (EventKind::Deadline | EventKind::Reminder | EventKind::Other, _) => {}
            (EventKind::Visit, Some(p)) => {
                let email = if s.email.trim().is_empty() {
                    &p.professional_email
                } else {
                    &s.email
                };
                add(&s.name, email);
                let visit = event.visit_index().and_then(|i| p.visits.get(i));
                if let Some(v) = visit
                    && v.line_manager_attending != TriState::No
                {
                    add(&p.line_manager_name, &p.line_manager_email);
                }
            }
            _ => add(&s.name, &s.email),
        }
        attendees
    }

    /// Calendar entries for the events `pick` selects (cancelled weeks are
    /// skipped). With `as_series`, any week of a repeating meeting brings in
    /// the whole series, once.
    pub fn cal_items(&self, pick: impl Fn(&Event) -> bool, as_series: bool) -> Vec<CalItem> {
        let mut out = Vec::new();
        let mut series_done = HashSet::new();
        let students = self.students.iter().filter(|s| !s.archived).map(Some);
        for s in students.chain([None]) {
            let mut events = Vec::new();
            match s {
                Some(s) => self.student_events(s, &mut events),
                None => self.entry_events(&mut events),
            }
            for e in events.into_iter().filter(|e| !e.cancelled && pick(e)) {
                let attendees = s.map(|s| Self::attendees(s, &e)).unwrap_or_default();
                match &e.series {
                    Some((uid, _)) if as_series => {
                        if series_done.insert(uid.clone())
                            && let Some(item) = self.series_item(uid, e.clone(), attendees)
                        {
                            out.push(item);
                        }
                    }
                    _ => out.push(CalItem {
                        event: e,
                        series: None,
                        attendees,
                    }),
                }
            }
        }
        out.sort_by_key(|i| (i.event.date, i.event.time));
        out
    }

    /// The whole repeating meeting `uid`, from the event of one of its weeks.
    fn series_item(&self, uid: &str, week: Event, attendees: Vec<Attendee>) -> Option<CalItem> {
        let m = self.meetings_of(week.student_id).find(|m| m.uid() == uid)?;
        let repeat = m.repeat.as_ref()?;
        let occurrences = m.occurrences(&self.settings);
        let last = occurrences.last()?.planned;
        Some(CalItem {
            event: Event {
                uid: uid.into(),
                date: m.date?,
                time: m.time,
                done: false,
                series: None,
                cancelled: false,
                ..week
            },
            series: Some(SeriesRule {
                every_weeks: repeat.every_weeks.max(1),
                until: last,
                exceptions: occurrences
                    .into_iter()
                    .filter(|o| o.cancelled || o.rescheduled())
                    .collect(),
            }),
            attendees,
        })
    }

    /// The meeting (whole series if it repeats), visit or defence `uid` of
    /// one student (or, for the nil id, an imported event), ready for an
    /// invite file or Google Agenda.
    pub fn invite(&self, student_id: Uuid, uid: &str) -> Option<CalItem> {
        let s = if student_id.is_nil() { None } else { Some(self.student(student_id)?) };
        let mut events = Vec::new();
        match s {
            Some(s) => self.student_events(s, &mut events),
            None => self.entry_events(&mut events),
        }
        let e = events.into_iter().find(|e| {
            e.editable() && (e.uid == uid || e.series.as_ref().is_some_and(|(series, _)| series == uid))
        })?;
        let attendees = s.map(|s| Self::attendees(s, &e)).unwrap_or_default();
        match e.series.as_ref().map(|(series, _)| series.clone()) {
            Some(series) => self.series_item(&series, e, attendees),
            None => Some(CalItem {
                event: e,
                series: None,
                attendees,
            }),
        }
    }

    /// Things that need chasing, most urgent first.
    pub fn attention(&self, today: NaiveDate) -> Vec<Attention> {
        let mut out = Vec::new();
        for s in self.students.iter().filter(|s| !s.archived) {
            let mut push = |severity, message: String| {
                out.push(Attention {
                    student_id: s.id,
                    severity,
                    message: format!("{} – {message}", s.display_name()),
                    detail: message,
                })
            };
            if let Some(tcc) = &s.tcc {
                if !tcc.initial_contact {
                    push(Severity::Pending, "TCC: ainda sem contato inicial".into());
                }
                for module in self.settings.modules_for(&tcc.programme) {
                    for cw in &module.courseworks {
                        let status = tcc.coursework.get(&cw.id);
                        let submitted = status.is_some_and(|st| st.submitted);
                        if let Some(due) = self.settings.effective_deadline(module, cw, status)
                            && !submitted
                            && due < today
                        {
                            push(
                                Severity::Overdue,
                                format!("{}: {} não entregue (prazo {})", module.code, cw.title, pt::numeric(due)),
                            );
                        }
                    }
                }
            }
            if let Some(ic) = &s.ic {
                step_attention(ic.steps(), today, |sev, msg| push(sev, format!("IC: {msg}")));
                grant_attention(&ic.grant, ic.grant_end(), today, |sev, msg| push(sev, format!("IC: {msg}")));
            }
            if let Some(p) = &s.placement {
                for (i, v) in p.visits.iter().enumerate() {
                    if v.completed {
                        continue;
                    }
                    if v.request_sent.is_some() && v.response != ReplyStatus::Replied {
                        let severity = if v.response == ReplyStatus::NoReply {
                            Severity::Overdue
                        } else {
                            Severity::Pending
                        };
                        push(severity, format!("visita {} de estágio: {}", i + 1, v.response.label().to_lowercase()));
                    }
                    if v.date.is_some() && v.line_manager_attending == TriState::Unknown {
                        push(Severity::Pending, format!("visita {}: presença do supervisor da empresa não confirmada", i + 1));
                    }
                }
                if let Some(due) = p.report.deadline.filter(|_| !p.report.done)
                    && due < today
                {
                    push(Severity::Overdue, format!("relatório de estágio não entregue (prazo {})", pt::numeric(due)));
                }
                if let Some(end) = p.tce_end
                    && end >= today
                    && end <= today + Duration::days(TCE_WARNING_DAYS)
                {
                    push(Severity::Pending, format!("termo de compromisso de estágio termina em {}", pt::numeric(end)));
                }
            }
            if let Some(p) = s.pos.as_ref().filter(|p| p.status.active()) {
                step_attention(p.steps(), today, &mut push);
                grant_attention(&p.grant, p.grant_end(), today, &mut push);
            }
            if let Some(a) = &s.advisee {
                for (sem, c) in [("1º sem.", &a.s1), ("2º sem.", &a.s2)] {
                    if c.contacted_on.is_some() && c.reply != ReplyStatus::Replied {
                        let severity = if c.reply == ReplyStatus::NoReply {
                            Severity::Overdue
                        } else {
                            Severity::Pending
                        };
                        push(severity, format!("tutoria {sem}: {}", c.reply.label().to_lowercase()));
                    }
                }
            }
        }
        out.sort_by_key(|a| a.severity);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    #[test]
    fn deadline_from_week_and_override() {
        let mut settings = Settings {
            sem1_start: Some(d(2026, 3, 2)),
            ..Default::default()
        };
        let mut module = settings.modules[0].clone();
        module.courseworks[0].week = Some(8);
        let cw = &module.courseworks[0];
        assert_eq!(settings.template_deadline(&module, cw), Some(d(2026, 4, 20)));

        let status = CwStatus {
            deadline_override: Some(d(2026, 4, 27)),
            ..Default::default()
        };
        assert_eq!(settings.effective_deadline(&module, cw, Some(&status)), Some(d(2026, 4, 27)));

        settings.sem1_start = None;
        assert_eq!(settings.template_deadline(&module, cw), None);
    }

    #[test]
    fn default_course_and_disciplines_are_generic() {
        let s = Settings::default();
        let codes = |p| s.modules_for(p).map(|m| m.code.clone()).collect::<Vec<_>>();
        assert_eq!(codes("graduacao"), ["TCC I", "TCC II"]);
        assert_eq!(s.programme_lists(), ["TCC"]);
        assert!(s.modules.iter().flat_map(|m| &m.courseworks).all(|c| c.week.is_none() && c.deadline.is_none()));
        assert!(s.hide_enrollment && !s.reveal_needs_password);
    }

    #[test]
    fn teaching_calendar() {
        let mut s = Settings {
            sem1_start: Some(d(2026, 3, 2)),
            sem2_start: Some(d(2026, 8, 3)),
            ..Default::default()
        };
        // Friday of week 18 unless set.
        assert_eq!(s.teaching_end(Semester::S1), Some(d(2026, 7, 3)));
        assert_eq!(s.teaching_week(d(2026, 3, 2)), Some((Semester::S1, 1)));
        assert_eq!(s.teaching_week(d(2026, 4, 6)), Some((Semester::S1, 6)));
        assert_eq!(s.teaching_week(d(2026, 7, 6)), None);
        s.sem1_teaching_end = Some(d(2026, 7, 10));
        assert_eq!(s.teaching_week(d(2026, 7, 6)), Some((Semester::S1, 19)));
        assert_eq!(s.semester_of(d(2026, 8, 10)), Semester::S2);
        assert_eq!(s.teaching_end(Semester::S2), Some(d(2026, 12, 4)));
        // Week 1 starting mid-week still ends on the Friday of week 18.
        assert_eq!(Settings::default_teaching_end(d(2026, 3, 4)), d(2026, 7, 3));
    }

    #[test]
    fn calendar_marks_meetings_held_and_postpones_them() {
        let one_off = Meeting {
            date: Some(d(2026, 10, 5)),
            time: NaiveTime::from_hms_opt(14, 0, 0),
            ..Default::default()
        };
        let weekly = Meeting {
            date: Some(d(2026, 10, 6)),
            repeat: Some(Repeat {
                until: Some(d(2026, 11, 3)),
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut db = Database::default();
        db.students.push(Student {
            advisee: Some(AdviseeRecord {
                s1: SemesterContact {
                    meetings: vec![one_off.clone(), weekly.clone()],
                    ..Default::default()
                },
                ..Default::default()
            }),
            pos: Some(PosRecord {
                defense: Some(d(2026, 10, 7)),
                ..Default::default()
            }),
            ..Default::default()
        });
        let find = |db: &Database, uid: &str| db.events().into_iter().find(|e| e.uid == uid).unwrap();

        let e = find(&db, &one_off.uid());
        assert_eq!(e.meeting_id(), Some(one_off.id));
        assert!(db.change_meeting(&e, MeetingChange::Held(true)));
        assert!(find(&db, &one_off.uid()).done);
        let later = NaiveTime::from_hms_opt(15, 30, 0);
        assert!(db.change_meeting(&e, MeetingChange::Postpone(d(2026, 10, 9), later)));
        let moved = find(&db, &one_off.uid());
        assert_eq!((moved.date, moved.time), (d(2026, 10, 9), later));

        // One week of a repeating meeting: only that week changes.
        let week = find(&db, &weekly.occurrence_uid(d(2026, 10, 13)));
        assert!(db.change_meeting(&week, MeetingChange::Held(true)));
        assert!(db.change_meeting(&week, MeetingChange::Postpone(d(2026, 10, 15), None)));
        let week = find(&db, &weekly.occurrence_uid(d(2026, 10, 13)));
        assert!(week.done && week.moved());
        assert_eq!(week.date, d(2026, 10, 15));
        assert!(!find(&db, &weekly.occurrence_uid(d(2026, 10, 20))).done);
        assert!(db.change_meeting(&week, MeetingChange::Held(false)));
        assert!(!find(&db, &weekly.occurrence_uid(d(2026, 10, 13))).done);

        // Not meetings.
        let defense = find(&db, &format!("pos-defense-{}", db.students[0].id));
        assert_eq!(defense.meeting_id(), None);
        assert!(!db.change_meeting(&defense, MeetingChange::Held(true)));
    }

    #[test]
    fn repeating_meeting_weeks() {
        let settings = Settings {
            sem2_start: Some(d(2026, 8, 3)),
            sem2_teaching_end: Some(d(2026, 12, 11)),
            ..Default::default()
        };
        let mut m = Meeting {
            date: Some(d(2026, 11, 2)),
            time: NaiveTime::from_hms_opt(14, 0, 0),
            repeat: Some(Repeat::default()),
            ..Default::default()
        };
        // Weekly from 2 Nov up to the end of teaching (Fri 11 Dec): 6 Mondays.
        let weeks = m.occurrences(&settings);
        assert_eq!(weeks.len(), 6);
        assert_eq!(weeks.last().unwrap().planned, d(2026, 12, 7));

        m.repeat.as_mut().unwrap().every_weeks = 2;
        assert_eq!(m.occurrences(&settings).len(), 3);
        m.repeat.as_mut().unwrap().every_weeks = 1;

        m.set_occurrence(Occurrence {
            planned: d(2026, 11, 9),
            cancelled: true,
            ..Default::default()
        });
        m.set_occurrence(Occurrence {
            planned: d(2026, 11, 16),
            moved_to: Some(d(2026, 11, 17)),
            ..Default::default()
        });
        m.set_occurrence(Occurrence {
            planned: d(2026, 11, 23),
            held: true,
            ..Default::default()
        });
        assert_eq!(m.repeat.as_ref().unwrap().exceptions.len(), 3);
        // Back to plain: forgotten.
        m.set_occurrence(Occurrence {
            planned: d(2026, 11, 23),
            ..Default::default()
        });
        assert_eq!(m.repeat.as_ref().unwrap().exceptions.len(), 2);

        let mut db = Database {
            settings,
            ..Default::default()
        };
        let student = Student {
            name: "Aluna Exemplo".into(),
            email: "aluna@exemplo.edu.br".into(),
            tcc: Some(TccRecord {
                meetings: vec![m.clone()],
                ..Default::default()
            }),
            ..Default::default()
        };
        let id = student.id;
        db.students.push(student);

        let events: Vec<_> = db
            .events()
            .into_iter()
            .filter(|e| e.kind == EventKind::TccMeeting)
            .collect();
        assert_eq!(events.len(), 6);
        assert_eq!(events.iter().filter(|e| e.cancelled).count(), 1);
        assert!(events.iter().any(|e| e.moved() && e.date == d(2026, 11, 17)));
        let all = db.events();
        let meetings = upcoming(&all, d(2026, 11, 1), 30)
            .iter()
            .filter(|e| e.kind == EventKind::TccMeeting)
            .count();
        assert_eq!(meetings, 4, "cancelled week not upcoming");

        // Invite for the meeting: the whole series with its exceptions.
        let item = db.invite(id, &m.uid()).unwrap();
        let rule = item.series.unwrap();
        assert_eq!(item.event.uid, m.uid());
        assert_eq!(item.event.date, d(2026, 11, 2));
        assert_eq!(rule.until, d(2026, 12, 7));
        assert_eq!(rule.exceptions.len(), 2);

        // Picking two weeks: one series as a series, two events otherwise.
        let picked = [m.occurrence_uid(d(2026, 11, 2)), m.occurrence_uid(d(2026, 11, 30))];
        assert_eq!(db.cal_items(|e| picked.contains(&e.uid), true).len(), 1);
        let singles = db.cal_items(|e| picked.contains(&e.uid), false);
        assert_eq!(singles.len(), 2);
        assert!(singles.iter().all(|i| i.series.is_none()));
    }

    #[test]
    fn serde_round_trip() {
        let mut db = Database::default();
        let mut s = Student {
            name: "Aluna Exemplo".into(),
            tcc: Some(TccRecord::default()),
            ic: Some(IcRecord::new()),
            placement: Some(PlacementRecord::default()),
            advisee: Some(AdviseeRecord::default()),
            pos: Some(PosRecord::new(Level::DoutoradoAcademico)),
            ..Default::default()
        };
        s.tcc.as_mut().unwrap().meetings.push(Meeting {
            date: Some(d(2026, 10, 1)),
            ..Default::default()
        });
        db.students.push(s);
        let json = serde_json::to_string(&db).unwrap();
        let back: Database = serde_json::from_str(&json).unwrap();
        assert_eq!(db, back);
        // Fields added later default when missing.
        let p: PosRecord = serde_json::from_str("{}").unwrap();
        assert_eq!(p, PosRecord::default());
    }

    #[test]
    fn invite_attendees() {
        let mut db = Database::default();
        let meeting = Meeting {
            date: Some(d(2026, 10, 30)),
            online: true,
            location: "Sala 201".into(),
            ..Default::default()
        };
        let mut placement = PlacementRecord {
            company: "Empresa Exemplo Ltda.".into(),
            line_manager_name: "Supervisora Exemplo".into(),
            line_manager_email: "supervisora@empresa.com.br".into(),
            ..Default::default()
        };
        placement.visits[1].date = Some(d(2026, 11, 5));
        placement.visits[1].line_manager_attending = TriState::Yes;
        let student = Student {
            name: "Aluna Exemplo".into(),
            email: "aluna@exemplo.edu.br".into(),
            tcc: Some(TccRecord {
                meetings: vec![meeting.clone()],
                ..Default::default()
            }),
            placement: Some(placement),
            ..Default::default()
        };
        let id = student.id;
        db.students.push(student);

        let inv = db.invite(id, &meeting.uid()).unwrap();
        assert!(inv.event.online);
        assert_eq!(inv.event.location, "", "online meetings ignore the room");
        assert_eq!(inv.attendees.len(), 1);
        assert_eq!(inv.attendees[0].email, "aluna@exemplo.edu.br");

        // Face-to-face visit: company supervisor invited, company as the location.
        let inv = db.invite(id, &visit_uid(id, 1)).unwrap();
        assert!(!inv.event.online);
        assert_eq!(inv.event.location, "Empresa Exemplo Ltda.");
        assert_eq!(inv.attendees.len(), 2);
        db.students[0].placement.as_mut().unwrap().visits[1].line_manager_attending = TriState::No;
        assert_eq!(db.invite(id, &visit_uid(id, 1)).unwrap().attendees.len(), 1);

        assert!(db.invite(id, "meeting-unknown").is_none());

        db.students[0].tcc.as_mut().unwrap().meetings[0].online = false;
        assert_eq!(db.invite(id, &meeting.uid()).unwrap().event.location, "Sala 201");
    }

    #[test]
    fn attention_flags_no_reply_and_missing_contact() {
        let mut db = Database::default();
        db.students.push(Student {
            name: "Aluno Exemplo".into(),
            tcc: Some(TccRecord::default()),
            advisee: Some(AdviseeRecord {
                s1: SemesterContact {
                    contacted_on: Some(d(2026, 9, 25)),
                    reply: ReplyStatus::NoReply,
                    ..Default::default()
                },
                ..Default::default()
            }),
            ..Default::default()
        });
        let items = db.attention(d(2026, 10, 21));
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].severity, Severity::Overdue);
        assert_eq!(items[0].message, "Aluno Exemplo – tutoria 1º sem.: sem resposta");
    }

    /// A doctorate started on 1 Mar 2026, with the default rules.
    fn example_doctorate() -> PosRecord {
        let mut p = PosRecord {
            start: Some(d(2026, 3, 1)),
            grant: Grant {
                agency: "CAPES".into(),
                modality: "Demanda Social".into(),
                ..Default::default()
            },
            ..PosRecord::new(Level::DoutoradoAcademico)
        };
        p.suggest(&LevelRule::default_for(Level::DoutoradoAcademico));
        p
    }

    #[test]
    fn suggested_deadlines_follow_the_rules_of_the_level() {
        let p = example_doctorate();
        let deadlines: Vec<(String, Option<NaiveDate>)> =
            p.steps().iter().map(|s| (s.label.clone(), s.milestone.deadline)).collect();
        assert_eq!(
            deadlines,
            [
                ("Integralização dos créditos", Some(d(2028, 3, 1))),
                ("Proficiência em língua estrangeira", Some(d(2028, 3, 1))),
                ("Exame de qualificação", Some(d(2028, 3, 1))),
                ("Relatório anual 2027", Some(d(2027, 3, 1))),
                ("Relatório anual 2028", Some(d(2028, 3, 1))),
                ("Relatório anual 2029", Some(d(2029, 3, 1))),
                ("Envio da tese à banca", Some(d(2030, 2, 1))),
                ("Prazo para a defesa da tese", Some(d(2030, 3, 1))),
                ("Entrega da versão final", None),
            ]
            .map(|(label, date)| (label.to_string(), date))
        );
        // CAPES: 48 months for a doctorate, ending the day before.
        assert_eq!(p.grant.months, 48);
        assert_eq!(p.grant_end(), Some(d(2030, 2, 28)));
        assert_eq!(p.grant.start, None, "the grant starts at enrolment");
        assert_eq!(p.year_of_study(d(2026, 2, 28)), None);
        assert_eq!(p.year_of_study(d(2027, 3, 1)), Some(2));

        // A parental-leave extension adds to the grant and to the course.
        let mut q = p.clone();
        q.grant.extension_months = 6;
        q.extension_months = 6;
        assert_eq!(q.grant_end(), Some(d(2030, 8, 31)));
        assert_eq!(q.rule_deadline(&LevelRule::default_for(q.level)), Some(d(2030, 9, 1)));

        // Dates already set are kept.
        let mut r = p.clone();
        r.qualification.deadline = Some(d(2027, 12, 15));
        r.suggest(&LevelRule::default_for(r.level));
        assert_eq!(r.qualification.deadline, Some(d(2027, 12, 15)));
    }

    #[test]
    fn lato_sensu_and_postdocs_have_fewer_steps() {
        let keys = |p: &PosRecord| p.steps().into_iter().map(|s| s.key).collect::<Vec<_>>();
        assert_eq!(keys(&PosRecord::new(Level::Especializacao)), ["credits", "submission", "final", "final-version"]);
        assert_eq!(keys(&PosRecord::new(Level::PosDoutorado)), ["final", "final-version"]);
        assert_eq!(PosRecord::new(Level::MestradoProfissional).group(), PosGroup::MestradoProfissional);
        assert_eq!(PosRecord::new(Level::DoutoradoProfissional).group(), PosGroup::Doutorado);
    }

    #[test]
    fn milestones_are_editable_by_key() {
        let mut p = example_doctorate();
        for step in p.steps() {
            assert!(p.milestone_mut(&step.key).is_some(), "{}", step.key);
        }
        p.milestone_mut("annual1").unwrap().done = true;
        assert!(p.annual_reports[0].done);
        assert!(p.milestone_mut("annual9").is_none());
        assert!(p.milestone_mut("annual0").is_none());
        let mut ic = IcRecord::new();
        for step in ic.steps() {
            assert!(ic.milestone_mut(&step.key).is_some(), "{}", step.key);
        }
    }

    #[test]
    fn deadlines_reminders_and_attention() {
        let p = example_doctorate();
        let db = Database {
            students: vec![Student {
                name: "Doutoranda Exemplo".into(),
                pos: Some(p),
                ..Default::default()
            }],
            ..Default::default()
        };
        let events = db.events();
        let reminders: Vec<&Event> = events.iter().filter(|e| e.kind == EventKind::Reminder).collect();
        assert_eq!(reminders[0].date, d(2027, 2, 1));
        assert_eq!(
            reminders[0].title,
            "Lembrete: Doutorado: relatório anual 2027 em 01/03/2027 – Doutoranda Exemplo"
        );
        assert!(events.iter().any(|e| e.title == "Fim da bolsa CAPES (Doutorado) – Doutoranda Exemplo"));

        // Reminder date passed: pending.
        let items = db.attention(d(2027, 2, 10));
        let messages: Vec<&str> = items.iter().map(|a| a.message.as_str()).collect();
        assert_eq!(messages, ["Doutoranda Exemplo – Relatório anual 2027: prazo em 01/03/2027"]);
        // Deadline passed: overdue, listed first.
        let items = db.attention(d(2027, 3, 2));
        assert_eq!(items[0].severity, Severity::Overdue);
        assert!(items[0].message.contains("prazo vencido em 01/03/2027"));
        // Grant ending within 90 days.
        let items = db.attention(d(2030, 1, 15));
        assert!(items.iter().any(|a| a.message.ends_with("bolsa CAPES termina em 28/02/2030")));

        // Students who have finished raise nothing.
        let mut db = db;
        db.students[0].pos.as_mut().unwrap().status = PosStatus::Graduated;
        assert!(db.attention(d(2027, 3, 4)).is_empty());
        assert!(db.events().iter().all(|e| e.kind != EventKind::Deadline));
    }

    #[test]
    fn ic_reports_follow_the_grant() {
        let mut ic = IcRecord::new();
        ic.grant.start = Some(d(2026, 9, 1));
        ic.suggest();
        assert_eq!(ic.partial_report.deadline, Some(d(2027, 3, 1)));
        assert_eq!(ic.final_report.deadline, Some(d(2027, 8, 31)));
        assert_eq!(ic.grant_end(), Some(d(2027, 8, 31)));
        assert_eq!(ic.next_step().map(|s| s.label), Some("Relatório parcial".into()));

        let db = Database {
            students: vec![Student {
                name: "Bolsista Exemplo".into(),
                ic: Some(ic),
                ..Default::default()
            }],
            ..Default::default()
        };
        let items = db.attention(d(2027, 2, 10));
        assert_eq!(items[0].message, "Bolsista Exemplo – IC: Relatório parcial: prazo em 01/03/2027");
    }

    #[test]
    fn external_students_can_leave_out_the_standard_steps() {
        let mut p = PosRecord {
            external: true,
            skip_standard: true,
            ..PosRecord::new(Level::MestradoAcademico)
        };
        let keys = |p: &PosRecord| p.steps().into_iter().map(|s| s.key).collect::<Vec<_>>();
        assert_eq!(keys(&p), ["final-version"]);

        let exam = CustomStep {
            title: "Seminário de acompanhamento".into(),
            milestone: Milestone::due(Some(d(2027, 4, 30))),
            remind: Lead { amount: 10, unit: LeadUnit::Days },
            ..Default::default()
        };
        let key = format!("custom-{}", exam.id);
        p.custom_steps.push(exam);
        let mut db = Database {
            students: vec![Student {
                name: "Mestranda Exemplo".into(),
                pos: Some(p),
                ..Default::default()
            }],
            ..Default::default()
        };
        let events = db.events();
        let titles: Vec<_> = events.iter().map(|e| (e.date, e.title.as_str())).collect();
        assert_eq!(
            titles,
            [
                (d(2027, 4, 20), "Lembrete: Mestrado: seminário de acompanhamento em 30/04/2027 – Mestranda Exemplo"),
                (d(2027, 4, 30), "Mestrado: seminário de acompanhamento – Mestranda Exemplo"),
            ]
        );
        assert!(db.attention(d(2027, 4, 19)).is_empty());
        assert_eq!(db.attention(d(2027, 4, 20)).len(), 1);

        db.students[0].pos.as_mut().unwrap().milestone_mut(&key).unwrap().done = true;
        assert!(db.attention(d(2027, 5, 1)).is_empty());

        assert_eq!(Lead { amount: 0, unit: LeadUnit::Weeks }.before(d(2027, 4, 30)), None);
        assert_eq!(Lead::months(1).before(d(2027, 3, 31)), Some(d(2027, 2, 28)));
    }

    #[test]
    fn cpf_is_recognised_and_masked() {
        // Valid check digits (a made-up number, not a real person's).
        assert!(is_cpf("529.982.247-25"));
        assert!(is_cpf("52998224725"));
        assert!(!is_cpf("529.982.247-24"));
        assert!(!is_cpf("111.111.111-11"));
        assert!(!is_cpf("2026001234"));
        assert_eq!(masked("529.982.247-25"), "•••.•••.•••-••");
        assert_eq!(masked("2026"), "••••••");
        assert_eq!(masked(""), "");

        // Searching by CPF works with or without punctuation; names and
        // calendar titles never include the number.
        let s = Student {
            name: "Aluna Exemplo".into(),
            enrollment: "529.982.247-25".into(),
            ..Default::default()
        };
        assert!(s.matches("52998224725"));
        assert!(s.matches("982.247"));
        assert_eq!(s.display_name(), "Aluna Exemplo");
        assert_eq!(Student::default().display_name(), "(aluno sem nome)");
    }

    #[test]
    fn agencies_include_federal_and_all_state_foundations() {
        let states: HashSet<&str> = AGENCIES.iter().filter(|a| a.kind == AgencyKind::State).map(|a| a.uf).collect();
        assert_eq!(states.len(), 27, "one FAP per state and the DF");
        for id in ["CNPq", "CAPES", "FAPESP", "FACEPE", "FAPEMIG", "FAPERJ"] {
            assert!(agency_by_id(id).is_some(), "{id}");
        }
        let ids: HashSet<&str> = AGENCIES.iter().map(|a| a.id).collect();
        assert_eq!(ids.len(), AGENCIES.len(), "ids are unique");
    }

    #[test]
    fn placement_report_and_agreement() {
        let mut p = PlacementRecord {
            tce_end: Some(d(2026, 11, 10)),
            report: Milestone::due(Some(d(2026, 10, 20))),
            ..Default::default()
        };
        p.visits[0].request_sent = Some(d(2026, 10, 1));
        let db = Database {
            students: vec![Student {
                name: "Estagiário Exemplo".into(),
                placement: Some(p),
                ..Default::default()
            }],
            ..Default::default()
        };
        let messages: Vec<String> = db.attention(d(2026, 10, 25)).into_iter().map(|a| a.message).collect();
        assert_eq!(
            messages,
            [
                "Estagiário Exemplo – relatório de estágio não entregue (prazo 20/10/2026)",
                "Estagiário Exemplo – visita 1 de estágio: aguardando resposta",
                "Estagiário Exemplo – termo de compromisso de estágio termina em 10/11/2026",
            ]
        );
    }

    #[test]
    fn calendar_edits_moves_and_deletes() {
        let at = |h| NaiveTime::from_hms_opt(h, 0, 0);
        let one_off = Meeting {
            date: Some(d(2026, 10, 5)),
            time: at(14),
            location: "Sala 201".into(),
            ..Default::default()
        };
        let weekly = Meeting {
            date: Some(d(2026, 10, 6)),
            time: at(10),
            repeat: Some(Repeat {
                until: Some(d(2026, 11, 3)),
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut placement = PlacementRecord::default();
        placement.add_visit();
        placement.visits[2].date = Some(d(2026, 11, 2));
        placement.visits[2].time = at(11);
        let mut db = Database::default();
        db.students.push(Student {
            tcc: Some(TccRecord {
                meetings: vec![one_off.clone(), weekly.clone()],
                defense: Some(d(2026, 12, 1)),
                ..Default::default()
            }),
            placement: Some(placement),
            pos: Some(PosRecord {
                defense: Some(d(2026, 10, 7)),
                ..Default::default()
            }),
            ..Default::default()
        });
        let id = db.students[0].id;
        let find = |db: &Database, uid: &str| db.events().into_iter().find(|e| e.uid == uid);

        // Edit a one-off meeting: day, time and place.
        let e = find(&db, &one_off.uid()).unwrap();
        let mut details = db.event_details(&e).unwrap();
        assert_eq!(details.title, None);
        assert_eq!(details.place.as_ref().unwrap().location, "Sala 201");
        details.date = d(2026, 10, 8);
        details.time = at(9);
        details.place = Some(Place {
            online: Some(true),
            platform: Platform::GoogleMeet,
            link: " meet.google.com/abc-defg-hij ".into(),
            location: "Sala 201".into(),
        });
        assert!(db.edit_event(&e, &details));
        let e = find(&db, &one_off.uid()).unwrap();
        assert_eq!((e.date, e.time, e.online, e.platform), (d(2026, 10, 8), at(9), true, Platform::GoogleMeet));
        assert_eq!(e.link, "meet.google.com/abc-defg-hij");

        // Drag a week of a repeating meeting: only that week moves, keeping its time.
        let week = find(&db, &weekly.occurrence_uid(d(2026, 10, 13))).unwrap();
        assert!(db.move_event(&week, d(2026, 10, 14)));
        let week = find(&db, &weekly.occurrence_uid(d(2026, 10, 13))).unwrap();
        assert!(week.moved());
        assert_eq!((week.date, week.time), (d(2026, 10, 14), at(10)));
        // Back on its day it's no longer a change.
        assert!(db.move_event(&week, d(2026, 10, 13)));
        let m = &db.students[0].tcc.as_ref().unwrap().meetings[1];
        assert!(m.repeat.as_ref().unwrap().exceptions.is_empty());

        // Delete one week (cancelled, then restored), then the whole series.
        let week = find(&db, &weekly.occurrence_uid(d(2026, 10, 20))).unwrap();
        assert!(db.delete_event(&week, false));
        assert!(find(&db, &weekly.occurrence_uid(d(2026, 10, 20))).unwrap().cancelled);
        assert!(db.change_meeting(&week, MeetingChange::Cancelled(false)));
        assert!(!find(&db, &weekly.occurrence_uid(d(2026, 10, 20))).unwrap().cancelled);
        assert!(db.delete_event(&week, true));
        assert!(find(&db, &weekly.occurrence_uid(d(2026, 10, 27))).is_none());
        assert_eq!(db.students[0].meetings().count(), 1);

        // The third visit and the defences move and lose their date;
        // deadlines can't be touched.
        let visit = find(&db, &visit_uid(id, 2)).unwrap();
        assert!(db.move_event(&visit, d(2026, 11, 4)));
        assert_eq!(find(&db, &visit_uid(id, 2)).unwrap().time, at(11));
        let defense = find(&db, &format!("pos-defense-{id}")).unwrap();
        let mut details = db.event_details(&defense).unwrap();
        details.place.as_mut().unwrap().location = "Auditório 5".into();
        assert!(db.edit_event(&defense, &details));
        assert_eq!(db.students[0].pos.as_ref().unwrap().defense_location, "Auditório 5");
        let tcc_defense = find(&db, &format!("tcc-defense-{id}")).unwrap();
        assert!(db.move_event(&tcc_defense, d(2026, 12, 3)));
        assert_eq!(db.students[0].tcc.as_ref().unwrap().defense, Some(d(2026, 12, 3)));
        assert!(db.delete_event(&visit, false) && db.delete_event(&defense, false));
        assert!(find(&db, &visit_uid(id, 2)).is_none() && find(&db, &format!("pos-defense-{id}")).is_none());
        assert_eq!(db.students[0].placement.as_ref().unwrap().visits.len(), 3, "the visit itself stays");
        let deadline = Event {
            uid: "cw-x".into(),
            kind: EventKind::Deadline,
            ..defense
        };
        assert!(!deadline.editable() && db.event_details(&deadline).is_none());
        assert!(!db.delete_event(&deadline, true));
    }

    #[test]
    fn importing_events() {
        let mut db = Database::default();
        db.settings.sem2_start = Some(d(2027, 8, 2));
        db.students.push(Student {
            name: "Ana Souza".into(),
            email: "ana.souza@exemplo.edu.br".into(),
            pos: Some(PosRecord::default()),
            advisee: Some(AdviseeRecord::default()),
            ..Default::default()
        });
        db.students.push(Student {
            name: "Bruno Lima".into(),
            tcc: Some(TccRecord::default()),
            ic: Some(IcRecord::new()),
            ..Default::default()
        });
        let (ana, bruno) = (db.students[0].id, db.students[1].id);
        let event = |uid: &str, title: &str, date, guests: &[&str]| ImportedEvent {
            uid: uid.into(),
            title: title.into(),
            meeting: Meeting {
                date: Some(date),
                ..Default::default()
            },
            guests: guests.iter().map(|g| g.to_string()).collect(),
            repeat_dropped: false,
        };
        // The student is found by a guest's address or by name in the title.
        let chat = event("a", "Conversa", d(2026, 11, 3), &["Ana.Souza@exemplo.edu.br"]);
        assert_eq!(db.suggest_student(&chat), Some((ana, MeetingRole::Pos)));
        let tutoria = event("b", "Tutoria – Ana Souza", d(2027, 9, 1), &[]);
        assert_eq!(db.suggest_student(&tutoria), Some((ana, MeetingRole::Advisee)));
        let ic = event("c", "bruno lima – orientação de IC", d(2026, 11, 4), &[]);
        assert_eq!(db.suggest_student(&ic), Some((bruno, MeetingRole::Ic)));
        let other = event("c2", "Bruno Lima", d(2026, 11, 4), &[]);
        assert_eq!(db.suggest_student(&other), Some((bruno, MeetingRole::Tcc)), "the first role");
        let dentist = event("d", "Dentista", d(2026, 11, 5), &[]);
        assert_eq!(db.suggest_student(&dentist), None);
        let skipped = event("e", "Seminário", d(2026, 11, 6), &[]);

        let added = db.import(vec![
            (chat, ImportTarget::Student(ana, MeetingRole::Pos)),
            (tutoria, ImportTarget::Student(ana, MeetingRole::Advisee)),
            (ic, ImportTarget::Student(bruno, MeetingRole::Ic)),
            (dentist, ImportTarget::Calendar),
            (skipped, ImportTarget::Ignore),
        ]);
        assert_eq!(added, 4);
        let s = &db.students[0];
        assert_eq!(s.pos.as_ref().unwrap().meetings[0].source, "a");
        assert_eq!(s.advisee.as_ref().unwrap().s2.meetings.len(), 1, "September is semester 2");
        assert_eq!(db.students[1].ic.as_ref().unwrap().meetings.len(), 1);
        assert_eq!(db.entries[0].title, "Dentista");
        let other = db.events().into_iter().find(|e| e.kind == EventKind::Other).unwrap();
        assert_eq!((other.title.as_str(), other.student_id), ("Dentista", Uuid::nil()));
        assert_eq!(db.cal_items(|e| e.kind == EventKind::Other, true).len(), 1);

        // Seen again: imported, skipped, or exported by the app itself.
        assert_eq!(db.known_import("a"), Some(Known::Imported));
        assert_eq!(db.known_import("d"), Some(Known::Imported));
        assert_eq!(db.known_import("e"), Some(Known::Skipped));
        assert_eq!(db.known_import("meeting-x@supervisor-br.local"), Some(Known::FromSupervisor));
        assert!(Known::Imported.duplicate() && Known::FromSupervisor.duplicate() && !Known::Skipped.duplicate());
        assert_eq!(db.known_import("new"), None);

        // An imported event can be renamed, and deleted.
        let mut details = db.event_details(&other).unwrap();
        details.title = Some("Dentista (remarcado)".into());
        assert!(db.edit_event(&other, &details));
        assert_eq!(db.entries[0].title, "Dentista (remarcado)");
        assert!(db.delete_event(&other, false));
        assert!(db.entries.is_empty());
    }

    #[test]
    fn weeks_start_on_sunday_or_monday() {
        let mut settings = Settings::default();
        assert!(settings.week_starts_sunday, "Sunday, as usual in Brazil");
        // Wednesday 28 October 2026, and a Sunday.
        assert_eq!(settings.week_start(d(2026, 10, 28)), d(2026, 10, 25));
        assert_eq!(settings.week_start(d(2026, 11, 1)), d(2026, 11, 1));
        settings.week_starts_sunday = false;
        assert_eq!(settings.week_start(d(2026, 10, 28)), d(2026, 10, 26));
        assert_eq!(settings.week_start(d(2026, 11, 1)), d(2026, 10, 26));
        // Files from before the option start on Sunday too.
        let old: Settings = serde_json::from_str(r#"{"academic_year": "2026"}"#).unwrap();
        assert!(old.week_starts_sunday);
    }

    #[test]
    fn anonymous_titles_leave_out_the_student() {
        let event = |student_id, title: &str| Event::dated(String::new(), student_id, d(2026, 10, 1), EventKind::Visit, title.into(), false);
        let id = Uuid::new_v4();
        assert_eq!(event(id, "Orientação de TCC – Bruno").anonymous_title(), "Orientação de TCC");
        assert_eq!(
            event(id, "Visita de estágio 1 (on-line) – Bruno @ Empresa – Filial").anonymous_title(),
            "Visita de estágio 1 (on-line)"
        );
        assert_eq!(event(id, "TCC I: Projeto de TCC – Bruno").anonymous_title(), "TCC I: Projeto de TCC");
        assert_eq!(event(Uuid::nil(), "Almoço com a Ana").anonymous_title(), "Evento");

        let db = Database {
            students: vec![Student {
                name: "Aluno Exemplo".into(),
                tcc: Some(TccRecord::default()),
                ..Default::default()
            }],
            ..Default::default()
        };
        let a = &db.attention(d(2026, 10, 1))[0];
        assert_eq!(a.message, "Aluno Exemplo – TCC: ainda sem contato inicial");
        assert_eq!(a.detail, "TCC: ainda sem contato inicial");
    }

    #[test]
    fn grant_end_can_be_typed_in_or_follow_the_final_deadline() {
        let mut p = PosRecord {
            start: Some(d(2026, 3, 1)),
            ..PosRecord::new(Level::MestradoAcademico)
        };
        p.grant.agency = "CAPES".into();
        p.grant.months = 24;
        assert_eq!(p.grant_end(), Some(d(2028, 2, 29)), "from enrolment");
        p.grant.start = Some(d(2026, 4, 1));
        assert_eq!(p.grant_end(), Some(d(2028, 3, 31)), "its own start wins");
        p.grant.end_override = Some(d(2028, 6, 30));
        assert_eq!(p.grant_end(), Some(d(2028, 6, 30)));
        assert_eq!(p.grant.calculated_end(p.start), Some(d(2028, 3, 31)));

        // Linked to the final deadline: it ends then, once there is one.
        p.grant.until_final = true;
        assert_eq!(p.grant_end(), Some(d(2028, 6, 30)), "no final deadline yet");
        p.final_deadline.deadline = Some(d(2028, 8, 15));
        assert_eq!(p.grant_end(), Some(d(2028, 8, 15)));
        let db = Database {
            students: vec![Student {
                name: "Mestranda Exemplo".into(),
                pos: Some(p.clone()),
                ..Default::default()
            }],
            ..Default::default()
        };
        assert!(db.events().iter().any(|e| e.uid.starts_with("pos-grant-end-") && e.date == d(2028, 8, 15)));
        assert!(db.attention(d(2028, 7, 1)).iter().any(|a| a.detail == "bolsa CAPES termina em 15/08/2028"));
        p.grant.agency.clear();
        assert_eq!(p.grant_end(), None, "no grant, no end");

        // IC: a typed-in end moves the suggested reports.
        let mut ic = IcRecord::new();
        ic.grant.start = Some(d(2026, 9, 1));
        ic.grant.end_override = Some(d(2027, 6, 30));
        ic.suggest();
        assert_eq!(ic.grant_end(), Some(d(2027, 6, 30)));
        assert_eq!(ic.final_report.deadline, Some(d(2027, 6, 30)));
        assert_eq!(ic.partial_report.deadline, Some(d(2027, 1, 30)));
    }

    #[test]
    fn placements_have_as_many_visits_as_needed() {
        // Older files have exactly two.
        let old: PlacementRecord = serde_json::from_str(r#"{"company": "Empresa", "visits": [{}, {"mode": "FaceToFace"}]}"#).unwrap();
        assert_eq!(old.visits.len(), 2);
        let mut p = PlacementRecord::default();
        p.add_visit();
        assert_eq!(p.visits[2].mode, VisitMode::Online, "after an in-person visit");
        p.visits.remove(0);
        p.visits[1].date = Some(d(2026, 11, 2));
        p.visits[1].request_sent = Some(d(2026, 10, 1));
        let db = Database {
            students: vec![Student {
                name: "Estagiária Exemplo".into(),
                placement: Some(p),
                ..Default::default()
            }],
            ..Default::default()
        };
        let id = db.students[0].id;
        let visit = db.events().into_iter().find(|e| e.kind == EventKind::Visit).unwrap();
        assert_eq!(visit.uid, visit_uid(id, 1));
        assert_eq!(visit.title, "Visita de estágio 2 (on-line) – Estagiária Exemplo");
        assert!(db.invite(id, &visit.uid).is_some());
        assert!(db.attention(d(2026, 10, 25)).iter().any(|a| a.detail == "visita 2 de estágio: aguardando resposta"));
    }
}
