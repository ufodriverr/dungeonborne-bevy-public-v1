# Building the game data

The game reads everything it shows and plays from a `_data` folder made from **your own**
Dungeonborne install. This repository ships none of it, and there is no one-click extractor for
it yet: the project's own extraction scripts aren't published (they may come later as a separate
project). This page is for people willing to build the folder themselves: what it holds, in what
form, and how it was made.

Keep what you extract for yourself. Do not share or upload it, and do not ask for it in issues.

## What you need

- **Dungeonborne on PC (Steam), installed yourself.** The game data is in
  `Dungeonborne/Content/Paks` (Unreal Engine pak and IoStore containers). Only read it; never
  modify the install, and never run the game, its anti-cheat or anything that attaches to it for
  this.
- **The AES key** of the encrypted containers. It is not included here, and this project does not
  say where to get it.
- **Mappings** (a `.usmap` file) for this build. Its packages use unversioned properties, so
  readers can't decode them without one. None is included here.
- A reader for Unreal packages that takes both, such as [CUE4Parse](https://github.com/FabianFG/CUE4Parse)
  (the library used for this project's data) or a tool built on it.
- [vgmstream](https://github.com/vgmstream/vgmstream) and ffmpeg (with libvorbis) for the sounds.
- About 50 GB free for the work, and about 7 GB for the finished `_data`.

## What the `_data` folder holds

The game finds the folder through `DUNGEONBORNE_DATA`, else `_data` next to the exe, else `_data`
in the working directory (`crates/db_formats/src/data_dir.rs`). Paths below are relative to it.

| Path | What | Form |
| --- | --- | --- |
| `assets/` | models, skeletons, animation clips, map scenes and cell scenes, textures, fonts, UI art | glTF 2.0 (`.gltf` + `.bin`, `.glb`) and PNG, kept in the game's own folder layout (`maps/Dungeonborne/...`, `animations/...`); glTF URIs are relative, so the tree must stay whole. Meshes are in metres, Y up. |
| `audio/ogg/` | every sound | OGG Vorbis, one file per Wwise media id |
| `audio/index.json` | the sound bank index: event name -> media ids, limits, 2D/3D | the project's JSON |
| `cache/config/tables.json` | the decoded config tables (classes, skills, items, affixes, loot, merchant, monsters' curves, strings) | made by `db_pipeline config` (see below) |
| `json/assets/Dungeonborne/Content/TGF/Character/Hero/`, `.../Character/Monster/`, `.../Ability/Data/Attributes/MonsterAttributeCurve.json` | class skill values, monster montages, ability sets and Blueprint defaults | the reader's JSON export of those packages (property names as in the game) |
| `recovered/maps/` | per map: lights, player starts, gameplay placement (spawn points, chests, traps, portals), placed effects, outdoor light, the run layout | the project's JSON |
| `recovered/ai/` | the monsters' behaviour trees | the project's JSON |
| `recovered/materials/`, `recovered/effects/`, `recovered/fx_materials/` | material overrides, Niagara/Cascade effect data, effect material formulas | the project's JSON and WGSL |
| `recovered/*.json` (root) | footstep surfaces, sound limits, emitter sounds, root motion, monster Blueprint values, destroyables | the project's JSON |
| `collision/` | per map: collision meshes and walkable grids | binary; the formats are in `crates/db_sim/src/map.rs` and `nav.rs` |
| `save/` | your profile and settings | written by the game |

The project's JSON files have no outside spec yet; the code that reads each one is its
reference (search the crates for the file name).

## How it was made

1. **Mount** the containers with the AES key and the mappings, and list every package.
2. **Config tables:** the game's design tables are protobuf messages inside the paks. Their schemas
   come from the executable's embedded protobuf descriptors; the decoded tables go to JSON, and
   `cargo run --release -p db_pipeline -- config --excel <tables json dir> --carved <string tables dir>`
   turns them into `cache/config/tables.json`.
3. **Models, animations and maps:** static and skeletal meshes, skeletons and animation sequences
   are exported to glTF with their textures as PNG; map levels and their World Partition cells
   become glTF scenes that place those meshes. Textures above 1024 px were scaled down.
4. **Gameplay data:** levels, Blueprints, montages, ability sets, gameplay effects, behaviour trees,
   materials and effects are exported to JSON and reduced to the `recovered/` files above.
5. **Collision:** the maps' collision meshes go to `collision/<level>.bin`; then
   `cargo run --release -p db_pipeline -- nav 01 02 castle1 castle2` bakes the walkable grids.
6. **Sounds:** the Wwise banks and media are converted to OGG with vgmstream and ffmpeg, and the
   banks' events indexed into `audio/index.json`.

Expect gaps: some packages don't decode with any mappings found so far, and the game then uses
stand-ins.
