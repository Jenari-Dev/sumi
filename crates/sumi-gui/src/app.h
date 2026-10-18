#pragma once
#include <QtGui/QGuiApplication>
#include <memory>

// A QApplication (QtWidgets) handed to Rust as its QGuiApplication base: KDE's
// platform theme needs widgets for the tray icon and native file dialogs.
std::unique_ptr<QGuiApplication> sumi_new_application();

// Start another copy of this program (told with --restart to wait for this one
// to let go of the single-instance name) and quit this one.
void sumi_restart_application();
