mod palette;
pub use palette::*;


slint::include_modules!();

pub fn create_window() -> MainWindow {
    MainWindow::new().unwrap()
}
