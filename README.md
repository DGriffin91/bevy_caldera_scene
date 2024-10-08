# Bevy Caldera Example

Currently only setup to load `hotel_01`.

Download scene from https://github.com/Activision/caldera

Reexport `map_source/prefabs/br/wz_vg/mp_wz_island/commercial/hotel_01.usd` as `hotel_01.glb`

When importing the USD file into blender, for `Object Types`, only select `Meshes`

Run with ex. `cargo run --profile=release-with-debug -- --random-materials` to include symbols with release mode. (Debug seems maybe unusable even with opt-level 3)

![demo](demo.jpg)

Press 1, 2, or 3 for various camera locations. Press B for benchmark (see console for results).