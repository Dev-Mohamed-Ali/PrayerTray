using System;
using System.Collections.Generic;
using System.Windows.Forms;
using PrayerTray.Calc;
using PrayerTray.Native;

namespace PrayerTray.I18n;

/// <summary>Embedded EN/AR string catalog (no .resx, no satellites — works under InvariantGlobalization
/// + single-file). Uninit-safe: defaults to English and never throws.</summary>
internal static class Strings
{
    public enum Language { En, Ar }

    public static Language Lang { get; private set; } = Language.En;
    public static bool IsRtl => Lang == Language.Ar;

    public static MessageBoxOptions MsgOpts =>
        IsRtl ? MessageBoxOptions.RtlReading | MessageBoxOptions.RightAlign : 0;

    public static void Init(string? cfg) => Lang = Resolve(cfg);
    public static void Set(string? cfg) => Lang = Resolve(cfg);

    static Language Resolve(string? cfg) => cfg switch
    {
        "ar" => Language.Ar,
        "en" => Language.En,
        _ => Interop.OsUiIsArabic() ? Language.Ar : Language.En, // "auto"/null/unknown
    };

    public static string T(string key)
    {
        var table = Lang == Language.Ar ? _ar : _en;
        if (table.TryGetValue(key, out var v)) return v;
        return _en.TryGetValue(key, out var en) ? en : key;
    }

    /// <summary>Composite-format lookup: T(key) used as a String.Format template.</summary>
    public static string F(string key, params object[] args) => string.Format(T(key), args);

    public static string Prayer(string key)
    {
        var t = Lang == Language.Ar ? _prayerAr : _prayerEn;
        return t.TryGetValue(key, out var v) ? v : key;
    }

    public static string AmPm(DateTime dt) => dt.Hour < 12 ? T("time.am") : T("time.pm");

    /// <summary>EN uses invariant "dddd, dd MMM"; AR composes from our own day/month tables (Western digits).</summary>
    public static string FormatPopupDate(DateTime d) =>
        Lang == Language.Ar
            ? $"{_arWeekdays[(int)d.DayOfWeek]}، {d.Day:00} {_arMonths[d.Month - 1]}"
            : d.ToString("dddd, dd MMM");

    /// <summary>"7 Ramadan 1448" / "7 رمضان 1448" — Umm al-Qura, Western digits, ±adjust days.</summary>
    public static string FormatHijri(DateTime d, int adjust)
    {
        var (y, m, day) = HijriDate.Convert(d, adjust);
        var months = Lang == Language.Ar ? _hijriAr : _hijriEn;
        return $"{day} {months[m - 1]} {y}";
    }

    static readonly Dictionary<string, string> _prayerEn = new()
    {
        ["fajr"] = "Fajr", ["sunrise"] = "Sunrise", ["dhuhr"] = "Dhuhr",
        ["asr"] = "Asr", ["maghrib"] = "Maghrib", ["isha"] = "Isha",
    };

    static readonly Dictionary<string, string> _prayerAr = new()
    {
        ["fajr"] = "الفجر", ["sunrise"] = "الشروق", ["dhuhr"] = "الظهر",
        ["asr"] = "العصر", ["maghrib"] = "المغرب", ["isha"] = "العشاء",
    };

    // DayOfWeek order: Sunday=0 .. Saturday=6
    static readonly string[] _arWeekdays =
    {
        "الأحد", "الإثنين", "الثلاثاء", "الأربعاء", "الخميس", "الجمعة", "السبت",
    };

    static readonly string[] _arMonths =
    {
        "يناير", "فبراير", "مارس", "أبريل", "مايو", "يونيو",
        "يوليو", "أغسطس", "سبتمبر", "أكتوبر", "نوفمبر", "ديسمبر",
    };

    static readonly string[] _hijriEn =
    {
        "Muharram", "Safar", "Rabi al-Awwal", "Rabi al-Thani", "Jumada al-Awwal", "Jumada al-Thani",
        "Rajab", "Sha'ban", "Ramadan", "Shawwal", "Dhu al-Qi'dah", "Dhu al-Hijjah",
    };

    static readonly string[] _hijriAr =
    {
        "محرم", "صفر", "ربيع الأول", "ربيع الآخر", "جمادى الأولى", "جمادى الآخرة",
        "رجب", "شعبان", "رمضان", "شوال", "ذو القعدة", "ذو الحجة",
    };

    static readonly Dictionary<string, string> _en = new()
    {
        ["app.name"] = "Prayer Tray",
        // tray menu
        ["menu.showTimes"] = "Show times",
        ["menu.refresh"] = "Refresh now",
        ["menu.startup"] = "Start with Windows",
        ["menu.settings"] = "Settings…",
        ["menu.stopSound"] = "Stop sound",
        ["menu.exit"] = "Exit",
        // tray text + countdown
        ["tray.now"] = "Now:",
        ["tray.next"] = "Next:",
        ["tray.in"] = "in",
        ["countdown.now"] = "now",
        ["unit.s"] = "s",
        ["unit.m"] = "m",
        ["time.am"] = "AM",
        ["time.pm"] = "PM",
        ["popup.in"] = "in",
        // balloons
        ["balloon.reminderTitle"] = "Prayer reminder",
        ["balloon.reminderBody"] = "{0} in {1} min ({2})",
        ["balloon.timeTitle"] = "Prayer time",
        ["balloon.timeBody"] = "It is now {0} ({1})",
        // crash
        ["crash.body"] = "Prayer Tray hit an unexpected error and recovered.\n" +
                         "Details were written to %APPDATA%\\PrayerTray\\error.log.",
        // settings — window + buttons
        ["settings.title"] = "Prayer Tray — Settings",
        ["btn.detect"] = "Detect",
        ["btn.openMaps"] = "Open Maps",
        ["btn.set"] = "Set",
        ["btn.test"] = "Test",
        ["btn.stop"] = "Stop",
        ["btn.save"] = "Save",
        ["btn.cancel"] = "Cancel",
        // settings — placeholders
        ["ph.paste"] = "lat, lng or map link",
        ["ph.customFile"] = "custom file",
        // settings — checkboxes
        ["chk.use24"] = "Use 24-hour clock",
        ["chk.hideFs"] = "Hide over fullscreen apps",
        ["chk.remind"] = "Remind me before each prayer",
        ["chk.playSound"] = "Play a sound",
        ["chk.showHijri"] = "Show Hijri date",
        // settings — row labels
        ["label.city"] = "City (label):",
        ["label.lat"] = "Latitude:",
        ["label.lng"] = "Longitude:",
        ["label.pickMap"] = "Pick on map:",
        ["label.pasteResult"] = "Paste result:",
        ["label.method"] = "Method:",
        ["label.asr"] = "Asr:",
        ["label.theme"] = "Theme:",
        ["label.font"] = "Font:",
        ["label.fontSize"] = "Font size:",
        ["label.hijriAdjust"] = "Hijri adjust (days):",
        ["label.widgetSide"] = "Widget side:",
        ["label.widgetGap"] = "Widget gap (px):",
        ["label.monitor"] = "Monitor:",
        ["label.minutesBefore"] = "Minutes before:",
        ["label.sound"] = "Sound:",
        ["label.customFile"] = "Custom file:",
        ["label.azan"] = "Azan:",
        ["label.azanFile"] = "Azan file:",
        ["label.language"] = "Language:",
        // settings — card titles
        ["card.location"] = "Location",
        ["card.calculation"] = "Calculation",
        ["card.appearance"] = "Appearance",
        ["card.notifications"] = "Notifications",
        // settings — combo items
        ["asr.standard"] = "Standard (Shafi'i, Maliki, Hanbali)",
        ["asr.hanafi"] = "Hanafi",
        ["side.right"] = "Right (near the clock)",
        ["side.left"] = "Left (corner)",
        ["monitor.primary"] = " — Primary",
        ["combo.customFile"] = "Custom file…",
        ["azan.off"] = "Off",
        ["sound.chime"] = "Chime",
        ["sound.bell"] = "Bell",
        ["sound.ding"] = "Ding",
        ["sound.beep"] = "Beep",
        ["sound.double"] = "Double beep",
        ["adhan.makkah"] = "Makkah",
        ["adhan.madinah"] = "Madinah",
        ["lang.auto"] = "System default",
        // settings — messages
        ["msg.detectFail"] = "Couldn't detect location — enter it manually.",
        ["msg.detectCaption"] = "Detect location",
        ["msg.pasteFail"] = "Couldn't read coordinates. Right-click your spot in Google Maps, click the\n" +
                            "lat/lng to copy them, and paste here — or paste the map's address-bar URL.",
        ["msg.pasteCaption"] = "Paste result",
        ["msg.latRange"] = "Latitude must be a number between -90 and 90.",
        ["msg.lngRange"] = "Longitude must be a number between -180 and 180.",
        ["msg.azanFile"] = "Pick an azan audio file, or set Azan to Off.",
        ["msg.invalidCaption"] = "Invalid input",
        ["city.myLocation"] = "My location",
    };

    static readonly Dictionary<string, string> _ar = new()
    {
        ["app.name"] = "أوقات الصلاة",
        ["menu.showTimes"] = "عرض الأوقات",
        ["menu.refresh"] = "تحديث الآن",
        ["menu.startup"] = "التشغيل مع ويندوز",
        ["menu.settings"] = "الإعدادات…",
        ["menu.stopSound"] = "إيقاف الصوت",
        ["menu.exit"] = "خروج",
        ["tray.now"] = "الآن:",
        ["tray.next"] = "التالي:",
        ["tray.in"] = "خلال",
        ["countdown.now"] = "الآن",
        ["unit.s"] = "ث",
        ["unit.m"] = "د",
        ["time.am"] = "ص",
        ["time.pm"] = "م",
        ["popup.in"] = "خلال",
        ["balloon.reminderTitle"] = "تذكير الصلاة",
        ["balloon.reminderBody"] = "{0} خلال {1} دقيقة ({2})",
        ["balloon.timeTitle"] = "وقت الصلاة",
        ["balloon.timeBody"] = "حان الآن وقت {0} ({1})",
        ["crash.body"] = "واجه أوقات الصلاة خطأً غير متوقع وتعافى منه.\n" +
                         "كُتبت التفاصيل في %APPDATA%\\PrayerTray\\error.log.",
        ["settings.title"] = "أوقات الصلاة — الإعدادات",
        ["btn.detect"] = "كشف",
        ["btn.openMaps"] = "فتح الخرائط",
        ["btn.set"] = "تعيين",
        ["btn.test"] = "تجربة",
        ["btn.stop"] = "إيقاف",
        ["btn.save"] = "حفظ",
        ["btn.cancel"] = "إلغاء",
        ["ph.paste"] = "خط العرض، الطول أو رابط خريطة",
        ["ph.customFile"] = "ملف مخصص",
        ["chk.use24"] = "نظام 24 ساعة",
        ["chk.hideFs"] = "إخفاء فوق تطبيقات ملء الشاشة",
        ["chk.remind"] = "تذكيري قبل كل صلاة",
        ["chk.playSound"] = "تشغيل صوت",
        ["chk.showHijri"] = "إظهار التاريخ الهجري",
        ["label.city"] = "المدينة (تسمية):",
        ["label.lat"] = "خط العرض:",
        ["label.lng"] = "خط الطول:",
        ["label.pickMap"] = "اختيار على الخريطة:",
        ["label.pasteResult"] = "لصق النتيجة:",
        ["label.method"] = "الطريقة:",
        ["label.asr"] = "العصر:",
        ["label.theme"] = "السمة:",
        ["label.font"] = "الخط:",
        ["label.fontSize"] = "حجم الخط:",
        ["label.hijriAdjust"] = "ضبط الهجري (أيام):",
        ["label.widgetSide"] = "جهة الأداة:",
        ["label.widgetGap"] = "تباعد الأداة (بكسل):",
        ["label.monitor"] = "الشاشة:",
        ["label.minutesBefore"] = "الدقائق قبل:",
        ["label.sound"] = "الصوت:",
        ["label.customFile"] = "ملف مخصص:",
        ["label.azan"] = "الأذان:",
        ["label.azanFile"] = "ملف الأذان:",
        ["label.language"] = "اللغة:",
        ["card.location"] = "الموقع",
        ["card.calculation"] = "الحساب",
        ["card.appearance"] = "المظهر",
        ["card.notifications"] = "التنبيهات",
        ["asr.standard"] = "افتراضي (شافعي، مالكي، حنبلي)",
        ["asr.hanafi"] = "حنفي",
        ["side.right"] = "يمين (قرب الساعة)",
        ["side.left"] = "يسار (الزاوية)",
        ["monitor.primary"] = " — رئيسية",
        ["combo.customFile"] = "ملف مخصص…",
        ["azan.off"] = "إيقاف",
        ["sound.chime"] = "رنين",
        ["sound.bell"] = "جرس",
        ["sound.ding"] = "نقرة",
        ["sound.beep"] = "صفير",
        ["sound.double"] = "صفير مزدوج",
        ["adhan.makkah"] = "مكة",
        ["adhan.madinah"] = "المدينة",
        ["lang.auto"] = "افتراضي النظام",
        ["msg.detectFail"] = "تعذّر كشف الموقع — أدخله يدويًا.",
        ["msg.detectCaption"] = "كشف الموقع",
        ["msg.pasteFail"] = "تعذّرت قراءة الإحداثيات. انقر بزر الفأرة الأيمن على موقعك في خرائط جوجل، وانقر على\n" +
                            "خط العرض/الطول لنسخهما والصقهما هنا — أو الصق رابط شريط العنوان للخريطة.",
        ["msg.pasteCaption"] = "لصق النتيجة",
        ["msg.latRange"] = "يجب أن يكون خط العرض رقمًا بين -90 و90.",
        ["msg.lngRange"] = "يجب أن يكون خط الطول رقمًا بين -180 و180.",
        ["msg.azanFile"] = "اختر ملف أذان صوتيًا، أو اضبط الأذان على إيقاف.",
        ["msg.invalidCaption"] = "إدخال غير صالح",
        ["city.myLocation"] = "موقعي",
    };
}
