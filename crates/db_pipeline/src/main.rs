//! `db_pipeline <command>`: converts extracted data into the game cache under `_data/cache/`.
//! Incremental and offline.

mod classes_doc;
mod config;
mod items_doc;
mod loot_doc;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str).unwrap_or("") {
        "config" => config::run(&config::Options::from_args(&args[1..])?),
        "classes-doc" => classes_doc::run("_data/cache/config".as_ref(), "docs/systems/CLASSES.md".as_ref()),
        "items-doc" => items_doc::run("_data/cache/config".as_ref(), "docs/systems/ITEMS.md".as_ref()),
        "loot-doc" => loot_doc::run("_data/cache/config".as_ref(), "docs/systems/LOOT.md".as_ref()),
        "nav" => {
            // Walkable grid per map level from its collision cache (levels by key or name).
            // `--placement-bounds <m>`: only over the levels' gameplay points grown by that margin.
            let mut levels: Vec<&String> = args[1..].iter().collect();
            let mut margin = None;
            if let Some(k) = levels.iter().position(|a| *a == "--placement-bounds") {
                margin = Some(levels.get(k + 1).ok_or_else(|| anyhow::anyhow!("--placement-bounds needs a margin in metres"))?.parse::<f32>()? * 100.0);
                levels.drain(k..k + 2);
            }
            for floor in levels {
                let t = std::time::Instant::now();
                let grid = db_sim::nav::bake_floor_within(floor, 100.0, margin)?;
                let path = format!("_data/collision/{}-nav.bin", db_sim::crypt::level_name(floor));
                std::fs::write(&path, grid.write())?;
                let mut sizes = grid.component_sizes();
                sizes.sort_unstable_by(|a, b| b.cmp(a));
                println!("floor {floor}: {} nodes, {} edges, largest components {:?}; {:.1} s -> {path}", grid.nodes.len(), grid.edges.len() / 2, &sizes[..sizes.len().min(5)], t.elapsed().as_secs_f32());
            }
            Ok(())
        }
        "" | "help" | "--help" => {
            println!(
                "usage: db_pipeline <command>\n  config [--excel DIR] [--carved DIR] [--out DIR] [--force]\n    \
                 typed config tables + string tables -> _data/cache/config/\n  classes-doc\n    \
                 class sheets and starting gear -> docs/systems/CLASSES.md\n  items-doc\n    \
                 seeded item/affix roll report -> docs/systems/ITEMS.md\n  loot-doc\n    \
                 loot source drop-rate report -> docs/systems/LOOT.md
  nav [--placement-bounds <m>] <level>...
                     walkable grid of a map level (01, 02, castle1 or a full name) -> _data/collision/<level>-nav.bin;
                     with --placement-bounds only over its gameplay points grown by that margin"
            );
            Ok(())
        }
        other => anyhow::bail!("unknown command `{other}`"),
    }
}
