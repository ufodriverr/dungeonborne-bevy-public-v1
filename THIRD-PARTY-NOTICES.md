# Third-party notices

Dungeonborne on Bevy is licensed under GPL-3.0-only (see `LICENSE`). It is built from the Rust crates
below (the Windows x64 dependency graph of `Cargo.lock`), each under its own licence as declared
in its package metadata, reproduced here as declared (including legacy slash notation). Where a
crate offers alternatives, a GPLv3-compatible one applies. The crates' own licence and notice
files are kept in `licenses/<crate>-<version>/`, and ship with the release.

These crates publish no licence file of their own; their declared licence applies, with the
standard text of that licence (MIT and Apache-2.0 in `licenses/_standard-texts/`; BSL-1.0,
CC0-1.0 and MIT-0 ask for no notice in a binary): `accesskit-0.24.1` (MIT OR Apache-2.0), `accesskit_consumer-0.35.0` (MIT OR Apache-2.0), `accesskit_windows-0.32.1` (MIT OR Apache-2.0), `accesskit_winit-0.32.2` (Apache-2.0), `avian_derive-0.2.3` (MIT OR Apache-2.0), `bevy_scene_macros-0.19.1` (MIT OR Apache-2.0), `clipboard-win-5.4.1` (BSL-1.0), `constgebra-0.1.4` (MIT OR Apache-2.0), `dasp_sample-0.11.0` (MIT OR Apache-2.0), `encase_derive-0.12.1` (MIT-0), `encase_derive_impl-0.12.1` (MIT-0), `gilrs-0.11.2` (Apache-2.0/MIT), `gilrs-core-0.6.8` (Apache-2.0/MIT), `gl_generator-0.14.0` (Apache-2.0), `gpu-descriptor-0.3.2` (MIT OR Apache-2.0), `gpu-descriptor-types-0.2.0` (MIT OR Apache-2.0), `hexf-parse-0.2.1` (CC0-1.0), `khronos_api-3.1.0` (Apache-2.0), `profiling-1.0.18` (MIT OR Apache-2.0), `rstar-0.12.2` (MIT OR Apache-2.0), `spirv-0.4.0+sdk-1.4.341.0` (Apache-2.0), `svg_fmt-0.4.5` (MIT/Apache-2.0), `taffy-0.10.1` (MIT).

## Licence summary

| Licence | Crates |
| --- | --- |
| MIT OR Apache-2.0 | 238 |
| MIT | 34 |
| Apache-2.0 OR MIT | 33 |
| Unicode-3.0 | 22 |
| Apache-2.0 | 14 |
| MIT/Apache-2.0 | 8 |
| Unlicense OR MIT | 6 |
| Zlib | 6 |
| Zlib OR Apache-2.0 OR MIT | 5 |
| MIT OR Apache-2.0 OR Zlib | 3 |
| MIT-0 | 3 |
| Apache-2.0/MIT | 3 |
| BSL-1.0 | 2 |
| MIT OR Zlib OR Apache-2.0 | 2 |
| BSD-3-Clause OR Apache-2.0 | 2 |
| BSD-3-Clause | 2 |
| BSD-2-Clause OR Apache-2.0 OR MIT | 2 |
| 0BSD OR MIT OR Apache-2.0 | 1 |
| Zlib AND (MIT OR Apache-2.0) | 1 |
| CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception | 1 |
| CC0-1.0 OR MIT-0 OR Apache-2.0 | 1 |
| Apache-2.0 AND MIT | 1 |
| Apache-2.0 / MIT | 1 |
| CC0-1.0 | 1 |
| ISC | 1 |
| (MIT OR Apache-2.0) AND Unicode-3.0 | 1 |

## Crates

| Crate | Version | Licence | Repository |
| --- | --- | --- | --- |
| accesskit | 0.24.1 | MIT OR Apache-2.0 | https://github.com/AccessKit/accesskit |
| accesskit_consumer | 0.35.0 | MIT OR Apache-2.0 | https://github.com/AccessKit/accesskit |
| accesskit_windows | 0.32.1 | MIT OR Apache-2.0 | https://github.com/AccessKit/accesskit |
| accesskit_winit | 0.32.2 | Apache-2.0 | https://github.com/AccessKit/accesskit |
| adler2 | 2.0.1 | 0BSD OR MIT OR Apache-2.0 | https://github.com/oyvindln/adler2 |
| aho-corasick | 1.1.5 | Unlicense OR MIT | https://github.com/BurntSushi/aho-corasick |
| allocator-api2 | 0.2.21 | MIT OR Apache-2.0 | https://github.com/zakarumych/allocator-api2 |
| anyhow | 1.0.104 | MIT OR Apache-2.0 | https://github.com/dtolnay/anyhow |
| approx | 0.5.1 | Apache-2.0 | https://github.com/brendanzab/approx |
| arbitrary-chunks | 0.4.1 | Apache-2.0 OR MIT | https://github.com/Nessex/arbitrary-chunks |
| arboard | 3.6.1 | MIT OR Apache-2.0 | https://github.com/1Password/arboard |
| arrayvec | 0.7.8 | MIT OR Apache-2.0 | https://github.com/bluss/arrayvec |
| ash | 0.38.0+1.3.281 | MIT OR Apache-2.0 | https://github.com/ash-rs/ash |
| assert_type_match | 0.1.1 | MIT OR Apache-2.0 | https://github.com/MrGVSV/assert_type_match |
| async-broadcast | 0.7.2 | MIT OR Apache-2.0 | https://github.com/smol-rs/async-broadcast |
| async-channel | 2.5.0 | Apache-2.0 OR MIT | https://github.com/smol-rs/async-channel |
| async-executor | 1.14.0 | Apache-2.0 OR MIT | https://github.com/smol-rs/async-executor |
| async-fs | 2.2.0 | Apache-2.0 OR MIT | https://github.com/smol-rs/async-fs |
| async-lock | 3.4.2 | Apache-2.0 OR MIT | https://github.com/smol-rs/async-lock |
| async-task | 4.7.1 | Apache-2.0 OR MIT | https://github.com/smol-rs/async-task |
| atomic-waker | 1.1.2 | Apache-2.0 OR MIT | https://github.com/smol-rs/atomic-waker |
| atomicow | 1.2.0 | MIT OR Apache-2.0 | https://github.com/bevyengine/atomicow |
| autocfg | 1.5.1 | Apache-2.0 OR MIT | https://github.com/cuviper/autocfg |
| avian3d | 0.7.0 | MIT OR Apache-2.0 | https://github.com/avianphysics/avian |
| avian_derive | 0.2.3 | MIT OR Apache-2.0 | https://github.com/avianphysics/avian |
| base64 | 0.22.1 | MIT OR Apache-2.0 | https://github.com/marshallpierce/rust-base64 |
| bevy | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_a11y | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_animation | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_animation_macros | 0.19.1 | MIT OR Apache-2.0 |  |
| bevy_anti_alias | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_app | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_asset | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_asset_macros | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_audio | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_camera | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_clipboard | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_color | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_core_pipeline | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_derive | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_dev_tools | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_diagnostic | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_ecs | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_ecs_macro_logic | 0.19.1 | MIT OR Apache-2.0 |  |
| bevy_ecs_macros | 0.19.1 | MIT OR Apache-2.0 |  |
| bevy_encase_derive | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_feathers | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_gilrs | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_gizmos | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_gizmos_macros | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_gizmos_render | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_gltf | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_heavy | 0.5.0 | MIT OR Apache-2.0 | https://github.com/Jondolf/bevy_heavy |
| bevy_image | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_input | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_input_focus | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_internal | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_light | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_log | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_macro_utils | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_material | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_material_macros | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_math | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_mesh | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_mikktspace | 1.0.0 | Zlib AND (MIT OR Apache-2.0) | https://github.com/bevyengine/bevy_mikktspace |
| bevy_pbr | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_picking | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_platform | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_post_process | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_ptr | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_reflect | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_reflect_derive | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_render | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_render_macros | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_scene | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_scene_macros | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_shader | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_sprite | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_sprite_render | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_state | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_state_macros | 0.19.1 | MIT OR Apache-2.0 |  |
| bevy_tasks | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_text | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_time | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_transform | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_transform_interpolation | 0.5.0 | MIT OR Apache-2.0 | https://github.com/Jondolf/bevy_transform_interpolation |
| bevy_ui | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_ui_render | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_ui_widgets | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_utils | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_window | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_winit | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bevy_world_serialization | 0.19.1 | MIT OR Apache-2.0 | https://github.com/bevyengine/bevy |
| bit-set | 0.9.1 | Apache-2.0 OR MIT | https://github.com/contain-rs/bit-set |
| bit-vec | 0.9.1 | Apache-2.0 OR MIT | https://github.com/contain-rs/bit-vec |
| bitflags | 2.13.2 | MIT OR Apache-2.0 | https://github.com/bitflags/bitflags |
| blake3 | 1.8.7 | CC0-1.0 OR Apache-2.0 OR Apache-2.0 WITH LLVM-exception | https://github.com/BLAKE3-team/BLAKE3 |
| blocking | 1.7.0 | Apache-2.0 OR MIT | https://github.com/smol-rs/blocking |
| bumpalo | 3.20.3 | MIT OR Apache-2.0 | https://github.com/fitzgen/bumpalo |
| bytemuck | 1.25.2 | Zlib OR Apache-2.0 OR MIT | https://github.com/Lokathor/bytemuck |
| bytemuck_derive | 1.12.1 | Zlib OR Apache-2.0 OR MIT | https://github.com/Lokathor/bytemuck |
| byteorder | 1.5.0 | Unlicense OR MIT | https://github.com/BurntSushi/byteorder |
| byteorder-lite | 0.1.0 | Unlicense OR MIT | https://github.com/image-rs/byteorder-lite |
| cc | 1.6.0 | MIT OR Apache-2.0 | https://github.com/rust-lang/cc-rs |
| cfg-if | 1.0.5 | MIT OR Apache-2.0 | https://github.com/rust-lang/cfg-if |
| cfg_aliases | 0.2.2 | MIT | https://github.com/katharostech/cfg_aliases |
| clipboard-win | 5.4.1 | BSL-1.0 | https://github.com/DoumanAsh/clipboard-win |
| codespan-reporting | 0.12.0 | Apache-2.0 | https://github.com/brendanzab/codespan |
| codespan-reporting | 0.13.1 | Apache-2.0 | https://github.com/brendanzab/codespan |
| concurrent-queue | 2.5.0 | Apache-2.0 OR MIT | https://github.com/smol-rs/concurrent-queue |
| const-fnv1a-hash | 1.1.0 | MIT | https://github.com/HindrikStegenga/const-fnv1a-hash |
| const_panic | 0.2.17 | Zlib | https://github.com/rodrimati1992/const_panic/ |
| const_soft_float | 0.1.4 | MIT OR Apache-2.0 | https://github.com/823984418/const_soft_float |
| constant_time_eq | 0.4.2 | CC0-1.0 OR MIT-0 OR Apache-2.0 | https://github.com/cesarb/constant_time_eq |
| constgebra | 0.1.4 | MIT OR Apache-2.0 | https://github.com/knickish/constgebra |
| convert_case | 0.10.0 | MIT | https://github.com/rutrum/convert-case |
| core_maths | 0.1.1 | MIT | https://github.com/robertbastian/core_maths |
| cpal | 0.17.3 | Apache-2.0 | https://github.com/RustAudio/cpal |
| cpufeatures | 0.3.1 | MIT OR Apache-2.0 | https://github.com/RustCrypto/utils |
| crc32fast | 1.5.2 | MIT OR Apache-2.0 | https://github.com/srijs/rust-crc32fast |
| critical-section | 1.2.0 | MIT OR Apache-2.0 | https://github.com/rust-embedded/critical-section |
| crossbeam-channel | 0.5.17 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam |
| crossbeam-queue | 0.3.14 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam |
| crossbeam-utils | 0.8.23 | MIT OR Apache-2.0 | https://github.com/crossbeam-rs/crossbeam |
| ctrlc | 3.5.2 | MIT/Apache-2.0 | https://github.com/Detegr/rust-ctrlc.git |
| cursor-icon | 1.2.0 | MIT OR Apache-2.0 OR Zlib | https://github.com/rust-windowing/cursor-icon |
| dasp_sample | 0.11.0 | MIT OR Apache-2.0 | https://github.com/rustaudio/sample.git |
| data-encoding | 2.11.1 | MIT | https://github.com/ia0/data-encoding |
| derive_more | 2.1.1 | MIT | https://github.com/JelteF/derive_more |
| derive_more-impl | 2.1.1 | MIT | https://github.com/JelteF/derive_more |
| displaydoc | 0.2.7 | MIT OR Apache-2.0 | https://github.com/yaahc/displaydoc |
| disqualified | 1.1.0 | MIT OR Apache-2.0 | https://github.com/bevyengine/disqualified |
| document-features | 0.2.12 | MIT OR Apache-2.0 | https://github.com/slint-ui/document-features |
| downcast-rs | 2.0.2 | MIT OR Apache-2.0 | https://github.com/marcianx/downcast-rs |
| dpi | 0.1.2 | Apache-2.0 AND MIT | https://github.com/rust-windowing/winit |
| either | 1.19.0 | MIT OR Apache-2.0 | https://github.com/rayon-rs/either |
| ena | 0.14.4 | MIT OR Apache-2.0 | https://github.com/rust-lang/ena |
| encase | 0.12.1 | MIT-0 | https://github.com/teoxoy/encase |
| encase_derive | 0.12.1 | MIT-0 | https://github.com/teoxoy/encase |
| encase_derive_impl | 0.12.1 | MIT-0 | https://github.com/teoxoy/encase |
| equivalent | 1.0.2 | Apache-2.0 OR MIT | https://github.com/indexmap-rs/equivalent |
| erased-serde | 0.4.10 | MIT OR Apache-2.0 | https://github.com/dtolnay/erased-serde |
| error-code | 3.4.0 | BSL-1.0 | https://github.com/DoumanAsh/error-code |
| euclid | 0.22.14 | MIT OR Apache-2.0 | https://github.com/servo/euclid |
| event-listener | 5.4.2 | Apache-2.0 OR MIT | https://github.com/smol-rs/event-listener |
| event-listener-strategy | 0.5.4 | Apache-2.0 OR MIT | https://github.com/smol-rs/event-listener-strategy |
| fastrand | 2.5.0 | Apache-2.0 OR MIT | https://github.com/smol-rs/fastrand |
| fdeflate | 0.3.7 | MIT OR Apache-2.0 | https://github.com/image-rs/fdeflate |
| find-msvc-tools | 0.1.14 | MIT OR Apache-2.0 | https://github.com/rust-lang/cc-rs |
| fixedbitset | 0.5.7 | MIT OR Apache-2.0 | https://github.com/petgraph/fixedbitset |
| flate2 | 1.1.10 | MIT OR Apache-2.0 | https://github.com/rust-lang/flate2-rs |
| fnv | 1.0.7 | Apache-2.0 / MIT | https://github.com/servo/rust-fnv |
| foldhash | 0.1.5 | Zlib | https://github.com/orlp/foldhash |
| foldhash | 0.2.0 | Zlib | https://github.com/orlp/foldhash |
| font-types | 0.11.3 | MIT OR Apache-2.0 | https://github.com/googlefonts/fontations |
| font-types | 0.12.6 | MIT OR Apache-2.0 | https://github.com/googlefonts/fontations |
| fontique | 0.9.0 | Apache-2.0 OR MIT | https://github.com/linebender/parley |
| futures-core | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-io | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-lite | 2.6.1 | Apache-2.0 OR MIT | https://github.com/smol-rs/futures-lite |
| futures-macro | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-task | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| futures-util | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/futures-rs |
| getrandom | 0.4.3 | MIT OR Apache-2.0 | https://github.com/rust-random/getrandom |
| gilrs | 0.11.2 | Apache-2.0/MIT | https://gitlab.com/gilrs-project/gilrs |
| gilrs-core | 0.6.8 | Apache-2.0/MIT | https://gitlab.com/gilrs-project/gilrs |
| gl_generator | 0.14.0 | Apache-2.0 | https://github.com/brendanzab/gl-rs/ |
| glam | 0.32.1 | MIT OR Apache-2.0 | https://github.com/bitshifter/glam-rs |
| glam_matrix_extras | 0.3.0 | MIT OR Apache-2.0 | https://github.com/Jondolf/glam_matrix_extras |
| glamx | 0.2.0 | MIT OR Apache-2.0 | https://github.com/dimforge/glamx |
| glow | 0.17.0 | MIT OR Apache-2.0 OR Zlib | https://github.com/grovesNL/glow |
| gltf | 1.4.1 | MIT OR Apache-2.0 | https://github.com/gltf-rs/gltf |
| gltf-derive | 1.4.1 | MIT OR Apache-2.0 | https://github.com/gltf-rs/gltf |
| gltf-json | 1.4.1 | MIT OR Apache-2.0 | https://github.com/gltf-rs/gltf |
| glutin_wgl_sys | 0.6.1 | Apache-2.0 | https://github.com/rust-windowing/glutin |
| gpu-allocator | 0.28.0 | MIT OR Apache-2.0 | https://github.com/Traverse-Research/gpu-allocator |
| gpu-descriptor | 0.3.2 | MIT OR Apache-2.0 | https://github.com/zakarumych/gpu-descriptor |
| gpu-descriptor-types | 0.2.0 | MIT OR Apache-2.0 | https://github.com/zakarumych/gpu-descriptor |
| grid | 1.0.1 | MIT | https://github.com/becheran/grid |
| guillotiere | 0.6.2 | MIT/Apache-2.0 | https://github.com/nical/guillotiere |
| half | 2.7.1 | MIT OR Apache-2.0 | https://github.com/VoidStarKat/half-rs |
| harfrust | 0.6.2 | MIT | https://github.com/harfbuzz/harfrust |
| hash32 | 0.3.1 | MIT OR Apache-2.0 | https://github.com/japaric/hash32 |
| hashbrown | 0.15.5 | MIT OR Apache-2.0 | https://github.com/rust-lang/hashbrown |
| hashbrown | 0.16.1 | MIT OR Apache-2.0 | https://github.com/rust-lang/hashbrown |
| hashbrown | 0.17.1 | MIT OR Apache-2.0 | https://github.com/rust-lang/hashbrown |
| heapless | 0.8.0 | MIT OR Apache-2.0 | https://github.com/rust-embedded/heapless |
| hexasphere | 18.0.0 | MIT OR Apache-2.0 | https://github.com/OptimisticPeach/hexasphere.git |
| hexf-parse | 0.2.1 | CC0-1.0 | https://github.com/lifthrasiir/hexf |
| icu_collections | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_locale_core | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_locale_fallback | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_locale_fallback_data | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_normalizer | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_normalizer_data | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_properties | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_properties_data | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_provider | 2.3.1 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_segmenter | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| icu_segmenter_data | 2.3.0 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| image | 0.25.10 | MIT OR Apache-2.0 | https://github.com/image-rs/image |
| indexmap | 2.14.2 | Apache-2.0 OR MIT | https://github.com/indexmap-rs/indexmap |
| inflections | 1.1.1 | MIT | https://docs.rs/inflections |
| inventory | 0.3.25 | MIT OR Apache-2.0 | https://github.com/dtolnay/inventory |
| itertools | 0.14.0 | MIT OR Apache-2.0 | https://github.com/rust-itertools/itertools |
| itertools | 0.15.0 | MIT OR Apache-2.0 | https://github.com/rust-itertools/itertools |
| itoa | 1.0.18 | MIT OR Apache-2.0 | https://github.com/dtolnay/itoa |
| jobserver | 0.1.35 | MIT OR Apache-2.0 | https://github.com/rust-lang/jobserver-rs |
| khronos-egl | 6.0.0 | MIT/Apache-2.0 | https://github.com/timothee-haudebourg/khronos-egl |
| khronos_api | 3.1.0 | Apache-2.0 | https://github.com/brendanzab/gl-rs/ |
| ktx2 | 0.5.0 | Apache-2.0 | https://github.com/BVE-Reborn/ktx2 |
| lazy_static | 1.5.1 | MIT OR Apache-2.0 | https://github.com/rust-lang-nursery/lazy-static.rs |
| lewton | 0.10.2 | MIT OR Apache-2.0 | https://github.com/RustAudio/lewton |
| libc | 0.2.190 | MIT OR Apache-2.0 | https://github.com/rust-lang/libc |
| libloading | 0.8.9 | ISC | https://github.com/nagisa/rust_libloading/ |
| libm | 0.2.16 | MIT | https://github.com/rust-lang/compiler-builtins |
| linebender_resource_handle | 0.1.1 | Apache-2.0 OR MIT | https://github.com/linebender/raw_resource_handle |
| litemap | 0.8.3 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| litrs | 1.0.0 | MIT OR Apache-2.0 | https://github.com/LukasKalbertodt/litrs |
| lock_api | 0.4.14 | MIT OR Apache-2.0 | https://github.com/Amanieu/parking_lot |
| log | 0.4.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/log |
| matchers | 0.2.0 | MIT | https://github.com/hawkw/matchers |
| memchr | 2.8.3 | Unlicense OR MIT | https://github.com/BurntSushi/memchr |
| memmap2 | 0.9.11 | MIT OR Apache-2.0 | https://github.com/RazrFalcon/memmap2-rs |
| miniz_oxide | 0.8.9 | MIT OR Zlib OR Apache-2.0 | https://github.com/Frommi/miniz_oxide/tree/master/miniz_oxide |
| miniz_oxide | 0.9.1 | MIT OR Zlib OR Apache-2.0 | https://github.com/Frommi/miniz_oxide/tree/master/miniz_oxide |
| moxcms | 0.8.1 | BSD-3-Clause OR Apache-2.0 | https://github.com/awxkee/moxcms.git |
| naga | 29.0.4 | MIT OR Apache-2.0 | https://github.com/gfx-rs/wgpu |
| naga_oil | 0.22.0 | MIT OR Apache-2.0 | https://github.com/bevyengine/naga_oil/ |
| nonmax | 0.5.5 | MIT OR Apache-2.0 | https://github.com/LPGhatguy/nonmax |
| ntapi | 0.4.3 | Apache-2.0 OR MIT | https://github.com/MSxDOS/ntapi |
| nu-ansi-term | 0.50.3 | MIT | https://github.com/nushell/nu-ansi-term |
| num-bigint | 0.4.8 | MIT OR Apache-2.0 | https://github.com/rust-num/num-bigint |
| num-complex | 0.4.6 | MIT OR Apache-2.0 | https://github.com/rust-num/num-complex |
| num-derive | 0.4.2 | MIT OR Apache-2.0 | https://github.com/rust-num/num-derive |
| num-integer | 0.1.47 | MIT OR Apache-2.0 | https://github.com/rust-num/num-integer |
| num-rational | 0.4.2 | MIT OR Apache-2.0 | https://github.com/rust-num/num-rational |
| num-traits | 0.2.19 | MIT OR Apache-2.0 | https://github.com/rust-num/num-traits |
| obvhs | 0.3.3 | MIT OR Apache-2.0 | https://github.com/DGriffin91/obvhs |
| offset-allocator | 0.2.0 | MIT | https://github.com/pcwalton/offset-allocator/ |
| ogg | 0.8.0 | BSD-3-Clause | https://github.com/RustAudio/ogg |
| once_cell | 1.21.4 | MIT OR Apache-2.0 | https://github.com/matklad/once_cell |
| ordered-float | 5.5.0 | MIT | https://github.com/reem/rust-ordered-float |
| parking | 2.2.1 | Apache-2.0 OR MIT | https://github.com/smol-rs/parking |
| parking_lot | 0.12.5 | MIT OR Apache-2.0 | https://github.com/Amanieu/parking_lot |
| parking_lot_core | 0.9.12 | MIT OR Apache-2.0 | https://github.com/Amanieu/parking_lot |
| parlance | 0.1.1 | Apache-2.0 OR MIT | https://github.com/linebender/parley |
| parley | 0.9.0 | Apache-2.0 OR MIT | https://github.com/linebender/parley |
| parley_data | 0.9.0 | Apache-2.0 OR MIT | https://github.com/linebender/parley |
| parry3d | 0.27.0 | Apache-2.0 | https://github.com/dimforge/parry |
| parry3d-f64 | 0.27.0 | Apache-2.0 | https://github.com/dimforge/parry |
| partition | 0.1.2 | MIT OR Apache-2.0 | https://github.com/llogiq/partition |
| paste | 1.0.15 | MIT OR Apache-2.0 | https://github.com/dtolnay/paste |
| percent-encoding | 2.3.2 | MIT OR Apache-2.0 | https://github.com/servo/rust-url/ |
| petgraph | 0.8.3 | MIT OR Apache-2.0 | https://github.com/petgraph/petgraph |
| pin-project-lite | 0.2.17 | Apache-2.0 OR MIT | https://github.com/taiki-e/pin-project-lite |
| piper | 0.2.5 | MIT OR Apache-2.0 | https://github.com/smol-rs/piper |
| pkg-config | 0.3.34 | MIT OR Apache-2.0 | https://github.com/rust-lang/pkg-config-rs |
| png | 0.18.1 | MIT OR Apache-2.0 | https://github.com/image-rs/image-png |
| portable-atomic | 1.15.0 | Apache-2.0 OR MIT | https://github.com/taiki-e/portable-atomic |
| potential_utf | 0.1.6 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| pp-rs | 0.2.1 | BSD-3-Clause | https://github.com/Kangz/glslpp-rs |
| ppv-lite86 | 0.2.21 | MIT OR Apache-2.0 | https://github.com/cryptocorrosion/cryptocorrosion |
| presser | 0.3.1 | MIT OR Apache-2.0 | https://github.com/EmbarkStudios/presser |
| proc-macro-error-attr3 | 3.1.1 | MIT OR Apache-2.0 | https://github.com/gamma0987/proc-macro-error3 |
| proc-macro-error3 | 3.1.1 | MIT OR Apache-2.0 | https://github.com/gamma0987/proc-macro-error3 |
| proc-macro2 | 1.0.107 | MIT OR Apache-2.0 | https://github.com/dtolnay/proc-macro2 |
| profiling | 1.0.18 | MIT OR Apache-2.0 | https://github.com/aclysma/profiling |
| pxfm | 0.1.30 | BSD-3-Clause OR Apache-2.0 | https://github.com/awxkee/pxfm |
| quote | 1.0.47 | MIT OR Apache-2.0 | https://github.com/dtolnay/quote |
| radsort | 0.1.1 | MIT OR Apache-2.0 | https://github.com/jakubvaltar/radsort |
| rand | 0.10.3 | MIT OR Apache-2.0 | https://github.com/rust-random/rand |
| rand_chacha | 0.10.0 | MIT OR Apache-2.0 | https://github.com/rust-random/rand |
| rand_core | 0.10.1 | MIT OR Apache-2.0 | https://github.com/rust-random/rand_core |
| rand_distr | 0.6.0 | MIT OR Apache-2.0 | https://github.com/rust-random/rand_distr |
| range-alloc | 0.1.5 | MIT OR Apache-2.0 | https://github.com/gfx-rs/range-alloc |
| raw-window-handle | 0.6.2 | MIT OR Apache-2.0 OR Zlib | https://github.com/rust-windowing/raw-window-handle |
| rdst | 0.20.14 | Apache-2.0 OR MIT | https://github.com/Nessex/rdst |
| read-fonts | 0.39.2 | MIT OR Apache-2.0 | https://github.com/googlefonts/fontations |
| read-fonts | 0.41.0 | MIT OR Apache-2.0 | https://github.com/googlefonts/fontations |
| rectangle-pack | 0.4.2 | MIT/Apache-2.0 | https://github.com/chinedufn/rectangle-pack |
| regex | 1.13.1 | MIT OR Apache-2.0 | https://github.com/rust-lang/regex |
| regex-automata | 0.4.18 | MIT OR Apache-2.0 | https://github.com/rust-lang/regex |
| regex-syntax | 0.8.11 | MIT OR Apache-2.0 | https://github.com/rust-lang/regex |
| renderdoc-sys | 1.1.0 | MIT OR Apache-2.0 | https://github.com/ebkalderon/renderdoc-rs |
| robust | 1.2.0 | MIT OR Apache-2.0 | https://github.com/georust/robust |
| rodio | 0.22.2 | MIT OR Apache-2.0 | https://github.com/RustAudio/rodio |
| ron | 0.12.2 | MIT OR Apache-2.0 | https://github.com/ron-rs/ron |
| rstar | 0.12.2 | MIT OR Apache-2.0 | https://github.com/georust/rstar |
| rustc-hash | 1.1.0 | Apache-2.0/MIT | https://github.com/rust-lang-nursery/rustc-hash |
| rustc_version | 0.4.1 | MIT OR Apache-2.0 | https://github.com/djc/rustc-version-rs |
| ruzstd | 0.8.3 | MIT | https://github.com/KillingSpark/zstd-rs |
| safe_arch | 0.7.4 | Zlib OR Apache-2.0 OR MIT | https://github.com/Lokathor/safe_arch |
| scopeguard | 1.2.0 | MIT OR Apache-2.0 | https://github.com/bluss/scopeguard |
| semver | 1.0.28 | MIT OR Apache-2.0 | https://github.com/dtolnay/semver |
| serde | 1.0.229 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| serde_core | 1.0.229 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| serde_derive | 1.0.229 | MIT OR Apache-2.0 | https://github.com/serde-rs/serde |
| serde_json | 1.0.151 | MIT OR Apache-2.0 | https://github.com/serde-rs/json |
| sharded-slab | 0.1.7 | MIT | https://github.com/hawkw/sharded-slab |
| shlex | 2.0.1 | MIT OR Apache-2.0 | https://github.com/comex/rust-shlex |
| simba | 0.9.1 | Apache-2.0 | https://github.com/dimforge/simba |
| simd-adler32 | 0.3.10 | MIT | https://github.com/mcountryman/simd-adler32 |
| skrifa | 0.42.1 | MIT OR Apache-2.0 | https://github.com/googlefonts/fontations |
| skrifa | 0.44.0 | MIT OR Apache-2.0 | https://github.com/googlefonts/fontations |
| slab | 0.4.12 | MIT | https://github.com/tokio-rs/slab |
| slotmap | 1.1.1 | Zlib | https://github.com/orlp/slotmap |
| smallvec | 1.16.2 | MIT OR Apache-2.0 | https://github.com/servo/rust-smallvec |
| smol_str | 0.2.2 | MIT OR Apache-2.0 | https://github.com/rust-analyzer/smol_str |
| spade | 2.15.1 | MIT OR Apache-2.0 | https://github.com/Stoeoef/spade |
| spin | 0.10.1 | MIT | https://github.com/mvdnes/spin-rs.git |
| spirv | 0.4.0+sdk-1.4.341.0 | Apache-2.0 | https://github.com/gfx-rs/rspirv |
| stable_deref_trait | 1.2.1 | MIT OR Apache-2.0 | https://github.com/storyyeller/stable_deref_trait |
| stackfuture | 0.3.1 | MIT | https://github.com/microsoft/stackfuture |
| static_assertions | 1.1.0 | MIT OR Apache-2.0 | https://github.com/nvzqz/static-assertions-rs |
| svg_fmt | 0.4.5 | MIT/Apache-2.0 | https://github.com/nical/rust_debug |
| swash | 0.2.10 | Apache-2.0 OR MIT | https://github.com/dfrg/swash |
| syn | 2.0.119 | MIT OR Apache-2.0 | https://github.com/dtolnay/syn |
| syn | 3.0.6 | MIT OR Apache-2.0 | https://github.com/dtolnay/syn |
| synstructure | 0.14.0 | MIT | https://github.com/mystor/synstructure |
| sys-locale | 0.3.2 | MIT OR Apache-2.0 | https://github.com/1Password/sys-locale |
| sysinfo | 0.38.4 | MIT | https://github.com/GuillaumeGomez/sysinfo |
| taffy | 0.10.1 | MIT | https://github.com/DioxusLabs/taffy |
| termcolor | 1.4.1 | Unlicense OR MIT | https://github.com/BurntSushi/termcolor |
| thiserror | 2.0.21 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror |
| thiserror-impl | 2.0.21 | MIT OR Apache-2.0 | https://github.com/dtolnay/thiserror |
| thread_local | 1.1.10 | MIT OR Apache-2.0 | https://github.com/Amanieu/thread_local-rs |
| tinystr | 0.8.4 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| tinyvec | 1.13.3 | Zlib OR Apache-2.0 OR MIT | https://github.com/Lokathor/tinyvec |
| toml_datetime | 1.1.2+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| toml_edit | 0.25.17+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| toml_parser | 1.1.5+spec-1.1.0 | MIT OR Apache-2.0 | https://github.com/toml-rs/toml |
| tracing | 0.1.44 | MIT | https://github.com/tokio-rs/tracing |
| tracing-attributes | 0.1.31 | MIT | https://github.com/tokio-rs/tracing |
| tracing-core | 0.1.36 | MIT | https://github.com/tokio-rs/tracing |
| tracing-log | 0.2.0 | MIT | https://github.com/tokio-rs/tracing |
| tracing-subscriber | 0.3.23 | MIT | https://github.com/tokio-rs/tracing |
| twox-hash | 2.1.5 | MIT | https://github.com/shepmaster/twox-hash |
| typeid | 1.0.3 | MIT OR Apache-2.0 | https://github.com/dtolnay/typeid |
| typewit | 1.15.2 | Zlib | https://github.com/rodrimati1992/typewit/ |
| unicode-ident | 1.0.26 | (MIT OR Apache-2.0) AND Unicode-3.0 | https://github.com/dtolnay/unicode-ident |
| unicode-segmentation | 1.13.3 | MIT OR Apache-2.0 | https://github.com/unicode-rs/unicode-segmentation |
| unicode-width | 0.2.2 | MIT OR Apache-2.0 | https://github.com/unicode-rs/unicode-width |
| unicode-xid | 0.2.6 | MIT OR Apache-2.0 | https://github.com/unicode-rs/unicode-xid |
| utf8_iter | 1.0.4 | Apache-2.0 OR MIT | https://github.com/hsivonen/utf8_iter |
| uuid | 1.27.0 | Apache-2.0 OR MIT | https://github.com/uuid-rs/uuid |
| variadics_please | 1.1.0 | MIT OR Apache-2.0 | https://github.com/bevyengine/variadics_please |
| vec_map | 0.8.2 | MIT/Apache-2.0 | https://github.com/contain-rs/vec-map |
| version_check | 0.9.5 | MIT/Apache-2.0 | https://github.com/SergioBenitez/version_check |
| weak-table | 0.3.2 | MIT | https://github.com/tov/weak-table-rs |
| wgpu | 29.0.4 | MIT OR Apache-2.0 | https://github.com/gfx-rs/wgpu |
| wgpu-core | 29.0.4 | MIT OR Apache-2.0 | https://github.com/gfx-rs/wgpu |
| wgpu-core-deps-windows-linux-android | 29.0.4 | MIT OR Apache-2.0 | https://github.com/gfx-rs/wgpu |
| wgpu-hal | 29.0.4 | MIT OR Apache-2.0 | https://github.com/gfx-rs/wgpu |
| wgpu-naga-bridge | 29.0.4 | MIT OR Apache-2.0 | https://github.com/gfx-rs/wgpu |
| wgpu-types | 29.0.4 | MIT OR Apache-2.0 | https://github.com/gfx-rs/wgpu |
| wide | 0.7.33 | Zlib OR Apache-2.0 OR MIT | https://github.com/Lokathor/wide |
| winapi | 0.3.9 | MIT/Apache-2.0 | https://github.com/retep998/winapi-rs |
| winapi-util | 0.1.11 | Unlicense OR MIT | https://github.com/BurntSushi/winapi-util |
| windows | 0.62.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-collections | 0.3.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-core | 0.62.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-future | 0.3.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-implement | 0.60.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-interface | 0.59.3 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-link | 0.2.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-numerics | 0.3.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-result | 0.4.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-strings | 0.5.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-sys | 0.52.0 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-sys | 0.60.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-sys | 0.61.2 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-targets | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-targets | 0.53.5 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows-threading | 0.2.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_x86_64_msvc | 0.52.6 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| windows_x86_64_msvc | 0.53.1 | MIT OR Apache-2.0 | https://github.com/microsoft/windows-rs |
| winit | 0.30.13 | Apache-2.0 | https://github.com/rust-windowing/winit |
| winnow | 1.0.4 | MIT | https://github.com/winnow-rs/winnow |
| writeable | 0.6.4 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| xml-rs | 0.8.29 | MIT | https://github.com/kornelski/xml-rs |
| yazi | 0.2.1 | Apache-2.0 OR MIT | https://github.com/dfrg/yazi |
| yoke | 0.8.3 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| yoke-derive | 0.8.4 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zeno | 0.3.3 | Apache-2.0 OR MIT | https://github.com/dfrg/zeno |
| zerocopy | 0.8.62 | BSD-2-Clause OR Apache-2.0 OR MIT | https://github.com/google/zerocopy |
| zerocopy-derive | 0.8.62 | BSD-2-Clause OR Apache-2.0 OR MIT | https://github.com/google/zerocopy |
| zerofrom | 0.1.8 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zerofrom-derive | 0.1.8 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zerotrie | 0.2.5 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zerovec | 0.11.8 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zerovec-derive | 0.11.6 | Unicode-3.0 | https://github.com/unicode-org/icu4x |
| zlib-rs | 0.6.8 | Zlib | https://github.com/trifectatechfoundation/zlib-rs |
| zmij | 1.0.23 | MIT | https://github.com/dtolnay/zmij |
