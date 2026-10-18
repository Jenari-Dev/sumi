//! Sumi desktop app (Qt Quick). Monitor control goes through sumid over D-Bus.

mod app;
mod backend;
mod clipboard;
mod i18n;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QQuickStyle, QString, QUrl};

fn main() {
    // Breeze is the pure-QML Plasma style; org.kde.desktop would need QtWidgets.
    // Only unstyled corners ever show it: the app draws its own controls.
    if std::env::var_os("QT_QUICK_CONTROLS_STYLE").is_none() {
        QQuickStyle::set_style(&QString::from("org.kde.breeze"));
    }
    // A QApplication: KDE's platform theme builds the tray icon and native
    // file dialogs with QtWidgets and aborts under a plain QGuiApplication.
    let mut app = app::ffi::sumi_new_application();
    if let Some(mut app) = app.as_mut() {
        app.as_mut().set_application_name(&QString::from("Sumi"));
        app.as_mut().set_organization_domain(&QString::from("jenaridev.github.io"));
        QGuiApplication::set_desktop_file_name(&QString::from("io.github.jenaridev.sumi"));
    }
    let mut engine = QQmlApplicationEngine::new();
    if let Some(mut engine) = engine.as_mut() {
        i18n::ffi::sumi_install_i18n(engine.as_mut());
        // The language is fixed for the run: SUMI_LANGUAGE, else the Application Settings choice.
        if let Some(language) = backend::chosen_language() {
            i18n::ffi::sumi_set_language(&QString::from(&language));
        }
        i18n::ffi::sumi_install_icons(engine.as_mut());
        engine.load(&QUrl::from("qrc:/qt/qml/io/github/jenaridev/sumi/qml/Main.qml"));
    }
    if let Some(app) = app.as_mut() {
        std::process::exit(app.exec());
    }
}
