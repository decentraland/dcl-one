use bevy_mesh::{Mesh, MeshVertexAttribute, VertexAttributeValues as Values, VertexFormat};
use bevy_platform::collections::HashMap;
use gltf::{
    accessor::{DataType, Dimensions},
    mesh::util::{ReadColors, ReadJoints, ReadTexCoords, ReadWeights},
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

/// An integer component that `KHR_mesh_quantization` lets a position, normal, tangent or
/// texture coordinate accessor hold in place of a float.
trait Dequantize: Copy {
    /// The float the component stands for: the component itself when the accessor is not
    /// normalized, otherwise the value scaled to [0, 1] or [-1, 1] as the glTF specification
    /// defines (a signed minimum maps to -1, so -128 and -127 both read as -1).
    fn dequantize(self, normalization: Normalization) -> f32;
}

macro_rules! dequantize {
    ($($ty:ty => $max:expr),* $(,)?) => {$(
        impl Dequantize for $ty {
            fn dequantize(self, normalization: Normalization) -> f32 {
                if normalization.0 {
                    (self as f32 / $max).max(-1.0)
                } else {
                    self as f32
                }
            }
        }
    )*};
}

dequantize! {
    i8 => 127.0,
    u8 => 255.0,
    i16 => 32767.0,
    u16 => 65535.0,
}

/// Reads an integer accessor as the floats it stands for.
fn dequantized<T: Dequantize, const N: usize>(
    it: impl Iterator<Item = [T; N]>,
    normalization: Normalization,
) -> Vec<[f32; N]> {
    it.map(|element| element.map(|component| component.dequantize(normalization)))
        .collect()
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
    // Additional on-disk formats used for RGB colors
    U16x3(gltf::accessor::Iter<'a, [u16; 3]>, Normalization),
    U8x3(gltf::accessor::Iter<'a, [u8; 3]>, Normalization),
    // Additional on-disk formats used for quantized positions and normals
    S16x3(gltf::accessor::Iter<'a, [i16; 3]>, Normalization),
    S8x3(gltf::accessor::Iter<'a, [i8; 3]>, Normalization),
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

    /// Materializes texture coordinate values, converting compatible formats to Float32x2.
    /// `KHR_mesh_quantization` allows every 8- and 16-bit integer type, normalized or not.
    fn into_tex_coord_values(self) -> Result<Values, AccessFailed> {
        match self {
            VertexAttributeIter::U8x2(it, Normalization(true)) => Ok(Values::Float32x2(
                ReadTexCoords::U8(it).into_f32().collect(),
            )),
            VertexAttributeIter::U16x2(it, Normalization(true)) => Ok(Values::Float32x2(
                ReadTexCoords::U16(it).into_f32().collect(),
            )),
            VertexAttributeIter::U8x2(it, n) => Ok(Values::Float32x2(dequantized(it, n))),
            VertexAttributeIter::U16x2(it, n) => Ok(Values::Float32x2(dequantized(it, n))),
            VertexAttributeIter::S8x2(it, n) => Ok(Values::Float32x2(dequantized(it, n))),
            VertexAttributeIter::S16x2(it, n) => Ok(Values::Float32x2(dequantized(it, n))),
            s => s.into_any_values(),
        }
    }

    /// Materializes position or normal values, converting the integer formats
    /// `KHR_mesh_quantization` allows for them to Float32x3
    fn into_float3_values(self) -> Result<Values, AccessFailed> {
        match self {
            VertexAttributeIter::S8x3(it, n) => Ok(Values::Float32x3(dequantized(it, n))),
            VertexAttributeIter::U8x3(it, n) => Ok(Values::Float32x3(dequantized(it, n))),
            VertexAttributeIter::S16x3(it, n) => Ok(Values::Float32x3(dequantized(it, n))),
            VertexAttributeIter::U16x3(it, n) => Ok(Values::Float32x3(dequantized(it, n))),
            s => s.into_any_values(),
        }
    }

    /// Materializes tangent values, converting the integer formats `KHR_mesh_quantization`
    /// allows for them to Float32x4
    fn into_float4_values(self) -> Result<Values, AccessFailed> {
        match self {
            VertexAttributeIter::S8x4(it, n) => Ok(Values::Float32x4(dequantized(it, n))),
            VertexAttributeIter::S16x4(it, n) => Ok(Values::Float32x4(dequantized(it, n))),
            s => s.into_any_values(),
        }
    }
}

enum ConversionMode {
    Any,
    Float3,
    Float4,
    Rgba,
    JointIndex,
    JointWeight,
    TexCoord,
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
        gltf::Semantic::Positions => Some((Mesh::ATTRIBUTE_POSITION, ConversionMode::Float3)),
        gltf::Semantic::Normals => Some((Mesh::ATTRIBUTE_NORMAL, ConversionMode::Float3)),
        gltf::Semantic::Tangents => Some((Mesh::ATTRIBUTE_TANGENT, ConversionMode::Float4)),
        gltf::Semantic::Colors(0) => Some((Mesh::ATTRIBUTE_COLOR, ConversionMode::Rgba)),
        gltf::Semantic::TexCoords(0) => Some((Mesh::ATTRIBUTE_UV_0, ConversionMode::TexCoord)),
        gltf::Semantic::TexCoords(1) => Some((Mesh::ATTRIBUTE_UV_1, ConversionMode::TexCoord)),
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
            ConversionMode::Float3 => iter.into_float3_values(),
            ConversionMode::Float4 => iter.into_float4_values(),
            ConversionMode::Rgba => iter.into_rgba_values(),
            ConversionMode::TexCoord => iter.into_tex_coord_values(),
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

#[cfg(test)]
mod tests {
    use super::{Dequantize, Normalization};

    #[test]
    fn normalized_integers_map_to_the_unit_range() {
        let norm = Normalization(true);
        assert_eq!(127i8.dequantize(norm), 1.0);
        assert_eq!((-127i8).dequantize(norm), -1.0);
        assert_eq!((-128i8).dequantize(norm), -1.0);
        assert_eq!(255u8.dequantize(norm), 1.0);
        assert_eq!(0u8.dequantize(norm), 0.0);
        assert_eq!(32767i16.dequantize(norm), 1.0);
        assert_eq!((-32768i16).dequantize(norm), -1.0);
        assert_eq!(65535u16.dequantize(norm), 1.0);
    }

    #[test]
    fn unnormalized_integers_keep_their_value() {
        let raw = Normalization(false);
        assert_eq!(8192u16.dequantize(raw), 8192.0);
        assert_eq!((-300i16).dequantize(raw), -300.0);
        assert_eq!(200u8.dequantize(raw), 200.0);
        assert_eq!((-5i8).dequantize(raw), -5.0);
    }
}
