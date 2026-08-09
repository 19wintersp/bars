#[cfg(not(any(windows, debug_assertions)))]
compile_error!("unsupported platform");

pub mod api;
pub mod dir;

pub static NAMESPACE: &str = "com.stopbars.euroscope";
