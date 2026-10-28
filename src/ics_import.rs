//! Reading another calendar's .ics file (Google Agenda, Outlook) so its
//! events can be imported as meetings. Weekly repeats come along with their
//! cancelled and moved weeks; other kinds of repeat keep only the first date.

use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, NaiveTime, Timelike};
use jiff::tz::TimeZone;

use crate::model::{ImportedEvent, Meeting, Occurrence, Platform, Repeat};

/// Undo the TEXT escaping of RFC 5545.
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n' | 'N') => out.push('\n'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// One content line: its name, parameters and value.
struct Line<'a> {
    name: String,
    params: Vec<(String, String)>,
    value: &'a str,
}

impl Line<'_> {
    fn param(&self, name: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// Split `NAME;PARAM=a;PARAM="b:c":value`, minding quoted parameter values.
fn split_line(line: &str) -> Option<Line<'_>> {
    let mut quoted = false;
    let colon = line.char_indices().find_map(|(i, c)| match c {
        '"' => {
            quoted = !quoted;
            None
        }
        ':' if !quoted => Some(i),
        _ => None,
    })?;
    let (head, value) = (&line[..colon], &line[colon + 1..]);
    let mut parts = head.split(';');
    let name = parts.next()?.trim().to_ascii_uppercase();
    let params = parts
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| (k.trim().to_ascii_uppercase(), v.trim().trim_matches('"').to_string()))
        .collect();
    Some(Line { name, params, value })
}

/// Windows zone names Outlook writes as `TZID`: Brazil's, and a few others.
fn windows_zone(name: &str) -> Option<&'static str> {
    Some(match name {
        "E. South America Standard Time" => "America/Sao_Paulo",
        "SA Eastern Standard Time" => "America/Fortaleza",
        "Bahia Standard Time" => "America/Bahia",
        "Tocantins Standard Time" => "America/Araguaina",
        "Central Brazilian Standard Time" => "America/Cuiaba",
        "SA Western Standard Time" => "America/Manaus",
        "SA Pacific Standard Time" => "America/Rio_Branco",
        "UTC-02" | "Mid-Atlantic Standard Time" => "America/Noronha",
        "Argentina Standard Time" => "America/Buenos_Aires",
        "Pacific SA Standard Time" => "America/Santiago",
        "GMT Standard Time" => "Europe/London",
        "Greenwich Standard Time" => "Atlantic/Reykjavik",
        "W. Europe Standard Time" => "Europe/Berlin",
        "Romance Standard Time" => "Europe/Paris",
        "Central Europe Standard Time" => "Europe/Budapest",
        "Central European Standard Time" => "Europe/Warsaw",
        "GTB Standard Time" => "Europe/Bucharest",
        "Eastern Standard Time" => "America/New_York",
        "Central Standard Time" => "America/Chicago",
        "Mountain Standard Time" => "America/Denver",
        "Pacific Standard Time" => "America/Los_Angeles",
        "India Standard Time" => "Asia/Kolkata",
        "China Standard Time" => "Asia/Shanghai",
        "Tokyo Standard Time" => "Asia/Tokyo",
        "AUS Eastern Standard Time" => "Australia/Sydney",
        "UTC" | "Coordinated Universal Time" => "UTC",
        _ => return None,
    })
}

fn zone_named(name: &str) -> Option<TimeZone> {
    let name = name.trim().trim_start_matches('/');
    jiff::tz::db()
        .get(name)
        .ok()
        .or_else(|| windows_zone(name).and_then(|n| jiff::tz::db().get(n).ok()))
}

fn civil(dt: NaiveDateTime) -> Option<jiff::civil::DateTime> {
    jiff::civil::DateTime::new(
        i16::try_from(dt.year()).ok()?,
        dt.month() as i8,
        dt.day() as i8,
        dt.hour() as i8,
        dt.minute() as i8,
        dt.second() as i8,
        0,
    )
    .ok()
}

fn naive(dt: jiff::civil::DateTime) -> Option<NaiveDateTime> {
    NaiveDate::from_ymd_opt(dt.year().into(), dt.month() as u32, dt.day() as u32)?
        .and_hms_opt(dt.hour() as u32, dt.minute() as u32, dt.second() as u32)
}

/// A DATE or DATE-TIME value as a local day and time (none for a whole
/// day). UTC times and times in another zone are moved to `local`; a zone
/// that isn't known is taken as local.
fn date_time(value: &str, tzid: Option<&str>, local: &TimeZone) -> Option<(NaiveDate, Option<NaiveTime>)> {
    let value = value.trim();
    let date = NaiveDate::parse_from_str(value.get(..8)?, "%Y%m%d").ok()?;
    let Some(rest) = value.get(8..).filter(|r| !r.is_empty()) else {
        return Some((date, None));
    };
    let utc = rest.ends_with(['Z', 'z']);
    let time = NaiveTime::parse_from_str(rest.trim_start_matches(['T', 't']).trim_end_matches(['Z', 'z']), "%H%M%S").ok()?;
    let wall = NaiveDateTime::new(date, time);
    let from = if utc {
        Some(TimeZone::UTC)
    } else {
        tzid.and_then(zone_named)
    };
    let moved = from.and_then(|zone| {
        let zoned = civil(wall)?.to_zoned(zone).ok()?;
        naive(zoned.with_time_zone(local.clone()).datetime())
    });
    let dt = moved.unwrap_or(wall);
    Some((dt.date(), Some(dt.time())))
}

/// A VEVENT as written, before its times are worked out.
#[derive(Default)]
struct Raw {
    uid: String,
    summary: String,
    description: String,
    location: String,
    url: String,
    conference: String,
    start: Option<(NaiveDate, Option<NaiveTime>)>,
    rrule: String,
    exdates: Vec<(NaiveDate, Option<NaiveTime>)>,
    recurrence_id: Option<(NaiveDate, Option<NaiveTime>)>,
    cancelled: bool,
    guests: Vec<String>,
}

fn read_events(text: &str, local: &TimeZone) -> Vec<Raw> {
    let unfolded = text
        .replace("\r\n ", "")
        .replace("\r\n\t", "")
        .replace("\n ", "")
        .replace("\n\t", "");
    let mut out = Vec::new();
    let mut current: Option<Raw> = None;
    // Components nested in the event (alarms) whose lines are skipped.
    let mut nested = 0;
    for line in unfolded.lines() {
        let Some(line) = split_line(line.trim_end_matches('\r')) else {
            continue;
        };
        let value = line.value.trim();
        match (line.name.as_str(), &mut current) {
            ("BEGIN", None) if value.eq_ignore_ascii_case("VEVENT") => current = Some(Raw::default()),
            ("BEGIN", Some(_)) => nested += 1,
            ("END", Some(_)) if nested > 0 => nested -= 1,
            ("END", Some(_)) if value.eq_ignore_ascii_case("VEVENT") => out.extend(current.take()),
            (_, Some(_)) if nested > 0 => {}
            (name, Some(raw)) => {
                let when = || date_time(value, line.param("TZID"), local);
                match name {
                    "UID" => raw.uid = value.to_string(),
                    "SUMMARY" => raw.summary = unescape(value),
                    "DESCRIPTION" => raw.description = unescape(value),
                    "LOCATION" => raw.location = unescape(value),
                    "URL" => raw.url = value.to_string(),
                    "X-GOOGLE-CONFERENCE" => raw.conference = value.to_string(),
                    "DTSTART" => raw.start = when(),
                    "RRULE" => raw.rrule = value.to_ascii_uppercase(),
                    "EXDATE" => raw.exdates.extend(
                        value
                            .split(',')
                            .filter_map(|v| date_time(v, line.param("TZID"), local)),
                    ),
                    "RECURRENCE-ID" => raw.recurrence_id = when(),
                    "STATUS" => raw.cancelled = value.eq_ignore_ascii_case("CANCELLED"),
                    "ATTENDEE" => {
                        if value.get(..7).is_some_and(|s| s.eq_ignore_ascii_case("mailto:")) {
                            raw.guests.push(value[7..].trim().to_string());
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    out
}

/// The first `https://` link in `text` that leads to a meeting service.
fn meeting_link(text: &str) -> Option<(Platform, String)> {
    let mut rest = text;
    while let Some(i) = rest.find("https://") {
        let link: String = rest[i..]
            .chars()
            .take_while(|c| !c.is_whitespace() && !matches!(c, '<' | '>' | '"' | ')' | ']'))
            .collect();
        rest = &rest[i + link.len().max(8)..];
        let host = link[8..].split(['/', '?', '#']).next().unwrap_or_default().to_ascii_lowercase();
        let platform = if host == "meet.google.com" {
            Platform::GoogleMeet
        } else if host.starts_with("teams.") && (host.ends_with("microsoft.com") || host.ends_with("live.com")) {
            Platform::Teams
        } else if host == "zoom.us" || host.ends_with(".zoom.us") {
            Platform::Zoom
        } else if host == "conferenciaweb.rnp.br" || host == "webconf.rnp.br" || host.ends_with(".webconf.rnp.br") {
            Platform::Rnp
        } else {
            continue;
        };
        return Some((platform, link.trim_end_matches(['.', ',', ';']).to_string()));
    }
    None
}

/// Names calendars put in the location of an online meeting.
fn online_placeholder(location: &str) -> bool {
    let l = location.trim().to_lowercase();
    l.is_empty()
        || l.starts_with("https://")
        || l.contains("microsoft teams meeting")
        || l.contains("reunião do microsoft teams")
        || l == "google meet"
        || l.starts_with("zoom meeting")
        || l.starts_with("reunião zoom")
        || l == "zoom"
        || l.contains("conferência web rnp")
}

/// A weekly rule as the app keeps it, or `None` if it repeats some other
/// way (daily, monthly, or on several days of the week).
fn weekly(rule: &str, start: NaiveDate, local: &TimeZone) -> Option<Repeat> {
    let mut freq = "";
    let mut interval = 1u32;
    let mut until = None;
    let mut count = None;
    for part in rule.split(';') {
        let Some((key, value)) = part.split_once('=') else {
            continue;
        };
        match key {
            "FREQ" => freq = value,
            "INTERVAL" => interval = value.parse().ok()?,
            "UNTIL" => until = date_time(value, None, local).map(|(d, _)| d),
            "COUNT" => count = Some(value.parse::<u32>().ok()?),
            "BYDAY" => {
                let day = match start.weekday() {
                    chrono::Weekday::Mon => "MO",
                    chrono::Weekday::Tue => "TU",
                    chrono::Weekday::Wed => "WE",
                    chrono::Weekday::Thu => "TH",
                    chrono::Weekday::Fri => "FR",
                    chrono::Weekday::Sat => "SA",
                    chrono::Weekday::Sun => "SU",
                };
                if value != day {
                    return None;
                }
            }
            "WKST" => {}
            _ => return None,
        }
    }
    if freq != "WEEKLY" || !(1..=4).contains(&interval) {
        return None;
    }
    let until = until.or_else(|| {
        let weeks = i64::from(count?.max(1) - 1) * i64::from(interval);
        Some(start + Duration::weeks(weeks))
    });
    Some(Repeat {
        every_weeks: interval as u8,
        until,
        exceptions: Vec::new(),
    })
}

fn to_event(raw: Raw, local: &TimeZone) -> Option<ImportedEvent> {
    let (date, time) = raw.start?;
    let link = [&raw.conference, &raw.url, &raw.location, &raw.description]
        .into_iter()
        .find_map(|text| meeting_link(text));
    let mut meeting = Meeting {
        date: Some(date),
        time,
        notes: raw.description.trim().to_string(),
        source: raw.uid.clone(),
        ..Default::default()
    };
    match link {
        Some((platform, link)) => {
            meeting.online = true;
            meeting.platform = platform;
            meeting.link = link;
            if !online_placeholder(&raw.location) {
                meeting.location = raw.location.trim().to_string();
            }
        }
        None => meeting.location = raw.location.trim().to_string(),
    }
    let mut repeat_dropped = false;
    if !raw.rrule.is_empty() {
        match weekly(&raw.rrule, date, local) {
            Some(mut repeat) => {
                repeat.exceptions = raw
                    .exdates
                    .iter()
                    .map(|(planned, _)| Occurrence {
                        planned: *planned,
                        cancelled: true,
                        ..Default::default()
                    })
                    .collect();
                meeting.repeat = Some(repeat);
            }
            None => repeat_dropped = true,
        }
    }
    Some(ImportedEvent {
        uid: raw.uid,
        title: raw.summary.trim().to_string(),
        meeting,
        guests: raw.guests,
        repeat_dropped,
    })
}

/// The events of an .ics file, earliest first, times in `local`. Cancelled
/// events are left out; changed weeks of a weekly series are folded into it.
pub fn parse(text: &str, local: &TimeZone) -> Vec<ImportedEvent> {
    let mut masters = Vec::new();
    let mut changes = Vec::new();
    for (n, mut raw) in read_events(text, local).into_iter().enumerate() {
        if raw.uid.trim().is_empty() {
            raw.uid = format!("no-uid-{n}");
        }
        if raw.recurrence_id.is_some() {
            changes.push(raw);
        } else if !raw.cancelled {
            masters.push(raw);
        }
    }
    let mut events: Vec<ImportedEvent> = masters.into_iter().filter_map(|raw| to_event(raw, local)).collect();
    for change in changes {
        let Some((planned, _)) = change.recurrence_id else { continue };
        let series = events
            .iter_mut()
            .find(|e| e.uid == change.uid)
            .and_then(|e| {
                let usual = e.meeting.time;
                e.meeting.repeat.as_mut().map(|r| (r, usual))
            });
        match series {
            Some((repeat, usual)) => {
                let (date, time) = change.start.unwrap_or((planned, usual));
                repeat.exceptions.retain(|o| o.planned != planned);
                repeat.exceptions.push(Occurrence {
                    planned,
                    cancelled: change.cancelled,
                    moved_to: (date != planned).then_some(date),
                    time: time.filter(|t| Some(*t) != usual),
                    ..Default::default()
                });
                repeat.exceptions.sort_by_key(|o| o.planned);
            }
            // A changed week of a series that isn't kept as one: its own event.
            None if !change.cancelled => {
                let uid = format!("{}/{}", change.uid, planned.format("%Y%m%d"));
                if let Some(mut e) = to_event(Raw { uid: uid.clone(), ..change }, local) {
                    e.meeting.source = uid;
                    events.push(e);
                }
            }
            None => {}
        }
    }
    events.sort_by_key(|e| (e.meeting.date, e.meeting.time));
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    /// São Paulo: UTC−3 all year.
    fn brasilia() -> TimeZone {
        TimeZone::posix("<-03>3").unwrap()
    }

    fn d(y: i32, m: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, day).unwrap()
    }

    fn t(h: u32, m: u32) -> Option<NaiveTime> {
        NaiveTime::from_hms_opt(h, m, 0)
    }

    const GOOGLE: &str = "BEGIN:VCALENDAR\r
PRODID:-//Google Inc//Google Calendar 70.9054//EN\r
VERSION:2.0\r
BEGIN:VEVENT\r
DTSTART:20261103T170000Z\r
DTEND:20261103T180000Z\r
RRULE:FREQ=WEEKLY;UNTIL=20261208T235959Z;BYDAY=TU\r
EXDATE;TZID=America/Sao_Paulo:20261117T140000\r
UID:abc123@google.com\r
ATTENDEE;CUTYPE=INDIVIDUAL;ROLE=REQ-PARTICIPANT;PARTSTAT=NEEDS-ACTION;CN=Ana\r
  Souza;X-NUM-GUESTS=0:mailto:Ana.Souza@exemplo.edu.br\r
X-GOOGLE-CONFERENCE:https://meet.google.com/abc-defg-hij\r
DESCRIPTION:Orientação semanal\\, plano da dissertação\\nEntrar: https://meet.google.com/abc-defg-hij\r
LOCATION:\r
STATUS:CONFIRMED\r
SUMMARY:Orientação (Mestrado) – Ana\r
BEGIN:VALARM\r
ACTION:DISPLAY\r
SUMMARY:Alarme, não o evento\r
TRIGGER:-P0DT0H10M0S\r
END:VALARM\r
END:VEVENT\r
BEGIN:VEVENT\r
DTSTART;TZID=America/Sao_Paulo:20261124T160000\r
DTEND;TZID=America/Sao_Paulo:20261124T170000\r
RECURRENCE-ID;TZID=America/Sao_Paulo:20261124T140000\r
UID:abc123@google.com\r
SUMMARY:Orientação (Mestrado) – Ana\r
END:VEVENT\r
BEGIN:VEVENT\r
DTSTART;VALUE=DATE:20261201\r
DTEND;VALUE=DATE:20261202\r
UID:semana@google.com\r
SUMMARY:Semana acadêmica\r
LOCATION:Bloco B\\, sala 201\r
END:VEVENT\r
BEGIN:VEVENT\r
DTSTART:20261105T130000Z\r
UID:gone@google.com\r
STATUS:CANCELLED\r
SUMMARY:Cancelado\r
END:VEVENT\r
END:VCALENDAR\r
";

    #[test]
    fn google_agenda_file() {
        let events = parse(GOOGLE, &brasilia());
        assert_eq!(events.len(), 2, "the cancelled event is left out");

        let ana = &events[0];
        assert_eq!(ana.title, "Orientação (Mestrado) – Ana");
        assert_eq!(ana.guests, ["Ana.Souza@exemplo.edu.br"]);
        let m = &ana.meeting;
        // 17:00 UTC is 14:00 in Brasília.
        assert_eq!((m.date, m.time), (Some(d(2026, 11, 3)), t(14, 0)));
        assert!(m.online);
        assert_eq!((m.platform, m.link.as_str()), (Platform::GoogleMeet, "https://meet.google.com/abc-defg-hij"));
        assert!(m.notes.starts_with("Orientação semanal, plano da dissertação\nEntrar:"));
        assert_eq!(m.source, "abc123@google.com");
        let r = m.repeat.as_ref().expect("weekly");
        assert_eq!((r.every_weeks, r.until), (1, Some(d(2026, 12, 8))));
        assert_eq!(r.exceptions.len(), 2);
        assert!(r.exceptions[0].cancelled && r.exceptions[0].planned == d(2026, 11, 17));
        let moved = &r.exceptions[1];
        assert_eq!((moved.planned, moved.moved_to, moved.time), (d(2026, 11, 24), None, t(16, 0)));

        let week = &events[1];
        assert_eq!(week.meeting.time, None);
        assert_eq!(week.meeting.location, "Bloco B, sala 201");
        assert!(!week.meeting.online);
    }

    #[test]
    fn outlook_zones_and_repeats() {
        let ics = "BEGIN:VCALENDAR\r
BEGIN:VEVENT\r
UID:040000008200E00074C5B7101A82E008\r
SUMMARY:Orientação de TCC\r
DTSTART;TZID=\"E. South America Standard Time\":20260601T100000\r
RRULE:FREQ=WEEKLY;INTERVAL=2;COUNT=4\r
LOCATION:Reunião do Microsoft Teams\r
DESCRIPTION:Participar: <https://teams.microsoft.com/l/meetup-join/19%3ameeting>\r
END:VEVENT\r
BEGIN:VEVENT\r
UID:lisboa\r
SUMMARY:Banca em Lisboa\r
DTSTART;TZID=Europe/Lisbon:20260601T150000\r
RRULE:FREQ=WEEKLY;BYDAY=MO,WE\r
LOCATION:https://conferenciaweb.rnp.br/sala/banca-exemplo\r
END:VEVENT\r
END:VCALENDAR\r
";
        let events = parse(ics, &brasilia());
        let teams = events.iter().find(|e| e.title == "Orientação de TCC").unwrap();
        // "E. South America Standard Time" is Brasília: 10:00 stays 10:00.
        assert_eq!(teams.meeting.time, t(10, 0));
        assert_eq!(teams.meeting.platform, Platform::Teams);
        assert_eq!(teams.meeting.link, "https://teams.microsoft.com/l/meetup-join/19%3ameeting");
        assert_eq!(teams.meeting.location, "", "the placeholder isn't kept as a room");
        let r = teams.meeting.repeat.as_ref().unwrap();
        assert_eq!((r.every_weeks, r.until), (2, Some(d(2026, 7, 13))));

        // 15:00 in Lisbon (UTC+1 in June) is 11:00 in Brasília; two days a
        // week can't be kept. The RNP link makes it an online meeting.
        let lisboa = events.iter().find(|e| e.uid == "lisboa").unwrap();
        assert_eq!(lisboa.meeting.time, t(11, 0));
        assert!(lisboa.meeting.repeat.is_none() && lisboa.repeat_dropped);
        assert_eq!(lisboa.meeting.platform, Platform::Rnp);
        assert!(lisboa.meeting.online && lisboa.meeting.location.is_empty());
    }

    #[test]
    fn links_found_in_text() {
        assert_eq!(
            meeting_link("veja https://exemplo.com/x e https://ufpe.zoom.us/j/123?pwd=a."),
            Some((Platform::Zoom, "https://ufpe.zoom.us/j/123?pwd=a".into()))
        );
        assert_eq!(
            meeting_link("Sala: https://webconf.rnp.br/abc"),
            Some((Platform::Rnp, "https://webconf.rnp.br/abc".into()))
        );
        assert_eq!(meeting_link("https://evilzoom.us/j/1"), None);
        assert_eq!(meeting_link("https://rnp.br.exemplo.com/x"), None);
        assert_eq!(meeting_link("nada aqui"), None);
        assert_eq!(unescape("a\\,b\\;c\\\\d\\ne"), "a,b;c\\d\ne");
    }
}
