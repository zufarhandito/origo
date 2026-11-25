//! Windows 11 modern File Explorer context menu extension for Origo.
//!
//! Exposes a cascading `Origo >` submenu when PNG/JPEG image files are selected,
//! following the OS UI language (Indonesian or English) and launching Origo via
//! temporary UTF-8 manifest + single-instance IPC.

#[cfg(windows)]
pub mod com;
pub mod launch;
pub mod locale;
pub mod selection;
