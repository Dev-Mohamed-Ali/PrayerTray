using System.Globalization;
using System.Text;
using System.Text.Json;
using PrayerTray.Calc;

// Generates, from the real .NET implementations:
//  1) reference_times.json  — prayer-time fixture (cities x methods x asr x highlat x dates)
//  2) hijri_fixture.json    — Gregorian->Hijri samples incl. adjust -2..2 and clamp edges
//  3) umalqura_data.rs      — exact UmAlQuraCalendar year table as Rust source

string outDir = args.Length > 0 ? args[0] : ".";
Directory.CreateDirectory(outDir);

// ---------- 1) prayer times fixture ----------
var cities = new (string name, double lat, double lng, double tz)[]
{
    ("Makkah", 21.4225, 39.8262, 3),
    ("Cairo", 30.0444, 31.2357, 2),
    ("London", 51.5074, -0.1278, 1),
    ("NewYork", 40.7128, -74.0060, -4),
    ("Jakarta", -6.2088, 106.8456, 7),
    ("Karachi", 24.8607, 67.0011, 5),
    ("Tromso", 69.6492, 18.9553, 2),      // high-latitude: midnight sun in June
    ("PuntaArenas", -53.1638, -70.9171, -3), // far south
};
var dates = new (int y, int m, int d)[]
{
    (2026, 1, 15), (2026, 3, 20), (2026, 6, 21), (2026, 9, 23), (2026, 12, 21), (2026, 7, 4),
};
var methods = CalcMethod.All.Keys.ToArray();
var asrs = new[] { AsrJuristic.Standard, AsrJuristic.Hanafi };
var rules = new[] { HighLatRule.None, HighLatRule.MidNight, HighLatRule.OneSeventh, HighLatRule.AngleBased };
var offsets = new Dictionary<string, int> { ["fajr"] = 2, ["dhuhr"] = -3, ["asr"] = 0, ["maghrib"] = 1, ["isha"] = -2 };

var cases = new List<object>();
foreach (var c in cities)
foreach (var dt in dates)
foreach (var mk in methods)
foreach (var asr in asrs)
foreach (var rule in rules)
{
    bool useOff = (dt.d + mk.Length) % 2 == 0; // deterministic mix of with/without offsets
    var t = PrayerCalculator.Compute(new DateTime(dt.y, dt.m, dt.d), c.lat, c.lng, c.tz,
        CalcMethod.All[mk], asr, useOff ? offsets : null, rule);
    cases.Add(new
    {
        city = c.name, lat = c.lat, lng = c.lng, tz = c.tz,
        date = $"{dt.y:D4}-{dt.m:D2}-{dt.d:D2}",
        method = mk, asr = (int)asr, highLat = (int)rule, offsets = useOff,
        times = t.ToDictionary(kv => kv.Key, kv => (int)kv.Value.TotalMinutes),
    });
}
File.WriteAllText(Path.Combine(outDir, "reference_times.json"),
    JsonSerializer.Serialize(cases, new JsonSerializerOptions { WriteIndented = true }));
Console.WriteLine($"reference_times.json: {cases.Count} cases");

// ---------- 2) hijri fixture ----------
var cal = new UmAlQuraCalendar();
var hij = new List<object>();
var rnd = new Random(42);
var min = cal.MinSupportedDateTime.Date;
var max = cal.MaxSupportedDateTime.Date;
for (int i = 0; i < 400; i++)
{
    var d = min.AddDays(rnd.Next((int)(max - min).TotalDays + 1));
    int adj = rnd.Next(-2, 3);
    var a = d.AddDays(adj);
    if (a < min) a = min; else if (a > max) a = max;
    hij.Add(new
    {
        date = d.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture), adjust = adj,
        hy = cal.GetYear(a), hm = cal.GetMonth(a), hd = cal.GetDayOfMonth(a),
    });
}
// clamp edges
foreach (var (d, adj) in new[] { (min, -2), (min, 0), (max, 2), (max, 0), (min.AddDays(1), -2), (max.AddDays(-1), 2) })
{
    var a = d.AddDays(adj);
    if (a < min) a = min; else if (a > max) a = max;
    hij.Add(new
    {
        date = d.ToString("yyyy-MM-dd", CultureInfo.InvariantCulture), adjust = adj,
        hy = cal.GetYear(a), hm = cal.GetMonth(a), hd = cal.GetDayOfMonth(a),
    });
}
File.WriteAllText(Path.Combine(outDir, "hijri_fixture.json"),
    JsonSerializer.Serialize(hij, new JsonSerializerOptions { WriteIndented = true }));
Console.WriteLine($"hijri_fixture.json: {hij.Count} cases");

// ---------- 3) UmAlQura table as Rust ----------
// Rata Die day number (days since 0001-01-01 = day 1) for the start of each Hijri year,
// plus the 12 month lengths; derived by walking the real calendar.
static long Rd(DateTime d) => (long)(d.Date - new DateTime(1, 1, 1)).TotalDays + 1;

int minYear = cal.GetYear(min);
int maxYear = cal.GetYear(max);
var sb = new StringBuilder();
sb.AppendLine("// GENERATED from .NET UmAlQuraCalendar by tools/genfix — do not hand-edit.");
sb.AppendLine("// (rd_of_muharram_1, [12 month lengths]) per Hijri year.");
sb.AppendLine($"pub const MIN_RD: i64 = {Rd(min)}; // {min:yyyy-MM-dd}");
sb.AppendLine($"pub const MAX_RD: i64 = {Rd(max)}; // {max:yyyy-MM-dd}");
sb.AppendLine($"pub const MIN_YEAR: i32 = {minYear};");
sb.AppendLine("#[allow(dead_code)]");
sb.AppendLine($"pub const MAX_YEAR: i32 = {maxYear};");
sb.AppendLine($"pub static YEARS: [(i64, [u8; 12]); {maxYear - minYear + 1}] = [");
for (int y = minYear; y <= maxYear; y++)
{
    var start = cal.ToDateTime(y, 1, 1, 0, 0, 0, 0);
    var lens = new int[12];
    for (int m = 1; m <= 12; m++)
    {
        // last month of the last year may exceed MaxSupportedDateTime; GetDaysInMonth still works
        lens[m - 1] = cal.GetDaysInMonth(y, m);
    }
    sb.AppendLine($"    ({Rd(start)}, [{string.Join(", ", lens)}]), // {y} AH, starts {start:yyyy-MM-dd}");
}
sb.AppendLine("];");
File.WriteAllText(Path.Combine(outDir, "umalqura_data.rs"), sb.ToString());
Console.WriteLine($"umalqura_data.rs: years {minYear}..{maxYear}");
