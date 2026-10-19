//! cardloader-twain: a TWAIN helper process ("sidecar") for cardloader.
//!
//! It speaks JSON lines on stdin/stdout (see `protocol.rs` and README.md) and
//! drives a scanner through the TWAIN Data Source Manager. TWAIN data sources
//! are in-process DLLs, so this binary must have the same bitness as the
//! driver — for the PaperStream IP default install that means 32-bit (i686).

mod caps;
mod dib;
mod protocol;
mod twain_ffi;

#[cfg(windows)]
mod dsm;
#[cfg(windows)]
mod session;
#[cfg(windows)]
mod window;

#[cfg(windows)]
fn main() {
    std::process::exit(session::run());
}

#[cfg(not(windows))]
fn main() {
    use protocol::{emit, ErrorCode, Event};
    emit(&Event::error(
        ErrorCode::UnsupportedPlatform,
        "cardloader-twain only works on Windows (TWAIN data sources are Windows DLLs)",
        true,
    ));
    std::process::exit(1);
}
