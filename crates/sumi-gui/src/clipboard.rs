//! System clipboard access for the QML backend (see clipboard.cpp).

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;

        include!("clipboard.h");
        fn sumi_clipboard_set_text(text: &QString);
    }
}
