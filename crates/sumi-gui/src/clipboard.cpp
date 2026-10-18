#include "clipboard.h"

#include <QClipboard>
#include <QGuiApplication>

void sumi_clipboard_set_text(const QString &text)
{
    if (auto *cb = QGuiApplication::clipboard()) {
        cb->setText(text);
    }
}
