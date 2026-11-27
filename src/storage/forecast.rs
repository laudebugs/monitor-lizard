//! Capacity history and the "full in about N days" forecast. One sample a day (used, purgeable and
//! free bytes, nothing about apps or folders), a straight-line fit over the last 14 days, and the
//! reply for `GET /api/storage/history`. Pure: the database lives in `history.rs`.
use serde::Serialize;
use serde_json::{json, Value};

/// Samples older than this are deleted and never returned.
pub const RETENTION_DAYS: i64 = 90;
/// The fit looks at this many days back from today.
pub const WINDOW_DAYS: i64 = 14;
/// Fewer samples than this in the fit: no forecast yet.
pub const MIN_DAYS: usize = 7;
/// A one-day fall in used space bigger than this share of the disk is a clean-up, not a trend.
const DROP_SHARE: f64 = 0.05;

/// One day's sample. `day` counts local calendar days since 1970-01-01; `ts` is unix seconds.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Day { pub day: i64, pub ts: u64, pub used: u64, pub purgeable: u64, pub free: u64 }

impl Day {
    /// Used space counting what macOS would reclaim: the line the disk fills along.
    pub fn occupied(&self) -> u64 { self.used.saturating_add(self.purgeable) }
    /// `free` is total minus occupied, so the three add up to the disk.
    pub fn total(&self) -> u64 { self.occupied().saturating_add(self.free) }
}

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Forecast {
    /// Fewer than `MIN_DAYS` samples in the fit.
    Collecting { days_have: usize, days_needed: usize },
    /// Usage is flat or shrinking.
    NotGrowing { fit_days: usize },
    /// `days_*` are counted from the last sample; `days_high` is null when the slope's lower bound is not
    /// growing, so the range has no upper end.
    Growing { fit_days: usize, bytes_per_day: f64, days_until_full: f64, days_low: f64, days_high: Option<f64> },
}

/// The days the fit may use: the last `WINDOW_DAYS` up to today (a sample dated after today came from a
/// clock that has since moved back), and only those after the latest large drop.
fn fit_days(days: &[Day], today: i64) -> Vec<Day> {
    let mut window: Vec<Day> = days.iter().copied().filter(|d| d.day <= today && d.day > today - WINDOW_DAYS).collect();
    window.sort_by_key(|d| d.day);
    let cut = (1..window.len()).rev().find(|&i| {
        let (before, after) = (window[i - 1], window[i]);
        before.occupied().saturating_sub(after.occupied()) as f64 > DROP_SHARE * before.total() as f64
    });
    window.split_off(cut.unwrap_or(0))
}

pub fn forecast(days: &[Day], today: i64) -> Forecast {
    let fit = fit_days(days, today);
    let n = fit.len();
    if n < MIN_DAYS { return Forecast::Collecting { days_have: n, days_needed: MIN_DAYS }; }
    let x0 = fit[0].day;
    let pts: Vec<(f64, f64)> = fit.iter().map(|d| ((d.day - x0) as f64, d.occupied() as f64)).collect();
    let nf = n as f64;
    let (mx, my) = (pts.iter().map(|p| p.0).sum::<f64>() / nf, pts.iter().map(|p| p.1).sum::<f64>() / nf);
    let sxx: f64 = pts.iter().map(|p| (p.0 - mx).powi(2)).sum();
    let sxy: f64 = pts.iter().map(|p| (p.0 - mx) * (p.1 - my)).sum();
    let slope = sxy / sxx;
    if !slope.is_finite() || slope <= 0.0 { return Forecast::NotGrowing { fit_days: n }; }
    let intercept = my - slope * mx;
    let sse: f64 = pts.iter().map(|p| (p.1 - (intercept + slope * p.0)).powi(2)).sum();
    let se = (sse / (nf - 2.0) / sxx).sqrt();
    let last = fit[n - 1];
    // From the fitted value, not the last raw sample, so one noisy day does not shift the whole range.
    let remaining = (last.total() as f64 - (intercept + slope * pts[n - 1].0)).max(0.0);
    let days_high = (slope - se > 0.0).then(|| remaining / (slope - se));
    Forecast::Growing { fit_days: n, bytes_per_day: slope, days_until_full: remaining / slope, days_low: remaining / (slope + se), days_high }
}

/// Change in occupied bytes over about the last week: the latest sample against the oldest one within
/// 7 days of it. None until the samples span at least 6 days.
pub fn week_change(days: &[Day], today: i64) -> Option<i64> {
    let mut v: Vec<Day> = days.iter().copied().filter(|d| d.day <= today).collect();
    v.sort_by_key(|d| d.day);
    let last = *v.last()?;
    let first = *v.iter().find(|d| d.day >= last.day - 7)?;
    (last.day - first.day >= 6).then(|| last.occupied() as i64 - first.occupied() as i64)
}

/// The reply for `GET /api/storage/history`. A database that cannot be opened is a 200 with
/// `available: false`, so the page keeps showing the current capacity and says the history is missing.
pub fn reply(series: Result<Vec<Day>, String>, today: i64) -> Value {
    match series {
        Err(reason) => json!({ "available": false, "reason": reason, "retention_days": RETENTION_DAYS, "window_days": WINDOW_DAYS, "samples": [], "total": null, "week_change_bytes": null, "forecast": { "status": "unavailable" } }),
        Ok(mut days) => {
            days.sort_by_key(|d| d.day);
            let total = days.iter().rev().find(|d| d.day <= today).map(Day::total);
            json!({ "available": true, "reason": null, "retention_days": RETENTION_DAYS, "window_days": WINDOW_DAYS, "today": today, "samples": days,
                "total": total, "week_change_bytes": week_change(&days, today), "forecast": forecast(&days, today) })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const GB: u64 = 1_000_000_000;
    const TOTAL: u64 = 1000 * GB;

    fn day(n: i64, occupied_gb: f64) -> Day {
        let occupied = (occupied_gb * GB as f64) as u64;
        Day { day: n, ts: (n as u64) * 86_400 + 43_200, used: occupied * 9 / 10, purgeable: occupied - occupied * 9 / 10, free: TOTAL - occupied }
    }
    /// `count` days ending at `last`, growing `per_day` GB a day from `start_gb`.
    fn series(last: i64, count: i64, start_gb: f64, per_day: f64) -> Vec<Day> {
        (0..count).map(|i| day(last - count + 1 + i, start_gb + per_day * i as f64)).collect()
    }
    fn growing(f: &Forecast) -> (f64, f64, f64, Option<f64>) {
        match f { Forecast::Growing { bytes_per_day, days_until_full, days_low, days_high, .. } => (*bytes_per_day, *days_until_full, *days_low, *days_high), other => panic!("not growing: {other:?}") }
    }

    #[test]
    fn a_perfect_line_gives_its_slope_and_the_days_left() {
        let s = series(100, 10, 700.0, 10.0); // ends at 790 GB, 210 GB free
        let (slope, days, low, high) = growing(&forecast(&s, 100));
        assert!((slope - 10.0 * GB as f64).abs() < 1e3, "{slope}");
        assert!((days - 21.0).abs() < 1e-6, "{days}");
        assert!((low - 21.0).abs() < 1e-6 && (high.unwrap() - 21.0).abs() < 1e-6, "no scatter, no range");
    }

    #[test]
    fn fewer_than_seven_days_is_collecting_with_no_forecast() {
        let s = series(100, 6, 700.0, 10.0);
        assert_eq!(forecast(&s, 100), Forecast::Collecting { days_have: 6, days_needed: 7 });
        assert_eq!(forecast(&[], 100), Forecast::Collecting { days_have: 0, days_needed: 7 });
        assert!(matches!(forecast(&series(100, 7, 700.0, 10.0), 100), Forecast::Growing { .. }), "seven days is enough");
    }

    #[test]
    fn days_with_gaps_count_by_samples_not_by_span() {
        // Seven samples spread over 13 days still fit; six do not.
        let gappy: Vec<Day> = [88, 90, 92, 94, 96, 98, 100].iter().enumerate().map(|(i, d)| day(*d, 700.0 + 10.0 * i as f64)).collect();
        assert!(matches!(forecast(&gappy, 100), Forecast::Growing { .. }));
        assert!(matches!(forecast(&gappy[1..], 100), Forecast::Collecting { days_have: 6, .. }));
    }

    #[test]
    fn flat_usage_is_not_growing() {
        assert_eq!(forecast(&series(100, 10, 700.0, 0.0), 100), Forecast::NotGrowing { fit_days: 10 });
    }

    #[test]
    fn shrinking_usage_is_not_growing_and_has_no_days_figure() {
        let f = forecast(&series(100, 10, 700.0, -3.0), 100);
        assert_eq!(f, Forecast::NotGrowing { fit_days: 10 });
        assert!(!serde_json::to_string(&f).unwrap().contains("days_until_full"));
    }

    #[test]
    fn a_large_one_day_drop_makes_the_fit_use_only_the_days_after_it() {
        // 14 days: growth, a 200 GB clean-up on day 94, then 10 GB a day again.
        let mut s = series(93, 8, 700.0, 10.0); // days 86..=93, 700..770
        s.extend(series(100, 7, 570.0, 10.0));  // days 94..=100, 570..630
        let (slope, days, _, _) = growing(&forecast(&s, 100));
        assert!((slope - 10.0 * GB as f64).abs() < 1e3, "the drop must not flatten the line: {slope}");
        assert!((days - 37.0).abs() < 1e-6, "{days}");
        // Only the after-drop days count, so three of them is not enough.
        let few: Vec<Day> = series(93, 11, 700.0, 10.0).into_iter().chain(series(96, 3, 500.0, 10.0).into_iter().map(|d| Day { day: d.day + 4, ..d })).collect();
        assert!(matches!(forecast(&few, 100), Forecast::Collecting { .. }), "{:?}", forecast(&few, 100));
    }

    #[test]
    fn a_small_dip_is_not_a_clean_up() {
        let mut s = series(100, 10, 700.0, 10.0);
        s[5] = day(s[5].day, 730.0); // 20 GB under the line, 2% of the disk
        assert!(matches!(forecast(&s, 100), Forecast::Growing { fit_days: 10, .. }));
    }

    #[test]
    fn scatter_widens_the_range_and_a_noisy_flat_line_has_no_upper_end() {
        let noisy: Vec<Day> = (0..14).map(|i| day(87 + i, 700.0 + 10.0 * i as f64 + if i % 2 == 0 { 8.0 } else { -8.0 })).collect();
        let (_, days, low, high) = growing(&forecast(&noisy, 100));
        assert!(low < days && days < high.unwrap(), "{low} {days} {high:?}");
        // Barely rising and very noisy: the slope could be zero, so the range has no upper end.
        let flat: Vec<Day> = (0..14).map(|i| day(87 + i, 700.0 + 0.2 * i as f64 + if i % 2 == 0 { 15.0 } else { -15.0 })).collect();
        match forecast(&flat, 100) {
            Forecast::Growing { days_high, .. } => assert_eq!(days_high, None),
            Forecast::NotGrowing { .. } => {}
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn only_the_last_fourteen_days_are_fitted() {
        let mut s = series(100, 14, 700.0, 10.0);
        s.extend((0..20).map(|i| day(60 + i, 100.0 + 40.0 * i as f64 * 0.1))); // old, steep, ignored
        let (slope, ..) = growing(&forecast(&s, 100));
        assert!((slope - 10.0 * GB as f64).abs() < 1e3, "{slope}");
        let f = forecast(&s, 100);
        assert!(matches!(f, Forecast::Growing { fit_days: 14, .. }), "{f:?}");
    }

    #[test]
    fn samples_dated_after_today_are_ignored_when_the_clock_has_moved_back() {
        let mut s = series(100, 10, 700.0, 10.0);
        s.extend(series(110, 10, 100.0, 0.0)); // written by a clock that was ahead
        let f = forecast(&s, 100);
        assert!(matches!(f, Forecast::Growing { fit_days: 10, .. }), "{f:?}");
    }

    #[test]
    fn a_full_disk_reports_zero_days() {
        let s: Vec<Day> = (0..8).map(|i| Day { day: 93 + i, ts: 0, used: (1000 + i as u64) * GB, purgeable: 0, free: 0 }).collect();
        let (_, days, ..) = growing(&forecast(&s, 100));
        assert!(days.abs() < 1e-6, "{days}");
    }

    #[test]
    fn week_change_compares_the_latest_day_with_one_a_week_earlier() {
        let s = series(100, 10, 700.0, 2.0);
        assert_eq!(week_change(&s, 100), Some(14 * GB as i64));
        assert_eq!(week_change(&series(100, 5, 700.0, 2.0), 100), None, "under six days apart");
        assert_eq!(week_change(&series(100, 10, 700.0, -1.0), 100), Some(-7 * GB as i64), "a negative change is reported as one");
    }

    #[test]
    fn the_reply_carries_the_series_the_forecast_and_the_total() {
        let v = reply(Ok(series(100, 8, 700.0, 10.0)), 100);
        assert_eq!(v["available"], true);
        assert_eq!(v["samples"].as_array().unwrap().len(), 8);
        assert_eq!(v["forecast"]["status"], "growing");
        assert_eq!(v["total"], TOTAL);
        for k in ["day", "ts", "used", "purgeable", "free"] { assert!(v["samples"][0].get(k).is_some(), "{k}"); }
        assert_eq!(v["samples"][0].as_object().unwrap().len(), 5, "only capacity numbers, nothing about apps or folders");
    }

    #[test]
    fn an_unavailable_database_says_so_and_gives_no_forecast() {
        let v = reply(Err("history unavailable: disk I/O error".into()), 100);
        assert_eq!((v["available"].clone(), v["forecast"]["status"].clone(), v["samples"].clone()), (false.into(), "unavailable".into(), json!([])));
        assert!(v["reason"].as_str().unwrap().contains("unavailable"));
    }
}
