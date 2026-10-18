#pragma once
#include <QtCore/QString>
#include <QtQml/QQmlApplicationEngine>

// Install KDE's i18n functions (i18n, i18nc, i18ndc, …) on the QML engine.
void sumi_install_i18n(QQmlApplicationEngine &engine);

// Use this language (a locale such as "ja" or "zh_CN") instead of the system's.
void sumi_set_language(const QString &locale);

// Messages the Rust side shows (the QML side calls i18n() itself).
QString sumi_tr_not_running();
QString sumi_tr_dbus_error(const QString &detail);

// An `image://icon/<name>` provider that loads icons from the current icon theme.
void sumi_install_icons(QQmlApplicationEngine &engine);
