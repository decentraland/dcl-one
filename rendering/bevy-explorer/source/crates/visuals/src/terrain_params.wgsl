// Terrain parameters shared by the shaders standing on the terrain and the ground's fragment
// (`TerrainParams` in terrain.rs).

struct TerrainParams {
    // first parcel of the step texture (x, unity z)
    min: vec2<i32>,
    size: vec2<i32>,
    max_steps: f32,
    // unity's occupancy texture size, only used to reproduce its uv arithmetic
    uv_size: f32,
    // the ground's grids are finer around this parcel (x, unity z)
    player_parcel: vec2<i32>,
    // the player's position within that parcel, 0 to 1 (x, unity z)
    player_offset: vec2<f32>,
    // coast bounds in unity x/z metres (min x, min z, max x, max z); the ground ends at the cliffs
    coast: vec4<f32>,
    // hills are displaced out to this many metres from the player (terrain.rs `hill_reach`)
    hill_reach: f32,
}

@group(2) @binding(101) var<uniform> terrain: TerrainParams;
