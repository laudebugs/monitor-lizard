//! User-configurable notification preferences, persisted to `~/.sys-monitor/settings.json`.
//! Off by default: nothing is ever sent unless the owner turns it on.
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

fn file() -> PathBuf {
    crate::paths::data_dir().join("settings.json")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Off,
    High,
    All,
}
impl Level {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "off" => Some(Level::Off),
            "high" => Some(Level::High),
            "all" => Some(Level::All),
            _ => None,
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub notifications: Level,
    #[serde(default = "default_true")]
    pub generic_text: bool,
    /// Minutes since local midnight (0-1439). Both set and equal means no quiet hours.
    #[serde(default)]
    pub quiet_start: Option<u16>,
    #[serde(default)]
    pub quiet_end: Option<u16>,
    /// Whether the findings present when notifications were last turned on from Off have been
    /// seeded as already-seen, so turning notifications on never fires a backlog. Reset to false
    /// by `update` whenever `notifications` moves away from `Off`.
    #[serde(default)]
    pub seeded: bool,
}
impl Default for Level {
    fn default() -> Self {
        Level::Off
    }
}
impl Default for Settings {
    fn default() -> Self {
        Settings { notifications: Level::Off, generic_text: true, quiet_start: None, quiet_end: None, seeded: false }
    }
}

impl Settings {
    /// `now_minutes`: minutes since local midnight. Quiet hours can wrap past midnight
    /// (e.g. 22:00 to 07:00); equal start and end means quiet hours are not configured.
    pub fn in_quiet_hours(&self, now_minutes: u16) -> bool {
        match (self.quiet_start, self.quiet_end) {
            (Some(s), Some(e)) if s == e => false,
            (Some(s), Some(e)) if s < e => now_minutes >= s && now_minutes < e,
            (Some(s), Some(e)) => now_minutes >= s || now_minutes < e,
            _ => false,
        }
    }
}

pub fn load() -> Settings {
    load_at(&file())
}
fn load_at(path: &std::path::Path) -> Settings {
    fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save(s: &Settings) -> std::io::Result<()> {
    save_at(&file(), s)
}
fn save_at(path: &std::path::Path, s: &Settings) -> std::io::Result<()> {
    fs::create_dir_all(path.parent().unwrap())?;
    fs::write(path, serde_json::to_string_pretty(s).unwrap())
}

/// Applies a settings-endpoint update: parses and validates every given field before writing
/// anything, and resets `seeded` when notifications move away from `Off` so the next dispatch
/// seeds the findings present at that moment rather than firing a backlog.
pub fn update(notifications: Option<&str>, generic_text: Option<&str>, quiet_start: Option<&str>, quiet_end: Option<&str>) -> Result<Settings, &'static str> {
    update_at(&file(), notifications, generic_text, quiet_start, quiet_end)
}
fn update_at(path: &std::path::Path, notifications: Option<&str>, generic_text: Option<&str>, quiet_start: Option<&str>, quiet_end: Option<&str>) -> Result<Settings, &'static str> {
    let mut s = load_at(path);
    let was_off = s.notifications == Level::Off;
    if let Some(v) = notifications {
        s.notifications = Level::parse(v).ok_or("notifications must be off, high or all")?;
    }
    if let Some(v) = generic_text {
        s.generic_text = match v {
            "true" => true,
            "false" => false,
            _ => return Err("generic_text must be true or false"),
        };
    }
    let parse_minutes = |v: &str| -> Result<Option<u16>, &'static str> {
        if v.is_empty() {
            return Ok(None);
        }
        v.parse::<u16>().ok().filter(|m| *m < 1440).map(Some).ok_or("quiet hour must be 0-1439 minutes since midnight")
    };
    if let Some(v) = quiet_start {
        s.quiet_start = parse_minutes(v)?;
    }
    if let Some(v) = quiet_end {
        s.quiet_end = parse_minutes(v)?;
    }
    if was_off && s.notifications != Level::Off {
        s.seeded = false;
    }
    save_at(path, &s).map_err(|_| "could not write settings")?;
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_hours_within_the_same_day() {
        let s = Settings { quiet_start: Some(13 * 60), quiet_end: Some(14 * 60), ..Settings::default() };
        assert!(!s.in_quiet_hours(12 * 60 + 59));
        assert!(s.in_quiet_hours(13 * 60));
        assert!(s.in_quiet_hours(13 * 60 + 30));
        assert!(!s.in_quiet_hours(14 * 60));
    }

    #[test]
    fn quiet_hours_wrapping_past_midnight() {
        let s = Settings { quiet_start: Some(22 * 60), quiet_end: Some(7 * 60), ..Settings::default() };
        assert!(s.in_quiet_hours(23 * 60));
        assert!(s.in_quiet_hours(0));
        assert!(s.in_quiet_hours(6 * 60 + 59));
        assert!(!s.in_quiet_hours(7 * 60));
        assert!(!s.in_quiet_hours(12 * 60));
    }

    #[test]
    fn equal_start_and_end_means_no_quiet_hours() {
        let s = Settings { quiet_start: Some(100), quiet_end: Some(100), ..Settings::default() };
        assert!(!s.in_quiet_hours(100));
        assert!(!s.in_quiet_hours(0));
    }

    #[test]
    fn unset_quiet_hours_never_apply() {
        assert!(!Settings::default().in_quiet_hours(0));
    }

    fn tmp_settings_path(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("sysmon-settings-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d.join("settings.json")
    }

    #[test]
    fn update_rejects_bad_values_without_writing_anything() {
        let p = tmp_settings_path("bad");
        assert_eq!(update_at(&p, Some("bogus"), None, None, None).unwrap_err(), "notifications must be off, high or all");
        assert_eq!(update_at(&p, None, Some("maybe"), None, None).unwrap_err(), "generic_text must be true or false");
        assert_eq!(update_at(&p, None, None, Some("1500"), None).unwrap_err(), "quiet hour must be 0-1439 minutes since midnight");
        assert!(!p.exists(), "no field is ever written if any one of them fails validation");
    }

    #[test]
    fn update_persists_and_round_trips() {
        let p = tmp_settings_path("roundtrip");
        update_at(&p, Some("high"), Some("false"), Some("1320"), Some("420")).unwrap();
        let s = load_at(&p);
        assert_eq!((s.notifications, s.generic_text, s.quiet_start, s.quiet_end), (Level::High, false, Some(1320), Some(420)));
    }

    #[test]
    fn turning_notifications_on_from_off_resets_seeded() {
        let p = tmp_settings_path("reseed");
        save_at(&p, &Settings { seeded: true, ..Settings::default() }).unwrap(); // off, already seeded from a prior run
        let s = update_at(&p, Some("high"), None, None, None).unwrap();
        assert!(!s.seeded, "moving off -> high must reseed, so the next dispatch doesn't fire a backlog");
    }

    #[test]
    fn staying_on_does_not_reset_seeded() {
        let p = tmp_settings_path("stay-on");
        save_at(&p, &Settings { notifications: Level::High, seeded: true, ..Settings::default() }).unwrap();
        let s = update_at(&p, Some("all"), None, None, None).unwrap();
        assert!(s.seeded, "high -> all is not off -> on, so the existing seed must be kept");
    }
}
