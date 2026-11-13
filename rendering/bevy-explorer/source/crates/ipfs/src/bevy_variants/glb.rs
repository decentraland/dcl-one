//! A `g` model variant is the original GLB with every embedded image abgen could convert stored
//! as a BC KTX2 (`mimeType: "image/ktx2"`, in a buffer view). The glTF loader decodes embedded
//! images itself and takes DDS but not KTX2, so the reader rewrites the GLB before the loader
//! sees it ([`bc_glb_dds`]): each KTX2 is unwrapped to its DDS (`super::ktx2`),
//! appended to the binary chunk, its buffer view pointed at it and its `mimeType` set to
//! `image/vnd-ms.dds`. Everything else keeps its bytes and offsets, so nothing that addresses
//! the buffer directly moves. The DDS is UNORM; the loader picks sRGB or linear from how the
//! material uses each image, as for the explorer's own converted GLBs. An image abgen left as
//! it was (PNG, JPEG, an external file) passes through untouched. A KTX2 that does not unwrap
//! fails the whole rewrite, and the reader falls back to the original.

use std::collections::HashMap;

use serde_json::Value;

use super::ktx2::{self, BcFormat};

const GLB_MAGIC: &[u8; 4] = b"glTF";
const CHUNK_JSON: u32 = 0x4E4F_534A;
const CHUNK_BIN: u32 = 0x004E_4942;
const HEADER_BYTES: usize = 12;
const CHUNK_HEADER_BYTES: usize = 8;
pub const KTX2_MIME: &str = "image/ktx2";
pub const DDS_MIME: &str = "image/vnd-ms.dds";

/// The embedded images of a `g` variant, by what the rewrite made of them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EmbeddedImages {
    pub bc1: u64,
    pub bc5: u64,
    pub bc7: u64,
    /// Embedded images abgen left in their original format (PNG, JPEG): the loader decodes
    /// them as it would the original's.
    pub kept: u64,
}

impl EmbeddedImages {
    fn add(&mut self, format: BcFormat) {
        match format {
            BcFormat::Bc1 => self.bc1 += 1,
            BcFormat::Bc5 => self.bc5 += 1,
            BcFormat::Bc7 => self.bc7 += 1,
        }
    }

    pub(super) fn merge(&mut self, other: EmbeddedImages) {
        self.bc1 += other.bc1;
        self.bc5 += other.bc5;
        self.bc7 += other.bc7;
        self.kept += other.kept;
    }
}

fn u32_at(bytes: &[u8], offset: usize) -> Option<u32> {
    bytes
        .get(offset..offset + 4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
}

fn as_index(value: &Value) -> Option<usize> {
    value.as_u64().and_then(|n| usize::try_from(n).ok())
}

/// The chunks of a GLB: its JSON, its binary chunk (if any) and the bytes of any later chunks,
/// kept as they are.
struct Chunks<'a> {
    json: &'a [u8],
    bin: Option<&'a [u8]>,
    rest: &'a [u8],
}

/// A GLB chunk: its type, its data and the offset just past it.
type Chunk<'a> = (u32, &'a [u8], usize);

fn chunks(glb: &[u8]) -> Result<Chunks<'_>, String> {
    if glb.len() < HEADER_BYTES || &glb[..4] != GLB_MAGIC {
        return Err("not a GLB".to_owned());
    }
    let version = u32_at(glb, 4).unwrap();
    if version != 2 {
        return Err(format!("GLB version {version}"));
    }
    let length = u32_at(glb, 8).unwrap() as usize;
    if length > glb.len() || length < HEADER_BYTES {
        return Err(format!("GLB declares {length} bytes, has {}", glb.len()));
    }
    let glb = &glb[..length];
    let chunk = |offset: usize| -> Result<Option<Chunk<'_>>, String> {
        if offset == glb.len() {
            return Ok(None);
        }
        let (Some(len), Some(kind)) = (u32_at(glb, offset), u32_at(glb, offset + 4)) else {
            return Err("truncated chunk header".to_owned());
        };
        let start = offset + CHUNK_HEADER_BYTES;
        let end = start
            .checked_add(len as usize)
            .filter(|end| *end <= glb.len())
            .ok_or_else(|| "chunk runs past the end of the GLB".to_owned())?;
        Ok(Some((kind, &glb[start..end], end)))
    };
    let Some((CHUNK_JSON, json, next)) = chunk(HEADER_BYTES)? else {
        return Err("the first chunk is not JSON".to_owned());
    };
    let (bin, rest) = match chunk(next)? {
        Some((CHUNK_BIN, bin, next)) => (Some(bin), &glb[next..]),
        _ => (None, &glb[next..]),
    };
    Ok(Chunks { json, bin, rest })
}

fn pad(bytes: &mut Vec<u8>, with: u8) {
    while !bytes.len().is_multiple_of(4) {
        bytes.push(with);
    }
}

/// The GLB the glTF loader reads for a `g` variant: every embedded `image/ktx2` unwrapped to its
/// DDS (see the module docs), with what became of each embedded image. A GLB with no KTX2 image
/// comes back as it was. Any KTX2 that is not a BC KTX2 this client unwraps, or does not
/// unwrap, is an error: the loader cannot read it, so the variant is unusable.
pub fn bc_glb_dds(glb: &[u8]) -> Result<(Vec<u8>, EmbeddedImages), String> {
    let chunks = chunks(glb)?;
    let mut json: Value =
        serde_json::from_slice(chunks.json).map_err(|e| format!("GLB JSON: {e}"))?;
    let mut images = EmbeddedImages::default();

    let image_count = json
        .get("images")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let mut bin: Option<Vec<u8>> = None;
    // buffer views already unwrapped, with their format: two images may share one
    let mut rewritten_views = HashMap::new();
    for index in 0..image_count {
        let image = &json["images"][index];
        let view = image.get("bufferView").map(|view| {
            as_index(view).ok_or_else(|| format!("image {index}: bufferView is not an index"))
        });
        let is_ktx2 = image.get("mimeType").and_then(Value::as_str) == Some(KTX2_MIME);
        let view = match (view, is_ktx2) {
            // an embedded image abgen left in its own format
            (Some(_), false) => {
                images.kept += 1;
                continue;
            }
            // an external file or a data uri: not this rewrite's
            (None, false) => continue,
            (None, true) => return Err(format!("image {index}: a KTX2 with no buffer view")),
            (Some(view), true) => view?,
        };
        if let Some(&format) = rewritten_views.get(&view) {
            json["images"][index]["mimeType"] = DDS_MIME.into();
            images.add(format);
            continue;
        }
        let (offset, length) = buffer_view_range(&json, view, chunks.bin, index)?;
        let source = &chunks.bin.unwrap()[offset..offset + length];
        let format = ktx2::bc_format(source).map_err(|e| format!("image {index}: {e}"))?;
        let dds =
            ktx2::bc_ktx2_to_dds(source, format).map_err(|e| format!("image {index}: {e}"))?;
        let bin = bin.get_or_insert_with(|| chunks.bin.unwrap().to_vec());
        pad(bin, 0);
        let new_offset = bin.len();
        bin.extend_from_slice(&dds);
        let buffer_view = &mut json["bufferViews"][view];
        buffer_view["byteOffset"] = new_offset.into();
        buffer_view["byteLength"] = dds.len().into();
        json["images"][index]["mimeType"] = DDS_MIME.into();
        images.add(format);
        rewritten_views.insert(view, format);
    }

    let Some(mut bin) = bin else {
        return Ok((glb.to_vec(), images));
    };
    json["buffers"][0]["byteLength"] = bin.len().into();
    pad(&mut bin, 0);
    let mut json = serde_json::to_vec(&json).map_err(|e| e.to_string())?;
    pad(&mut json, b' ');

    let length = HEADER_BYTES
        + CHUNK_HEADER_BYTES
        + json.len()
        + CHUNK_HEADER_BYTES
        + bin.len()
        + chunks.rest.len();
    let declared =
        u32::try_from(length).map_err(|_| format!("{length} bytes is too big for a GLB"))?;
    let mut out = Vec::new();
    out.try_reserve_exact(length).map_err(|e| e.to_string())?;
    out.extend_from_slice(GLB_MAGIC);
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&declared.to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(&CHUNK_JSON.to_le_bytes());
    out.extend_from_slice(&json);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(&CHUNK_BIN.to_le_bytes());
    out.extend_from_slice(&bin);
    out.extend_from_slice(chunks.rest);
    Ok((out, images))
}

/// Where an image's buffer view lies in the binary chunk, checked: the view must be in the
/// GLB's own buffer (buffer 0, with no uri) and inside it.
fn buffer_view_range(
    json: &Value,
    view: usize,
    bin: Option<&[u8]>,
    image: usize,
) -> Result<(usize, usize), String> {
    let buffer_view = json
        .get("bufferViews")
        .and_then(|views| views.get(view))
        .ok_or_else(|| format!("image {image}: no buffer view {view}"))?;
    let buffer = buffer_view.get("buffer").and_then(as_index);
    let glb_buffer = json
        .get("buffers")
        .and_then(|buffers| buffers.get(0))
        .is_some_and(|buffer| buffer.as_object().is_some_and(|b| !b.contains_key("uri")));
    let Some(bin) = bin.filter(|_| buffer == Some(0) && glb_buffer) else {
        return Err(format!("image {image}: not in the GLB's binary chunk"));
    };
    let offset = buffer_view.get("byteOffset").map_or(Some(0), as_index);
    let length = buffer_view.get("byteLength").and_then(as_index);
    match (offset, length) {
        (Some(offset), Some(length))
            if offset
                .checked_add(length)
                .is_some_and(|end| end <= bin.len()) =>
        {
            Ok((offset, length))
        }
        _ => Err(format!(
            "image {image}: buffer view {view} lies outside the binary chunk"
        )),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::bevy_variants::ktx2::tests::{bc7_two_levels, ktx2};
    use serde_json::json;

    /// A GLB with `json` (whose `buffers` are filled in unless it names some) and `bin`.
    pub(crate) fn glb(mut json: Value, bin: &[u8]) -> Vec<u8> {
        if json.get("buffers").is_none() {
            json["buffers"] = json!([{ "byteLength": bin.len() }]);
        }
        let mut json = serde_json::to_vec(&json).unwrap();
        pad(&mut json, b' ');
        let mut bin = bin.to_vec();
        pad(&mut bin, 0);
        let mut out = GLB_MAGIC.to_vec();
        out.extend_from_slice(&2u32.to_le_bytes());
        let length = HEADER_BYTES + 2 * CHUNK_HEADER_BYTES + json.len() + bin.len();
        out.extend_from_slice(&(length as u32).to_le_bytes());
        out.extend_from_slice(&(json.len() as u32).to_le_bytes());
        out.extend_from_slice(&CHUNK_JSON.to_le_bytes());
        out.extend_from_slice(&json);
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(&CHUNK_BIN.to_le_bytes());
        out.extend_from_slice(&bin);
        out
    }

    /// A binary chunk holding `parts` back to back, 4-byte aligned, and a buffer view for each.
    fn layout(parts: &[&[u8]]) -> (Vec<u8>, Vec<Value>) {
        let mut bin = Vec::new();
        let mut views = Vec::new();
        for part in parts {
            pad(&mut bin, 0);
            views.push(json!({ "buffer": 0, "byteOffset": bin.len(), "byteLength": part.len() }));
            bin.extend_from_slice(part);
        }
        (bin, views)
    }

    fn parse(glb: &[u8]) -> (Value, Vec<u8>) {
        let chunks = chunks(glb).unwrap();
        (
            serde_json::from_slice(chunks.json).unwrap(),
            chunks.bin.unwrap().to_vec(),
        )
    }

    fn view_bytes<'a>(json: &Value, bin: &'a [u8], view: usize) -> &'a [u8] {
        let view = &json["bufferViews"][view];
        let offset = view["byteOffset"].as_u64().unwrap() as usize;
        let length = view["byteLength"].as_u64().unwrap() as usize;
        &bin[offset..offset + length]
    }

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nnot really a png";
    const JPEG: &[u8] = b"\xff\xd8\xff\xe0not really a jpeg";
    /// Vertex data, at view 0: it must keep its bytes and offset.
    const POSITIONS: &[u8] = &[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];

    #[test]
    fn bc1_bc5_and_bc7_images_become_dds_and_everything_else_stays_put() {
        let (bc7, _, _) = bc7_two_levels();
        let bc1 = ktx2(131, 4, 4, &[vec![7; 8]]);
        let bc5 = ktx2(141, 4, 4, &[vec![9; 16]]);
        let (bin, views) = layout(&[POSITIONS, &bc7, &bc1, &bc5, PNG]);
        let source = glb(
            json!({
                "asset": { "version": "2.0" },
                "accessors": [
                    { "bufferView": 0, "componentType": 5126, "count": 1, "type": "VEC3" }
                ],
                "bufferViews": views,
                "images": [
                    { "bufferView": 1, "mimeType": "image/ktx2" },
                    { "bufferView": 2, "mimeType": "image/ktx2" },
                    { "bufferView": 3, "mimeType": "image/ktx2", "name": "normal" },
                    { "bufferView": 4, "mimeType": "image/png" },
                    { "uri": "textures/external.png" },
                ],
                "textures": [{ "source": 0 }, { "source": 1 }, { "source": 2 }],
            }),
            &bin,
        );

        let (rewritten, images) = bc_glb_dds(&source).unwrap();
        assert_eq!(
            images,
            EmbeddedImages {
                bc1: 1,
                bc5: 1,
                bc7: 1,
                kept: 1
            }
        );
        let (json, out_bin) = parse(&rewritten);
        // the declared length is the file's, and the buffer's is the binary chunk's data
        assert_eq!(u32_at(&rewritten, 8).unwrap() as usize, rewritten.len());
        assert!(json["buffers"][0]["byteLength"].as_u64().unwrap() as usize <= out_bin.len());
        // each KTX2 is now its DDS, in place of the KTX2 under the same view
        for (image, source) in [(0, &bc7), (1, &bc1), (2, &bc5)] {
            assert_eq!(json["images"][image]["mimeType"], DDS_MIME);
            let view = json["images"][image]["bufferView"].as_u64().unwrap() as usize;
            let dds = ktx2::bc_ktx2_dds(source).unwrap().unwrap();
            assert_eq!(view_bytes(&json, &out_bin, view), dds);
            assert_eq!(
                json["bufferViews"][view]["byteOffset"].as_u64().unwrap() % 4,
                0
            );
        }
        assert_eq!(json["images"][2]["name"], "normal");
        // the vertex data and the PNG keep their bytes, offsets and MIME type
        assert_eq!(json["bufferViews"][0], views[0]);
        assert_eq!(view_bytes(&json, &out_bin, 0), POSITIONS);
        assert_eq!(json["bufferViews"][4], views[4]);
        assert_eq!(view_bytes(&json, &out_bin, 4), PNG);
        assert_eq!(json["images"][3]["mimeType"], "image/png");
        assert_eq!(json["images"][4], json!({ "uri": "textures/external.png" }));
        assert_eq!(json["accessors"][0]["bufferView"], 0);
        // what the loader reads: a GLB the rewrite has nothing left to do to
        let (again, images) = bc_glb_dds(&rewritten).unwrap();
        assert_eq!(again, rewritten);
        assert_eq!(images.kept, 4);
    }

    #[test]
    fn a_glb_whose_images_are_png_or_jpeg_passes_through_untouched() {
        let (bin, views) = layout(&[POSITIONS, PNG, JPEG]);
        let source = glb(
            json!({
                "asset": { "version": "2.0" },
                "bufferViews": views,
                "images": [
                    { "bufferView": 1, "mimeType": "image/png" },
                    { "bufferView": 2, "mimeType": "image/jpeg" },
                ],
            }),
            &bin,
        );
        let (out, images) = bc_glb_dds(&source).unwrap();
        assert_eq!(out, source);
        assert_eq!(
            images,
            EmbeddedImages {
                kept: 2,
                ..Default::default()
            }
        );
        // no images at all
        let plain = glb(json!({ "asset": { "version": "2.0" } }), POSITIONS);
        assert_eq!(
            bc_glb_dds(&plain).unwrap(),
            (plain, EmbeddedImages::default())
        );
    }

    pub(crate) fn with_one_ktx2(ktx2: &[u8]) -> Vec<u8> {
        let (bin, views) = layout(&[POSITIONS, ktx2]);
        glb(
            json!({
                "asset": { "version": "2.0" },
                "bufferViews": views,
                "images": [{ "bufferView": 1, "mimeType": "image/ktx2" }],
            }),
            &bin,
        )
    }

    #[test]
    fn a_ktx2_that_does_not_unwrap_fails_the_rewrite() {
        let (bc7, level0, level1) = bc7_two_levels();
        // truncated inside its last level
        let truncated = with_one_ktx2(&bc7[..bc7.len() - 1]);
        assert!(bc_glb_dds(&truncated)
            .unwrap_err()
            .contains("outside the file"));
        // sRGB BC7, and Basis Universal (vkFormat 0): not a BC KTX2 this client unwraps
        let srgb = with_one_ktx2(&ktx2(146, 8, 4, &[level0, level1]));
        assert!(bc_glb_dds(&srgb).unwrap_err().contains("vkFormat 146"));
        let basis = with_one_ktx2(&ktx2(0, 4, 4, &[vec![0; 16]]));
        assert!(bc_glb_dds(&basis).unwrap_err().contains("vkFormat 0"));
        // not a KTX2 at all, under a KTX2 MIME type
        assert!(bc_glb_dds(&with_one_ktx2(PNG))
            .unwrap_err()
            .contains("not a KTX2"));
    }

    #[test]
    fn a_ktx2_outside_the_binary_chunk_or_with_no_buffer_view_fails() {
        let (bc7, _, _) = bc7_two_levels();
        let (bin, mut views) = layout(&[POSITIONS, &bc7]);
        views[1]["byteLength"] = (bc7.len() + 64).into();
        let past_the_end = glb(
            json!({
                "asset": { "version": "2.0" },
                "bufferViews": views,
                "images": [{ "bufferView": 1, "mimeType": "image/ktx2" }],
            }),
            &bin,
        );
        assert!(bc_glb_dds(&past_the_end).unwrap_err().contains("outside"));
        let data_uri = glb(
            json!({
                "asset": { "version": "2.0" },
                "images": [{ "uri": "data:image/ktx2;base64,AAAA", "mimeType": "image/ktx2" }],
            }),
            POSITIONS,
        );
        assert!(bc_glb_dds(&data_uri)
            .unwrap_err()
            .contains("no buffer view"));
    }

    #[test]
    fn a_glb_whose_first_buffer_is_not_an_object_fails() {
        let (bc7, _, _) = bc7_two_levels();
        let (mut json, bin) = parse(&with_one_ktx2(&bc7));
        json["buffers"] = json!([5]);
        let source = glb(json, &bin);
        assert!(bc_glb_dds(&source)
            .unwrap_err()
            .contains("not in the GLB's binary chunk"));
    }

    #[test]
    fn two_images_sharing_a_ktx2_view_are_unwrapped_once() {
        let bc1 = ktx2(131, 4, 4, &[vec![7; 8]]);
        let source = with_one_ktx2(&bc1);
        let (mut json, bin) = parse(&source);
        json["images"] = json!([
            { "bufferView": 1, "mimeType": "image/ktx2" },
            { "bufferView": 1, "mimeType": "image/ktx2" },
        ]);
        json.as_object_mut().unwrap().remove("buffers");
        let source = glb(json, &bin);
        let (out, images) = bc_glb_dds(&source).unwrap();
        assert_eq!(images.bc1, 2);
        let (json, out_bin) = parse(&out);
        assert_eq!(json["images"][1]["mimeType"], DDS_MIME);
        let dds = ktx2::bc_ktx2_dds(&bc1).unwrap().unwrap();
        assert_eq!(view_bytes(&json, &out_bin, 1), dds);
        // one DDS appended, not two
        assert_eq!(
            out_bin.len(),
            bin.len().next_multiple_of(4) + dds.len().next_multiple_of(4)
        );
    }

    #[test]
    fn something_that_is_not_a_glb_fails() {
        assert!(bc_glb_dds(b"not a glb at all").is_err());
        let source = with_one_ktx2(&ktx2(131, 4, 4, &[vec![7; 8]]));
        assert!(bc_glb_dds(&source[..source.len() - 4])
            .unwrap_err()
            .contains("declares"));
        let mut version_1 = source.clone();
        version_1[4] = 1;
        assert!(bc_glb_dds(&version_1).unwrap_err().contains("version 1"));
    }
}
