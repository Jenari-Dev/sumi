#include "i18n.h"

#include <KLocalizedString>
#include <KLocalization>
#include <KLocalizedQmlContext>

#include <QIcon>
#include <QPixmap>
#include <QStandardPaths>
#include <QtQuick/QQuickImageProvider>

void sumi_install_i18n(QQmlApplicationEngine &engine)
{
    // The catalogue name translations will be installed under (sumi.mo).
    KLocalizedString::setApplicationDomain(QByteArrayLiteral("sumi"));
    KLocalization::setupLocalizedContext(&engine);
}

void sumi_set_language(const QString &locale)
{
    // Overrides the languages KLocalizedString would take from the environment.
    KLocalizedString::setLanguages({locale});
}

QString sumi_tr_not_running()
{
    return i18n("sumid isn't running — start it with: systemctl --user start sumid");
}

QString sumi_tr_dbus_error(const QString &detail)
{
    return i18n("D-Bus: %1", detail);
}

namespace {
class IconProvider : public QQuickImageProvider
{
public:
    IconProvider() : QQuickImageProvider(QQuickImageProvider::Pixmap)
    {
        // Platform themes (KDE, GTK) set the icon search paths; a bare platform (offscreen,
        // some minimal sessions) does not, so add the XDG icon directories ourselves.
        QStringList paths = QIcon::themeSearchPaths();
        const QStringList dirs = QStandardPaths::locateAll(QStandardPaths::GenericDataLocation, QStringLiteral("icons"), QStandardPaths::LocateDirectory);
        for (const QString &dir : dirs) {
            if (!paths.contains(dir)) {
                paths << dir;
            }
        }
        QIcon::setThemeSearchPaths(paths);
    }

    QPixmap requestPixmap(const QString &id, QSize *size, const QSize &requestedSize) override
    {
        const int edge = requestedSize.width() > 0 ? requestedSize.width() : 32;
        QIcon icon = id.startsWith(QLatin1Char('/')) ? QIcon(id) : QIcon::fromTheme(id);
        QPixmap pixmap = icon.isNull() ? QPixmap() : icon.pixmap(QSize(edge, edge));
        if (pixmap.isNull()) {
            // Unknown icon: a transparent pixel, so the fallback glyph behind it shows.
            pixmap = QPixmap(1, 1);
            pixmap.fill(Qt::transparent);
        }
        *size = pixmap.size();
        return pixmap;
    }
};
}

void sumi_install_icons(QQmlApplicationEngine &engine)
{
    engine.addImageProvider(QStringLiteral("icon"), new IconProvider);
}
