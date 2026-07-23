use anyhow::{Context, Result, bail};
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use chrono_tz::Tz;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}

impl DateRange {
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Result<Self> {
        if start >= end {
            bail!("start must be earlier than end")
        }
        Ok(Self { start, end })
    }

    pub fn parse(start: &str, end: &str, timezone: Tz) -> Result<Self> {
        Self::new(
            parse_bound(start, timezone).context("invalid start time")?,
            parse_bound(end, timezone).context("invalid end time")?,
        )
    }
}

fn parse_bound(value: &str, timezone: Tz) -> Result<DateTime<Utc>> {
    if let Ok(parsed) = DateTime::parse_from_rfc3339(value) {
        return Ok(parsed.with_timezone(&Utc));
    }

    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .with_context(|| "expected YYYY-MM-DD or RFC 3339")?;
    let local = date
        .and_hms_opt(0, 0, 0)
        .context("date cannot be represented at midnight")?;
    timezone
        .from_local_datetime(&local)
        .single()
        .map(|value| value.with_timezone(&Utc))
        .context("local time is ambiguous or does not exist")
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use chrono_tz::Asia::Seoul;

    use super::DateRange;

    #[test]
    fn parses_local_date_as_seoul_midnight() {
        let range = DateRange::parse("2026-07-01", "2026-07-02", Seoul).unwrap();
        assert_eq!(
            range.start,
            Utc.with_ymd_and_hms(2026, 6, 30, 15, 0, 0).unwrap()
        );
        assert_eq!(
            range.end,
            Utc.with_ymd_and_hms(2026, 7, 1, 15, 0, 0).unwrap()
        );
    }

    #[test]
    fn rejects_reversed_range() {
        assert!(DateRange::parse("2026-07-02", "2026-07-01", Seoul).is_err());
    }
}
