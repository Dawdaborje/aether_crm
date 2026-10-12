//! Business-time deadline from first_response_hours + optional holiday calendar.

use aether_sdk::dates::{
    format_date, format_datetime, parse_date, parse_datetime, weekdays, Calendar, Duration, NaiveDateTime,
    Timelike, Weekday,
};
use aether_sdk::decimal::Decimal;
use aether_sdk::prelude::*;

use crate::common::text;

/// Eight-hour business day window used when a holiday calendar is attached.
const DAY_START_HOUR: u32 = 9;
const DAY_END_HOUR: u32 = 17;
const BUSINESS_MINUTES_PER_DAY: i64 = (DAY_END_HOUR - DAY_START_HOUR) as i64 * 60;

fn hours_to_minutes(hours: &Decimal) -> Result<i64> {
    if hours.is_negative() {
        return Err(Error::msg("first_response_hours must be zero or more"));
    }
    let minutes = hours.times(Decimal::whole(60, 0)?, 0)?;
    minutes
        .to_string()
        .parse::<i64>()
        .map_err(|_| Error::msg("first_response_hours is not a number"))
}

fn load_calendar(calendar_id: &str, from: aether_sdk::dates::NaiveDate, to: aether_sdk::dates::NaiveDate) -> Result<Calendar> {
    let span: Value = plugins::call(
        "calendar",
        "calendar_span",
        &json!({
            "calendar": calendar_id,
            "from": format_date(from),
            "to": format_date(to),
        }),
    )?;
    let weekend_names: Vec<String> = span
        .get("weekend")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
        .unwrap_or_else(|| vec!["sat".into(), "sun".into()]);
    let weekend = weekdays(&weekend_names).unwrap_or_else(|_| vec![Weekday::Sat, Weekday::Sun]);
    let mut holidays = Vec::new();
    if let Some(rows) = span.get("holidays").and_then(Value::as_array) {
        for row in rows {
            if text(row, "kind") == Some("optional") {
                continue;
            }
            if row.get("is_half_day") == Some(&json!(true)) {
                continue;
            }
            if let Some(date) = text(row, "date").and_then(|d| parse_date(d).ok()) {
                holidays.push(date);
            }
        }
    }
    Ok(Calendar { weekend, holidays })
}

fn snap_into_window(cal: &Calendar, mut moment: NaiveDateTime) -> NaiveDateTime {
    loop {
        let day = moment.date();
        if !cal.is_working_day(day) {
            let next = cal.next_working_day(day + Duration::days(1));
            moment = next.and_hms_opt(DAY_START_HOUR, 0, 0).unwrap_or(moment);
            continue;
        }
        let hour = moment.hour();
        if hour < DAY_START_HOUR {
            moment = day.and_hms_opt(DAY_START_HOUR, 0, 0).unwrap_or(moment);
            return moment;
        }
        if hour >= DAY_END_HOUR {
            let next = cal.next_working_day(day + Duration::days(1));
            moment = next.and_hms_opt(DAY_START_HOUR, 0, 0).unwrap_or(moment);
            continue;
        }
        return moment;
    }
}

fn add_business_minutes(cal: &Calendar, start: NaiveDateTime, mut minutes: i64) -> NaiveDateTime {
    let mut moment = snap_into_window(cal, start);
    while minutes > 0 {
        let day = moment.date();
        let end_of_day = day.and_hms_opt(DAY_END_HOUR, 0, 0).unwrap_or(moment);
        let available = (end_of_day - moment).num_minutes().max(0);
        if minutes <= available {
            return moment + Duration::minutes(minutes);
        }
        minutes -= available;
        let next = cal.next_working_day(day + Duration::days(1));
        moment = next.and_hms_opt(DAY_START_HOUR, 0, 0).unwrap_or(moment);
    }
    moment
}

/// Compute `response_by` from now and policy hours.
pub fn response_deadline(now_text: &str, hours: &Decimal, calendar_id: Option<&str>) -> Result<String> {
    let minutes = hours_to_minutes(hours)?;
    let now = parse_datetime(now_text)?;
    if minutes == 0 {
        return Ok(format_datetime(now));
    }
    if let Some(calendar_id) = calendar_id.filter(|c| !c.is_empty()) {
        let from = now.date();
        let days_ahead = (minutes / BUSINESS_MINUTES_PER_DAY) + 60;
        let to = from + Duration::days(days_ahead);
        let cal = load_calendar(calendar_id, from, to)?;
        return Ok(format_datetime(add_business_minutes(&cal, now, minutes)));
    }
    Ok(format_datetime(now + Duration::minutes(minutes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_sdk::dates::{NaiveDate, Weekday};

    #[test]
    fn wall_clock_deadline_adds_hours() {
        let hours = Decimal::parse("2").unwrap();
        let by = response_deadline("2026-10-05T10:00:00Z", &hours, None).unwrap();
        assert_eq!(by, "2026-10-05T12:00:00Z");
    }

    #[test]
    fn zero_hours_is_immediate() {
        let hours = Decimal::zero(2);
        let by = response_deadline("2026-10-05T10:00:00Z", &hours, None).unwrap();
        assert_eq!(by, "2026-10-05T10:00:00Z");
    }

    #[test]
    fn negative_hours_refused() {
        let hours = Decimal::parse("-1").unwrap();
        assert!(response_deadline("2026-10-05T10:00:00Z", &hours, None).is_err());
    }

    #[test]
    fn business_minutes_skip_weekend() {
        // Friday 16:00 + 2h business → Monday 10:00 (1h Fri + 1h Mon).
        let cal = Calendar { weekend: vec![Weekday::Sat, Weekday::Sun], holidays: Vec::new() };
        let friday = NaiveDate::from_ymd_opt(2026, 10, 9).unwrap().and_hms_opt(16, 0, 0).unwrap();
        let by = add_business_minutes(&cal, friday, 120);
        assert_eq!(format_datetime(by), "2026-10-12T10:00:00Z");
    }

    #[test]
    fn business_minutes_skip_holiday() {
        let holiday = NaiveDate::from_ymd_opt(2026, 10, 12).unwrap(); // Monday
        let cal = Calendar {
            weekend: vec![Weekday::Sat, Weekday::Sun],
            holidays: vec![holiday],
        };
        let friday = NaiveDate::from_ymd_opt(2026, 10, 9).unwrap().and_hms_opt(16, 0, 0).unwrap();
        let by = add_business_minutes(&cal, friday, 120);
        // Fri 16→17 (1h), Mon holiday, Tue 09→10 (1h)
        assert_eq!(format_datetime(by), "2026-10-13T10:00:00Z");
    }
}
