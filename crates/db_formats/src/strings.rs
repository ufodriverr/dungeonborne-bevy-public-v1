//! String tables carved from the legacy pak: UTF-8 CSV with the header `Key,SourceString`.
//! Fields may be quoted; inside quotes `""` is a literal quote and newlines are allowed.
//! Source text is the game's Chinese source language.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, bail};

/// The header line every string table starts with.
pub const HEADER: &str = "Key,SourceString";

/// One string table: key → source string, in file order of first appearance.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct StringTable {
    pub entries: BTreeMap<String, String>,
}

impl StringTable {
    /// True if `bytes` look like a string table (UTF-8, optional BOM, `Key,SourceString` header).
    pub fn sniff(bytes: &[u8]) -> bool {
        let b = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
        b.starts_with(HEADER.as_bytes())
    }

    /// Parse the CSV text. Duplicate keys with different values are an error.
    pub fn parse(text: &str) -> anyhow::Result<Self> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut rows = parse_csv(text)?.into_iter();
        match rows.next() {
            Some(h) if h == ["Key", "SourceString"] => {}
            other => bail!("expected header {HEADER}, got {other:?}"),
        }
        let mut entries = BTreeMap::new();
        for (i, row) in rows.enumerate() {
            let [key, value]: [String; 2] = row.try_into().map_err(|r: Vec<String>| anyhow::anyhow!("row {}: {} fields", i + 2, r.len()))?;
            if let Some(old) = entries.insert(key.clone(), value.clone())
                && old != value
            {
                bail!("row {}: key `{key}` repeated with a different value", i + 2);
            }
        }
        Ok(Self { entries })
    }

    /// Read and parse a file.
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        Self::parse(&text).with_context(|| format!("in {}", path.display()))
    }
}

/// Minimal RFC 4180 reader: comma separator, `"` quoting, CRLF or LF line ends.
fn parse_csv(text: &str) -> anyhow::Result<Vec<Vec<String>>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut chars = text.chars().peekable();
    let mut quoted = false;
    let mut at_field_start = true;
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    chars.next();
                    field.push('"');
                }
                '"' => quoted = false,
                _ => field.push(c),
            }
            continue;
        }
        match c {
            '"' if at_field_start => {
                quoted = true;
                at_field_start = false;
            }
            ',' => {
                row.push(std::mem::take(&mut field));
                at_field_start = true;
            }
            '\r' if chars.peek() == Some(&'\n') => {}
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
                at_field_start = true;
            }
            _ => {
                field.push(c);
                at_field_start = false;
            }
        }
    }
    if quoted {
        bail!("unterminated quoted field");
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quoted_fields() {
        let t = StringTable::parse("\u{feff}Key,SourceString\r\n\"a\",\"x, \"\"y\"\"\nz\"\nb,plain\n").unwrap();
        assert_eq!(t.entries["a"], "x, \"y\"\nz");
        assert_eq!(t.entries["b"], "plain");
        assert_eq!(t.entries.len(), 2);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(StringTable::parse("Nope\n").is_err());
        assert!(StringTable::parse("Key,SourceString\n\"open").is_err());
        assert!(StringTable::parse("Key,SourceString\na,1\na,2\n").is_err());
        assert!(StringTable::parse("Key,SourceString\na,1,2\n").is_err());
    }

    #[test]
    fn sniffs_header() {
        assert!(StringTable::sniff(b"\xEF\xBB\xBFKey,SourceString\n"));
        assert!(!StringTable::sniff(b"PK\x03\x04"));
    }
}
