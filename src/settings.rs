//! User-configurable notification preferences, persisted to `~/.monitor-lizard/settings.json`.
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
    /// Off by default: downsampled CPU/memory history is kept in SQLite only once the owner opts
    /// in, same as every other disk-persisted feature in this app.
    #[serde(default)]
    pub metrics_history: bool,
    /// The local classifier agent's kill switch: off by default, same as every other opt-in
    /// feature here. Set only via `monitor-lizard agent on`/`off`, not `/api/settings`, since there
    /// is no page control to drive it yet.
    #[serde(default)]
    pub agent_enabled: bool,
    /// Off by default: hourly bytes written per app group are kept only once the owner opts in on
    /// the Storage page, because app names with hourly counts reveal habits.
    #[serde(default)]
    pub disk_writes_history: bool,
}
impl Default for Level {
    fn default() -> Self {
        Level::Off
    }
}
impl Default for Settings {
    fn default() -> Self {
        Settings { notifications: Level::Off, generic_text: true, quiet_start: None, quiet_end: None, seeded: false, metrics_history: false, agent_enabled: false, disk_writes_history: false }
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

#[cfg(test)]
pub(crate) fn save_at_for_test(path: &std::path::Path) { save_at(path, &Settings::default()).unwrap(); }

pub fn save(s: &Settings) -> std::io::Result<()> {
    save_at(&file(), s)
}

/// `monitor-lizard agent on`/`off`'s whole job: read-modify-write just this one field, leaving
/// every other setting as it was.
pub fn set_agent_enabled(enabled: bool) -> std::io::Result<()> {
    set_agent_enabled_at(&file(), enabled)
}
fn set_agent_enabled_at(path: &std::path::Path, enabled: bool) -> std::io::Result<()> {
    let mut s = load_at(path);
    s.agent_enabled = enabled;
    save_at(path, &s)
}
fn save_at(path: &std::path::Path, s: &Settings) -> std::io::Result<()> {
    use std::io::Write;
    let dir = path.parent().ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("the settings path {} has no folder", path.display())))?;
    crate::paths::create_private_dir(dir).map_err(|e| crate::paths::with_context("create the folder for the settings,", dir, e))?;
    let text = serde_json::to_string_pretty(s).map_err(std::io::Error::other)?;
    crate::paths::create_private_file_truncating(path).and_then(|mut f| f.write_all(text.as_bytes()))
        .map_err(|e| crate::paths::with_context("write the settings to", path, e))
}

/// The fields `POST /api/settings` may change, each as the raw text from the query, `None` when not
/// given. A struct rather than one parameter per field, so a new setting adds a field, not a
/// parameter at every call site.
#[derive(Debug, Default)]
pub struct Update {
    pub notifications: Option<String>,
    pub generic_text: Option<String>,
    pub quiet_start: Option<String>,
    pub quiet_end: Option<String>,
    pub metrics_history: Option<String>,
    pub disk_writes_history: Option<String>,
}

/// Applies a settings-endpoint update: parses and validates every given field before writing
/// anything, and resets `seeded` when notifications move away from `Off` so the next dispatch
/// seeds the findings present at that moment rather than firing a backlog.
pub fn update(u: &Update) -> Result<Settings, &'static str> {
    update_at(&file(), u)
}
fn parse_bool(v: Option<&str>, err: &'static str) -> Result<Option<bool>, &'static str> {
    match v {
        None => Ok(None),
        Some("true") => Ok(Some(true)),
        Some("false") => Ok(Some(false)),
        Some(_) => Err(err),
    }
}
fn update_at(path: &std::path::Path, u: &Update) -> Result<Settings, &'static str> {
    let mut s = load_at(path);
    let was_off = s.notifications == Level::Off;
    if let Some(v) = u.notifications.as_deref() {
        s.notifications = Level::parse(v).ok_or("notifications must be off, high or all")?;
    }
    if let Some(v) = parse_bool(u.generic_text.as_deref(), "generic_text must be true or false")? { s.generic_text = v; }
    let parse_minutes = |v: &str| -> Result<Option<u16>, &'static str> {
        if v.is_empty() {
            return Ok(None);
        }
        v.parse::<u16>().ok().filter(|m| *m < 1440).map(Some).ok_or("quiet hour must be 0-1439 minutes since midnight")
    };
    if let Some(v) = u.quiet_start.as_deref() {
        s.quiet_start = parse_minutes(v)?;
    }
    if let Some(v) = u.quiet_end.as_deref() {
        s.quiet_end = parse_minutes(v)?;
    }
    if let Some(v) = parse_bool(u.metrics_history.as_deref(), "metrics_history must be true or false")? { s.metrics_history = v; }
    if let Some(v) = parse_bool(u.disk_writes_history.as_deref(), "disk_writes_history must be true or false")? { s.disk_writes_history = v; }
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
        let d = std::env::temp_dir().join(format!("monlizard-settings-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d.join("settings.json")
    }

    #[test]
    fn update_rejects_bad_values_without_writing_anything() {
        let p = tmp_settings_path("bad");
        assert_eq!(update_at(&p, &Update { notifications: Some("bogus".into()), ..Update::default() }).unwrap_err(), "notifications must be off, high or all");
        assert_eq!(update_at(&p, &Update { generic_text: Some("maybe".into()), ..Update::default() }).unwrap_err(), "generic_text must be true or false");
        assert_eq!(update_at(&p, &Update { quiet_start: Some("1500".into()), ..Update::default() }).unwrap_err(), "quiet hour must be 0-1439 minutes since midnight");
        assert!(!p.exists(), "no field is ever written if any one of them fails validation");
    }

    #[test]
    fn update_persists_and_round_trips() {
        let p = tmp_settings_path("roundtrip");
        update_at(&p, &Update { notifications: Some("high".into()), generic_text: Some("false".into()), quiet_start: Some("1320".into()), quiet_end: Some("420".into()), ..Update::default() }).unwrap();
        let s = load_at(&p);
        assert_eq!((s.notifications, s.generic_text, s.quiet_start, s.quiet_end), (Level::High, false, Some(1320), Some(420)));
    }

    #[test]
    fn turning_notifications_on_from_off_resets_seeded() {
        let p = tmp_settings_path("reseed");
        save_at(&p, &Settings { seeded: true, ..Settings::default() }).unwrap(); // off, already seeded from a prior run
        let s = update_at(&p, &Update { notifications: Some("high".into()), ..Update::default() }).unwrap();
        assert!(!s.seeded, "moving off -> high must reseed, so the next dispatch doesn't fire a backlog");
    }

    #[test]
    fn staying_on_does_not_reset_seeded() {
        let p = tmp_settings_path("stay-on");
        save_at(&p, &Settings { notifications: Level::High, seeded: true, ..Settings::default() }).unwrap();
        let s = update_at(&p, &Update { notifications: Some("all".into()), ..Update::default() }).unwrap();
        assert!(s.seeded, "high -> all is not off -> on, so the existing seed must be kept");
    }

    #[test]
    fn metrics_history_defaults_off_and_can_be_turned_on() {
        assert!(!Settings::default().metrics_history);
        let p = tmp_settings_path("metrics-history");
        let s = update_at(&p, &Update { metrics_history: Some("true".into()), ..Update::default() }).unwrap();
        assert!(s.metrics_history);
        let s = update_at(&p, &Update { metrics_history: Some("false".into()), ..Update::default() }).unwrap();
        assert!(!s.metrics_history);
        assert_eq!(update_at(&p, &Update { metrics_history: Some("maybe".into()), ..Update::default() }).unwrap_err(), "metrics_history must be true or false");
    }

    #[test]
    fn disk_writes_history_defaults_off_and_can_be_turned_on_and_off() {
        assert!(!Settings::default().disk_writes_history);
        let p = tmp_settings_path("disk-writes-history");
        let on = |v: &str| Update { disk_writes_history: Some(v.into()), ..Update::default() };
        assert!(update_at(&p, &on("true")).unwrap().disk_writes_history);
        assert!(load_at(&p).disk_writes_history, "the change is saved, not only returned");
        assert!(!update_at(&p, &on("false")).unwrap().disk_writes_history);
        assert_eq!(update_at(&p, &on("yes")).unwrap_err(), "disk_writes_history must be true or false");
    }

    #[test]
    fn agent_enabled_defaults_off_and_set_agent_enabled_flips_only_that_field() {
        assert!(!Settings::default().agent_enabled);
        let p = tmp_settings_path("agent-enabled");
        update_at(&p, &Update { notifications: Some("high".into()), ..Update::default() }).unwrap(); // some other field already set
        set_agent_enabled_at(&p, true).unwrap();
        let s = load_at(&p);
        assert!(s.agent_enabled);
        assert_eq!(s.notifications, Level::High, "flipping the kill switch must not disturb other settings");
        set_agent_enabled_at(&p, false).unwrap();
        assert!(!load_at(&p).agent_enabled);
    }

    #[test]
    fn a_new_settings_file_and_its_folder_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let p = tmp_settings_path("modes");
        save_at(&p, &Settings::default()).unwrap();
        assert_eq!(fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(fs::metadata(p.parent().unwrap()).unwrap().permissions().mode() & 0o777, 0o700);
    }
}
