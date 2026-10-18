pragma Singleton

import QtQuick

// Stand-in for KDE's i18n() family in the tests: the app calls the real functions
// (installed on the engine by KLocalization); run.sh rewrites `i18n(` to `Tr.i18n(`
// in the test copy. Placeholders (%1, %2 …) behave like KLocalizedString's.
// A test can fill `catalogue` ({"English": "Translated"}) to see what gets translated.
QtObject {
    property var catalogue: ({})
    // > 1 makes every string without a catalogue entry that much longer (each word grows by
    // repeating its own first letters): a stand-in for a language that needs more room (German ...).
    property real stretch: 1

    function grow(msg) {
        return msg.replace(/[^\s%\d.,:;!?()\/\u2026\u2212+-]+/g, w => w + w.slice(0, Math.ceil((stretch - 1) * w.length)));
    }
    function lookup(msg) {
        if (catalogue[msg] !== undefined) {
            return catalogue[msg];
        }
        return stretch > 1 ? grow(msg) : msg;
    }
    function sub(s, args) {
        return s.replace(/%(\d+)/g, (m, i) => args[i - 1] !== undefined ? String(args[i - 1]) : m);
    }
    function i18n(msg, ...a) {
        return sub(lookup(msg), a);
    }
    function i18nc(ctx, msg, ...a) {
        return sub(lookup(msg), a);
    }
    function i18np(one, many, n, ...a) {
        return sub(n === 1 ? lookup(one) : lookup(many), [n, ...a]);
    }
    function i18ncp(ctx, one, many, n, ...a) {
        return sub(n === 1 ? lookup(one) : lookup(many), [n, ...a]);
    }
}
