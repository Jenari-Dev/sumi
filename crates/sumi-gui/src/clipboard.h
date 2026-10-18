#pragma once
#include <QtCore/QString>

// Put text on the system clipboard (must run on the GUI thread).
void sumi_clipboard_set_text(const QString &text);
