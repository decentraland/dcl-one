use bevy_math::Vec3;
use bevy_mesh::{
    morph::MorphAttributes, Mesh, MeshVertexAttribute, VertexAttributeValues as Values,
    VertexFormat,
};
use bevy_platform::collections::HashMap;
use gltf::{
    accessor::{DataType, Dimensions},
    mesh::util::{ReadColors, ReadJoints, ReadWeights},
};
use thiserror::Error;

/// Represents whether integer data requires normalization
#[derive(Copy, Clone)]
struct Normalization(bool);

impl Normalization {
    fn apply_either<T, U>(
        self,
        value: T,
        normalized_ctor: impl Fn(T) -> U,
        unnormalized_ctor: impl Fn(T) -> U,
    ) -> U {
        if self.0 {
            normalized_ctor(value)
        } else {
            unnormalized_ctor(value)
        }
    }
}

/// An error that occurs when accessing buffer data
#[derive(Error, Debug)]
pub(crate) enum AccessFailed {
    #[error("Malformed vertex attribute data")]
    MalformedData,
    #[error("Unsupported vertex attribute format")]
    UnsupportedFormat,
}

/// Helper for reading buffer data
struct BufferAccessor<'a> {
    accessor: gltf::Accessor<'a>,
    buffer_data: &'a Vec<Vec<u8>>,
    normalization: Normalization,
}

impl<'a> BufferAccessor<'a> {
    /// Creates an iterator over the elements in this accessor
    fn iter<T: gltf::accessor::Item>(self) -> Result<gltf::accessor::Iter<'a, T>, AccessFailed> {
        gltf::accessor::Iter::new(self.accessor, |buffer: gltf::Buffer| {
            self.buffer_data.get(buffer.index()).map(Vec::as_slice)
        })
        .ok_or(AccessFailed::MalformedData)
    }

    /// Applies the element iterator to a constructor or fails if normalization is required
    fn with_no_norm<T: gltf::accessor::Item, U>(
        self,
        ctor: impl Fn(gltf::accessor::Iter<'a, T>) -> U,
    ) -> Result<U, AccessFailed> {
        if self.normalization.0 {
            return Err(AccessFailed::UnsupportedFormat);
        }
        self.iter().map(ctor)
    }

    /// Applies the element iterator and the normalization flag to a constructor
    fn with_norm<T: gltf::accessor::Item, U>(
        self,
        ctor: impl Fn(gltf::accessor::Iter<'a, T>, Normalization) -> U,
    ) -> Result<U, AccessFailed> {
        let normalized = self.normalization;
        self.iter().map(|v| ctor(v, normalized))
    }
}

/// An enum of the iterators user by different vertex attribute formats
enum VertexAttributeIter<'a> {
    // For reading native WGPU formats
    F32(gltf::accessor::Iter<'a, f32>),
    U32(gltf::accessor::Iter<'a, u32>),
    F32x2(gltf::accessor::Iter<'a, [f32; 2]>),
    U32x2(gltf::accessor::Iter<'a, [u32; 2]>),
    F32x3(gltf::accessor::Iter<'a, [f32; 3]>),
    U32x3(gltf::accessor::Iter<'a, [u32; 3]>),
    F32x4(gltf::accessor::Iter<'a, [f32; 4]>),
    U32x4(gltf::accessor::Iter<'a, [u32; 4]>),
    S16x2(gltf::accessor::Iter<'a, [i16; 2]>, Normalization),
    U16x2(gltf::accessor::Iter<'a, [u16; 2]>, Normalization),
    S16x4(gltf::accessor::Iter<'a, [i16; 4]>, Normalization),
    U16x4(gltf::accessor::Iter<'a, [u16; 4]>, Normalization),
    S8x2(gltf::accessor::Iter<'a, [i8; 2]>, Normalization),
    U8x2(gltf::accessor::Iter<'a, [u8; 2]>, Normalization),
    S8x4(gltf::accessor::Iter<'a, [i8; 4]>, Normalization),
    U8x4(gltf::accessor::Iter<'a, [u8; 4]>, Normalization),
    // Additional on-disk formats used for RGB colors and quantized vectors
    U16x3(gltf::accessor::Iter<'a, [u16; 3]>, Normalization),
    U8x3(gltf::accessor::Iter<'a, [u8; 3]>, Normalization),
    S16x3(gltf::accessor::Iter<'a, [i16; 3]>, Normalization),
    S8x3(gltf::accessor::Iter<'a, [i8; 3]>, Normalization),
}

/// An integer vertex attribute component.
trait Dequantize: Copy {
    /// The component as a float, scaled into [-1, 1] or [0, 1] when normalized.
    fn dequantize(self, normalized: bool) -> f32;
}

macro_rules! impl_dequantize {
    ($($ty:ty),*) => {$(
        impl Dequantize for $ty {
            fn dequantize(self, normalized: bool) -> f32 {
                if normalized {
                    (self as f32 * (<$ty>::MAX as f32).recip()).max(-1.0)
                } else {
                    self as f32
                }
            }
        }
    )*};
}

impl_dequantize!(i8, u8, i16, u16);

fn dequantize<T: Dequantize, const N: usize>(
    it: impl Iterator<Item = [T; N]>,
    Normalization(normalized): Normalization,
) -> impl Iterator<Item = [f32; N]> {
    it.map(move |v| v.map(|c| c.dequantize(normalized)))
}

impl<'a> VertexAttributeIter<'a> {
    /// Creates an iterator over the elements in a vertex attribute accessor
    fn from_accessor(
        accessor: gltf::Accessor<'a>,
        buffer_data: &'a Vec<Vec<u8>>,
    ) -> Result<VertexAttributeIter<'a>, AccessFailed> {
        let normalization = Normalization(accessor.normalized());
        let format = (accessor.data_type(), accessor.dimensions());
        let acc = BufferAccessor {
            accessor,
            buffer_data,
            normalization,
        };
        match format {
            (DataType::F32, Dimensions::Scalar) => acc.with_no_norm(VertexAttributeIter::F32),
            (DataType::U32, Dimensions::Scalar) => acc.with_no_norm(VertexAttributeIter::U32),
            (DataType::F32, Dimensions::Vec2) => acc.with_no_norm(VertexAttributeIter::F32x2),
            (DataType::U32, Dimensions::Vec2) => acc.with_no_norm(VertexAttributeIter::U32x2),
            (DataType::F32, Dimensions::Vec3) => acc.with_no_norm(VertexAttributeIter::F32x3),
            (DataType::U32, Dimensions::Vec3) => acc.with_no_norm(VertexAttributeIter::U32x3),
            (DataType::F32, Dimensions::Vec4) => acc.with_no_norm(VertexAttributeIter::F32x4),
            (DataType::U32, Dimensions::Vec4) => acc.with_no_norm(VertexAttributeIter::U32x4),
            (DataType::I16, Dimensions::Vec2) => acc.with_norm(VertexAttributeIter::S16x2),
            (DataType::U16, Dimensions::Vec2) => acc.with_norm(VertexAttributeIter::U16x2),
            (DataType::I16, Dimensions::Vec4) => acc.with_norm(VertexAttributeIter::S16x4),
            (DataType::U16, Dimensions::Vec4) => acc.with_norm(VertexAttributeIter::U16x4),
            (DataType::I8, Dimensions::Vec2) => acc.with_norm(VertexAttributeIter::S8x2),
            (DataType::U8, Dimensions::Vec2) => acc.with_norm(VertexAttributeIter::U8x2),
            (DataType::I8, Dimensions::Vec4) => acc.with_norm(VertexAttributeIter::S8x4),
            (DataType::U8, Dimensions::Vec4) => acc.with_norm(VertexAttributeIter::U8x4),
            (DataType::U16, Dimensions::Vec3) => acc.with_norm(VertexAttributeIter::U16x3),
            (DataType::U8, Dimensions::Vec3) => acc.with_norm(VertexAttributeIter::U8x3),
            (DataType::I16, Dimensions::Vec3) => acc.with_norm(VertexAttributeIter::S16x3),
            (DataType::I8, Dimensions::Vec3) => acc.with_norm(VertexAttributeIter::S8x3),
            _ => Err(AccessFailed::UnsupportedFormat),
        }
    }

    /// Materializes float values, converting integer formats to Float32x2, Float32x3 or Float32x4
    fn into_float_values(self) -> Result<Values, AccessFailed> {
        Ok(match self {
            Self::S8x2(it, n) => Values::Float32x2(dequantize(it, n).collect()),
            Self::U8x2(it, n) => Values::Float32x2(dequantize(it, n).collect()),
            Self::S16x2(it, n) => Values::Float32x2(dequantize(it, n).collect()),
            Self::U16x2(it, n) => Values::Float32x2(dequantize(it, n).collect()),
            Self::S8x3(it, n) => Values::Float32x3(dequantize(it, n).collect()),
            Self::U8x3(it, n) => Values::Float32x3(dequantize(it, n).collect()),
            Self::S16x3(it, n) => Values::Float32x3(dequantize(it, n).collect()),
            Self::U16x3(it, n) => Values::Float32x3(dequantize(it, n).collect()),
            Self::S8x4(it, n) => Values::Float32x4(dequantize(it, n).collect()),
            Self::U8x4(it, n) => Values::Float32x4(dequantize(it, n).collect()),
            Self::S16x4(it, n) => Values::Float32x4(dequantize(it, n).collect()),
            Self::U16x4(it, n) => Values::Float32x4(dequantize(it, n).collect()),
            s => return s.into_any_values(),
        })
    }

    /// Materializes normal or tangent values, converting signed normalized formats to Float32x3 or Float32x4
    fn into_direction_values(self) -> Result<Values, AccessFailed> {
        match self {
            s @ (Self::S8x3(_, Normalization(true))
            | Self::S16x3(_, Normalization(true))
            | Self::S8x4(_, Normalization(true))
            | Self::S16x4(_, Normalization(true))) => s.into_float_values(),
            s => s.into_any_values(),
        }
    }

    /// Materializes morph target position displacements, converting signed formats to floats
    fn into_displacements(self) -> Result<Vec<[f32; 3]>, AccessFailed> {
        Ok(match self {
            Self::F32x3(it) => it.collect(),
            Self::S8x3(it, n) => dequantize(it, n).collect(),
            Self::S16x3(it, n) => dequantize(it, n).collect(),
            _ => return Err(AccessFailed::UnsupportedFormat),
        })
    }

    /// Materializes morph target normal or tangent displacements, converting signed normalized formats to floats
    fn into_direction_displacements(self) -> Result<Vec<[f32; 3]>, AccessFailed> {
        match self {
            Self::S8x3(_, Normalization(false)) | Self::S16x3(_, Normalization(false)) => {
                Err(AccessFailed::UnsupportedFormat)
            }
            s => s.into_displacements(),
        }
    }

    /// Materializes values for any supported format of vertex attribute
    fn into_any_values(self) -> Result<Values, AccessFailed> {
        match self {
            VertexAttributeIter::F32(it) => Ok(Values::Float32(it.collect())),
            VertexAttributeIter::U32(it) => Ok(Values::Uint32(it.collect())),
            VertexAttributeIter::F32x2(it) => Ok(Values::Float32x2(it.collect())),
            VertexAttributeIter::U32x2(it) => Ok(Values::Uint32x2(it.collect())),
            VertexAttributeIter::F32x3(it) => Ok(Values::Float32x3(it.collect())),
            VertexAttributeIter::U32x3(it) => Ok(Values::Uint32x3(it.collect())),
            VertexAttributeIter::F32x4(it) => Ok(Values::Float32x4(it.collect())),
            VertexAttributeIter::U32x4(it) => Ok(Values::Uint32x4(it.collect())),
            VertexAttributeIter::S16x2(it, n) => {
                Ok(n.apply_either(it.collect(), Values::Snorm16x2, Values::Sint16x2))
            }
            VertexAttributeIter::U16x2(it, n) => {
                Ok(n.apply_either(it.collect(), Values::Unorm16x2, Values::Uint16x2))
            }
            VertexAttributeIter::S16x4(it, n) => {
                Ok(n.apply_either(it.collect(), Values::Snorm16x4, Values::Sint16x4))
            }
            VertexAttributeIter::U16x4(it, n) => {
                Ok(n.apply_either(it.collect(), Values::Unorm16x4, Values::Uint16x4))
            }
            VertexAttributeIter::S8x2(it, n) => {
                Ok(n.apply_either(it.collect(), Values::Snorm8x2, Values::Sint8x2))
            }
            VertexAttributeIter::U8x2(it, n) => {
                Ok(n.apply_either(it.collect(), Values::Unorm8x2, Values::Uint8x2))
            }
            VertexAttributeIter::S8x4(it, n) => {
                Ok(n.apply_either(it.collect(), Values::Snorm8x4, Values::Sint8x4))
            }
            VertexAttributeIter::U8x4(it, n) => {
                Ok(n.apply_either(it.collect(), Values::Unorm8x4, Values::Uint8x4))
            }
            _ => Err(AccessFailed::UnsupportedFormat),
        }
    }

    /// Materializes RGBA values, converting compatible formats to Float32x4
    fn into_rgba_values(self) -> Result<Values, AccessFailed> {
        match self {
            VertexAttributeIter::U8x3(it, Normalization(true)) => Ok(Values::Float32x4(
                ReadColors::RgbU8(it).into_rgba_f32().collect(),
            )),
            VertexAttributeIter::U16x3(it, Normalization(true)) => Ok(Values::Float32x4(
                ReadColors::RgbU16(it).into_rgba_f32().collect(),
            )),
            VertexAttributeIter::F32x3(it) => Ok(Values::Float32x4(
                ReadColors::RgbF32(it).into_rgba_f32().collect(),
            )),
            VertexAttributeIter::U8x4(it, Normalization(true)) => Ok(Values::Float32x4(
                ReadColors::RgbaU8(it).into_rgba_f32().collect(),
            )),
            VertexAttributeIter::U16x4(it, Normalization(true)) => Ok(Values::Float32x4(
                ReadColors::RgbaU16(it).into_rgba_f32().collect(),
            )),
            s => s.into_any_values(),
        }
    }

    /// Materializes joint index values, converting compatible formats to Uint16x4
    fn into_joint_index_values(self) -> Result<Values, AccessFailed> {
        match self {
            VertexAttributeIter::U8x4(it, Normalization(false)) => {
                Ok(Values::Uint16x4(ReadJoints::U8(it).into_u16().collect()))
            }
            s => s.into_any_values(),
        }
    }

    /// Materializes joint weight values, converting compatible formats to Float32x4
    fn into_joint_weight_values(self) -> Result<Values, AccessFailed> {
        match self {
            VertexAttributeIter::U8x4(it, Normalization(true)) => {
                Ok(Values::Float32x4(ReadWeights::U8(it).into_f32().collect()))
            }
            VertexAttributeIter::U16x4(it, Normalization(true)) => {
                Ok(Values::Float32x4(ReadWeights::U16(it).into_f32().collect()))
            }
            s => s.into_any_values(),
        }
    }
}

enum ConversionMode {
    Any,
    Float,
    Direction,
    Rgba,
    JointIndex,
    JointWeight,
}

#[derive(Error, Debug)]
pub(crate) enum ConvertAttributeError {
    #[error("Vertex attribute {0} has format {1:?} but expected {3:?} for target attribute {2}")]
    WrongFormat(String, VertexFormat, String, VertexFormat),
    #[error("{0} in accessor {1}")]
    AccessFailed(AccessFailed, usize),
    #[error("Unknown vertex attribute {0}")]
    UnknownName(String),
}

pub(crate) fn convert_attribute(
    semantic: gltf::Semantic,
    accessor: gltf::Accessor,
    buffer_data: &Vec<Vec<u8>>,
    custom_vertex_attributes: &HashMap<Box<str>, MeshVertexAttribute>,
) -> Result<(MeshVertexAttribute, Values), ConvertAttributeError> {
    if let Some((attribute, conversion)) = match &semantic {
        gltf::Semantic::Positions => Some((Mesh::ATTRIBUTE_POSITION, ConversionMode::Float)),
        gltf::Semantic::Normals => Some((Mesh::ATTRIBUTE_NORMAL, ConversionMode::Direction)),
        gltf::Semantic::Tangents => Some((Mesh::ATTRIBUTE_TANGENT, ConversionMode::Direction)),
        gltf::Semantic::Colors(0) => Some((Mesh::ATTRIBUTE_COLOR, ConversionMode::Rgba)),
        gltf::Semantic::TexCoords(0) => Some((Mesh::ATTRIBUTE_UV_0, ConversionMode::Float)),
        gltf::Semantic::TexCoords(1) => Some((Mesh::ATTRIBUTE_UV_1, ConversionMode::Float)),
        gltf::Semantic::Joints(0) => {
            Some((Mesh::ATTRIBUTE_JOINT_INDEX, ConversionMode::JointIndex))
        }
        gltf::Semantic::Weights(0) => {
            Some((Mesh::ATTRIBUTE_JOINT_WEIGHT, ConversionMode::JointWeight))
        }
        gltf::Semantic::Extras(name) => custom_vertex_attributes
            .get(name.as_str())
            .map(|attr| (*attr, ConversionMode::Any)),
        _ => None,
    } {
        let raw_iter = VertexAttributeIter::from_accessor(accessor.clone(), buffer_data);
        let converted_values = raw_iter.and_then(|iter| match conversion {
            ConversionMode::Any => iter.into_any_values(),
            ConversionMode::Float => iter.into_float_values(),
            ConversionMode::Direction => iter.into_direction_values(),
            ConversionMode::Rgba => iter.into_rgba_values(),
            ConversionMode::JointIndex => iter.into_joint_index_values(),
            ConversionMode::JointWeight => iter.into_joint_weight_values(),
        });
        match converted_values {
            Ok(values) => {
                let loaded_format = VertexFormat::from(&values);
                if attribute.format == loaded_format {
                    Ok((attribute, values))
                } else {
                    Err(ConvertAttributeError::WrongFormat(
                        semantic.to_string(),
                        loaded_format,
                        attribute.name.to_string(),
                        attribute.format,
                    ))
                }
            }
            Err(err) => Err(ConvertAttributeError::AccessFailed(err, accessor.index())),
        }
    } else {
        Err(ConvertAttributeError::UnknownName(semantic.to_string()))
    }
}

/// The primitive's `POSITION` bounds in the units of its loaded positions.
///
/// glTF stores the bounds of an integer accessor as integers, before
/// normalization.
pub(crate) fn position_bounds(primitive: &gltf::Primitive) -> (Vec3, Vec3) {
    let bounds = primitive.bounding_box();
    // `bounding_box` already requires the accessor.
    let positions = primitive.get(&gltf::Semantic::Positions).unwrap();
    let normalized = positions.normalized();
    let dequantize = |bound: [f32; 3]| {
        Vec3::from(bound.map(|c| match positions.data_type() {
            DataType::I8 => (c as i8).dequantize(normalized),
            DataType::U8 => (c as u8).dequantize(normalized),
            DataType::I16 => (c as i16).dequantize(normalized),
            DataType::U16 => (c as u16).dequantize(normalized),
            _ => c,
        }))
    };
    (dequantize(bounds.min), dequantize(bounds.max))
}

/// The per-vertex displacements of each of the primitive's morph targets, one list per target.
pub(crate) fn morph_targets(
    primitive: &gltf::Primitive,
    buffer_data: &Vec<Vec<u8>>,
) -> Result<Vec<Vec<MorphAttributes>>, ConvertAttributeError> {
    fn read<'a>(
        accessor: Option<gltf::Accessor<'a>>,
        buffer_data: &'a Vec<Vec<u8>>,
        into: impl FnOnce(VertexAttributeIter<'a>) -> Result<Vec<[f32; 3]>, AccessFailed>,
    ) -> Result<Vec<[f32; 3]>, ConvertAttributeError> {
        let Some(accessor) = accessor else {
            return Ok(Vec::new());
        };
        // An accessor without a buffer view or sparse values is all zeros.
        if accessor.view().is_none() && accessor.sparse().is_none() {
            return Ok(vec![[0.0; 3]; accessor.count()]);
        }
        let index = accessor.index();
        VertexAttributeIter::from_accessor(accessor, buffer_data)
            .and_then(into)
            .map_err(|err| ConvertAttributeError::AccessFailed(err, index))
    }

    let mut attributes = Vec::new();
    for target in primitive.morph_targets() {
        let positions = read(
            target.positions(),
            buffer_data,
            VertexAttributeIter::into_displacements,
        )?;
        let normals = read(
            target.normals(),
            buffer_data,
            VertexAttributeIter::into_direction_displacements,
        )?;
        let tangents = read(
            target.tangents(),
            buffer_data,
            VertexAttributeIter::into_direction_displacements,
        )?;

        let count = positions.len().max(normals.len()).max(tangents.len());
        let at = |values: &[[f32; 3]], i: usize| values.get(i).map_or(Vec3::ZERO, |&v| v.into());
        attributes.push(
            (0..count)
                .map(|i| {
                    MorphAttributes::from([at(&positions, i), at(&normals, i), at(&tangents, i)])
                })
                .collect(),
        );
    }
    Ok(attributes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &[u8]) -> gltf::Gltf {
        gltf::Gltf::from_slice_without_validation(json).unwrap()
    }

    /// Each target's displacements as position, normal and tangent.
    fn displacements(primitive: &gltf::Primitive, buffer: Vec<u8>) -> Vec<Vec<[Vec3; 3]>> {
        morph_targets(primitive, &vec![buffer])
            .unwrap()
            .into_iter()
            .map(|target| {
                target
                    .into_iter()
                    .map(|a| [a.position, a.normal, a.tangent])
                    .collect()
            })
            .collect()
    }

    #[test]
    fn rejects_normals_outside_khr_mesh_quantization() {
        let gltf = parse(
            br#"
{
    "asset": { "version": "2.0" },
    "buffers": [{ "byteLength": 4 }],
    "bufferViews": [{ "buffer": 0, "byteLength": 4 }],
    "accessors": [{
        "bufferView": 0, "componentType": 5121, "normalized": true, "count": 1, "type": "VEC3"
    }]
}
"#,
        );
        let normals = convert_attribute(
            gltf::Semantic::Normals,
            gltf.accessors().next().unwrap(),
            &vec![vec![0; 4]],
            &HashMap::default(),
        );
        assert!(normals.is_err());
    }

    #[test]
    fn dequantizes_normalized_position_bounds() {
        let gltf = parse(
            br#"
{
    "asset": { "version": "2.0" },
    "accessors": [{
        "componentType": 5123, "normalized": true, "count": 1, "type": "VEC3",
        "min": [0, 0, 0], "max": [65535, 0, 0]
    }],
    "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0 } }] }]
}
"#,
        );
        let primitive = gltf.meshes().next().unwrap().primitives().next().unwrap();
        assert_eq!(position_bounds(&primitive), (Vec3::ZERO, Vec3::X));
    }

    /// An accessor without a buffer view reads as zeros, even when it is a
    /// target's only attribute.
    #[test]
    fn reads_float_and_zero_morph_targets() {
        let gltf = parse(
            br#"
{
    "asset": { "version": "2.0" },
    "buffers": [{ "byteLength": 12 }],
    "bufferViews": [{ "buffer": 0, "byteLength": 12 }],
    "accessors": [
        { "bufferView": 0, "componentType": 5126, "count": 1, "type": "VEC3" },
        { "componentType": 5126, "count": 1, "type": "VEC3" }
    ],
    "meshes": [{ "primitives": [{
        "attributes": {},
        "targets": [{ "POSITION": 0, "NORMAL": 1 }, { "NORMAL": 1 }]
    }] }]
}
"#,
        );
        let buffer = [1.0f32, 2.0, 3.0].map(f32::to_le_bytes).concat();
        let primitive = gltf.meshes().next().unwrap().primitives().next().unwrap();
        assert_eq!(
            displacements(&primitive, buffer),
            vec![
                vec![[Vec3::new(1.0, 2.0, 3.0), Vec3::ZERO, Vec3::ZERO]],
                vec![[Vec3::ZERO; 3]],
            ]
        );
    }

    #[test]
    fn dequantizes_morph_targets() {
        let gltf = parse(
            br#"
{
    "asset": { "version": "2.0" },
    "buffers": [{ "byteLength": 12 }],
    "bufferViews": [
        { "buffer": 0, "byteLength": 8 },
        { "buffer": 0, "byteOffset": 8, "byteLength": 4 }
    ],
    "accessors": [
        { "bufferView": 0, "componentType": 5122, "count": 1, "type": "VEC3" },
        { "bufferView": 1, "componentType": 5120, "normalized": true, "count": 1, "type": "VEC3" }
    ],
    "meshes": [{ "primitives": [{
        "attributes": {},
        "targets": [{ "POSITION": 0, "NORMAL": 1 }]
    }] }]
}
"#,
        );
        let mut buffer = Vec::new();
        [1i16, -2, 3]
            .iter()
            .for_each(|c| buffer.extend(c.to_le_bytes()));
        buffer.extend([0; 2]);
        buffer.extend([0, (-127i8) as u8, 0, 0]);
        let primitive = gltf.meshes().next().unwrap().primitives().next().unwrap();
        assert_eq!(
            displacements(&primitive, buffer),
            vec![vec![[Vec3::new(1.0, -2.0, 3.0), Vec3::NEG_Y, Vec3::ZERO]]]
        );
    }
}
