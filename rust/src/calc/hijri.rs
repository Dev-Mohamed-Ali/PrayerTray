//! Gregorian → Hijri (Umm al-Qura), table-generated from .NET UmAlQuraCalendar so dates
//! match what the C# build shows. adjust_days shifts for moon sighting; input clamps to range.

use super::umalqura_data::{MAX_RD, MIN_RD, MIN_YEAR, YEARS};
use crate::datetime::Date;

pub fn convert(date: Date, adjust_days: i32) -> (i32, u32, u32) {
    let rd = (date.to_rd() + adjust_days as i64).clamp(MIN_RD, MAX_RD);

    // Last year whose start is <= rd.
    let idx = match YEARS.binary_search_by(|(start, _)| start.cmp(&rd)) {
        Ok(i) => i,
        Err(i) => i - 1, // i >= 1 because rd >= MIN_RD = YEARS[0].0
    };
    let (start, lens) = &YEARS[idx];
    let mut days = rd - start;
    for (m, &len) in lens.iter().enumerate() {
        if days < len as i64 {
            return (MIN_YEAR + idx as i32, m as u32 + 1, days as u32 + 1);
        }
        days -= len as i64;
    }
    // rd beyond the table's last day only when rd == MAX_RD in the final year; clamp to its end.
    (MIN_YEAR + idx as i32, 12, lens[11] as u32)
}
