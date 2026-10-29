# Building

## Requirements

- Rust stable 1.95 or newer for `x86_64-pc-windows-msvc` (edition 2024).
- Visual Studio **Desktop development with C++** workload, including the Windows SDK.

The first build takes several minutes (Bevy and its dependencies).

## Development build

```powershell
cargo run --locked --release -p db_game
```

`cargo test --workspace` runs the unit tests; many of them read the game data and need a
`_data` folder in this directory.

## Release build

```powershell
.\build-release.ps1
```

It builds `target/release/dungeonborne.exe` with the local user profile and the source folder
remapped out of the paths compiled into the binary, then assembles
`dist/dungeonborne-bevy-<version>-windows-x64.zip` with the executable and the project's notices
(README, HOW-TO-PLAY, EXTRACTION, LEGAL, CREDITS, RELEASE-NOTES, LICENSE, THIRD-PARTY-NOTICES and
the crates' licence texts in `licenses/`). `-Offline` passes `--offline` to Cargo.

GPLv3 asks that a binary come with its Corresponding Source: publish the release ZIP next to the
tagged source of the same version (GitHub attaches the source archive to every release).

The release ZIP contains no game files. Never zip a folder that holds `_data`.

## Third-party notices

`THIRD-PARTY-NOTICES.md` lists every crate of the Windows x64 dependency graph with its licence,
from `cargo metadata`, and `licenses/` keeps each crate's own licence and notice files. Regenerate
both after changing dependencies.
