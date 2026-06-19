using System;
using System.Collections.Generic;

namespace PrayerTray.Calc;

public enum AsrJuristic { Standard = 1, Hanafi = 2 }

public record CalcMethod(string Name, double FajrAngle, double IshaAngle, double IshaMinutes)
{
    // IshaMinutes > 0 means isha is a fixed offset after maghrib (Makkah), not an angle.
    public static readonly Dictionary<string, CalcMethod> All = new()
    {
        ["MWL"]     = new("Muslim World League", 18, 17, 0),
        ["ISNA"]    = new("Islamic Society of North America", 15, 15, 0),
        ["Egypt"]   = new("Egyptian General Authority", 19.5, 17.5, 0),
        ["Makkah"]  = new("Umm al-Qura, Makkah", 18.5, 0, 90),
        ["Karachi"] = new("Univ. of Islamic Sciences, Karachi", 18, 18, 0),
    };
}

/// <summary>Computes the five daily prayer times plus sunrise for a given date/location, fully offline.</summary>
public static class PrayerCalculator
{
    const double RiseSetAngle = 0.833; // sun altitude at sunrise/sunset incl. refraction

    public static Dictionary<string, TimeSpan> Compute(
        DateTime date, double lat, double lng, double tzHours, CalcMethod method, AsrJuristic asr)
    {
        double jDate = Julian(date.Year, date.Month, date.Day) - lng / (15.0 * 24.0);

        // initial guesses (hours), refined by one iteration
        var t = new Dictionary<string, double>
        {
            ["fajr"] = 5, ["sunrise"] = 6, ["dhuhr"] = 12,
            ["asr"] = 13, ["maghrib"] = 18, ["isha"] = 18,
        };

        for (int i = 0; i < 2; i++)
        {
            t["fajr"]    = SunAngleTime(jDate, lat, method.FajrAngle, t["fajr"] / 24, ccw: true);
            t["sunrise"] = SunAngleTime(jDate, lat, RiseSetAngle, t["sunrise"] / 24, ccw: true);
            t["dhuhr"]   = MidDay(jDate, t["dhuhr"] / 24);
            t["asr"]     = AsrTime(jDate, lat, (int)asr, t["asr"] / 24);
            t["maghrib"] = SunAngleTime(jDate, lat, RiseSetAngle, t["maghrib"] / 24, ccw: false);
            t["isha"]    = method.IshaMinutes > 0
                ? t["maghrib"] + method.IshaMinutes / 60.0
                : SunAngleTime(jDate, lat, method.IshaAngle, t["isha"] / 24, ccw: false);
        }

        // High-latitude safety: near the poles in summer the sun never reaches the fajr/isha
        // angle, so ArcCos yields NaN. Fall back to the angle-based night-portion method.
        double night = FixHour(t["sunrise"] - t["maghrib"]);
        t["fajr"] = AdjustHighLat(t["fajr"], t["sunrise"], method.FajrAngle, night, ccw: true);
        if (method.IshaMinutes <= 0)
            t["isha"] = AdjustHighLat(t["isha"], t["maghrib"], method.IshaAngle, night, ccw: false);

        var result = new Dictionary<string, TimeSpan>();
        double adjust = tzHours - lng / 15.0;
        foreach (var (k, v) in t)
            result[k] = ToTimeSpan(v + adjust);
        return result;
    }

    static double AdjustHighLat(double time, double baseTime, double angle, double night, bool ccw)
    {
        double portion = (angle / 60.0) * night; // angle-based night fraction
        double diff = ccw ? FixHour(baseTime - time) : FixHour(time - baseTime);
        if (double.IsNaN(time) || diff > portion)
            time = baseTime + (ccw ? -portion : portion);
        return time;
    }

    static TimeSpan ToTimeSpan(double hours)
    {
        if (double.IsNaN(hours)) return TimeSpan.Zero;
        hours = FixHour(hours);
        int h = (int)Math.Floor(hours);
        int m = (int)Math.Round((hours - h) * 60);
        if (m == 60) { m = 0; h = (h + 1) % 24; }
        return new TimeSpan(h, m, 0);
    }

    // --- astronomy ---
    static double Julian(int year, int month, int day)
    {
        if (month <= 2) { year -= 1; month += 12; }
        double a = Math.Floor(year / 100.0);
        double b = 2 - a + Math.Floor(a / 4);
        return Math.Floor(365.25 * (year + 4716)) + Math.Floor(30.6001 * (month + 1)) + day + b - 1524.5;
    }

    static (double decl, double eqt) SunPosition(double jd)
    {
        double d = jd - 2451545.0;
        double g = FixAngle(357.529 + 0.98560028 * d);
        double q = FixAngle(280.459 + 0.98564736 * d);
        double l = FixAngle(q + 1.915 * Sin(g) + 0.020 * Sin(2 * g));
        double e = 23.439 - 0.00000036 * d;
        double decl = ArcSin(Sin(e) * Sin(l));
        double ra = FixHour(ArcTan2(Cos(e) * Sin(l), Cos(l)) / 15);
        double eqt = q / 15 - ra;
        return (decl, eqt);
    }

    static double MidDay(double jDate, double t)
    {
        double eqt = SunPosition(jDate + t).eqt;
        return FixHour(12 - eqt);
    }

    static double SunAngleTime(double jDate, double lat, double angle, double t, bool ccw)
    {
        double decl = SunPosition(jDate + t).decl;
        double noon = MidDay(jDate, t);
        double inner = (-Sin(angle) - Sin(decl) * Sin(lat)) / (Cos(decl) * Cos(lat));
        double v = (1.0 / 15.0) * ArcCos(inner);
        return noon + (ccw ? -v : v);
    }

    static double AsrTime(double jDate, double lat, double factor, double t)
    {
        double decl = SunPosition(jDate + t).decl;
        double angle = -ArcCot(factor + Tan(Math.Abs(lat - decl)));
        return SunAngleTime(jDate, lat, angle, t, ccw: false);
    }

    // --- degree-based trig helpers ---
    static double Dtr(double d) => d * Math.PI / 180.0;
    static double Rtd(double r) => r * 180.0 / Math.PI;
    static double Sin(double d) => Math.Sin(Dtr(d));
    static double Cos(double d) => Math.Cos(Dtr(d));
    static double Tan(double d) => Math.Tan(Dtr(d));
    static double ArcSin(double x) => Rtd(Math.Asin(x));
    static double ArcCos(double x) => Rtd(Math.Acos(x));
    static double ArcCot(double x) => Rtd(Math.Atan(1.0 / x));
    static double ArcTan2(double y, double x) => Rtd(Math.Atan2(y, x));
    static double FixAngle(double a) => Fix(a, 360);
    static double FixHour(double a) => Fix(a, 24);
    static double Fix(double a, double b)
    {
        a -= b * Math.Floor(a / b);
        return a < 0 ? a + b : a;
    }
}
