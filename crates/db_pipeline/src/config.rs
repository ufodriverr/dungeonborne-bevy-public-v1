//! `db_pipeline config`: decoded config tables and carved string tables → `_data/cache/config/`.
//!
//! Every table is parsed with the strict generated types (unknown fields are errors), every string
//! table is parsed, and the skill text of every class is checked before anything is written.
//! Output: `tables.json` ([`ConfigTables`]), `strings.json` (file → key → text), `manifest.json`.
//! The run is skipped when the input hash in the manifest is unchanged.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use db_formats::ConfigTables;
use db_formats::config::generated::EnRoleCareerType;
use db_formats::strings::StringTable;
use serde::{Deserialize, Serialize};

/// Bump when the output format or the checks change, so old caches are rebuilt.
const PIPELINE_VERSION: u32 = 1;

#[derive(Debug)]
pub struct Options {
    pub excel: PathBuf,
    pub carved: PathBuf,
    pub out: PathBuf,
    pub force: bool,
}

impl Options {
    pub fn from_args(args: &[String]) -> anyhow::Result<Self> {
        let mut o = Self {
            excel: "_data/json/excel".into(),
            carved: "_data/carved".into(),
            out: "_data/cache/config".into(),
            force: false,
        };
        let mut it = args.iter();
        while let Some(a) = it.next() {
            match a.as_str() {
                "--excel" => o.excel = value(&mut it, a)?,
                "--carved" => o.carved = value(&mut it, a)?,
                "--out" => o.out = value(&mut it, a)?,
                "--force" => o.force = true,
                _ => bail!("unknown option `{a}`"),
            }
        }
        Ok(o)
    }
}

fn value<'a>(it: &mut impl Iterator<Item = &'a String>, flag: &str) -> anyhow::Result<PathBuf> {
    it.next().map(PathBuf::from).with_context(|| format!("{flag} needs a value"))
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Manifest {
    pipeline_version: u32,
    input_hash: String,
    tables: usize,
    rows: usize,
    string_files: Vec<String>,
    strings: usize,
}

/// All inputs in a stable order: excel JSON files, then string tables found by their header.
fn inputs(o: &Options) -> anyhow::Result<(Vec<PathBuf>, Vec<PathBuf>)> {
    let mut excel = Vec::new();
    for e in std::fs::read_dir(&o.excel).with_context(|| format!("reading {}", o.excel.display()))? {
        let p = e?.path();
        if p.extension().is_some_and(|x| x == "json") {
            excel.push(p);
        }
    }
    excel.sort();
    let mut chunks: Vec<PathBuf> = std::fs::read_dir(&o.carved)
        .with_context(|| format!("reading {}", o.carved.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    chunks.sort();
    let mut strings = Vec::new();
    for dir in chunks {
        for e in std::fs::read_dir(&dir)? {
            let p = e?.path();
            if p.is_file() && is_string_table(&p)? {
                strings.push(p);
            }
        }
    }
    strings.sort();
    Ok((excel, strings))
}

fn is_string_table(p: &Path) -> anyhow::Result<bool> {
    use std::io::Read;
    let mut head = [0u8; 24];
    let n = std::fs::File::open(p)?.read(&mut head)?;
    Ok(StringTable::sniff(&head[..n]))
}

/// FNV-1a 64 over (relative name, length, bytes) of every input plus the pipeline version.
fn hash_inputs(files: &[PathBuf], base: &Path) -> anyhow::Result<String> {
    let mut h = Fnv::new();
    h.write(&PIPELINE_VERSION.to_le_bytes());
    for f in files {
        let name = f.strip_prefix(base).unwrap_or(f).to_string_lossy().replace('\\', "/");
        let bytes = std::fs::read(f).with_context(|| format!("reading {}", f.display()))?;
        h.write(name.as_bytes());
        h.write(&(bytes.len() as u64).to_le_bytes());
        h.write(&bytes);
    }
    Ok(format!("{:016x}", h.0))
}

struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 ^= u64::from(*b);
            self.0 = self.0.wrapping_mul(0x0100_0000_01b3);
        }
    }
}

/// The 8 playable adventurer classes (`ExcelRoleType`, F11).
const CLASSES: [EnRoleCareerType; 8] = [
    EnRoleCareerType::Fighter,
    EnRoleCareerType::Swordmaster,
    EnRoleCareerType::Priest,
    EnRoleCareerType::Rogue,
    EnRoleCareerType::Wizard,
    EnRoleCareerType::Deathknight,
    EnRoleCareerType::Lich,
    EnRoleCareerType::DruidPanther,
];

/// Every class has a name string, and every skill row has its name and description strings.
fn check_skill_text(t: &ConfigTables, strings: &BTreeMap<String, String>) -> anyhow::Result<()> {
    let mut missing = Vec::new();
    for career in CLASSES {
        let rows: Vec<_> = t
            .excel_role_type
            .iter()
            .filter(|r| r.meta.as_ref().is_some_and(|m| m.career == career))
            .collect();
        if rows.is_empty() {
            missing.push(format!("class {career:?}: no ExcelRoleType row"));
        }
        for r in rows {
            if !strings.contains_key(&r.str_name) {
                missing.push(format!("class {career:?}: name `{}`", r.str_name));
            }
        }
    }
    for s in &t.excel_role_skill {
        for key in [&s.skill_name, &s.skill_descr] {
            if !strings.contains_key(key) {
                missing.push(format!("skill {}: `{key}`", s.skill_id));
            }
        }
    }
    if !missing.is_empty() {
        bail!("missing skill/class text:\n  {}", missing.join("\n  "));
    }
    Ok(())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("renaming to {}", path.display()))
}

pub fn run(o: &Options) -> anyhow::Result<()> {
    let (excel, string_files) = inputs(o)?;
    let all: Vec<PathBuf> = excel.iter().chain(&string_files).cloned().collect();
    let base = o.excel.parent().and_then(Path::parent).unwrap_or(Path::new("."));
    let hash = hash_inputs(&all, base)?;
    let manifest_path = o.out.join("manifest.json");
    if !o.force
        && let Ok(text) = std::fs::read_to_string(&manifest_path)
        && let Ok(m) = serde_json::from_str::<Manifest>(&text)
        && m.input_hash == hash
        && m.pipeline_version == PIPELINE_VERSION
    {
        println!("config: up to date ({hash})");
        return Ok(());
    }

    let tables = ConfigTables::load(&o.excel)?;
    let mut by_file = BTreeMap::new();
    let mut merged = BTreeMap::new();
    for f in &string_files {
        let t = StringTable::load(f)?;
        for (k, v) in &t.entries {
            merged.entry(k.clone()).or_insert_with(|| v.clone());
        }
        let name = f.strip_prefix(&o.carved).unwrap_or(f).to_string_lossy().replace('\\', "/");
        by_file.insert(name, t.entries);
    }
    check_skill_text(&tables, &merged)?;

    let tables_json = serde_json::to_vec(&tables)?;
    let rows = serde_json::from_slice::<BTreeMap<String, Vec<serde_json::Value>>>(&tables_json)?
        .values()
        .map(Vec::len)
        .sum();
    let manifest = Manifest {
        pipeline_version: PIPELINE_VERSION,
        input_hash: hash.clone(),
        tables: db_formats::config::TABLES.len(),
        rows,
        string_files: by_file.keys().cloned().collect(),
        strings: merged.len(),
    };
    std::fs::create_dir_all(&o.out)?;
    write_atomic(&o.out.join("tables.json"), &tables_json)?;
    write_atomic(&o.out.join("strings.json"), &serde_json::to_vec(&by_file)?)?;
    write_atomic(&manifest_path, &serde_json::to_vec_pretty(&manifest)?)?;
    println!(
        "config: {} tables, {} rows, {} string tables ({} keys) -> {} ({hash})",
        manifest.tables,
        manifest.rows,
        manifest.string_files.len(),
        manifest.strings,
        o.out.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use db_formats::config::generated::{DRoleMetaIndex, ExcelRoleSkill, ExcelRoleType};

    #[test]
    fn fnv_is_stable() {
        let mut h = Fnv::new();
        h.write(b"a");
        assert_eq!(h.0, 0xaf63_dc4c_8601_ec8c);
    }

    #[test]
    fn skill_text_check_reports_missing_keys() {
        let mut t = ConfigTables::default();
        for (i, career) in CLASSES.iter().enumerate() {
            t.excel_role_type.push(ExcelRoleType {
                meta: Some(DRoleMetaIndex { career: *career, ..Default::default() }),
                str_name: format!("CareerName_{i}"),
                ..Default::default()
            });
        }
        t.excel_role_skill.push(ExcelRoleSkill {
            skill_id: 101001,
            skill_name: "Skill_Name_101001".into(),
            skill_descr: "Skill_Description_101001".into(),
            ..Default::default()
        });
        let mut strings: BTreeMap<String, String> = (0..8).map(|i| (format!("CareerName_{i}"), "x".into())).collect();
        strings.insert("Skill_Name_101001".into(), "x".into());
        let err = check_skill_text(&t, &strings).unwrap_err().to_string();
        assert!(err.contains("Skill_Description_101001"), "{err}");
        strings.insert("Skill_Description_101001".into(), "x".into());
        check_skill_text(&t, &strings).unwrap();
    }
}
