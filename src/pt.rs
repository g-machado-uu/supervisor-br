//! Brazilian Portuguese dates, written by hand so they don't depend on the
//! computer's locale (chrono's `%a` and `%b` are always English).

use chrono::{Datelike, NaiveDate};

pub const WEEKDAYS_SHORT: [&str; 7] = ["seg", "ter", "qua", "qui", "sex", "sáb", "dom"];
pub const WEEKDAYS: [&str; 7] = [
    "segunda-feira",
    "terça-feira",
    "quarta-feira",
    "quinta-feira",
    "sexta-feira",
    "sábado",
    "domingo",
];
/// Initials for the columns of a month grid.
pub const WEEKDAY_INITIALS: [&str; 7] = ["S", "T", "Q", "Q", "S", "S", "D"];
pub const MONTHS_SHORT: [&str; 12] = [
    "jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez",
];
pub const MONTHS: [&str; 12] = [
    "janeiro",
    "fevereiro",
    "março",
    "abril",
    "maio",
    "junho",
    "julho",
    "agosto",
    "setembro",
    "outubro",
    "novembro",
    "dezembro",
];

/// `days` (listed from Monday) in the order a calendar shows them: from
/// Sunday, as usual in Brazil, or from Monday.
fn week_order(days: [&'static str; 7], sunday_first: bool) -> [&'static str; 7] {
    let mut out = days;
    if sunday_first {
        out.rotate_right(1);
    }
    out
}

/// Short weekday names for the columns of a calendar.
pub fn weekdays_short(sunday_first: bool) -> [&'static str; 7] {
    week_order(WEEKDAYS_SHORT, sunday_first)
}

/// Weekday initials for the columns of a small month grid.
pub fn weekday_initials(sunday_first: bool) -> [&'static str; 7] {
    week_order(WEEKDAY_INITIALS, sunday_first)
}

fn wd(d: NaiveDate) -> usize {
    d.weekday().num_days_from_monday() as usize
}

fn mo(d: NaiveDate) -> usize {
    d.month0() as usize
}

/// "seg, 26 out 2026"
pub fn date(d: NaiveDate) -> String {
    format!("{}, {:02} {} {}", WEEKDAYS_SHORT[wd(d)], d.day(), MONTHS_SHORT[mo(d)], d.year())
}

/// "26 out"
pub fn day_month(d: NaiveDate) -> String {
    format!("{:02} {}", d.day(), MONTHS_SHORT[mo(d)])
}

/// "26 out 2026"
pub fn day_month_year(d: NaiveDate) -> String {
    format!("{:02} {} {}", d.day(), MONTHS_SHORT[mo(d)], d.year())
}

/// "seg, 26 out"
pub fn weekday_day_month(d: NaiveDate) -> String {
    format!("{}, {:02} {}", WEEKDAYS_SHORT[wd(d)], d.day(), MONTHS_SHORT[mo(d)])
}

/// "seg 26"
pub fn weekday_day(d: NaiveDate) -> String {
    format!("{} {:02}", WEEKDAYS_SHORT[wd(d)], d.day())
}

/// "segunda-feira, 26 de outubro de 2026"
pub fn long(d: NaiveDate) -> String {
    format!("{}, {} de {} de {}", WEEKDAYS[wd(d)], d.day(), MONTHS[mo(d)], d.year())
}

/// "Outubro de 2026"
pub fn month_year(d: NaiveDate) -> String {
    let month = MONTHS[mo(d)];
    let mut chars = month.chars();
    let first = chars.next().map(|c| c.to_uppercase().collect::<String>()).unwrap_or_default();
    format!("{first}{} de {}", chars.as_str(), d.year())
}

/// "26/10/2026"
pub fn numeric(d: NaiveDate) -> String {
    d.format("%d/%m/%Y").to_string()
}

/// "seg" — the short weekday on its own.
pub fn weekday_short(d: NaiveDate) -> &'static str {
    WEEKDAYS_SHORT[wd(d)]
}

/// "out" — the short month on its own.
pub fn month_short(d: NaiveDate) -> &'static str {
    MONTHS_SHORT[mo(d)]
}

/// "1 aluno" / "3 alunos".
pub fn count(n: usize, one: &str, many: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {many}") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_in_portuguese() {
        let d = NaiveDate::from_ymd_opt(2026, 10, 26).unwrap();
        assert_eq!(date(d), "seg, 26 out 2026");
        assert_eq!(long(d), "segunda-feira, 26 de outubro de 2026");
        assert_eq!(month_year(d), "Outubro de 2026");
        assert_eq!(numeric(d), "26/10/2026");
        assert_eq!(day_month(d), "26 out");
        let s = NaiveDate::from_ymd_opt(2026, 3, 7).unwrap();
        assert_eq!(weekday_day(s), "sáb 07");
        assert_eq!(month_year(s), "Março de 2026");
        assert_eq!(count(1, "aluno", "alunos"), "1 aluno");
        assert_eq!(count(0, "aluno", "alunos"), "0 alunos");
        assert_eq!(weekdays_short(true), ["dom", "seg", "ter", "qua", "qui", "sex", "sáb"]);
        assert_eq!(weekdays_short(false)[0], "seg");
        assert_eq!(weekday_initials(true), ["D", "S", "T", "Q", "Q", "S", "S"]);
    }
}
