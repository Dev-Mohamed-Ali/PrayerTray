using System;
using System.Collections.Generic;
using System.Windows.Forms;
using PrayerTray.Calc;
using PrayerTray.Native;

namespace PrayerTray.I18n;

/// <summary>Embedded multi-language string catalog (no .resx, no satellites — works under
/// InvariantGlobalization + single-file). Uninit-safe: any missing key falls back to English.</summary>
internal static class Strings
{
    public enum Language { En, Ar, Fr, Tr, Ur, Id }

    public static Language Lang { get; private set; } = Language.En;

    static readonly HashSet<Language> _rtlLangs = new() { Language.Ar, Language.Ur };
    public static bool IsRtl => _rtlLangs.Contains(Lang);

    public static MessageBoxOptions MsgOpts =>
        IsRtl ? MessageBoxOptions.RtlReading | MessageBoxOptions.RightAlign : 0;

    public static void Init(string? cfg) => Lang = Resolve(cfg);
    public static void Set(string? cfg) => Lang = Resolve(cfg);

    static Language Resolve(string? cfg) => cfg switch
    {
        "ar" => Language.Ar,
        "en" => Language.En,
        "fr" => Language.Fr,
        "tr" => Language.Tr,
        "ur" => Language.Ur,
        "id" => Language.Id,
        _ => Interop.OsUiPrimaryLang() switch // "auto"/null/unknown -> match the OS UI language
        {
            0x01 => Language.Ar,
            0x0C => Language.Fr,
            0x1F => Language.Tr,
            0x20 => Language.Ur,
            0x21 => Language.Id,
            _ => Language.En,
        },
    };

    public static string T(string key)
    {
        if (_ui.TryGetValue(Lang, out var t) && t.TryGetValue(key, out var v)) return v;
        return _en.TryGetValue(key, out var en) ? en : key;
    }

    /// <summary>Composite-format lookup: T(key) used as a String.Format template.</summary>
    public static string F(string key, params object[] args) => string.Format(T(key), args);

    public static string Prayer(string key)
    {
        if (_prayers.TryGetValue(Lang, out var t) && t.TryGetValue(key, out var v)) return v;
        return _prayerEn.TryGetValue(key, out var en) ? en : key;
    }

    public static string AmPm(DateTime dt) => dt.Hour < 12 ? T("time.am") : T("time.pm");

    /// <summary>"Weekday, dd Month" with Western digits. Languages with their own day/month tables format
    /// from them; the rest use the invariant English pattern.</summary>
    public static string FormatPopupDate(DateTime d)
    {
        if (_weekdays.TryGetValue(Lang, out var wd) && _months.TryGetValue(Lang, out var mo))
        {
            string sep = IsRtl ? "، " : ", ";
            return $"{wd[(int)d.DayOfWeek]}{sep}{d.Day:00} {mo[d.Month - 1]}";
        }
        return d.ToString("dddd, dd MMM");
    }

    /// <summary>"7 Ramadan 1448" — Umm al-Qura, Western digits, ±adjust days, localized month name.</summary>
    public static string FormatHijri(DateTime d, int adjust)
    {
        var (y, m, day) = HijriDate.Convert(d, adjust);
        var months = _hijri.TryGetValue(Lang, out var t) ? t : _hijriEn;
        return $"{day} {months[m - 1]} {y}";
    }

    public static string Event(string key)
    {
        if (_events.TryGetValue(Lang, out var t) && t.TryGetValue(key, out var v)) return v;
        return _eventsEn.TryGetValue(key, out var en) ? en : key;
    }

    // ===================== event names =====================
    static readonly Dictionary<string, string> _eventsEn = new()
    {
        ["newYear"] = "Islamic New Year",
        ["ashura"] = "Day of Ashura",
        ["mawlid"] = "Mawlid al-Nabi",
        ["isra"] = "Isra and Mi'raj",
        ["midShaban"] = "Mid-Sha'ban",
        ["ramadanStart"] = "Ramadan begins",
        ["laylatQadr"] = "Laylat al-Qadr (27th)",
        ["eidFitr"] = "Eid al-Fitr",
        ["arafah"] = "Day of Arafah",
        ["eidAdha"] = "Eid al-Adha",
        ["tashreeq"] = "Days of Tashreeq",
        ["whiteDays"] = "White days (fasting)",
    };

    static readonly Dictionary<string, string> _eventsAr = new()
    {
        ["newYear"] = "رأس السنة الهجرية",
        ["ashura"] = "يوم عاشوراء",
        ["mawlid"] = "المولد النبوي",
        ["isra"] = "الإسراء والمعراج",
        ["midShaban"] = "ليلة النصف من شعبان",
        ["ramadanStart"] = "بداية رمضان",
        ["laylatQadr"] = "ليلة القدر (27)",
        ["eidFitr"] = "عيد الفطر",
        ["arafah"] = "يوم عرفة",
        ["eidAdha"] = "عيد الأضحى",
        ["tashreeq"] = "أيام التشريق",
        ["whiteDays"] = "الأيام البيض (صيام)",
    };

    static readonly Dictionary<string, string> _eventsFr = new()
    {
        ["newYear"] = "Nouvel An hégirien",
        ["ashura"] = "Jour de l'Achoura",
        ["mawlid"] = "Mawlid an-Nabi",
        ["isra"] = "Isra et Mi'raj",
        ["midShaban"] = "Mi-Cha'ban",
        ["ramadanStart"] = "Début du Ramadan",
        ["laylatQadr"] = "Laylat al-Qadr (27e)",
        ["eidFitr"] = "Aïd al-Fitr",
        ["arafah"] = "Jour de Arafat",
        ["eidAdha"] = "Aïd al-Adha",
        ["tashreeq"] = "Jours de Tachriq",
        ["whiteDays"] = "Jours blancs (jeûne)",
    };

    static readonly Dictionary<string, string> _eventsTr = new()
    {
        ["newYear"] = "Hicri Yılbaşı",
        ["ashura"] = "Aşure Günü",
        ["mawlid"] = "Mevlid Kandili",
        ["isra"] = "Miraç Kandili",
        ["midShaban"] = "Berat Kandili",
        ["ramadanStart"] = "Ramazan başlangıcı",
        ["laylatQadr"] = "Kadir Gecesi (27.)",
        ["eidFitr"] = "Ramazan Bayramı",
        ["arafah"] = "Arefe Günü",
        ["eidAdha"] = "Kurban Bayramı",
        ["tashreeq"] = "Teşrik günleri",
        ["whiteDays"] = "Beyaz günler (oruç)",
    };

    static readonly Dictionary<string, string> _eventsUr = new()
    {
        ["newYear"] = "اسلامی نیا سال",
        ["ashura"] = "یومِ عاشورہ",
        ["mawlid"] = "میلاد النبی",
        ["isra"] = "اسراء و معراج",
        ["midShaban"] = "شبِ برات",
        ["ramadanStart"] = "رمضان کا آغاز",
        ["laylatQadr"] = "شبِ قدر (27)",
        ["eidFitr"] = "عید الفطر",
        ["arafah"] = "یومِ عرفہ",
        ["eidAdha"] = "عید الاضحیٰ",
        ["tashreeq"] = "ایامِ تشریق",
        ["whiteDays"] = "ایامِ بیض (روزہ)",
    };

    static readonly Dictionary<string, string> _eventsId = new()
    {
        ["newYear"] = "Tahun Baru Islam",
        ["ashura"] = "Hari Asyura",
        ["mawlid"] = "Maulid Nabi",
        ["isra"] = "Isra Mikraj",
        ["midShaban"] = "Nisfu Syakban",
        ["ramadanStart"] = "Awal Ramadan",
        ["laylatQadr"] = "Lailatul Qadar (27)",
        ["eidFitr"] = "Idulfitri",
        ["arafah"] = "Hari Arafah",
        ["eidAdha"] = "Iduladha",
        ["tashreeq"] = "Hari Tasyrik",
        ["whiteDays"] = "Hari-hari putih (puasa)",
    };

    // ===================== prayer names =====================
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

    static readonly Dictionary<string, string> _prayerFr = new()
    {
        ["fajr"] = "Fajr", ["sunrise"] = "Lever du soleil", ["dhuhr"] = "Dhuhr",
        ["asr"] = "Asr", ["maghrib"] = "Maghrib", ["isha"] = "Isha",
    };

    static readonly Dictionary<string, string> _prayerTr = new()
    {
        ["fajr"] = "İmsak", ["sunrise"] = "Güneş", ["dhuhr"] = "Öğle",
        ["asr"] = "İkindi", ["maghrib"] = "Akşam", ["isha"] = "Yatsı",
    };

    static readonly Dictionary<string, string> _prayerUr = new()
    {
        ["fajr"] = "فجر", ["sunrise"] = "طلوعِ آفتاب", ["dhuhr"] = "ظہر",
        ["asr"] = "عصر", ["maghrib"] = "مغرب", ["isha"] = "عشاء",
    };

    static readonly Dictionary<string, string> _prayerId = new()
    {
        ["fajr"] = "Subuh", ["sunrise"] = "Terbit", ["dhuhr"] = "Zuhur",
        ["asr"] = "Asar", ["maghrib"] = "Magrib", ["isha"] = "Isya",
    };

    // ===================== Gregorian weekday/month names (for the popup date) =====================
    // DayOfWeek order: Sunday=0 .. Saturday=6
    static readonly string[] _arWeekdays = { "الأحد", "الإثنين", "الثلاثاء", "الأربعاء", "الخميس", "الجمعة", "السبت" };
    static readonly string[] _arMonths =
    {
        "يناير", "فبراير", "مارس", "أبريل", "مايو", "يونيو",
        "يوليو", "أغسطس", "سبتمبر", "أكتوبر", "نوفمبر", "ديسمبر",
    };

    static readonly string[] _frWeekdays = { "dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi" };
    static readonly string[] _frMonths =
    {
        "janvier", "février", "mars", "avril", "mai", "juin",
        "juillet", "août", "septembre", "octobre", "novembre", "décembre",
    };

    static readonly string[] _trWeekdays = { "Pazar", "Pazartesi", "Salı", "Çarşamba", "Perşembe", "Cuma", "Cumartesi" };
    static readonly string[] _trMonths =
    {
        "Ocak", "Şubat", "Mart", "Nisan", "Mayıs", "Haziran",
        "Temmuz", "Ağustos", "Eylül", "Ekim", "Kasım", "Aralık",
    };

    static readonly string[] _urWeekdays = { "اتوار", "پیر", "منگل", "بدھ", "جمعرات", "جمعہ", "ہفتہ" };
    static readonly string[] _urMonths =
    {
        "جنوری", "فروری", "مارچ", "اپریل", "مئی", "جون",
        "جولائی", "اگست", "ستمبر", "اکتوبر", "نومبر", "دسمبر",
    };

    static readonly string[] _idWeekdays = { "Minggu", "Senin", "Selasa", "Rabu", "Kamis", "Jumat", "Sabtu" };
    static readonly string[] _idMonths =
    {
        "Januari", "Februari", "Maret", "April", "Mei", "Juni",
        "Juli", "Agustus", "September", "Oktober", "November", "Desember",
    };

    // ===================== Hijri month names =====================
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
    static readonly string[] _hijriFr =
    {
        "Mouharram", "Safar", "Rabi al-Awwal", "Rabi al-Thani", "Joumada al-Oula", "Joumada al-Thania",
        "Rajab", "Cha'ban", "Ramadan", "Chawwal", "Dhou al-Qi'da", "Dhou al-Hijja",
    };
    static readonly string[] _hijriTr =
    {
        "Muharrem", "Safer", "Rebiülevvel", "Rebiülahir", "Cemaziyelevvel", "Cemaziyelahir",
        "Recep", "Şaban", "Ramazan", "Şevval", "Zilkade", "Zilhicce",
    };
    static readonly string[] _hijriUr =
    {
        "محرم", "صفر", "ربیع الاول", "ربیع الثانی", "جمادی الاول", "جمادی الثانی",
        "رجب", "شعبان", "رمضان", "شوال", "ذوالقعدہ", "ذوالحجہ",
    };
    static readonly string[] _hijriId =
    {
        "Muharam", "Safar", "Rabiulawal", "Rabiulakhir", "Jumadilawal", "Jumadilakhir",
        "Rajab", "Syakban", "Ramadan", "Syawal", "Zulkaidah", "Zulhijah",
    };

    // ===================== English UI =====================
    static readonly Dictionary<string, string> _en = new()
    {
        ["app.name"] = "Prayer Tray",
        ["menu.showTimes"] = "Show times",
        ["menu.refresh"] = "Refresh now",
        ["menu.startup"] = "Start with Windows",
        ["menu.settings"] = "Settings…",
        ["menu.stopSound"] = "Stop sound",
        ["menu.exit"] = "Exit",
        ["tray.now"] = "Now:",
        ["tray.next"] = "Next:",
        ["tray.in"] = "in",
        ["countdown.now"] = "now",
        ["unit.s"] = "s",
        ["unit.m"] = "m",
        ["time.am"] = "AM",
        ["time.pm"] = "PM",
        ["popup.in"] = "in",
        ["balloon.reminderTitle"] = "Prayer reminder",
        ["balloon.reminderBody"] = "{0} in {1} min ({2})",
        ["toast.test"] = "This is a test notification.",
        ["balloon.timeTitle"] = "Prayer time",
        ["balloon.timeBody"] = "It is now {0} ({1})",
        ["balloon.fastTitle"] = "Sunnah fasting",
        ["balloon.fastBody"] = "Tomorrow: {0} — consider fasting (suhoor before Fajr)",
        ["balloon.jumuahTitle"] = "Jumu'ah",
        ["balloon.kahfBody"] = "Jumu'ah Mubarak — read Surah Al-Kahf today",
        ["balloon.jumuahBody"] = "Jumu'ah prayer soon — prepare for the mosque",
        ["fast.monday"] = "Monday",
        ["fast.thursday"] = "Thursday",
        ["crash.body"] = "Prayer Tray hit an unexpected error and recovered.\n" +
                         "Details were written to %APPDATA%\\PrayerTray\\error.log.",
        ["settings.title"] = "Prayer Tray — Settings",
        ["btn.detect"] = "Detect",
        ["btn.openMaps"] = "Open Maps",
        ["btn.set"] = "Set",
        ["btn.test"] = "Test",
        ["btn.stop"] = "Stop",
        ["btn.save"] = "Save",
        ["btn.cancel"] = "Cancel",
        ["ph.paste"] = "lat, lng or map link",
        ["ph.customFile"] = "custom file",
        ["chk.use24"] = "Use 24-hour clock",
        ["chk.hideFs"] = "Hide over fullscreen apps",
        ["chk.richToasts"] = "Use Windows notifications (rich toasts)",
        ["chk.remind"] = "Remind me before each prayer",
        ["chk.playSound"] = "Play a sound",
        ["chk.showHijri"] = "Show Hijri date",
        ["chk.showEvents"] = "Show Islamic events & special days",
        ["chk.sunnahFast"] = "Remind me about Sunnah fasting (eve before)",
        ["chk.fridayReminder"] = "Friday: Jumu'ah & Al-Kahf reminder",
        ["chk.netSpeed"] = "Show internet speed",
        ["event.inDays"] = "{0} in {1} days",
        ["event.tomorrow"] = "{0} tomorrow",
        ["label.city"] = "City (label):",
        ["label.lat"] = "Latitude:",
        ["label.lng"] = "Longitude:",
        ["label.pickMap"] = "Pick on map:",
        ["label.pasteResult"] = "Paste result:",
        ["label.method"] = "Method:",
        ["label.asr"] = "Asr:",
        ["label.highLat"] = "High latitude:",
        ["label.tuneTimes"] = "Fine-tune times (± min):",
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
        ["card.location"] = "Location",
        ["card.calculation"] = "Calculation",
        ["card.appearance"] = "Appearance",
        ["card.religious"] = "Religious",
        ["card.notifications"] = "Notifications",
        ["asr.standard"] = "Standard (Shafi'i, Maliki, Hanbali)",
        ["asr.hanafi"] = "Hanafi",
        ["highLat.None"] = "None",
        ["highLat.AngleBased"] = "Angle-based (recommended)",
        ["highLat.MidNight"] = "Middle of the night",
        ["highLat.OneSeventh"] = "One-seventh of the night",
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

    // ===================== Arabic UI =====================
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
        ["toast.test"] = "هذا إشعار تجريبي.",
        ["balloon.timeTitle"] = "وقت الصلاة",
        ["balloon.timeBody"] = "حان الآن وقت {0} ({1})",
        ["balloon.fastTitle"] = "صيام السنة",
        ["balloon.fastBody"] = "غدًا: {0} — فكّر في الصيام (السحور قبل الفجر)",
        ["balloon.jumuahTitle"] = "الجمعة",
        ["balloon.kahfBody"] = "جمعة مباركة — اقرأ سورة الكهف اليوم",
        ["balloon.jumuahBody"] = "صلاة الجمعة قريبًا — استعدّ للمسجد",
        ["fast.monday"] = "الإثنين",
        ["fast.thursday"] = "الخميس",
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
        ["chk.richToasts"] = "استخدام إشعارات ويندوز (إشعارات منسّقة)",
        ["chk.remind"] = "تذكيري قبل كل صلاة",
        ["chk.playSound"] = "تشغيل صوت",
        ["chk.showHijri"] = "إظهار التاريخ الهجري",
        ["chk.showEvents"] = "إظهار المناسبات والأيام المميزة",
        ["chk.sunnahFast"] = "تذكيري بصيام السنة (مساء اليوم السابق)",
        ["chk.fridayReminder"] = "الجمعة: تذكير الجمعة وسورة الكهف",
        ["chk.netSpeed"] = "إظهار سرعة الإنترنت",
        ["event.inDays"] = "{0} خلال {1} يومًا",
        ["event.tomorrow"] = "{0} غدًا",
        ["label.city"] = "المدينة (تسمية):",
        ["label.lat"] = "خط العرض:",
        ["label.lng"] = "خط الطول:",
        ["label.pickMap"] = "اختيار على الخريطة:",
        ["label.pasteResult"] = "لصق النتيجة:",
        ["label.method"] = "الطريقة:",
        ["label.asr"] = "العصر:",
        ["label.highLat"] = "خطوط العرض العالية:",
        ["label.tuneTimes"] = "ضبط الأوقات (± دقيقة):",
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
        ["card.religious"] = "ديني",
        ["card.notifications"] = "التنبيهات",
        ["asr.standard"] = "افتراضي (شافعي، مالكي، حنبلي)",
        ["asr.hanafi"] = "حنفي",
        ["highLat.None"] = "بدون",
        ["highLat.AngleBased"] = "حسب الزاوية (موصى به)",
        ["highLat.MidNight"] = "منتصف الليل",
        ["highLat.OneSeventh"] = "سُبع الليل",
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

    // ===================== French UI =====================
    static readonly Dictionary<string, string> _fr = new()
    {
        ["app.name"] = "Heures de Prière",
        ["menu.showTimes"] = "Afficher les horaires",
        ["menu.refresh"] = "Actualiser",
        ["menu.startup"] = "Démarrer avec Windows",
        ["menu.settings"] = "Paramètres…",
        ["menu.stopSound"] = "Arrêter le son",
        ["menu.exit"] = "Quitter",
        ["tray.now"] = "Maintenant :",
        ["tray.next"] = "Suivant :",
        ["tray.in"] = "dans",
        ["countdown.now"] = "maintenant",
        ["unit.s"] = "s",
        ["unit.m"] = "min",
        ["time.am"] = "AM",
        ["time.pm"] = "PM",
        ["popup.in"] = "dans",
        ["balloon.reminderTitle"] = "Rappel de prière",
        ["balloon.reminderBody"] = "{0} dans {1} min ({2})",
        ["toast.test"] = "Ceci est une notification de test.",
        ["balloon.timeTitle"] = "Heure de prière",
        ["balloon.timeBody"] = "C'est l'heure de {0} ({1})",
        ["balloon.fastTitle"] = "Jeûne surérogatoire",
        ["balloon.fastBody"] = "Demain : {0} — pensez à jeûner (souhour avant le Fajr)",
        ["balloon.jumuahTitle"] = "Joumou'a",
        ["balloon.kahfBody"] = "Joumou'a Moubarak — lisez la sourate Al-Kahf aujourd'hui",
        ["balloon.jumuahBody"] = "Prière du vendredi bientôt — préparez-vous pour la mosquée",
        ["fast.monday"] = "lundi",
        ["fast.thursday"] = "jeudi",
        ["crash.body"] = "Heures de Prière a rencontré une erreur inattendue et a récupéré.\n" +
                         "Les détails ont été écrits dans %APPDATA%\\PrayerTray\\error.log.",
        ["settings.title"] = "Heures de Prière — Paramètres",
        ["btn.detect"] = "Détecter",
        ["btn.openMaps"] = "Ouvrir Maps",
        ["btn.set"] = "Définir",
        ["btn.test"] = "Tester",
        ["btn.stop"] = "Arrêter",
        ["btn.save"] = "Enregistrer",
        ["btn.cancel"] = "Annuler",
        ["ph.paste"] = "lat, lng ou lien carte",
        ["ph.customFile"] = "fichier perso",
        ["chk.use24"] = "Format 24 heures",
        ["chk.hideFs"] = "Masquer en plein écran",
        ["chk.richToasts"] = "Utiliser les notifications Windows",
        ["chk.remind"] = "Me rappeler avant chaque prière",
        ["chk.playSound"] = "Jouer un son",
        ["chk.showHijri"] = "Afficher la date hégirienne",
        ["chk.showEvents"] = "Afficher les événements islamiques",
        ["chk.sunnahFast"] = "Me rappeler le jeûne surérogatoire (la veille)",
        ["chk.fridayReminder"] = "Vendredi : rappel Joumou'a et Al-Kahf",
        ["chk.netSpeed"] = "Afficher le débit Internet",
        ["event.inDays"] = "{0} dans {1} jours",
        ["event.tomorrow"] = "{0} demain",
        ["label.city"] = "Ville (libellé) :",
        ["label.lat"] = "Latitude :",
        ["label.lng"] = "Longitude :",
        ["label.pickMap"] = "Choisir sur la carte :",
        ["label.pasteResult"] = "Coller le résultat :",
        ["label.method"] = "Méthode :",
        ["label.asr"] = "Asr :",
        ["label.highLat"] = "Haute latitude :",
        ["label.tuneTimes"] = "Ajuster les horaires (± min) :",
        ["label.theme"] = "Thème :",
        ["label.font"] = "Police :",
        ["label.fontSize"] = "Taille de police :",
        ["label.hijriAdjust"] = "Ajustement hégirien (jours) :",
        ["label.widgetSide"] = "Côté du widget :",
        ["label.widgetGap"] = "Marge du widget (px) :",
        ["label.monitor"] = "Écran :",
        ["label.minutesBefore"] = "Minutes avant :",
        ["label.sound"] = "Son :",
        ["label.customFile"] = "Fichier perso :",
        ["label.azan"] = "Adhan :",
        ["label.azanFile"] = "Fichier adhan :",
        ["label.language"] = "Langue :",
        ["card.location"] = "Emplacement",
        ["card.calculation"] = "Calcul",
        ["card.appearance"] = "Apparence",
        ["card.religious"] = "Religieux",
        ["card.notifications"] = "Notifications",
        ["asr.standard"] = "Standard (Chafi'i, Maliki, Hanbali)",
        ["asr.hanafi"] = "Hanafi",
        ["highLat.None"] = "Aucune",
        ["highLat.AngleBased"] = "Basée sur l'angle (recommandé)",
        ["highLat.MidNight"] = "Milieu de la nuit",
        ["highLat.OneSeventh"] = "Un septième de la nuit",
        ["side.right"] = "Droite (près de l'horloge)",
        ["side.left"] = "Gauche (coin)",
        ["monitor.primary"] = " — Principal",
        ["combo.customFile"] = "Fichier perso…",
        ["azan.off"] = "Désactivé",
        ["sound.chime"] = "Carillon",
        ["sound.bell"] = "Cloche",
        ["sound.ding"] = "Ding",
        ["sound.beep"] = "Bip",
        ["sound.double"] = "Double bip",
        ["adhan.makkah"] = "La Mecque",
        ["adhan.madinah"] = "Médine",
        ["lang.auto"] = "Par défaut du système",
        ["msg.detectFail"] = "Impossible de détecter la position — saisissez-la manuellement.",
        ["msg.detectCaption"] = "Détecter la position",
        ["msg.pasteFail"] = "Impossible de lire les coordonnées. Faites un clic droit sur votre lieu dans Google Maps,\n" +
                            "cliquez sur les coordonnées pour les copier, puis collez-les ici — ou collez l'URL de la carte.",
        ["msg.pasteCaption"] = "Coller le résultat",
        ["msg.latRange"] = "La latitude doit être un nombre entre -90 et 90.",
        ["msg.lngRange"] = "La longitude doit être un nombre entre -180 et 180.",
        ["msg.azanFile"] = "Choisissez un fichier audio d'adhan, ou désactivez l'adhan.",
        ["msg.invalidCaption"] = "Saisie invalide",
        ["city.myLocation"] = "Ma position",
    };

    // ===================== Turkish UI =====================
    static readonly Dictionary<string, string> _tr = new()
    {
        ["app.name"] = "Namaz Vakitleri",
        ["menu.showTimes"] = "Vakitleri göster",
        ["menu.refresh"] = "Şimdi yenile",
        ["menu.startup"] = "Windows ile başlat",
        ["menu.settings"] = "Ayarlar…",
        ["menu.stopSound"] = "Sesi durdur",
        ["menu.exit"] = "Çıkış",
        ["tray.now"] = "Şimdi:",
        ["tray.next"] = "Sıradaki:",
        ["tray.in"] = "kalan",
        ["countdown.now"] = "şimdi",
        ["unit.s"] = "sn",
        ["unit.m"] = "dk",
        ["time.am"] = "ÖÖ",
        ["time.pm"] = "ÖS",
        ["popup.in"] = "kalan",
        ["balloon.reminderTitle"] = "Namaz hatırlatıcısı",
        ["balloon.reminderBody"] = "{0} için {1} dk kaldı ({2})",
        ["toast.test"] = "Bu bir test bildirimidir.",
        ["balloon.timeTitle"] = "Namaz vakti",
        ["balloon.timeBody"] = "{0} vakti girdi ({1})",
        ["balloon.fastTitle"] = "Nafile oruç",
        ["balloon.fastBody"] = "Yarın: {0} — oruç tutmayı düşünün (imsaktan önce sahur)",
        ["balloon.jumuahTitle"] = "Cuma",
        ["balloon.kahfBody"] = "Cumanız mübarek olsun — bugün Kehf suresini okuyun",
        ["balloon.jumuahBody"] = "Cuma namazı yaklaştı — camiye hazırlanın",
        ["fast.monday"] = "Pazartesi",
        ["fast.thursday"] = "Perşembe",
        ["crash.body"] = "Namaz Vakitleri beklenmeyen bir hatayla karşılaştı ve kurtarıldı.\n" +
                         "Ayrıntılar %APPDATA%\\PrayerTray\\error.log dosyasına yazıldı.",
        ["settings.title"] = "Namaz Vakitleri — Ayarlar",
        ["btn.detect"] = "Algıla",
        ["btn.openMaps"] = "Haritalar",
        ["btn.set"] = "Ayarla",
        ["btn.test"] = "Test",
        ["btn.stop"] = "Durdur",
        ["btn.save"] = "Kaydet",
        ["btn.cancel"] = "İptal",
        ["ph.paste"] = "enlem, boylam veya harita bağlantısı",
        ["ph.customFile"] = "özel dosya",
        ["chk.use24"] = "24 saat biçimi",
        ["chk.hideFs"] = "Tam ekranda gizle",
        ["chk.richToasts"] = "Windows bildirimlerini kullan",
        ["chk.remind"] = "Her namazdan önce hatırlat",
        ["chk.playSound"] = "Ses çal",
        ["chk.showHijri"] = "Hicri tarihi göster",
        ["chk.showEvents"] = "İslami önemli günleri göster",
        ["chk.sunnahFast"] = "Nafile orucu hatırlat (bir gece önce)",
        ["chk.fridayReminder"] = "Cuma: Cuma ve Kehf hatırlatıcısı",
        ["chk.netSpeed"] = "İnternet hızını göster",
        ["event.inDays"] = "{0} {1} gün içinde",
        ["event.tomorrow"] = "{0} yarın",
        ["label.city"] = "Şehir (etiket):",
        ["label.lat"] = "Enlem:",
        ["label.lng"] = "Boylam:",
        ["label.pickMap"] = "Haritada seç:",
        ["label.pasteResult"] = "Sonucu yapıştır:",
        ["label.method"] = "Yöntem:",
        ["label.asr"] = "İkindi:",
        ["label.highLat"] = "Yüksek enlem:",
        ["label.tuneTimes"] = "Vakitleri ince ayarla (± dk):",
        ["label.theme"] = "Tema:",
        ["label.font"] = "Yazı tipi:",
        ["label.fontSize"] = "Yazı boyutu:",
        ["label.hijriAdjust"] = "Hicri düzeltme (gün):",
        ["label.widgetSide"] = "Widget tarafı:",
        ["label.widgetGap"] = "Widget boşluğu (px):",
        ["label.monitor"] = "Ekran:",
        ["label.minutesBefore"] = "Önceki dakika:",
        ["label.sound"] = "Ses:",
        ["label.customFile"] = "Özel dosya:",
        ["label.azan"] = "Ezan:",
        ["label.azanFile"] = "Ezan dosyası:",
        ["label.language"] = "Dil:",
        ["card.location"] = "Konum",
        ["card.calculation"] = "Hesaplama",
        ["card.appearance"] = "Görünüm",
        ["card.religious"] = "Dini",
        ["card.notifications"] = "Bildirimler",
        ["asr.standard"] = "Standart (Şafii, Maliki, Hanbeli)",
        ["asr.hanafi"] = "Hanefi",
        ["highLat.None"] = "Yok",
        ["highLat.AngleBased"] = "Açı tabanlı (önerilen)",
        ["highLat.MidNight"] = "Gece yarısı",
        ["highLat.OneSeventh"] = "Gecenin yedide biri",
        ["side.right"] = "Sağ (saatin yanında)",
        ["side.left"] = "Sol (köşe)",
        ["monitor.primary"] = " — Birincil",
        ["combo.customFile"] = "Özel dosya…",
        ["azan.off"] = "Kapalı",
        ["sound.chime"] = "Çıngırak",
        ["sound.bell"] = "Zil",
        ["sound.ding"] = "Ding",
        ["sound.beep"] = "Bip",
        ["sound.double"] = "Çift bip",
        ["adhan.makkah"] = "Mekke",
        ["adhan.madinah"] = "Medine",
        ["lang.auto"] = "Sistem varsayılanı",
        ["msg.detectFail"] = "Konum algılanamadı — elle girin.",
        ["msg.detectCaption"] = "Konum algıla",
        ["msg.pasteFail"] = "Koordinatlar okunamadı. Google Haritalar'da konumunuza sağ tıklayın,\n" +
                            "koordinatlara tıklayıp kopyalayın ve buraya yapıştırın — ya da harita URL'sini yapıştırın.",
        ["msg.pasteCaption"] = "Sonucu yapıştır",
        ["msg.latRange"] = "Enlem -90 ile 90 arasında bir sayı olmalıdır.",
        ["msg.lngRange"] = "Boylam -180 ile 180 arasında bir sayı olmalıdır.",
        ["msg.azanFile"] = "Bir ezan ses dosyası seçin veya Ezan'ı kapatın.",
        ["msg.invalidCaption"] = "Geçersiz giriş",
        ["city.myLocation"] = "Konumum",
    };

    // ===================== Urdu UI =====================
    static readonly Dictionary<string, string> _ur = new()
    {
        ["app.name"] = "نماز کے اوقات",
        ["menu.showTimes"] = "اوقات دکھائیں",
        ["menu.refresh"] = "ابھی تازہ کریں",
        ["menu.startup"] = "ونڈوز کے ساتھ شروع کریں",
        ["menu.settings"] = "ترتیبات…",
        ["menu.stopSound"] = "آواز بند کریں",
        ["menu.exit"] = "خروج",
        ["tray.now"] = "ابھی:",
        ["tray.next"] = "اگلی:",
        ["tray.in"] = "میں",
        ["countdown.now"] = "ابھی",
        ["unit.s"] = "س",
        ["unit.m"] = "م",
        ["time.am"] = "صبح",
        ["time.pm"] = "شام",
        ["popup.in"] = "میں",
        ["balloon.reminderTitle"] = "نماز کی یاد دہانی",
        ["balloon.reminderBody"] = "{0} {1} منٹ میں ({2})",
        ["toast.test"] = "یہ ایک ٹیسٹ اطلاع ہے۔",
        ["balloon.timeTitle"] = "نماز کا وقت",
        ["balloon.timeBody"] = "اب {0} کا وقت ہے ({1})",
        ["balloon.fastTitle"] = "نفلی روزہ",
        ["balloon.fastBody"] = "کل: {0} — روزہ رکھنے پر غور کریں (فجر سے پہلے سحری)",
        ["balloon.jumuahTitle"] = "جمعہ",
        ["balloon.kahfBody"] = "جمعہ مبارک — آج سورۃ الکہف پڑھیں",
        ["balloon.jumuahBody"] = "جمعہ کی نماز قریب ہے — مسجد کے لیے تیار ہوں",
        ["fast.monday"] = "پیر",
        ["fast.thursday"] = "جمعرات",
        ["crash.body"] = "نماز کے اوقات کو غیر متوقع خرابی پیش آئی اور بحال ہو گئی۔\n" +
                         "تفصیلات %APPDATA%\\PrayerTray\\error.log میں لکھی گئیں۔",
        ["settings.title"] = "نماز کے اوقات — ترتیبات",
        ["btn.detect"] = "پتہ لگائیں",
        ["btn.openMaps"] = "نقشہ کھولیں",
        ["btn.set"] = "مقرر کریں",
        ["btn.test"] = "آزمائیں",
        ["btn.stop"] = "روکیں",
        ["btn.save"] = "محفوظ کریں",
        ["btn.cancel"] = "منسوخ",
        ["ph.paste"] = "عرض، طول یا نقشہ لنک",
        ["ph.customFile"] = "حسب ضرورت فائل",
        ["chk.use24"] = "24 گھنٹے کا نظام",
        ["chk.hideFs"] = "فل اسکرین پر چھپائیں",
        ["chk.richToasts"] = "ونڈوز اطلاعات استعمال کریں",
        ["chk.remind"] = "ہر نماز سے پہلے یاد دہانی",
        ["chk.playSound"] = "آواز چلائیں",
        ["chk.showHijri"] = "ہجری تاریخ دکھائیں",
        ["chk.showEvents"] = "اسلامی مواقع دکھائیں",
        ["chk.sunnahFast"] = "نفلی روزے کی یاد دہانی (ایک رات پہلے)",
        ["chk.fridayReminder"] = "جمعہ: جمعہ اور الکہف یاد دہانی",
        ["chk.netSpeed"] = "انٹرنیٹ کی رفتار دکھائیں",
        ["event.inDays"] = "{0} {1} دن میں",
        ["event.tomorrow"] = "{0} کل",
        ["label.city"] = "شہر (نام):",
        ["label.lat"] = "عرض البلد:",
        ["label.lng"] = "طول البلد:",
        ["label.pickMap"] = "نقشے پر منتخب کریں:",
        ["label.pasteResult"] = "نتیجہ پیسٹ کریں:",
        ["label.method"] = "طریقہ:",
        ["label.asr"] = "عصر:",
        ["label.highLat"] = "بلند عرض البلد:",
        ["label.tuneTimes"] = "اوقات کی باریک ترتیب (± منٹ):",
        ["label.theme"] = "تھیم:",
        ["label.font"] = "فونٹ:",
        ["label.fontSize"] = "فونٹ سائز:",
        ["label.hijriAdjust"] = "ہجری ترتیب (دن):",
        ["label.widgetSide"] = "ویجٹ کی جانب:",
        ["label.widgetGap"] = "ویجٹ کا فاصلہ (px):",
        ["label.monitor"] = "اسکرین:",
        ["label.minutesBefore"] = "کتنے منٹ پہلے:",
        ["label.sound"] = "آواز:",
        ["label.customFile"] = "حسب ضرورت فائل:",
        ["label.azan"] = "اذان:",
        ["label.azanFile"] = "اذان فائل:",
        ["label.language"] = "زبان:",
        ["card.location"] = "مقام",
        ["card.calculation"] = "حساب",
        ["card.appearance"] = "ظاہری شکل",
        ["card.religious"] = "دینی",
        ["card.notifications"] = "اطلاعات",
        ["asr.standard"] = "معیاری (شافعی، مالکی، حنبلی)",
        ["asr.hanafi"] = "حنفی",
        ["highLat.None"] = "کوئی نہیں",
        ["highLat.AngleBased"] = "زاویہ پر مبنی (تجویز کردہ)",
        ["highLat.MidNight"] = "نصف شب",
        ["highLat.OneSeventh"] = "رات کا ساتواں حصہ",
        ["side.right"] = "دائیں (گھڑی کے قریب)",
        ["side.left"] = "بائیں (کونہ)",
        ["monitor.primary"] = " — بنیادی",
        ["combo.customFile"] = "حسب ضرورت فائل…",
        ["azan.off"] = "بند",
        ["sound.chime"] = "گھنٹی",
        ["sound.bell"] = "گھنٹہ",
        ["sound.ding"] = "ڈنگ",
        ["sound.beep"] = "بیپ",
        ["sound.double"] = "ڈبل بیپ",
        ["adhan.makkah"] = "مکہ",
        ["adhan.madinah"] = "مدینہ",
        ["lang.auto"] = "سسٹم ڈیفالٹ",
        ["msg.detectFail"] = "مقام کا پتہ نہیں چل سکا — دستی طور پر درج کریں۔",
        ["msg.detectCaption"] = "مقام کا پتہ لگائیں",
        ["msg.pasteFail"] = "نقاط نہیں پڑھے جا سکے۔ گوگل میپس میں اپنی جگہ پر دائیں کلک کریں، نقاط پر کلک کر کے\n" +
                            "انہیں کاپی کریں اور یہاں پیسٹ کریں — یا نقشے کا URL پیسٹ کریں۔",
        ["msg.pasteCaption"] = "نتیجہ پیسٹ کریں",
        ["msg.latRange"] = "عرض البلد -90 اور 90 کے درمیان عدد ہونا چاہیے۔",
        ["msg.lngRange"] = "طول البلد -180 اور 180 کے درمیان عدد ہونا چاہیے۔",
        ["msg.azanFile"] = "اذان کی آڈیو فائل منتخب کریں، یا اذان بند کریں۔",
        ["msg.invalidCaption"] = "غلط اندراج",
        ["city.myLocation"] = "میرا مقام",
    };

    // ===================== Indonesian UI =====================
    static readonly Dictionary<string, string> _id = new()
    {
        ["app.name"] = "Waktu Salat",
        ["menu.showTimes"] = "Tampilkan waktu",
        ["menu.refresh"] = "Segarkan sekarang",
        ["menu.startup"] = "Mulai bersama Windows",
        ["menu.settings"] = "Pengaturan…",
        ["menu.stopSound"] = "Hentikan suara",
        ["menu.exit"] = "Keluar",
        ["tray.now"] = "Sekarang:",
        ["tray.next"] = "Berikutnya:",
        ["tray.in"] = "dalam",
        ["countdown.now"] = "sekarang",
        ["unit.s"] = "d",
        ["unit.m"] = "mnt",
        ["time.am"] = "AM",
        ["time.pm"] = "PM",
        ["popup.in"] = "dalam",
        ["balloon.reminderTitle"] = "Pengingat salat",
        ["balloon.reminderBody"] = "{0} dalam {1} mnt ({2})",
        ["toast.test"] = "Ini adalah notifikasi uji.",
        ["balloon.timeTitle"] = "Waktu salat",
        ["balloon.timeBody"] = "Sekarang waktu {0} ({1})",
        ["balloon.fastTitle"] = "Puasa sunah",
        ["balloon.fastBody"] = "Besok: {0} — pertimbangkan berpuasa (sahur sebelum Subuh)",
        ["balloon.jumuahTitle"] = "Jumat",
        ["balloon.kahfBody"] = "Jumat Mubarak — baca Surah Al-Kahf hari ini",
        ["balloon.jumuahBody"] = "Salat Jumat segera — bersiaplah ke masjid",
        ["fast.monday"] = "Senin",
        ["fast.thursday"] = "Kamis",
        ["crash.body"] = "Waktu Salat mengalami kesalahan tak terduga dan telah pulih.\n" +
                         "Detail ditulis ke %APPDATA%\\PrayerTray\\error.log.",
        ["settings.title"] = "Waktu Salat — Pengaturan",
        ["btn.detect"] = "Deteksi",
        ["btn.openMaps"] = "Buka Peta",
        ["btn.set"] = "Atur",
        ["btn.test"] = "Uji",
        ["btn.stop"] = "Berhenti",
        ["btn.save"] = "Simpan",
        ["btn.cancel"] = "Batal",
        ["ph.paste"] = "lat, lng atau tautan peta",
        ["ph.customFile"] = "berkas khusus",
        ["chk.use24"] = "Gunakan format 24 jam",
        ["chk.hideFs"] = "Sembunyikan saat layar penuh",
        ["chk.richToasts"] = "Gunakan notifikasi Windows",
        ["chk.remind"] = "Ingatkan sebelum setiap salat",
        ["chk.playSound"] = "Putar suara",
        ["chk.showHijri"] = "Tampilkan tanggal Hijriah",
        ["chk.showEvents"] = "Tampilkan hari besar Islam",
        ["chk.sunnahFast"] = "Ingatkan puasa sunah (malam sebelumnya)",
        ["chk.fridayReminder"] = "Jumat: pengingat Jumat & Al-Kahf",
        ["chk.netSpeed"] = "Tampilkan kecepatan internet",
        ["event.inDays"] = "{0} dalam {1} hari",
        ["event.tomorrow"] = "{0} besok",
        ["label.city"] = "Kota (label):",
        ["label.lat"] = "Lintang:",
        ["label.lng"] = "Bujur:",
        ["label.pickMap"] = "Pilih di peta:",
        ["label.pasteResult"] = "Tempel hasil:",
        ["label.method"] = "Metode:",
        ["label.asr"] = "Asar:",
        ["label.highLat"] = "Lintang tinggi:",
        ["label.tuneTimes"] = "Sesuaikan waktu (± mnt):",
        ["label.theme"] = "Tema:",
        ["label.font"] = "Font:",
        ["label.fontSize"] = "Ukuran font:",
        ["label.hijriAdjust"] = "Penyesuaian Hijriah (hari):",
        ["label.widgetSide"] = "Sisi widget:",
        ["label.widgetGap"] = "Jarak widget (px):",
        ["label.monitor"] = "Monitor:",
        ["label.minutesBefore"] = "Menit sebelum:",
        ["label.sound"] = "Suara:",
        ["label.customFile"] = "Berkas khusus:",
        ["label.azan"] = "Azan:",
        ["label.azanFile"] = "Berkas azan:",
        ["label.language"] = "Bahasa:",
        ["card.location"] = "Lokasi",
        ["card.calculation"] = "Perhitungan",
        ["card.appearance"] = "Tampilan",
        ["card.religious"] = "Keagamaan",
        ["card.notifications"] = "Notifikasi",
        ["asr.standard"] = "Standar (Syafi'i, Maliki, Hanbali)",
        ["asr.hanafi"] = "Hanafi",
        ["highLat.None"] = "Tidak ada",
        ["highLat.AngleBased"] = "Berbasis sudut (disarankan)",
        ["highLat.MidNight"] = "Tengah malam",
        ["highLat.OneSeventh"] = "Sepertujuh malam",
        ["side.right"] = "Kanan (dekat jam)",
        ["side.left"] = "Kiri (sudut)",
        ["monitor.primary"] = " — Utama",
        ["combo.customFile"] = "Berkas khusus…",
        ["azan.off"] = "Mati",
        ["sound.chime"] = "Lonceng",
        ["sound.bell"] = "Bel",
        ["sound.ding"] = "Ding",
        ["sound.beep"] = "Bip",
        ["sound.double"] = "Bip ganda",
        ["adhan.makkah"] = "Makkah",
        ["adhan.madinah"] = "Madinah",
        ["lang.auto"] = "Bawaan sistem",
        ["msg.detectFail"] = "Tidak dapat mendeteksi lokasi — masukkan secara manual.",
        ["msg.detectCaption"] = "Deteksi lokasi",
        ["msg.pasteFail"] = "Tidak dapat membaca koordinat. Klik kanan lokasi Anda di Google Maps, klik\n" +
                            "koordinat untuk menyalinnya, lalu tempel di sini — atau tempel URL peta.",
        ["msg.pasteCaption"] = "Tempel hasil",
        ["msg.latRange"] = "Lintang harus berupa angka antara -90 dan 90.",
        ["msg.lngRange"] = "Bujur harus berupa angka antara -180 dan 180.",
        ["msg.azanFile"] = "Pilih berkas audio azan, atau matikan Azan.",
        ["msg.invalidCaption"] = "Masukan tidak valid",
        ["city.myLocation"] = "Lokasi saya",
    };

    // ===================== lookup maps (declared last so all tables above are initialized) =====================
    static readonly Dictionary<Language, Dictionary<string, string>> _ui = new()
    {
        [Language.En] = _en, [Language.Ar] = _ar, [Language.Fr] = _fr,
        [Language.Tr] = _tr, [Language.Ur] = _ur, [Language.Id] = _id,
    };

    static readonly Dictionary<Language, Dictionary<string, string>> _prayers = new()
    {
        [Language.En] = _prayerEn, [Language.Ar] = _prayerAr, [Language.Fr] = _prayerFr,
        [Language.Tr] = _prayerTr, [Language.Ur] = _prayerUr, [Language.Id] = _prayerId,
    };

    static readonly Dictionary<Language, Dictionary<string, string>> _events = new()
    {
        [Language.En] = _eventsEn, [Language.Ar] = _eventsAr, [Language.Fr] = _eventsFr,
        [Language.Tr] = _eventsTr, [Language.Ur] = _eventsUr, [Language.Id] = _eventsId,
    };

    static readonly Dictionary<Language, string[]> _hijri = new()
    {
        [Language.En] = _hijriEn, [Language.Ar] = _hijriAr, [Language.Fr] = _hijriFr,
        [Language.Tr] = _hijriTr, [Language.Ur] = _hijriUr, [Language.Id] = _hijriId,
    };

    static readonly Dictionary<Language, string[]> _weekdays = new()
    {
        [Language.Ar] = _arWeekdays, [Language.Fr] = _frWeekdays, [Language.Tr] = _trWeekdays,
        [Language.Ur] = _urWeekdays, [Language.Id] = _idWeekdays,
    };

    static readonly Dictionary<Language, string[]> _months = new()
    {
        [Language.Ar] = _arMonths, [Language.Fr] = _frMonths, [Language.Tr] = _trMonths,
        [Language.Ur] = _urMonths, [Language.Id] = _idMonths,
    };
}
