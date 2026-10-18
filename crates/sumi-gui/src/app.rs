//! The application object (a QApplication; see app.cpp).

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qguiapplication.h");
        type QGuiApplication = cxx_qt_lib::QGuiApplication;

        include!("app.h");
        fn sumi_new_application() -> UniquePtr<QGuiApplication>;
        fn sumi_restart_application();
    }
}
