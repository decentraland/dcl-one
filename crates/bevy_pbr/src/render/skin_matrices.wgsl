// Computes the skinning matrix of every joint of every skin, once a frame:
// the world matrix of the joint's entity times the skin's inverse bindpose.

// One world matrix per joint entity.
@group(0) @binding(0) var<storage, read> joint_matrices: array<mat4x4<f32>>;
// Per skin joint: the inverse bindpose, with the slot of the joint's world
// matrix in `joint_matrices` stored in the first column's `w`.
@group(0) @binding(1) var<storage, read> skin_joints: array<mat4x4<f32>>;
// Per skin joint: the skinning matrix the vertex shader reads.
@group(0) @binding(2) var<storage, read_write> skin_matrices: array<mat4x4<f32>>;

// The dispatch covers the entries in rows of `num_workgroups.x` workgroups.
@compute @workgroup_size(64)
fn main(
    @builtin(global_invocation_id) id: vec3<u32>,
    @builtin(num_workgroups) num_workgroups: vec3<u32>,
) {
    let index = id.y * num_workgroups.x * 64u + id.x;
    if index >= arrayLength(&skin_joints) || index >= arrayLength(&skin_matrices) {
        return;
    }
    let entry = skin_joints[index];
    let slot = u32(entry[0].w);
    let inverse_bindpose = mat4x4<f32>(
        vec4<f32>(entry[0].xyz, 0.0),
        vec4<f32>(entry[1].xyz, 0.0),
        vec4<f32>(entry[2].xyz, 0.0),
        vec4<f32>(entry[3].xyz, 1.0),
    );
    skin_matrices[index] = joint_matrices[slot] * inverse_bindpose;
}
