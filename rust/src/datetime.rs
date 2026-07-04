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

#[cfg(test)]
mod tests {
    use super::*;

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
