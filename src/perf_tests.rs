//! Timings for the per-frame and save paths, on a realistic dataset.
//! Run with: cargo test --release perf_ -- --ignored --nocapture

use std::time::Instant;

use chrono::{Duration, NaiveDate, NaiveTime};

use crate::app::Derived;
use crate::model::*;
use crate::storage::{self, Vault};

fn d(y: i32, m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, day).unwrap()
}

fn notes(n: usize) -> String {
    "Discutimos o andamento da revisão bibliográfica e os próximos passos. ".repeat(n)
}

/// A repeating meeting with `held` weeks already marked, each with notes.
fn repeating(start: NaiveDate, every: u8, until: Option<NaiveDate>, held: usize) -> Meeting {
    let exceptions = (0..held)
        .map(|w| Occurrence {
            planned: start + Duration::weeks(w as i64 * i64::from(every)),
            held: true,
            notes: notes(2),
            ..Default::default()
        })
        .collect();
    Meeting {
        date: Some(start),
        time: NaiveTime::from_hms_opt(14, 0, 0),
        location: "Sala 201".into(),
        repeat: Some(Repeat { every_weeks: every, until, exceptions }),
        notes: notes(1),
        ..Default::default()
    }
}

/// One academic year: 40 TCC, 10 IC, 6 postgraduate, 15 tutoria and 10 estágio students.
fn year(y: i32, archived: bool) -> Vec<Student> {
    let (s1, s2) = (d(y, 3, 2), d(y, 8, 3));
    let held = |current, past| if archived { past } else { current };
    let mut out = Vec::new();
    for i in 0..40 {
        out.push(Student {
            name: format!("Aluno {y}-{i}"),
            enrollment: format!("{y}{i:06}"),
            email: format!("a{i}@exemplo.edu.br"),
            course: "Engenharia de Computação".into(),
            archived,
            notes: notes(3),
            tcc: Some(TccRecord {
                programme: "graduacao".into(),
                initial_contact: true,
                meetings: vec![repeating(s1, 1, None, held(5, 12)), repeating(s2, 1, None, held(0, 12))],
                ..Default::default()
            }),
            ..Default::default()
        });
    }
    for i in 0..6 {
        let mut pos = PosRecord::new(Level::DoutoradoAcademico);
        pos.start = Some(s1);
        pos.suggest(&LevelRule::default_for(pos.level));
        pos.meetings = vec![repeating(s1, 2, Some(d(y + 1, 8, 31)), held(3, 24))];
        out.push(Student { name: format!("Pós {y}-{i}"), archived, notes: notes(5), pos: Some(pos), ..Default::default() });
    }
    for i in 0..10 {
        let mut ic = IcRecord::new();
        ic.grant.start = Some(s2);
        ic.suggest();
        ic.meetings = vec![repeating(s2, 2, None, held(2, 8))];
        out.push(Student { name: format!("IC {y}-{i}"), archived, ic: Some(ic), ..Default::default() });
    }
    for i in 0..15 {
        out.push(Student { name: format!("Tutoria {y}-{i}"), archived, advisee: Some(AdviseeRecord::default()), ..Default::default() });
    }
    for i in 0..10 {
        out.push(Student { name: format!("Estágio {y}-{i}"), archived, placement: Some(PlacementRecord::default()), ..Default::default() });
    }
    out
}

fn dataset(archived_years: i32) -> Database {
    let mut db = Database::default();
    db.settings.sem1_start = Some(d(2026, 3, 2));
    db.settings.sem2_start = Some(d(2026, 8, 3));
    for y in (2026 - archived_years)..2026 {
        db.students.extend(year(y, true));
    }
    db.students.extend(year(2026, false));
    db
}

fn time<T>(label: &str, runs: u32, mut f: impl FnMut() -> T) {
    for _ in 0..3 {
        std::hint::black_box(f());
    }
    let start = Instant::now();
    for _ in 0..runs {
        std::hint::black_box(f());
    }
    let ms = start.elapsed().as_secs_f64() * 1000.0 / f64::from(runs);
    println!("  {label:<40} {ms:8.3} ms");
}

#[test]
#[ignore = "timing, not a check; run on demand"]
fn perf_timings() {
    let today = d(2026, 10, 26);
    for archived_years in [0, 5] {
        let db = dataset(archived_years);
        println!("{} students, {} archived years:", db.students.len(), archived_years);
        time("change check (fingerprint)", 100, || storage::fingerprint(&db));
        time("dashboard data", 100, || {
            let events = db.events();
            (upcoming(&events, today, 14).len(), db.attention(today))
        });
        let print = storage::fingerprint(&db);
        let mut cache = None;
        time("dashboard data, cached", 100, || {
            let d = Derived::of(&mut cache, &db, print, today);
            (upcoming(&d.events, today, 14).len(), d.attention.len())
        });
        time("events()", 100, || db.events());
        let dir = tempfile::tempdir().unwrap();
        let mut vault = Vault::create(&dir.path().join("data.svt"), "correct horse battery staple", &db).unwrap();
        time("autosave (Vault::save)", 20, || vault.save(&db).unwrap());
        time("autosave, UI-thread part (Vault::seal)", 20, || vault.seal(&db).unwrap());
    }
}
