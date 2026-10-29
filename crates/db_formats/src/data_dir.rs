//! Where the gitignored game data (`_data/`) is read from.
//!
//! One folder, chosen once and as a whole: `DUNGEONBORNE_DATA` if set, else `_data` beside the
//! executable (a packaged copy, `_dist/`), else `_data` in the working directory (runs from the repo
//! root), else the repo's own `_data` (tests and development builds run from anywhere). Because the
//! folder is chosen as a whole, a packaged copy never reads a single missing file from the repo.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The `_data` folder in use.
pub fn data_root() -> &'static Path {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let env = std::env::var_os("DUNGEONBORNE_DATA").map(PathBuf::from);
        let exe = std::env::current_exe().ok().and_then(|e| e.parent().map(|d| d.join("_data")));
        let cwd = Some(PathBuf::from("_data"));
        // Development builds and tests (run in the crate's folder) also look in the repo; release builds don't
        // carry the build folder's path.
        #[cfg(debug_assertions)]
        let repo = Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../_data"));
        #[cfg(not(debug_assertions))]
        let repo: Option<PathBuf> = None;
        let pick = [env, exe, cwd, repo].into_iter().flatten().find(|p| p.is_dir()).unwrap_or_else(|| PathBuf::from("_data"));
        std::fs::canonicalize(&pick).unwrap_or(pick)
    })
}

/// A path under the `_data` folder in use.
pub fn data_path(rel: impl AsRef<Path>) -> PathBuf {
    data_root().join(rel)
}
