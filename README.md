# Dungeonborne on Bevy

A free, unofficial fan-made engine reimplementation of **Dungeonborne**, written in Rust on
[Bevy](https://bevyengine.org). This is a personal, noncommercial project made out of love for
the game, for learning and preservation. It is single-player and fully offline: it never
connects to the game's servers and has nothing to do with the game's anti-cheat.

**No game files are included.** The game reads its maps, characters, animations, effects and
sounds from data you extract yourself from Dungeonborne installed on your own PC. It does not
run the original client's code. See [LEGAL.md](LEGAL.md).

## Version 0.8.0

The first public release. It isn't finished, but it plays:

- **Maps:** the Crypt (both floors) and the Gothic Castle (both floors), streamed in cells with
  their own lights, traps, doors, chests, altars and extraction portals. Plus a **Test Arena**
  with every monster, every weapon on tables and class switching (F1-F8).
- **Classes:** Fighter, Swordmaster, Priest, Rogue, Wizard, Death Knight, Lich and Druid, with
  their Q/E skills, weapon combos, heavy attacks, blocking, staffs and crossbows, and the skills'
  own effects and sounds.
- **Monsters** run their behaviour trees and attacks from the game data. Bosses: Werewolf, Reaper,
  Cyclops, Wendigo, Shadow King, Khazra, Executioner, Beholder and Cursed King.
- **Runs:** chests and mimics, items with affixes, potions, flasks, torches, the stash, the
  merchant, gold, extraction or death.
- **Settings:** resolution, window mode, quality presets, field of view, mouse sensitivity,
  volumes and key rebinding.

Expect bugs and differences from the original. Values the game data doesn't give are best
guesses, marked `HYPOTHESIS` in the code. Controls are in [HOW-TO-PLAY.md](HOW-TO-PLAY.md);
changes are in the [release notes](RELEASE-NOTES.md).

## Get the game data

The game needs a `_data` folder built from your own install. There is no one-click extractor
yet: [EXTRACTION.md](EXTRACTION.md) describes what the folder holds and how it is made, for
anyone willing to build it. Keep that data for yourself: do not share or upload it.

This repository contains no code that decrypts game files and no keys or mappings.

## Play on Windows

Extract the entire Windows x64 release ZIP to a writable folder, put your `_data` folder next to
`dungeonborne.exe` (or set the `DUNGEONBORNE_DATA` environment variable to its path) and start
`dungeonborne.exe`. Without data it says so and closes. Your character and settings are saved
in `_data/save/`. No Rust installation is needed for the packaged build.

## Build from source

Install Rust stable (1.95 or newer) for `x86_64-pc-windows-msvc` and the Visual Studio
**Desktop development with C++** workload, including the Windows SDK. From this directory:

```powershell
cargo build --locked --release -p db_game
```

The executable is `target/release/dungeonborne.exe`; it also finds a `_data` folder in this
directory. See [BUILDING.md](BUILDING.md) for the release build and
[THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md) for the dependencies' licences.

| Crate | What it does |
| --- | --- |
| `db_formats` | reads the data files and finds the `_data` folder |
| `db_sim` | the game rules: movement, combat, skills, monsters and their AI, items, loot, runs (fixed timestep, seeded) |
| `db_client` | everything you see and hear: maps, models, animations, effects, sound, menus, HUD |
| `db_game` | the executable |
| `db_pipeline` | turns extracted config tables into the game's cache and bakes walkable grids |

## Licence and affiliation

Copyright (c) 2026 ufodriverr and contributors. Original project code and documentation are
licensed under [GNU GPL version 3 only](LICENSE) (`GPL-3.0-only`), without warranty.

This project is not affiliated with, sponsored by or endorsed by Mithril Interactive or any other
rights holder of Dungeonborne. "Dungeonborne" and other names identify compatibility and belong
to their respective owners. Rights-holder contact and removal: see [LEGAL.md](LEGAL.md). Credits:
[CREDITS.md](CREDITS.md).
