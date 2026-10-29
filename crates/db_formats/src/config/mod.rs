//! Decoded xresloader config tables: `_data/json/excel/<Table>.json` as
//! `{ "source_file", "header", "type", "rows": [...] }`.
//!
//! [`generated`] holds one serde struct per protobuf message (all with `deny_unknown_fields`)
//! and [`ConfigTables`], which loads all 109 tables.

pub mod generated;
pub mod types;

pub use generated::{ConfigTables, TABLES};

use std::path::Path;

use anyhow::Context;
use serde::Deserialize;
use serde::de::DeserializeOwned;

/// One decoded config table with typed rows.
#[derive(Debug, Clone, Deserialize)]
pub struct ConfigTable<R> {
    /// Carved file the table came from (for provenance).
    pub source_file: String,
    /// Fully qualified protobuf message type, e.g. `tgf.config.ExcelRoleType`.
    #[serde(rename = "type")]
    pub message_type: String,
    /// Table rows.
    pub rows: Vec<R>,
}

impl<R: DeserializeOwned> ConfigTable<R> {
    /// Parse a table from its JSON text.
    pub fn from_json(text: &str) -> anyhow::Result<Self> {
        serde_json::from_str(text).context("parsing config table JSON")
    }

    /// Load `<dir>/<name>.json`.
    pub fn load(dir: &Path, name: &str) -> anyhow::Result<Self> {
        let path = dir.join(format!("{name}.json"));
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
        Self::from_json(&text).with_context(|| format!("in {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(deny_unknown_fields)]
    struct LevelExp {
        level: u32,
        max_exp: u64,
    }

    #[test]
    fn parses_table_envelope() {
        let json = r#"{"source_file":"00001.xresdata","header":{"xres_ver":"2.19.2"},
            "type":"tgf.config.ExcelRoleLevelExp","rows":[{"level":1,"max_exp":100}]}"#;
        let t = ConfigTable::<LevelExp>::from_json(json).unwrap();
        assert_eq!(t.message_type, "tgf.config.ExcelRoleLevelExp");
        assert_eq!(t.rows, vec![LevelExp { level: 1, max_exp: 100 }]);
    }
}
