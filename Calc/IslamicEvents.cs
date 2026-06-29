using System;
using System.Collections.Generic;

namespace PrayerTray.Calc;

/// <summary>Islamic special days derived from the Umm al-Qura Hijri date (offline). Returns stable keys;
/// display names are localized by the string catalog.</summary>
public static class IslamicEvents
{
    static readonly HashSet<string> Major = new()
    {
        "newYear", "ashura", "mawlid", "ramadanStart", "eidFitr", "arafah", "eidAdha",
    };

    /// <summary>Today's event key (a fixed day wins over the monthly white days), or null.</summary>
    public static string? ForDate(DateTime date, int adjust)
    {
        var (_, m, d) = HijriDate.Convert(date, adjust);
        return Fixed(m, d) ?? (d is 13 or 14 or 15 ? "whiteDays" : null);
    }

    /// <summary>The next major event and how many days away (scans ~13 months forward), or null.</summary>
    public static (string key, int days)? NextMajor(DateTime date, int adjust)
    {
        for (int i = 1; i <= 400; i++)
        {
            var (_, m, d) = HijriDate.Convert(date.Date.AddDays(i), adjust);
            var k = Fixed(m, d);
            if (k != null && Major.Contains(k)) return (k, i);
        }
        return null;
    }

    static string? Fixed(int m, int d) => (m, d) switch
    {
        (1, 1) => "newYear",
        (1, 10) => "ashura",
        (3, 12) => "mawlid",
        (7, 27) => "isra",
        (8, 15) => "midShaban",
        (9, 1) => "ramadanStart",
        (9, 27) => "laylatQadr",
        (10, 1) => "eidFitr",
        (12, 9) => "arafah",
        (12, 10) => "eidAdha",
        (12, 11) or (12, 12) or (12, 13) => "tashreeq",
        _ => null,
    };
}
