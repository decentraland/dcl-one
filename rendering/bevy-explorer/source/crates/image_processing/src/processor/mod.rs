mod draco;
#[cfg(not(target_arch = "wasm32"))]
mod native_fs;
#[cfg(target_arch = "wasm32")]
mod wasm_fs;
use std::{collections::VecDeque, io::Write};

use base64::{engine::general_purpose, Engine};
use bevy::log::{
    tracing::{error, info},
    warn,
};
use block_compression::BC7Settings;
use ddsfile::{Caps2, Dds, DxgiFormat};
use gltf::Glb;
#[cfg(not(target_arch = "wasm32"))]
use native_fs::{read_file, write_file};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
#[cfg(target_arch = "wasm32")]
use wasm_fs::{read_file, write_file};

use crate::{AssetForProcessing, ProcessingAssetType};

pub(crate) async fn process_events(
    mut req_rx: UnboundedReceiver<AssetForProcessing>,
    resp_sx: UnboundedSender<Result<AssetForProcessing, ()>>,
) {
    // requests taken from the channel, not yet processed
    let mut pending: VecDeque<AssetForProcessing> = VecDeque::new();
    loop {
        if pending.is_empty() {
            let Some(req) = req_rx.blocking_recv() else {
                return;
            };
            pending.push_back(req);
        }
        while let Ok(req) = req_rx.try_recv() {
            pending.push_back(req);
        }
        // a gltf that failed to load shows nothing until it is repaired, while a texture
        // compression only improves one that is already showing: repairs go first, so a
        // millisecond repair never waits behind a queue of 200ms compressions. ktx2 cache
        // writes are pure i/o and free their stashed dds, so they go next
        let priority = |req: &AssetForProcessing| match req.ty {
            ProcessingAssetType::UnloadableGltf => 0,
            ProcessingAssetType::Ktx2Cache => 1,
            ProcessingAssetType::Gltf | ProcessingAssetType::Image => 2,
        };
        let next = pending
            .iter()
            .enumerate()
            .min_by_key(|(_, req)| priority(req))
            .map(|(i, _)| i)
            .unwrap_or(0);
        let req = pending.remove(next).expect("pending is not empty");
        let cache_write = matches!(req.ty, ProcessingAssetType::Ktx2Cache);

        // the read yields the entry's key (on web, the cache entry the url resolves to)
        let Ok((raw_bytes, key)) = read_file(&req.cache_path).await else {
            error!("can't read {:?}", req.cache_path);
            if cache_write {
                take_ktx2_result(&req);
            }
            let _ = resp_sx.send(Err(()));
            continue;
        };

        let data = match req.ty {
            ProcessingAssetType::Ktx2Cache => match take_ktx2_result(&req) {
                Some(dds) => dds,
                None => {
                    error!("no ktx2 transcode stashed for {:?}", req.base_path);
                    let _ = resp_sx.send(Err(()));
                    continue;
                }
            },
            ProcessingAssetType::Gltf | ProcessingAssetType::UnloadableGltf => {
                let expect_repair = matches!(req.ty, ProcessingAssetType::UnloadableGltf);
                match process_gltf(&raw_bytes, expect_repair) {
                    Ok(gltf) => gltf,
                    Err(e) => {
                        error!("failed to process gltf {:?}: {:?}", req.base_path, e);
                        let _ = resp_sx.send(Err(()));
                        continue;
                    }
                }
            }
            ProcessingAssetType::Image => match process_image(&raw_bytes) {
                Ok(dds) => dds,
                Err(e) => {
                    error!("failed to process image {:?}: {:?}", req.base_path, e);
                    let _ = resp_sx.send(Err(()));
                    continue;
                }
            },
        };

        let Ok(()) = write_file(&key, &data).await else {
            error!("can't write {:?}", req.base_path);
            let _ = resp_sx.send(Err(()));
            continue;
        };

        let _ = resp_sx.send(Ok(req));
    }
}

/// The dds `Ktx2Loader` stashed for `req.base_path`, removed from the stash.
fn take_ktx2_result(req: &AssetForProcessing) -> Option<Vec<u8>> {
    crate::ktx2_loader::KTX2_RESULTS
        .lock()
        .ok()?
        .remove(&req.base_path.to_string())
        .map(|(_, dds)| dds)
}

#[derive(Debug)]
pub(crate) enum ImageProcessError {
    CantLoad,
    DdsFailed,
}

fn process_image(raw_bytes: &[u8]) -> Result<Vec<u8>, ImageProcessError> {
    let Ok(img) = image::load_from_memory(raw_bytes) else {
        return Err(ImageProcessError::CantLoad);
    };

    compress_decoded(img)
}

fn compress_decoded(img: image::DynamicImage) -> Result<Vec<u8>, ImageProcessError> {
    let initial_width = img.width();
    let initial_height = img.height();
    let resized = if initial_width > 1024
        || initial_height > 1024
        || !initial_width.is_multiple_of(4)
        || !initial_height.is_multiple_of(4)
    {
        let downratio = (1024.0 / initial_width.max(initial_height) as f32).min(1.0);
        // clamp to one block: extreme aspect ratios can round a dimension to zero
        let resized_width = ((initial_width as f32 * downratio * 0.25).round() as u32).max(1) * 4;
        let resized_height = ((initial_height as f32 * downratio * 0.25).round() as u32).max(1) * 4;
        img.resize_exact(
            resized_width,
            resized_height,
            image::imageops::FilterType::CatmullRom,
        )
    } else {
        img
    };

    let width = resized.width();
    let height = resized.height();

    let rgba_pixels = resized.to_rgba8().into_raw();

    let mut compressed_data = vec![0u8; (width * height) as usize];

    info!(
        "size: {}/{} -> {}/{}",
        initial_width, initial_height, width, height
    );
    block_compression::encode::compress_rgba8(
        block_compression::CompressionVariant::BC7(BC7Settings::alpha_ultrafast()),
        &rgba_pixels,
        &mut compressed_data,
        width,
        height,
        width * 4,
    );

    bc7_dds(width, height, compressed_data)
}

/// A Basis Universal KTX2 image (`KHR_texture_basisu`) as BC7: UASTC blocks are rewritten as BC7
/// blocks, with no decode and search as `compress_decoded` does. The largest level within that
/// path's 1024 cap is taken; a level still over it, or not whole 4x4 blocks, goes through RGBA
/// and `compress_decoded` so the result matches what a PNG of that size would give.
pub(crate) fn transcode_ktx2(bytes: &[u8]) -> Result<Vec<u8>, ImageProcessError> {
    use basisu::{DecodeFlags, TargetFormat, Transcoder};

    check_ktx2_header(bytes)?;
    let texture = Transcoder::new(bytes).map_err(|_| ImageProcessError::CantLoad)?;
    let level = (0..texture.level_count())
        .find(|&level| {
            texture
                .image_level_info(level)
                .is_ok_and(|info| info.width.max(info.height) <= 1024)
        })
        .unwrap_or(0);
    let info = texture
        .image_level_info(level)
        .map_err(|_| ImageProcessError::CantLoad)?;

    if info.width.max(info.height) <= 1024
        && info.width.is_multiple_of(4)
        && info.height.is_multiple_of(4)
        && texture.supports(TargetFormat::Bc7Rgba)
    {
        let blocks = texture
            .transcode(level, TargetFormat::Bc7Rgba, DecodeFlags::NONE)
            .map_err(|_| ImageProcessError::CantLoad)?;
        return bc7_dds(info.width, info.height, blocks);
    }

    let rgba = texture
        .transcode(level, TargetFormat::Rgba32, DecodeFlags::NONE)
        .map_err(|_| ImageProcessError::CantLoad)?;
    let img = image::RgbaImage::from_raw(info.width, info.height, rgba)
        .ok_or(ImageProcessError::CantLoad)?;
    compress_decoded(image::DynamicImage::ImageRgba8(img))
}

const KTX2_MAGIC: &[u8] = b"\xABKTX 20\xBB\r\n\x1A\n";

/// The largest KTX2 width or height taken. Its RGBA fallback is 256 MB, within the 512 MB
/// `image` allows a PNG.
const KTX2_MAX_SIZE: u32 = 8192;

/// Checks a KTX2 header against the file before `basisu` reads it: `basisu` sizes buffers from
/// the declared dimensions and level lengths without checking them against the data, and ktx2
/// files are scene content. Only a single 2D UASTC image within `KTX2_MAX_SIZE` passes, every
/// range must lie within the file, and no level may declare more than 16 bytes per 4x4 block.
/// ETC1S is rejected: its BasisLZ global data holds lengths `basisu` adds unchecked, which wrap
/// on wasm32.
fn check_ktx2_header(bytes: &[u8]) -> Result<(), ImageProcessError> {
    let u32_at = |at: usize| -> Option<u32> {
        Some(u32::from_le_bytes(bytes.get(at..at + 4)?.try_into().ok()?))
    };
    let u64_at = |at: usize| -> Option<u64> {
        Some(u64::from_le_bytes(bytes.get(at..at + 8)?.try_into().ok()?))
    };
    let within = |offset: u64, length: u64| {
        offset
            .checked_add(length)
            .is_some_and(|end| end <= bytes.len() as u64)
    };

    let valid = || -> Option<bool> {
        let (width, height) = (u32_at(20)?, u32_at(24)?);
        let (depth, layers, faces) = (u32_at(28)?, u32_at(32)?, u32_at(36)?);
        let levels = u32_at(40)?.max(1);
        // basis universal data (vkFormat undefined), stored plain or zstd-supercompressed
        let (format, scheme) = (u32_at(12)?, u32_at(44)?);
        if !bytes.starts_with(KTX2_MAGIC)
            || format != 0
            || !matches!(scheme, 0 | 2)
            || !(1..=KTX2_MAX_SIZE).contains(&width)
            || !(1..=KTX2_MAX_SIZE).contains(&height)
            // one 2d image: the loader and the gltf repair take a single texture
            || depth > 1
            || layers > 1
            || faces != 1
            // a full mip chain is the most a valid file declares
            || levels > u32::BITS - width.max(height).leading_zeros()
        {
            return Some(false);
        }
        // data format descriptor, key/value data and supercompression global data
        if !within(u32_at(48)?.into(), u32_at(52)?.into())
            || !within(u32_at(56)?.into(), u32_at(60)?.into())
            || !within(u64_at(64)?, u64_at(72)?)
        {
            return Some(false);
        }
        for level in 0..levels {
            let at = 80 + 24 * level as usize;
            let (offset, length, uncompressed) = (u64_at(at)?, u64_at(at + 8)?, u64_at(at + 16)?);
            let blocks = u64::from((width >> level).max(1).div_ceil(4))
                * u64::from((height >> level).max(1).div_ceil(4));
            if !within(offset, length) || uncompressed > blocks * 16 {
                return Some(false);
            }
        }
        Some(true)
    };

    match valid() {
        Some(true) => Ok(()),
        _ => Err(ImageProcessError::CantLoad),
    }
}

fn bc7_dds(
    width: u32,
    height: u32,
    compressed_data: Vec<u8>,
) -> Result<Vec<u8>, ImageProcessError> {
    let Ok(mut dds) = Dds::new_dxgi(ddsfile::NewDxgiParams {
        height,
        width,
        depth: None,
        format: DxgiFormat::BC7_UNorm,
        mipmap_levels: None,
        array_layers: None,
        caps2: Some(Caps2::empty()),
        is_cubemap: false,
        resource_dimension: ddsfile::D3D10ResourceDimension::Texture2D,
        alpha_mode: ddsfile::AlphaMode::PreMultiplied,
    }) else {
        return Err(ImageProcessError::DdsFailed);
    };

    dds.data = compressed_data;

    let mut output = Vec::new();
    let Ok(()) = dds.write(&mut output) else {
        return Err(ImageProcessError::DdsFailed);
    };

    if dds.get_height() == 0 || dds.get_width() == 0 {
        error!("returned size zero processing {}/{}", width, height);
        return Err(ImageProcessError::DdsFailed);
    }

    Ok(output)
}

#[derive(Debug)]
#[allow(dead_code)] // we use the ignored debug impl
enum GltfProcessError {
    ImageError(ImageProcessError),
    InvalidHeader,
    ParseFailed,
    Draco(draco::DracoError),
    NothingToRepair,
}

impl From<ImageProcessError> for GltfProcessError {
    fn from(value: ImageProcessError) -> Self {
        Self::ImageError(value)
    }
}

fn process_gltf(raw_bytes: &[u8], expect_repair: bool) -> Result<Vec<u8>, GltfProcessError> {
    // failed-load requests can hand us arbitrary bytes, so parse fallibly
    let (mut root, mut old_bin): (gltf_json::Root, Vec<u8>) = if raw_bytes.starts_with(b"glTF") {
        let glb = Glb::from_slice(raw_bytes).map_err(|_| GltfProcessError::ParseFailed)?;
        (
            gltf_json::deserialize::from_slice(&glb.json)
                .map_err(|_| GltfProcessError::ParseFailed)?,
            glb.bin.map(|b| b.into_owned()).unwrap_or_default(),
        )
    } else if raw_bytes.starts_with(b"{") {
        (
            gltf_json::deserialize::from_slice(raw_bytes)
                .map_err(|_| GltfProcessError::ParseFailed)?,
            Vec::default(),
        )
    } else {
        return Err(GltfProcessError::InvalidHeader);
    };

    // ai code below
    // absorb data URI buffers into the binary chunk
    if let Some(first_buffer) = root.buffers.first_mut() {
        if let Some(uri) = &first_buffer.uri {
            if uri.starts_with("data:") {
                // This is a .gltf with embedded binary.
                // We should decode it and make it the "real" binary chunk.
                let parts: Vec<&str> = uri.split(',').collect();
                let decoded = general_purpose::STANDARD
                    .decode(parts[1])
                    .map_err(|_| GltfProcessError::ParseFailed)?;

                // If we already had a binary chunk (from a GLB), append this.
                // If we came from JSON, 'old_bin' was empty, so this becomes the start.
                // (But usually, valid GLTF/GLB is mutually exclusive on this).
                if old_bin.is_empty() {
                    // This is our new base binary!
                    // We treat this decoded data as the "old_bin" for the rest of the logic.
                    old_bin = decoded;
                }

                // Clear the URI so it becomes a valid GLB buffer
                first_buffer.uri = None;
            }
        }
    }

    // decompress draco geometry before the texture pass so the rebuilt binary
    // chunk contains the decoded accessor data
    let draco_views = if root
        .extensions_used
        .iter()
        .any(|e| e == "KHR_draco_mesh_compression")
    {
        draco::decompress(&mut root, &mut old_bin).map_err(GltfProcessError::Draco)?
    } else {
        Vec::default()
    };

    // point KHR_texture_basisu textures at their KTX2 image, which the texture pass below
    // transcodes to BC7, and drop the extension the loader rejects. each texture's original
    // source and extension are kept, to put back if its KTX2 can't be transcoded
    let basisu_required = root
        .extensions_required
        .iter()
        .any(|e| e == "KHR_texture_basisu");
    let mut basisu_textures = Vec::new();
    for (index, texture) in root.textures.iter_mut().enumerate() {
        let Some(extensions) = texture.extensions.as_mut() else {
            continue;
        };
        let Some(basisu) = extensions.others.remove("KHR_texture_basisu") else {
            continue;
        };
        let Some(source) = basisu.get("source").and_then(|source| source.as_u64()) else {
            return Err(GltfProcessError::ParseFailed);
        };
        basisu_textures.push((index, texture.source, basisu));
        texture.source = gltf_json::Index::new(source as u32);
    }
    root.extensions_used.retain(|e| e != "KHR_texture_basisu");
    root.extensions_required
        .retain(|e| e != "KHR_texture_basisu");

    if expect_repair && draco_views.is_empty() && basisu_textures.is_empty() {
        // the gltf failed to load for some reason other than draco compression or basisu
        // textures; there is nothing we can do to fix it
        return Err(GltfProcessError::NothingToRepair);
    }

    // collect all Raw Image Data first
    // store them in a temp vector so we can safely mutate the JSON later.
    let mut raw_images: Vec<Option<Vec<u8>>> = Vec::new();
    let mut zombie_views = vec![false; root.buffer_views.len()];
    for view in draco_views {
        // drop the compressed draco data; the decoded views replace it
        zombie_views[view] = true;
    }

    for image in &root.images {
        let pixels = extract_pixels_safe(image, &root.buffer_views, &old_bin);
        raw_images.push(pixels);

        // mark associated view as zombie
        if let Some(idx) = image.buffer_view {
            zombie_views[idx.value()] = true;
        }
    }

    // rebuild binary (non-image chunks only)
    let mut new_bin: Vec<u8> = Vec::new();

    for (i, view) in root.buffer_views.iter_mut().enumerate() {
        if zombie_views[i] {
            // Nullify zombie views
            view.byte_length = 0u64.into();
            view.byte_offset = Some(0u64.into());
        } else {
            // Copy geometry data
            let start = view.byte_offset.unwrap_or_default().0 as usize;
            let len = view.byte_length.0 as usize;
            // Pad
            while !new_bin.len().is_multiple_of(4) {
                new_bin.push(0);
            }
            let new_offset = new_bin.len() as u64;

            if old_bin.len() >= start + len {
                new_bin.extend_from_slice(&old_bin[start..start + len]);
            } else {
                warn!("invalid gltf binary, replacing with zeros");
                new_bin.extend(std::iter::repeat_n(0, len));
            }
            view.byte_offset = Some(new_offset.into());
        }
    }

    // compress & append textures
    let mut failed_ktx2 = Vec::new();
    for (i, raw_opt) in raw_images.into_iter().enumerate() {
        if let Some(raw_pixels) = raw_opt.as_ref().filter(|raw| raw.starts_with(KTX2_MAGIC)) {
            // the loader can't read KTX2 at all, so these are transcoded whatever their size
            match transcode_ktx2(raw_pixels) {
                Ok(bc7_data) => {
                    root.images[i].mime_type =
                        Some(gltf_json::image::MimeType("image/vnd-ms.dds".into()));
                    append_image(&mut root, &mut new_bin, i, bc7_data);
                }
                // with the extension required there is no fallback to load instead
                Err(e) if basisu_required => return Err(e.into()),
                Err(e) => {
                    warn!("ktx2 image {i} can't be transcoded ({e:?}), keeping its fallback");
                    failed_ktx2.push(i);
                    // the original view was zombied above, so carry the bytes over
                    if root.images[i].buffer_view.is_some() {
                        append_image(&mut root, &mut new_bin, i, raw_pixels.clone());
                    }
                }
            }
        } else if let Some(raw_pixels) = raw_opt {
            // apply the same worth-compressing gate as engine::check_assets: bc7
            // saves nothing on tiny images, and a 1-pixel dimension would resize
            // to zero. tiny or undecodable images keep their original bytes
            // instead of failing the whole gltf
            let decoded = image::load_from_memory(&raw_pixels).ok().filter(|img| {
                img.width() > 2
                    && img.height() > 2
                    && img.width() as usize * img.height() as usize * 4 > 1024
            });

            let new_data = match decoded {
                Some(img) => {
                    let bc7_data = compress_decoded(img)?;
                    root.images[i].mime_type =
                        Some(gltf_json::image::MimeType("image/vnd-ms.dds".into()));
                    bc7_data
                }
                // the original view was zombied above, so carry the bytes over
                // unchanged (mime type stays as declared)
                None if root.images[i].buffer_view.is_some() => raw_pixels,
                // data-uri image: the untouched json still points at it
                None => continue,
            };

            append_image(&mut root, &mut new_bin, i, new_data);
        }
    }

    // textures whose KTX2 failed go back to their fallback source, with the extension
    let mut restored = false;
    for (index, source, basisu) in basisu_textures {
        let texture = &mut root.textures[index];
        if !failed_ktx2.contains(&texture.source.value()) {
            continue;
        }
        texture.source = source;
        if let Some(extensions) = texture.extensions.as_mut() {
            extensions
                .others
                .insert("KHR_texture_basisu".to_owned(), basisu);
        }
        restored = true;
    }
    if restored {
        root.extensions_used.push("KHR_texture_basisu".to_owned());
    }

    // Update Buffer Total Size
    if root.buffers.is_empty() {
        root.buffers.push(gltf_json::Buffer {
            byte_length: Default::default(),
            name: None,
            uri: None,
            extensions: None,
            extras: None,
        });
    }
    root.buffers[0].byte_length = new_bin.len().into();

    // Serialize
    let mut output = Vec::new();
    write_glb(&mut output, &root, &new_bin).unwrap();
    Ok(output)
}

/// Append `data` to the binary chunk as a new view and point image `i` at it.
fn append_image(root: &mut gltf_json::Root, new_bin: &mut Vec<u8>, i: usize, data: Vec<u8>) {
    while !new_bin.len().is_multiple_of(4) {
        new_bin.push(0);
    }
    let offset = new_bin.len() as u64;
    new_bin.extend_from_slice(&data);

    let new_view_idx = root.buffer_views.len();
    root.buffer_views.push(gltf_json::buffer::View {
        buffer: gltf_json::Index::new(0),
        byte_length: data.len().into(),
        byte_offset: Some(offset.into()),
        byte_stride: None,
        name: Some("BC7_Data".into()),
        target: None,
        extensions: None,
        extras: Default::default(),
    });

    root.images[i].buffer_view = Some(gltf_json::Index::new(new_view_idx as u32));
    root.images[i].uri = None;
}

// Helper that doesn't borrow 'root' mutably
fn extract_pixels_safe(
    img: &gltf_json::Image,
    views: &[gltf_json::buffer::View],
    bin: &[u8],
) -> Option<Vec<u8>> {
    if let Some(uri) = &img.uri {
        if uri.starts_with("data:") {
            let parts: Vec<&str> = uri.split(',').collect();
            return general_purpose::STANDARD.decode(parts[1]).ok();
        }
    }
    if let Some(idx) = img.buffer_view {
        let view = &views[idx.value()];
        let start = view.byte_offset.unwrap_or_default().0 as usize;
        let end = start + view.byte_length.0 as usize;
        if end <= bin.len() {
            return Some(bin[start..end].to_vec());
        }
    }
    None
}

pub fn write_glb<W: Write>(
    writer: &mut W,
    root: &gltf_json::Root,
    binary_payload: &[u8],
) -> std::io::Result<()> {
    // ai below
    // 1. Serialize JSON
    let json_string = gltf_json::serialize::to_string(root).expect("Serialization failed");
    let mut json_bytes = json_string.as_bytes().to_vec();

    // 2. Pad JSON (Must be multiple of 4, padded with spaces 0x20)
    while !json_bytes.len().is_multiple_of(4) {
        json_bytes.push(0x20);
    }

    // 3. Pad Binary (Must be multiple of 4, padded with zeros 0x00)
    // Note: We copy to a vector here to pad. For huge files,
    // you might want to write padding bytes directly to the writer instead.
    let mut bin_data = binary_payload.to_vec();
    while !bin_data.len().is_multiple_of(4) {
        bin_data.push(0x00);
    }

    // 4. Calculate Header
    let json_len = json_bytes.len() as u32;
    let bin_len = bin_data.len() as u32;
    let total_len = 12                   // Header
                  + 8 + json_len         // JSON Chunk (Length + Type + Data)
                  + 8 + bin_len; // BIN Chunk (Length + Type + Data)

    // 5. Write Header
    writer.write_all(b"glTF")?; // Magic
    writer.write_all(&2u32.to_le_bytes())?; // Version
    writer.write_all(&total_len.to_le_bytes())?; // Total Length

    // 6. Write JSON Chunk
    writer.write_all(&json_len.to_le_bytes())?;
    writer.write_all(b"JSON")?;
    writer.write_all(&json_bytes)?;

    // 7. Write Binary Chunk
    writer.write_all(&bin_len.to_le_bytes())?;
    writer.write_all(b"BIN\0")?;
    writer.write_all(&bin_data)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png_bytes(width: u32, height: u32) -> Vec<u8> {
        let img = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            width,
            height,
            image::Rgba([255, 255, 255, 255]),
        ));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    fn parse_glb(bytes: &[u8]) -> (gltf_json::Root, Vec<u8>) {
        let glb = Glb::from_slice(bytes).unwrap();
        (
            gltf_json::deserialize::from_slice(&glb.json).unwrap(),
            glb.bin.map(|b| b.into_owned()).unwrap_or_default(),
        )
    }

    fn view_bytes<'a>(root: &gltf_json::Root, bin: &'a [u8], image: usize) -> &'a [u8] {
        let view = &root.buffer_views[root.images[image].buffer_view.unwrap().value()];
        let start = view.byte_offset.unwrap_or_default().0 as usize;
        &bin[start..start + view.byte_length.0 as usize]
    }

    #[test]
    fn resize_never_rounds_a_dimension_to_zero() {
        assert!(process_image(&png_bytes(1, 1)).is_ok());
        assert!(process_image(&png_bytes(2048, 2)).is_ok());
    }

    #[test]
    fn tiny_data_uri_image_passes_through_untouched() {
        let json = format!(
            r#"{{"asset":{{"version":"2.0"}},"images":[
                {{"mimeType":"image/png","uri":"data:image/png;base64,{}"}},
                {{"mimeType":"image/png","uri":"data:image/png;base64,{}"}}]}}"#,
            general_purpose::STANDARD.encode(png_bytes(1, 1)),
            general_purpose::STANDARD.encode(png_bytes(32, 32)),
        );

        let output = process_gltf(json.as_bytes(), false).unwrap();
        let (root, bin) = parse_glb(&output);

        // tiny image untouched
        assert!(root.images[0].uri.as_ref().unwrap().starts_with("data:"));
        assert!(root.images[0].buffer_view.is_none());
        // large image compressed
        assert_eq!(
            root.images[1].mime_type.as_ref().unwrap().0,
            "image/vnd-ms.dds"
        );
        assert!(view_bytes(&root, &bin, 1).starts_with(b"DDS "));
    }

    #[test]
    fn tiny_buffer_view_image_keeps_original_bytes() {
        let png = png_bytes(1, 1);
        let mut root = gltf_json::Root::default();
        root.buffers.push(gltf_json::Buffer {
            byte_length: png.len().into(),
            name: None,
            uri: None,
            extensions: None,
            extras: Default::default(),
        });
        root.buffer_views.push(gltf_json::buffer::View {
            buffer: gltf_json::Index::new(0),
            byte_length: png.len().into(),
            byte_offset: None,
            byte_stride: None,
            name: None,
            target: None,
            extensions: None,
            extras: Default::default(),
        });
        root.images.push(gltf_json::Image {
            buffer_view: Some(gltf_json::Index::new(0)),
            mime_type: Some(gltf_json::image::MimeType("image/png".into())),
            name: None,
            uri: None,
            extensions: None,
            extras: Default::default(),
        });
        let mut input = Vec::new();
        write_glb(&mut input, &root, &png).unwrap();

        let output = process_gltf(&input, false).unwrap();
        let (root, bin) = parse_glb(&output);

        assert_eq!(root.images[0].mime_type.as_ref().unwrap().0, "image/png");
        assert_eq!(view_bytes(&root, &bin, 0), png);
    }

    // 8x8 and 6x6 UASTC gradients, zstd-supercompressed (`basisu -uastc -ktx2`)
    const UASTC_8: &[u8] = include_bytes!("../../testdata/uastc_8.ktx2");
    const UASTC_6: &[u8] = include_bytes!("../../testdata/uastc_6.ktx2");

    fn dds_size(bytes: &[u8]) -> (u32, u32) {
        let dds = Dds::read(&mut std::io::Cursor::new(bytes)).unwrap();
        assert_eq!(dds.get_dxgi_format(), Some(DxgiFormat::BC7_UNorm));
        (dds.get_width(), dds.get_height())
    }

    #[test]
    fn uastc_transcodes_to_bc7_in_whole_blocks() {
        assert_eq!(dds_size(&transcode_ktx2(UASTC_8).unwrap()), (8, 8));
        // not whole blocks: decoded and encoded like a png of that size
        assert_eq!(dds_size(&transcode_ktx2(UASTC_6).unwrap()), (8, 8));
    }

    #[test]
    fn basisu_gltf_is_repaired_to_a_dds_texture() {
        let json = format!(
            r#"{{"asset":{{"version":"2.0"}},
                "extensionsUsed":["KHR_texture_basisu"],
                "extensionsRequired":["KHR_texture_basisu"],
                "images":[{{"mimeType":"image/ktx2","uri":"data:image/ktx2;base64,{}"}}],
                "textures":[{{"extensions":{{"KHR_texture_basisu":{{"source":0}}}}}}]}}"#,
            general_purpose::STANDARD.encode(UASTC_8),
        );

        let output = process_gltf(json.as_bytes(), true).unwrap();
        let (root, bin) = parse_glb(&output);

        assert!(root.extensions_used.is_empty());
        assert!(root.extensions_required.is_empty());
        assert_eq!(root.textures[0].source.value(), 0);
        assert_eq!(
            root.images[0].mime_type.as_ref().unwrap().0,
            "image/vnd-ms.dds"
        );
        assert_eq!(dds_size(view_bytes(&root, &bin, 0)), (8, 8));
        // the loader accepts it now
        assert!(gltf::Gltf::from_slice(&output).is_ok());
    }

    #[test]
    fn ktx2_with_an_oversized_or_inconsistent_header_is_rejected() {
        let patched = |at: usize, value: &[u8]| {
            let mut bytes = UASTC_8.to_vec();
            bytes[at..at + value.len()].copy_from_slice(value);
            bytes
        };
        // 40000x40000: the rgba fallback alone would be 6.4 GB
        let huge = [&40_000u32.to_le_bytes()[..], &40_000u32.to_le_bytes()].concat();
        assert!(transcode_ktx2(&patched(20, &huge)).is_err());
        // level 0 reaching past the end of the file
        assert!(transcode_ktx2(&patched(88, &u64::MAX.to_le_bytes())).is_err());
        // level 0 declaring more bytes than an 8x8 image's four blocks hold
        assert!(transcode_ktx2(&patched(96, &(1u64 << 40).to_le_bytes())).is_err());
        // more levels than an 8x8 mip chain has
        assert!(transcode_ktx2(&patched(40, &5u32.to_le_bytes())).is_err());
        assert!(transcode_ktx2(&UASTC_8[..UASTC_8.len() / 2]).is_err());
        // BasisLZ (ETC1S), and a declared GPU format such as BC7
        assert!(transcode_ktx2(&patched(44, &1u32.to_le_bytes())).is_err());
        assert!(transcode_ktx2(&patched(12, &145u32.to_le_bytes())).is_err());
    }

    #[test]
    fn basisu_texture_with_a_bad_ktx2_keeps_its_fallback() {
        let bad_ktx2 = [KTX2_MAGIC, &[0; 100]].concat();
        let json = |required: &str| {
            format!(
                r#"{{"asset":{{"version":"2.0"}},
                    "extensionsUsed":["KHR_texture_basisu"],
                    "extensionsRequired":[{required}],
                    "images":[{{"mimeType":"image/png","uri":"data:image/png;base64,{}"}},
                              {{"mimeType":"image/ktx2","uri":"data:image/ktx2;base64,{}"}}],
                    "textures":[{{"source":0,"extensions":{{"KHR_texture_basisu":{{"source":1}}}}}}]}}"#,
                general_purpose::STANDARD.encode(png_bytes(2, 2)),
                general_purpose::STANDARD.encode(&bad_ktx2),
            )
        };

        let output = process_gltf(json("").as_bytes(), false).unwrap();
        let (root, _) = parse_glb(&output);
        assert_eq!(root.textures[0].source.value(), 0);
        let extensions = root.textures[0].extensions.as_ref().unwrap();
        assert!(extensions.others.contains_key("KHR_texture_basisu"));
        assert_eq!(root.extensions_used, ["KHR_texture_basisu"]);
        assert!(gltf::Gltf::from_slice(&output).is_ok());

        // required: nothing to fall back to
        let required = json(r#""KHR_texture_basisu""#);
        assert!(process_gltf(required.as_bytes(), true).is_err());
    }

    #[test]
    fn failed_gltf_without_draco_or_basisu_is_not_repaired() {
        let json = r#"{"asset":{"version":"2.0"}}"#;
        assert!(matches!(
            process_gltf(json.as_bytes(), true),
            Err(GltfProcessError::NothingToRepair)
        ));
    }
}
