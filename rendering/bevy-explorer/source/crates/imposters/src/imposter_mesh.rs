use bevy::asset::{RenderAssetTransferPriority, RenderAssetUsages};
use bevy::math::{Vec2, Vec3, Vec3Swizzles};
use bevy::reflect::prelude::*;
use bevy::render::mesh::{Indices, Mesh, MeshBuilder, PrimitiveTopology};
use bevy::transform::components::Transform;
use common::partition::PARCEL_SIZE;

use crate::imposter_spec::ImposterSpec;
use crate::render::SceneImposter;

#[derive(Clone, Copy, Debug, Reflect)]
#[reflect(Default, Debug, Clone)]
pub struct ImposterMesh {
    pub min: Vec3,
    pub max: Vec3,
    pub with_bake_attributes: bool,
}

impl Default for ImposterMesh {
    fn default() -> Self {
        Self {
            min: Vec3::splat(-0.5),
            max: Vec3::splat(0.5),
            with_bake_attributes: true,
        }
    }
}

impl ImposterMesh {
    pub(crate) fn render_proxy(
        spec: &ImposterSpec,
        source_level: usize,
        target: &SceneImposter,
    ) -> Option<(Transform, Self)> {
        // Extend the proxy to the world floor. Keep the bake's transform when
        // clipping the vertices so the shader samples the same baked region.
        let min = Vec3::new(
            spec.region_min.x,
            spec.region_min.y.min(0.0),
            spec.region_min.z,
        );
        let max = spec.region_max;
        let transform = Transform::from_translation((min + max) * 0.5)
            .with_scale((max - min).max(Vec3::splat(0.001)) * (1.0 + source_level as f32 / 1000.0));

        let unit = Self::default();
        if target.as_ingredient || source_level == target.level {
            return Some((transform, unit));
        }

        let origin = target.parcel.as_vec2() * Vec2::new(PARCEL_SIZE, -PARCEL_SIZE);
        let size = (1 << target.level) as f32 * PARCEL_SIZE;
        let tile_min = origin - Vec2::new(0.0, size);
        let tile_max = origin + Vec2::new(size, 0.0);
        let center = transform.translation.xz();
        let scale = transform.scale.xz();

        let min = ((tile_min - center) / scale).max(unit.min.xz());
        let max = ((tile_max - center) / scale).min(unit.max.xz());
        if !min.cmplt(max).all() {
            return None;
        }

        if min == unit.min.xz() && max == unit.max.xz() {
            return Some((transform, unit));
        }

        Some((
            transform,
            Self {
                min: Vec3::new(min.x, unit.min.y, min.y),
                max: Vec3::new(max.x, unit.max.y, max.y),
                with_bake_attributes: false,
            },
        ))
    }
}

impl MeshBuilder for ImposterMesh {
    fn build(&self) -> Mesh {
        let min = self.min;
        let max = self.max;

        let positions = vec![
            [min.x, min.y, min.z],
            [max.x, min.y, min.z],
            [min.x, max.y, min.z],
            [max.x, max.y, min.z],
            [min.x, min.y, max.z],
            [max.x, min.y, max.z],
            [min.x, max.y, max.z],
            [max.x, max.y, max.z],
        ];

        let indices = Indices::U16(vec![
            4, 5, 7, 7, 6, 4, 2, 3, 1, 1, 0, 2, 1, 3, 7, 7, 5, 1, 4, 6, 2, 2, 0, 4, 3, 2, 6, 6, 7,
            3,
            // 5, 4, 0, 0, 1, 5, // don't need a bottom
        ]);

        let mut mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        );

        if self.with_bake_attributes {
            mesh = mesh
                .with_inserted_attribute(
                    Mesh::ATTRIBUTE_NORMAL,
                    positions
                        .iter()
                        .map(|p| Vec3::new(p[0], p[1], p[2]).normalize())
                        .collect::<Vec<_>>(),
                )
                .with_inserted_attribute(
                    Mesh::ATTRIBUTE_UV_0,
                    positions.iter().map(|p| [p[0], p[1]]).collect::<Vec<_>>(),
                );
        }

        mesh = mesh
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
            .with_inserted_indices(indices);
        mesh.transfer_priority = RenderAssetTransferPriority::Immediate;

        mesh
    }
}

#[cfg(test)]
mod tests {
    use bevy::math::IVec2;
    use bevy::render::mesh::MeshAabb;

    use super::*;

    #[test]
    fn adjacent_parcels_do_not_overlap_when_using_the_same_two_level_fallback() {
        let spec = ImposterSpec {
            scale: 52.48464,
            region_min: Vec3::new(-576.0, 0.9433594, -64.0),
            region_max: Vec3::new(-512.0, 12.00592, 0.0),
            overhang: 4.4367585,
        };
        let footprint = |x| {
            let target = SceneImposter {
                parcel: IVec2::new(x, 1),
                level: 0,
                as_ingredient: false,
            };
            let (transform, mesh) = ImposterMesh::render_proxy(&spec, 2, &target).unwrap();
            let aabb = mesh.build().compute_aabb().unwrap();
            let min = transform.transform_point(aabb.min().into()).xz();
            let max = transform.transform_point(aabb.max().into()).xz();
            (min, max)
        };

        let (left_min, left_max) = footprint(-35);
        let (right_min, right_max) = footprint(-34);

        assert_eq!(left_min, Vec2::new(-560.0, -32.0));
        assert_eq!(left_max, Vec2::new(-544.0, -16.0));
        assert_eq!(right_min, Vec2::new(-544.0, -32.0));
        assert_eq!(right_max, Vec2::new(-528.0, -16.0));
    }

    #[test]
    fn a_coarser_bake_outside_the_requested_tile_has_no_proxy() {
        let spec = ImposterSpec {
            scale: 16.0,
            region_min: Vec3::new(-576.0, 1.0, -32.0),
            region_max: Vec3::new(-562.0, 12.0, -16.0),
            overhang: 0.0,
        };
        let target = SceneImposter {
            parcel: IVec2::new(-35, 1),
            level: 0,
            as_ingredient: false,
        };

        assert!(ImposterMesh::render_proxy(&spec, 2, &target).is_none());
    }

    #[test]
    fn ready_tiles_and_bake_ingredients_use_the_shared_cube() {
        let spec = ImposterSpec {
            scale: 32.0,
            region_min: Vec3::new(-64.0, 1.0, 0.0),
            region_max: Vec3::new(-32.0, 12.0, 32.0),
            overhang: 0.0,
        };
        let mut target = SceneImposter {
            parcel: IVec2::new(-4, -2),
            level: 1,
            as_ingredient: false,
        };
        let (transform, displayed) = ImposterMesh::render_proxy(&spec, 1, &target).unwrap();
        assert_eq!(displayed.min, Vec3::splat(-0.5));
        assert_eq!(displayed.max, Vec3::splat(0.5));
        assert!(displayed.with_bake_attributes);

        target.as_ingredient = true;
        let (bake_transform, ingredient) = ImposterMesh::render_proxy(&spec, 1, &target).unwrap();
        assert_eq!(bake_transform, transform);
        assert_eq!(ingredient.min, Vec3::splat(-0.5));
        assert_eq!(ingredient.max, Vec3::splat(0.5));
        assert!(ingredient.with_bake_attributes);
    }
}
