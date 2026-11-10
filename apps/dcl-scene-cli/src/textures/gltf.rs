//! glTF 2.0 models (GLB and `.gltf`) read and rewritten as JSON values, so
//! every field and extension this module does not touch survives as it was.
//! What it changes: images it is handed a KTX2 file for become external
//! `.ktx2` images, the textures sampling them name them through
//! `KHR_texture_basisu` (no fallback `source`), and a GLB's binary chunk loses
//! the bytes those images no longer use.

use super::ColorMode;
use anyhow::{Context, Result, bail};
use base64::Engine;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub const BASISU_EXT: &str = "KHR_texture_basisu";
const DRACO_EXT: &str = "KHR_draco_mesh_compression";
const GLB_MAGIC: &[u8; 4] = b"glTF";
const CHUNK_JSON: u32 = 0x4E4F_534A;
const CHUNK_BIN: u32 = 0x004E_4942;

/// Extensions known not to address the binary chunk by offset, nor to name a
/// buffer view anywhere but where [`repack`] remaps it (Draco). A GLB using any
/// other extension keeps its binary chunk byte for byte: an image's bytes stay
/// in it, unused, rather than risk a reference this module cannot see.
fn repack_safe(ext: &str) -> bool {
    ext.starts_with("KHR_materials_")
        || matches!(
            ext,
            DRACO_EXT
                | BASISU_EXT
                | "KHR_texture_transform"
                | "KHR_lights_punctual"
                | "KHR_mesh_quantization"
                | "KHR_animation_pointer"
                | "KHR_node_visibility"
                | "KHR_xmp_json_ld"
                | "EXT_mesh_gpu_instancing"
                | "EXT_texture_webp"
                | "MSFT_texture_dds"
        )
}

pub struct Model {
    pub json: Value,
    /// The GLB's binary chunk; `None` for a `.gltf` and a GLB without one.
    pub bin: Option<Vec<u8>>,
    pub glb: bool,
}

fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    let b = bytes.get(at..at + 4)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

impl Model {
    pub fn parse(bytes: &[u8]) -> Result<Model> {
        if !bytes.starts_with(GLB_MAGIC) {
            let text = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
            let json: Value = serde_json::from_slice(text).context("not glTF JSON")?;
            if !json.is_object() {
                bail!("glTF JSON is not an object");
            }
            return Ok(Model {
                json,
                bin: None,
                glb: false,
            });
        }
        let version = u32_at(bytes, 4).context("truncated GLB header")?;
        if version != 2 {
            bail!("GLB version {version} (only 2 is glTF 2.0)");
        }
        let declared = u32_at(bytes, 8).context("truncated GLB header")? as usize;
        let end = declared.min(bytes.len());
        let (mut json, mut bin) = (None, None);
        let mut pos = 12;
        while pos + 8 <= end {
            let len = u32_at(bytes, pos).unwrap_or(0) as usize;
            let kind = u32_at(bytes, pos + 4).unwrap_or(0);
            let start = pos + 8;
            let stop = start
                .checked_add(len)
                .filter(|s| *s <= end)
                .context("truncated GLB chunk")?;
            match kind {
                CHUNK_JSON if json.is_none() => json = Some(&bytes[start..stop]),
                CHUNK_BIN if bin.is_none() => bin = Some(bytes[start..stop].to_vec()),
                _ => {}
            }
            pos = stop;
        }
        let json = json.context("GLB without a JSON chunk")?;
        let trimmed_len = json
            .iter()
            .rposition(|b| !matches!(b, b' ' | 0 | b'\t' | b'\n' | b'\r'))
            .map_or(0, |i| i + 1);
        let json: Value = serde_json::from_slice(&json[..trimmed_len]).context("GLB JSON chunk")?;
        if !json.is_object() {
            bail!("glTF JSON is not an object");
        }
        Ok(Model {
            json,
            bin,
            glb: true,
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        if !self.glb {
            let mut out = serde_json::to_vec_pretty(&self.json).expect("JSON values serialize");
            out.push(b'\n');
            return out;
        }
        let mut json = serde_json::to_vec(&self.json).expect("JSON values serialize");
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        let bin = self.bin.as_ref().map(|b| {
            let mut b = b.clone();
            while !b.len().is_multiple_of(4) {
                b.push(0);
            }
            b
        });
        let total = 12 + 8 + json.len() + bin.as_ref().map_or(0, |b| 8 + b.len());
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(GLB_MAGIC);
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(json.len() as u32).to_le_bytes());
        out.extend_from_slice(&CHUNK_JSON.to_le_bytes());
        out.extend_from_slice(&json);
        if let Some(bin) = bin {
            out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
            out.extend_from_slice(&CHUNK_BIN.to_le_bytes());
            out.extend_from_slice(&bin);
        }
        out
    }

    fn array(&self, key: &str) -> &[Value] {
        self.json
            .get(key)
            .and_then(Value::as_array)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn strings(&self, key: &str) -> Vec<String> {
        self.array(key)
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    }
}

fn index(v: &Value, key: &str) -> Option<usize> {
    v.get(key)?.as_u64().map(|i| i as usize)
}

/// Where an image's bytes are.
#[derive(Debug, PartialEq)]
pub enum ImageSource {
    /// Inside the model: a buffer view or a `data:` URI.
    Embedded(Vec<u8>),
    /// A file of the scene, by its scene-relative path.
    External(String),
}

/// A `data:` URI's payload: base64, else percent-encoded text.
fn data_uri(uri: &str) -> Option<Vec<u8>> {
    let rest = uri.strip_prefix("data:")?;
    let (meta, payload) = rest.split_once(',')?;
    match meta.ends_with(";base64") {
        true => base64::engine::general_purpose::STANDARD
            .decode(payload.trim())
            .ok(),
        false => Some(percent_decode(payload)?.into_bytes()),
    }
}

fn percent_decode(s: &str) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = s.get(i + 1..i + 3)?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// A relative URI inside the model at `model_rel`, as a scene-relative path:
/// `None` for an absolute URI (a scheme, a leading `/`) or one climbing out of
/// the scene.
pub fn resolve_uri(model_rel: &str, uri: &str) -> Option<String> {
    let uri = uri.split(['?', '#']).next().unwrap_or("");
    if uri.is_empty() || uri.starts_with('/') || uri.contains('\\') {
        return None;
    }
    if let Some(colon) = uri.find(':')
        && !uri[..colon].contains('/')
    {
        return None;
    }
    let uri = percent_decode(uri)?;
    let mut parts: Vec<&str> = model_rel.split('/').collect();
    parts.pop();
    for seg in uri.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            seg => parts.push(seg),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// The URI the model at `model_rel` names the scene file `target_rel` by.
pub fn relative_uri(model_rel: &str, target_rel: &str) -> String {
    let mut from: Vec<&str> = model_rel.split('/').collect();
    from.pop();
    let to: Vec<&str> = target_rel.split('/').collect();
    let common = from
        .iter()
        .zip(&to)
        .take_while(|(a, b)| a == b)
        .count()
        .min(to.len().saturating_sub(1));
    let mut out: Vec<&str> = vec![".."; from.len() - common];
    out.extend(&to[common..]);
    out.join("/")
}

/// The bytes of buffer `buffer`: a GLB's binary chunk, a `data:` URI, or a
/// file beside the model read through `read`.
fn buffer_bytes(
    model: &Model,
    model_rel: &str,
    buffer: usize,
    read: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Option<Vec<u8>> {
    let b = model.array("buffers").get(buffer)?;
    match b.get("uri").and_then(Value::as_str) {
        None if buffer == 0 && model.glb => model.bin.clone(),
        None => None,
        Some(uri) if uri.starts_with("data:") => data_uri(uri),
        Some(uri) => read(&resolve_uri(model_rel, uri)?),
    }
}

/// Image `i`'s bytes or file; `Err` says why there is nothing to read.
pub fn image_source(
    model: &Model,
    model_rel: &str,
    i: usize,
    read: &dyn Fn(&str) -> Option<Vec<u8>>,
) -> Result<ImageSource, String> {
    let image = model.array("images").get(i).ok_or("no such image")?;
    if let Some(uri) = image.get("uri").and_then(Value::as_str) {
        if uri.starts_with("data:") {
            return data_uri(uri)
                .map(ImageSource::Embedded)
                .ok_or_else(|| "undecodable data: URI".to_string());
        }
        return resolve_uri(model_rel, uri)
            .map(ImageSource::External)
            .ok_or_else(|| format!("URI {uri} is not a file of the scene"));
    }
    let view_index = index(image, "bufferView").ok_or("image has neither uri nor bufferView")?;
    let view = model
        .array("bufferViews")
        .get(view_index)
        .ok_or("bufferView out of range")?;
    let buffer = index(view, "buffer").ok_or("bufferView without a buffer")?;
    let bytes = buffer_bytes(model, model_rel, buffer, read).ok_or("unreadable buffer")?;
    let start = index(view, "byteOffset").unwrap_or(0);
    let len = index(view, "byteLength").ok_or("bufferView without byteLength")?;
    start
        .checked_add(len)
        .and_then(|end| bytes.get(start..end))
        .map(|b| ImageSource::Embedded(b.to_vec()))
        .ok_or_else(|| "bufferView past the end of its buffer".to_string())
}

/// How a texture slot's texels are read: colour slots are sRGB, normals are
/// normal maps, everything else is data.
fn slot_mode(key: &str) -> ColorMode {
    match key {
        "baseColorTexture"
        | "emissiveTexture"
        | "diffuseTexture"
        | "sheenColorTexture"
        | "specularColorTexture"
        | "specularGlossinessTexture" => ColorMode::Srgb,
        "normalTexture" | "clearcoatNormalTexture" => ColorMode::Normal,
        _ => ColorMode::Linear,
    }
}

/// Every texture-info slot under a material (its extensions included): a key
/// ending in `Texture` holding an object with an `index`.
fn texture_slots(v: &Value, out: &mut Vec<(String, usize)>) {
    match v {
        Value::Object(map) => {
            for (k, child) in map {
                if k.ends_with("Texture")
                    && let Some(i) = index(child, "index")
                {
                    out.push((k.clone(), i));
                }
                texture_slots(child, out);
            }
        }
        Value::Array(items) => items.iter().for_each(|c| texture_slots(c, out)),
        _ => {}
    }
}

/// What the module decided about one image of a model.
#[derive(Debug, Clone, PartialEq)]
pub enum ImagePlan {
    /// Sampled by no texture: not a texture image, left alone silently.
    Unused,
    /// Left as it is, and why.
    Keep(String),
    /// To be encoded with this colour mode; `mixed` when colour and data
    /// slots share it.
    Encode { mode: ColorMode, mixed: bool },
}

/// One plan per image, from the textures and the material slots sampling it.
pub fn plan_images(model: &Model) -> Vec<ImagePlan> {
    let images = model.array("images");
    let textures = model.array("textures");
    let mut sampled = vec![false; images.len()];
    let mut blocked: BTreeMap<usize, String> = BTreeMap::new();
    for (t, texture) in textures.iter().enumerate() {
        let core = index(texture, "source");
        let ext_sources: Vec<(String, usize)> = texture
            .get("extensions")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter_map(|(name, ext)| Some((name.clone(), index(ext, "source")?)))
            .collect();
        if let Some(i) = core.filter(|i| *i < images.len()) {
            sampled[i] = true;
            if let Some((name, _)) = ext_sources.first() {
                blocked
                    .entry(i)
                    .or_insert_with(|| format!("texture {t} also uses {name}"));
            }
        }
        for (name, i) in ext_sources {
            if i < images.len() {
                blocked
                    .entry(i)
                    .or_insert_with(|| format!("texture {t} samples it through {name}"));
            }
        }
    }
    let mut uses: Vec<Vec<ColorMode>> = vec![Vec::new(); images.len()];
    for material in model.array("materials") {
        let mut slots = Vec::new();
        texture_slots(material, &mut slots);
        for (key, t) in slots {
            if let Some(i) = textures
                .get(t)
                .and_then(|tex| index(tex, "source"))
                .filter(|i| *i < images.len())
            {
                uses[i].push(slot_mode(&key));
            }
        }
    }
    images
        .iter()
        .enumerate()
        .map(|(i, image)| {
            if let Some(why) = blocked.get(&i) {
                return ImagePlan::Keep(why.clone());
            }
            if !sampled[i] {
                return ImagePlan::Unused;
            }
            let ktx2 = image.get("mimeType").and_then(Value::as_str) == Some("image/ktx2")
                || image
                    .get("uri")
                    .and_then(Value::as_str)
                    .is_some_and(|u| u.to_ascii_lowercase().ends_with(".ktx2"));
            if ktx2 {
                return ImagePlan::Keep("already KTX2".into());
            }
            let (mode, mixed) = resolve_mode(&uses[i]);
            ImagePlan::Encode { mode, mixed }
        })
        .collect()
}

/// sRGB when any slot reads colour (`mixed` when another reads data);
/// otherwise a normal map when every slot is a normal slot; otherwise linear,
/// which is also what an image no material slot names gets.
pub fn resolve_mode(uses: &[ColorMode]) -> (ColorMode, bool) {
    let srgb = uses.contains(&ColorMode::Srgb);
    let data = uses.iter().any(|m| *m != ColorMode::Srgb);
    if srgb {
        return (ColorMode::Srgb, data);
    }
    if !uses.is_empty() && uses.iter().all(|m| *m == ColorMode::Normal) {
        return (ColorMode::Normal, false);
    }
    (ColorMode::Linear, false)
}

/// Points each image of `replacements` at its `.ktx2` URI, moves the textures
/// sampling it onto `KHR_texture_basisu`, and drops the bytes it leaves unused
/// from a GLB's binary chunk. Returns whether the binary chunk was repacked.
pub fn apply(model: &mut Model, replacements: &BTreeMap<usize, String>) -> bool {
    if replacements.is_empty() {
        return false;
    }
    let mut freed_views = BTreeSet::new();
    if let Some(images) = model.json.get_mut("images").and_then(Value::as_array_mut) {
        for (&i, uri) in replacements {
            let Some(image) = images.get_mut(i).and_then(Value::as_object_mut) else {
                continue;
            };
            if let Some(v) = image.remove("bufferView").and_then(|v| v.as_u64()) {
                freed_views.insert(v as usize);
            }
            image.remove("uri");
            image.remove("mimeType");
            image.insert("uri".into(), json!(uri));
            image.insert("mimeType".into(), json!("image/ktx2"));
        }
    }
    if let Some(textures) = model.json.get_mut("textures").and_then(Value::as_array_mut) {
        for texture in textures.iter_mut().filter_map(Value::as_object_mut) {
            let Some(source) = texture.get("source").and_then(Value::as_u64) else {
                continue;
            };
            if !replacements.contains_key(&(source as usize)) {
                continue;
            }
            texture.remove("source");
            let extensions = texture
                .entry("extensions")
                .or_insert_with(|| Value::Object(Map::new()));
            if let Some(extensions) = extensions.as_object_mut() {
                extensions.insert(BASISU_EXT.into(), json!({ "source": source }));
            }
        }
    }
    for list in ["extensionsUsed", "extensionsRequired"] {
        let root = model.json.as_object_mut().expect("parse checked an object");
        let entry = root.entry(list).or_insert_with(|| json!([]));
        if let Some(names) = entry.as_array_mut()
            && !names.iter().any(|n| n.as_str() == Some(BASISU_EXT))
        {
            names.push(json!(BASISU_EXT));
        }
    }
    let used = model.strings("extensionsUsed");
    let safe = used.iter().all(|e| repack_safe(e));
    model.glb && model.bin.is_some() && safe && repack(model, &freed_views)
}

/// Every buffer-view reference this module knows how to remap: accessors
/// (and their sparse parts), images, and Draco primitives.
fn view_refs(json: &mut Value) -> Vec<&mut Value> {
    let mut out = Vec::new();
    let Some(root) = json.as_object_mut() else {
        return out;
    };
    for (key, list) in root.iter_mut() {
        let Some(items) = list.as_array_mut() else {
            continue;
        };
        match key.as_str() {
            "accessors" => {
                for a in items.iter_mut().filter_map(Value::as_object_mut) {
                    for (k, v) in a.iter_mut() {
                        match k.as_str() {
                            "bufferView" => out.push(v),
                            "sparse" => {
                                let parts = v.as_object_mut().into_iter().flatten();
                                for (part, body) in parts {
                                    if (part == "indices" || part == "values")
                                        && let Some(view) = body.get_mut("bufferView")
                                    {
                                        out.push(view);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
            }
            "images" => {
                for image in items.iter_mut() {
                    if let Some(v) = image.get_mut("bufferView") {
                        out.push(v);
                    }
                }
            }
            "meshes" => {
                for mesh in items.iter_mut() {
                    let Some(prims) = mesh.get_mut("primitives").and_then(Value::as_array_mut)
                    else {
                        continue;
                    };
                    for prim in prims {
                        if let Some(v) = prim
                            .get_mut("extensions")
                            .and_then(|e| e.get_mut(DRACO_EXT))
                            .and_then(|d| d.get_mut("bufferView"))
                        {
                            out.push(v);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Drops the views in `freed` nothing references any more, rebuilds the
/// binary chunk from the views left (4-byte aligned, in view order) and
/// renumbers every reference. A view past the end of the chunk aborts it,
/// leaving the model as it was.
fn repack(model: &mut Model, freed: &BTreeSet<usize>) -> bool {
    let referenced: BTreeSet<usize> = view_refs(&mut model.json)
        .into_iter()
        .filter_map(|v| v.as_u64().map(|i| i as usize))
        .collect();
    let dropped: BTreeSet<usize> = freed.difference(&referenced).copied().collect();
    if dropped.is_empty() {
        return false;
    }
    let old_bin = model.bin.as_deref().unwrap_or_default();
    let views = model.array("bufferViews").to_vec();
    let mut new_bin: Vec<u8> = Vec::with_capacity(old_bin.len());
    let mut new_views = Vec::with_capacity(views.len());
    let mut renumber: BTreeMap<usize, usize> = BTreeMap::new();
    for (i, view) in views.into_iter().enumerate() {
        if dropped.contains(&i) {
            continue;
        }
        let mut view = view;
        if index(&view, "buffer") == Some(0) {
            let start = index(&view, "byteOffset").unwrap_or(0);
            let Some(bytes) = index(&view, "byteLength")
                .and_then(|len| start.checked_add(len))
                .and_then(|end| old_bin.get(start..end))
            else {
                return false;
            };
            while !new_bin.len().is_multiple_of(4) {
                new_bin.push(0);
            }
            if let Some(obj) = view.as_object_mut() {
                obj.insert("byteOffset".into(), json!(new_bin.len()));
            }
            new_bin.extend_from_slice(bytes);
        }
        renumber.insert(i, new_views.len());
        new_views.push(view);
    }
    for r in view_refs(&mut model.json) {
        if let Some(new) = r.as_u64().and_then(|old| renumber.get(&(old as usize))) {
            *r = json!(new);
        }
    }
    if new_bin.is_empty() {
        // a buffer's byteLength is at least 1
        new_bin = vec![0; 4];
    }
    let root = model.json.as_object_mut().expect("parse checked an object");
    root.insert("bufferViews".into(), Value::Array(new_views));
    if let Some(buffer) = root
        .get_mut("buffers")
        .and_then(|b| b.get_mut(0))
        .and_then(Value::as_object_mut)
    {
        buffer.insert("byteLength".into(), json!(new_bin.len()));
    }
    model.bin = Some(new_bin);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glb(json: Value, bin: &[u8]) -> Vec<u8> {
        Model {
            json,
            bin: Some(bin.to_vec()),
            glb: true,
        }
        .to_bytes()
    }

    /// positions (view 0), a PNG (view 1), a Draco blob (view 2), and an
    /// external image sampled as a normal map.
    fn sample() -> (Value, Vec<u8>) {
        let mut bin = vec![1u8; 12];
        bin.extend_from_slice(b"PNGBYTES");
        bin.extend_from_slice(b"DRACO!!!");
        let json = json!({
            "asset": {"version": "2.0"},
            "extensionsUsed": ["KHR_draco_mesh_compression"],
            "extensionsRequired": ["KHR_draco_mesh_compression"],
            "buffers": [{"byteLength": bin.len()}],
            "bufferViews": [
                {"buffer": 0, "byteOffset": 0, "byteLength": 12},
                {"buffer": 0, "byteOffset": 12, "byteLength": 8},
                {"buffer": 0, "byteOffset": 20, "byteLength": 8}
            ],
            "accessors": [{"bufferView": 0, "componentType": 5126, "count": 1, "type": "VEC3"}],
            "meshes": [{"primitives": [{
                "attributes": {"POSITION": 0},
                "extensions": {"KHR_draco_mesh_compression": {"bufferView": 2, "attributes": {"POSITION": 0}}}
            }]}],
            "images": [
                {"bufferView": 1, "mimeType": "image/png", "name": "albedo"},
                {"uri": "tex/normal%20map.png"}
            ],
            "textures": [{"source": 0, "sampler": 0}, {"source": 1}],
            "samplers": [{}],
            "materials": [{
                "pbrMetallicRoughness": {"baseColorTexture": {"index": 0}},
                "normalTexture": {"index": 1}
            }]
        });
        (json, bin)
    }

    #[test]
    fn glb_round_trips_and_pads_chunks() {
        let (json, bin) = sample();
        let bytes = glb(json.clone(), &bin);
        assert!(bytes.len().is_multiple_of(4));
        assert_eq!(u32_at(&bytes, 8), Some(bytes.len() as u32));
        let model = Model::parse(&bytes).unwrap();
        assert_eq!(model.json, json);
        assert_eq!(model.bin.as_deref(), Some(bin.as_slice()));
        assert!(model.glb);
    }

    #[test]
    fn embedded_image_is_moved_out_and_the_chunk_repacked_draco_untouched() {
        let (json, bin) = sample();
        let mut model = Model::parse(&glb(json, &bin)).unwrap();
        let replacements = BTreeMap::from([(0, "../optimized-textures/aa.ktx2".to_string())]);
        assert!(apply(&mut model, &replacements));
        let j = &model.json;
        assert_eq!(
            j["images"][0],
            json!({"name": "albedo", "uri": "../optimized-textures/aa.ktx2", "mimeType": "image/ktx2"})
        );
        assert_eq!(
            j["textures"][0],
            json!({"sampler": 0, "extensions": {"KHR_texture_basisu": {"source": 0}}})
        );
        assert_eq!(j["textures"][1], json!({"source": 1}));
        assert_eq!(
            j["extensionsUsed"],
            json!(["KHR_draco_mesh_compression", "KHR_texture_basisu"])
        );
        assert_eq!(
            j["extensionsRequired"],
            json!(["KHR_draco_mesh_compression", "KHR_texture_basisu"])
        );
        // view 1 is gone: Draco's view 2 becomes 1, its bytes intact
        assert_eq!(j["bufferViews"].as_array().unwrap().len(), 2);
        assert_eq!(j["accessors"][0]["bufferView"], json!(0));
        let draco = &j["meshes"][0]["primitives"][0]["extensions"]["KHR_draco_mesh_compression"];
        assert_eq!(draco["bufferView"], json!(1));
        assert_eq!(draco["attributes"], json!({"POSITION": 0}));
        let bin = model.bin.as_ref().unwrap();
        assert_eq!(j["buffers"][0]["byteLength"], json!(bin.len()));
        assert_eq!(&bin[..12], &[1u8; 12]);
        assert_eq!(j["bufferViews"][1]["byteOffset"], json!(12));
        assert_eq!(&bin[12..20], b"DRACO!!!");
        assert!(!bin.windows(8).any(|w| w == b"PNGBYTES"));
        // and it still parses as a GLB
        let again = Model::parse(&model.to_bytes()).unwrap();
        assert_eq!(&again.json, j);
    }

    #[test]
    fn external_image_is_repointed_without_touching_the_chunk() {
        let (json, bin) = sample();
        let mut model = Model::parse(&glb(json, &bin)).unwrap();
        let replacements = BTreeMap::from([(1, "x.ktx2".to_string())]);
        assert!(!apply(&mut model, &replacements), "no bytes freed");
        assert_eq!(model.bin.as_deref(), Some(bin.as_slice()));
        assert_eq!(
            model.json["images"][1],
            json!({"uri": "x.ktx2", "mimeType": "image/ktx2"})
        );
        assert_eq!(
            model.json["textures"][1],
            json!({"extensions": {"KHR_texture_basisu": {"source": 1}}})
        );
        assert_eq!(model.json["bufferViews"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn an_unknown_extension_keeps_the_binary_chunk_as_it_was() {
        let (mut json, bin) = sample();
        json["extensionsUsed"] = json!(["KHR_draco_mesh_compression", "EXT_meshopt_compression"]);
        let mut model = Model::parse(&glb(json, &bin)).unwrap();
        let replacements = BTreeMap::from([(0, "a.ktx2".to_string())]);
        assert!(!apply(&mut model, &replacements));
        assert_eq!(model.bin.as_deref(), Some(bin.as_slice()));
        assert_eq!(model.json["bufferViews"].as_array().unwrap().len(), 3);
        assert_eq!(
            model.json["extensionsUsed"],
            json!([
                "KHR_draco_mesh_compression",
                "EXT_meshopt_compression",
                "KHR_texture_basisu"
            ])
        );
    }

    #[test]
    fn a_view_shared_with_an_accessor_is_kept() {
        let (mut json, bin) = sample();
        json["accessors"]
            .as_array_mut()
            .unwrap()
            .push(json!({"bufferView": 1, "componentType": 5121, "count": 8, "type": "SCALAR"}));
        let mut model = Model::parse(&glb(json, &bin)).unwrap();
        assert!(!apply(
            &mut model,
            &BTreeMap::from([(0, "a.ktx2".to_string())])
        ));
        assert_eq!(model.json["bufferViews"].as_array().unwrap().len(), 3);
        assert_eq!(model.bin.as_deref(), Some(bin.as_slice()));
    }

    #[test]
    fn data_uri_images_and_gltf_json_rewrite() {
        let png = b"\x89PNG fake";
        let uri = format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        );
        let json = json!({
            "asset": {"version": "2.0"},
            "images": [{"uri": uri}],
            "textures": [{"source": 0}],
            "materials": [{"emissiveTexture": {"index": 0}}]
        });
        let bytes = serde_json::to_vec(&json).unwrap();
        let mut model = Model::parse(&bytes).unwrap();
        assert!(!model.glb);
        let source = image_source(&model, "m/a.gltf", 0, &|_| None).unwrap();
        assert_eq!(source, ImageSource::Embedded(png.to_vec()));
        apply(&mut model, &BTreeMap::from([(0, "../k.ktx2".to_string())]));
        let out = Model::parse(&model.to_bytes()).unwrap();
        assert_eq!(out.json["images"][0]["uri"], json!("../k.ktx2"));
        assert!(!String::from_utf8_lossy(&model.to_bytes()).contains("base64"));
    }

    #[test]
    fn image_sources_resolve_relative_to_the_model() {
        let (json, bin) = sample();
        let model = Model::parse(&glb(json, &bin)).unwrap();
        assert_eq!(
            image_source(&model, "models/a.glb", 0, &|_| None),
            Ok(ImageSource::Embedded(b"PNGBYTES".to_vec()))
        );
        assert_eq!(
            image_source(&model, "models/a.glb", 1, &|_| None),
            Ok(ImageSource::External("models/tex/normal map.png".into()))
        );
    }

    #[test]
    fn uris_resolve_and_relativize() {
        assert_eq!(
            resolve_uri("a/b/m.glb", "../t.png").as_deref(),
            Some("a/t.png")
        );
        assert_eq!(
            resolve_uri("m.glb", "./x/%41.png").as_deref(),
            Some("x/A.png")
        );
        assert_eq!(resolve_uri("m.glb", "../escape.png"), None);
        assert_eq!(resolve_uri("m.glb", "https://example.org/t.png"), None);
        assert_eq!(resolve_uri("m.glb", "/abs.png"), None);
        assert_eq!(
            relative_uri("models/sub/m.glb", "optimized-textures/ab.ktx2"),
            "../../optimized-textures/ab.ktx2"
        );
        assert_eq!(
            relative_uri("m.glb", "optimized-textures/ab.ktx2"),
            "optimized-textures/ab.ktx2"
        );
        assert_eq!(
            relative_uri("optimized-textures/m.glb", "optimized-textures/ab.ktx2"),
            "ab.ktx2"
        );
    }

    #[test]
    fn colour_modes_follow_material_slots() {
        use ColorMode::*;
        assert_eq!(resolve_mode(&[Srgb]), (Srgb, false));
        assert_eq!(resolve_mode(&[Normal]), (Normal, false));
        assert_eq!(resolve_mode(&[Normal, Normal]), (Normal, false));
        assert_eq!(resolve_mode(&[Linear]), (Linear, false));
        assert_eq!(resolve_mode(&[Normal, Linear]), (Linear, false));
        assert_eq!(resolve_mode(&[Srgb, Linear]), (Srgb, true));
        assert_eq!(resolve_mode(&[]), (Linear, false));

        let json = json!({
            "images": [{"uri": "a.png"}, {"uri": "b.png"}, {"uri": "c.png"},
                       {"uri": "d.png"}, {"uri": "e.ktx2"}, {"uri": "f.png"}, {"uri": "g.png"}],
            "textures": [{"source": 0}, {"source": 1}, {"source": 2}, {"source": 3},
                         {"source": 4},
                         {"source": 5, "extensions": {"EXT_texture_webp": {"source": 6}}}],
            "materials": [{
                "pbrMetallicRoughness": {
                    "baseColorTexture": {"index": 0},
                    "metallicRoughnessTexture": {"index": 2}
                },
                "normalTexture": {"index": 1},
                "occlusionTexture": {"index": 0},
                "extensions": {"KHR_materials_clearcoat": {"clearcoatTexture": {"index": 3}}}
            }]
        });
        let model = Model {
            json,
            bin: None,
            glb: false,
        };
        let plans = plan_images(&model);
        assert_eq!(
            plans[0],
            ImagePlan::Encode {
                mode: Srgb,
                mixed: true
            }
        );
        assert_eq!(
            plans[1],
            ImagePlan::Encode {
                mode: Normal,
                mixed: false
            }
        );
        assert_eq!(
            plans[2],
            ImagePlan::Encode {
                mode: Linear,
                mixed: false
            }
        );
        assert_eq!(
            plans[3],
            ImagePlan::Encode {
                mode: Linear,
                mixed: false
            }
        );
        assert_eq!(plans[4], ImagePlan::Keep("already KTX2".into()));
        assert!(matches!(plans[5], ImagePlan::Keep(_)));
        assert!(matches!(plans[6], ImagePlan::Keep(_)));
    }

    #[test]
    fn unsampled_images_are_unused() {
        let model = Model {
            json: json!({"images": [{"uri": "a.png"}]}),
            bin: None,
            glb: false,
        };
        assert_eq!(plan_images(&model), vec![ImagePlan::Unused]);
    }

    #[test]
    fn malformed_glbs_are_errors_not_panics() {
        assert!(Model::parse(b"glTF").is_err());
        assert!(Model::parse(b"glTF\x01\0\0\0\x0c\0\0\0").is_err());
        let mut truncated = glb(json!({"asset": {"version": "2.0"}}), b"1234");
        truncated[12] = 0xff;
        assert!(Model::parse(&truncated).is_err());
        assert!(Model::parse(b"not json").is_err());
    }
}
