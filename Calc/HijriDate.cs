using System;
using System.Globalization;

namespace PrayerTray.Calc;

/// <summary>Gregorian → Hijri (Umm al-Qura) conversion, fully offline. UmAlQuraCalendar is managed BCL
/// arithmetic (no ICU), so it works under InvariantGlobalization. adjustDays shifts for moon sighting.</summary>
public static class HijriDate
{
    static readonly UmAlQuraCalendar Cal = new();

    public static (int year, int month, int day) Convert(DateTime date, int adjustDays = 0)
    {
        var d = date.Date.AddDays(adjustDays);
        if (d < Cal.MinSupportedDateTime) d = Cal.MinSupportedDateTime;
        else if (d > Cal.MaxSupportedDateTime) d = Cal.MaxSupportedDateTime;
        return (Cal.GetYear(d), Cal.GetMonth(d), Cal.GetDayOfMonth(d));
    }
}
