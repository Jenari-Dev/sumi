pragma Singleton

import QtQuick

// The languages of the monitor's menu (VCP 0xCC) and the locale each maps to.
// Sumi offers the same set for its own windows; names are written in the
// language itself and are never translated.
QtObject {
    // vcp: the 0xCC value, locale: what KLocalizedString is given
    readonly property var list: [
        { vcp: 2, locale: "en", name: "English" },
        { vcp: 3, locale: "fr", name: "Français" },
        { vcp: 4, locale: "de", name: "Deutsch" },
        { vcp: 10, locale: "es", name: "Español" },
        { vcp: 5, locale: "it", name: "Italiano" },
        { vcp: 20, locale: "nl", name: "Nederlands" },
        { vcp: 9, locale: "ru", name: "Русский" },
        { vcp: 30, locale: "pl", name: "Polski" },
        { vcp: 18, locale: "cs", name: "Čeština" },
        { vcp: 17, locale: "hr", name: "Hrvatski" },
        { vcp: 26, locale: "hu", name: "Magyar" },
        { vcp: 31, locale: "ro", name: "Română" },
        { vcp: 8, locale: "pt", name: "Português" },
        { vcp: 12, locale: "tr", name: "Türkçe" },
        { vcp: 13, locale: "zh_CN", name: "简体中文" },
        { vcp: 1, locale: "zh_TW", name: "繁體中文" },
        { vcp: 6, locale: "ja", name: "日本語" },
        { vcp: 7, locale: "ko", name: "한국어" },
        { vcp: 38, locale: "fa", name: "فارسی" },
        { vcp: 35, locale: "th", name: "ไทย" },
        { vcp: 36, locale: "uk", name: "Українська" },
        { vcp: 39, locale: "id", name: "Bahasa Indonesia" },
        { vcp: 37, locale: "vi", name: "Tiếng Việt" }
    ]

    // The locale for a 0xCC value ("" for one we do not know). The vendor treats 0 as English.
    function localeFor(vcp) {
        if (vcp === 0) {
            return "en";
        }
        const l = list.find(x => x.vcp === vcp);
        return l ? l.locale : "";
    }
}
