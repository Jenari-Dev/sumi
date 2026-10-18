#include "app.h"

#include <QtCore/QCoreApplication>
#include <QtCore/QProcess>
#include <QtWidgets/QApplication>

std::unique_ptr<QGuiApplication> sumi_new_application()
{
    // QApplication keeps references to argc/argv for its lifetime. Sumi parses
    // its own arguments; Qt options come from the environment.
    static int argc = 1;
    static char arg0[] = "sumi-app";
    static char *argv[] = {arg0, nullptr};
    return std::make_unique<QApplication>(argc, argv);
}

void sumi_restart_application()
{
    QProcess::startDetached(QCoreApplication::applicationFilePath(), {QStringLiteral("--restart")});
    QCoreApplication::quit();
}
