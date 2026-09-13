//! Civil-date helpers (Howard Hinnant's algorithms). Rata Die: day 1 = 0001-01-01.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl Date {
    pub fn new(year: i32, month: u32, day: u32) -> Self {
        Self { year, month, day }
    }

    /// Days since 0001-01-01 (= day 1), matching .NET DateTime day arithmetic.
    pub fn to_rd(self) -> i64 {
        let y = if self.month <= 2 { self.year - 1 } else { self.year } as i64;
        let era = y.div_euclid(400);
        let yoe = y - era * 400;
        let mp = (self.month as i64 + 9) % 12;
        let doy = (153 * mp + 2) / 5 + self.day as i64 - 1;
        let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
        era * 146097 + doe - 305 // -306 civil epoch shift, +1 for 1-based Rata Die
    }

    pub fn from_rd(rd: i64) -> Self {
        let z = rd + 305;
        let era = z.div_euclid(146097);
        let doe = z - era * 146097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let year = (era * 400 + yoe + if month <= 2 { 1 } else { 0 }) as i32;
        Self { year, month, day }
    }

    pub fn add_days(self, days: i64) -> Self {
        Self::from_rd(self.to_rd() + days)
    }

    /// 0 = Sunday .. 6 = Saturday (matches .NET DayOfWeek).
    pub fn weekday(self) -> u32 {
        (self.to_rd() % 7) as u32
    }
}

/// Length of `month`, leap years included (the next month's first day, minus this one's).
pub fn days_in_month(year: i32, month: u32) -> u32 {
    let (ny, nm) = if month == 12 { (year + 1, 1) } else { (year, month + 1) };
    (Date::new(ny, nm, 1).to_rd() - Date::new(year, month, 1).to_rd()) as u32
}

/// The most recent `cycle_day` on or before `today`, clamped to months that are too short.
pub fn cycle_start(today: Date, cycle_day: u32) -> Date {
    let want = cycle_day.clamp(1, 31);
    let here = Date::new(today.year, today.month, want.min(days_in_month(today.year, today.month)));
    if here.to_rd() <= today.to_rd() {
        return here;
    }
    let (y, m) = if today.month == 1 { (today.year - 1, 12) } else { (today.year, today.month - 1) };
    Date::new(y, m, want.min(days_in_month(y, m)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_lengths_follow_the_leap_rule() {
        assert_eq!(days_in_month(2026, 2), 28);
        assert_eq!(days_in_month(2024, 2), 29);
        assert_eq!(days_in_month(2000, 2), 29); // divisible by 400
        assert_eq!(days_in_month(1900, 2), 28); // divisible by 100, not 400
        assert_eq!(days_in_month(2026, 12), 31);
    }

    #[test]
    fn cycle_start_is_the_most_recent_billing_day() {
        // Before this month's day -> last month's.
        assert_eq!(cycle_start(Date::new(2026, 9, 13), 15), Date::new(2026, 8, 15));
        // On or after it -> this month's.
        assert_eq!(cycle_start(Date::new(2026, 9, 15), 15), Date::new(2026, 9, 15));
        assert_eq!(cycle_start(Date::new(2026, 9, 20), 15), Date::new(2026, 9, 15));
        // Day 1 is the plain calendar month.
        assert_eq!(cycle_start(Date::new(2026, 9, 13), 1), Date::new(2026, 9, 1));
    }

    #[test]
    fn cycle_start_clamps_short_months_and_crosses_the_year() {
        // 31 in a 31-day month stays put; the month before is February.
        assert_eq!(cycle_start(Date::new(2026, 3, 31), 31), Date::new(2026, 3, 31));
        assert_eq!(cycle_start(Date::new(2026, 3, 5), 31), Date::new(2026, 2, 28));
        assert_eq!(cycle_start(Date::new(2024, 3, 5), 31), Date::new(2024, 2, 29));
        // January reaches back into the previous year.
        assert_eq!(cycle_start(Date::new(2026, 1, 5), 15), Date::new(2025, 12, 15));
        // Out-of-range days are clamped, never panic.
        assert_eq!(cycle_start(Date::new(2026, 9, 13), 99), Date::new(2026, 8, 31));
        assert_eq!(cycle_start(Date::new(2026, 9, 13), 0), Date::new(2026, 9, 1));
    }

    #[test]
    fn rd_roundtrip_and_known_values() {
        assert_eq!(Date::new(1, 1, 1).to_rd(), 1);
        assert_eq!(Date::new(1900, 4, 30).to_rd(), 693715); // UmAlQura min
        assert_eq!(Date::new(2077, 11, 16).to_rd(), 758564); // UmAlQura max
        let mut rd = Date::new(1899, 12, 28).to_rd();
        for _ in 0..70000 {
            assert_eq!(Date::from_rd(rd).to_rd(), rd);
            rd += 1;
        }
    }

    #[test]
    fn weekday_matches_dotnet() {
        assert_eq!(Date::new(2026, 7, 4).weekday(), 6); // Saturday
        assert_eq!(Date::new(2026, 7, 5).weekday(), 0); // Sunday
    }
}
