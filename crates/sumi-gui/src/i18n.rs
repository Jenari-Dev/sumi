//! KDE i18n context, translated messages and the icon provider for QML (see i18n.cpp).

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("cxx-qt-lib/qqmlapplicationengine.h");
        type QQmlApplicationEngine = cxx_qt_lib::QQmlApplicationEngine;

        include!("i18n.h");
        fn sumi_install_i18n(engine: Pin<&mut QQmlApplicationEngine>);
        fn sumi_install_icons(engine: Pin<&mut QQmlApplicationEngine>);
        fn sumi_set_language(locale: &QString);
        fn sumi_tr_not_running() -> QString;
        fn sumi_tr_dbus_error(detail: &QString) -> QString;
    }
}
