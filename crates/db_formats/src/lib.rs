//! Readers for data produced by the extraction tools in `tools/` (never game code).
//!
//! - [`config`]: the decoded xresloader config tables, typed.
//! - [`strings`]: the carved string tables (`Key,SourceString` CSV).

pub mod config;
pub mod data_dir;
pub mod strings;

pub use config::{ConfigTable, ConfigTables};
pub use data_dir::{data_path, data_root};
