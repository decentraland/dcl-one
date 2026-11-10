//! A BC texture variant arrives as KTX2 (zstd-supercompressed levels) and is handed to the image
//! loaders as DDS: they detect DDS by its magic whatever the path's extension, and a DDS in
//! UNORM takes sRGB or linear from how the texture is requested. Every size read from the file
//! is checked against what the header implies before anything is allocated or decoded.

/// The BC payloads a texture variant may carry; the manifest's `format` names one through
/// `super::SUPPORTED_FORMATS`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BcFormat {
    Bc1,
    Bc5,
    Bc7,
}

impl BcFormat {
    /// The UNORM `VkFormat`s the format's KTX2 may declare (BC1 with or without alpha).
    fn vk_formats(self) -> &'static [u32] {
        match self {
            Self::Bc1 => &[131, 133],
            Self::Bc5 => &[141],
            Self::Bc7 => &[145],
        }
    }

    fn block_bytes(self) -> u64 {
        match self {
            Self::Bc1 => 8,
            Self::Bc5 | Self::Bc7 => 16,
        }
    }

    fn dxgi_format(self) -> u32 {
        match self {
            Self::Bc1 => 71, // DXGI_FORMAT_BC1_UNORM
            Self::Bc5 => 83, // DXGI_FORMAT_BC5_UNORM
            Self::Bc7 => 98, // DXGI_FORMAT_BC7_UNORM
        }
    }

    fn level_bytes(self, width: u32, height: u32) -> u64 {
        u64::from(width.div_ceil(4)) * u64::from(height.div_ceil(4)) * self.block_bytes()
    }
}

const KTX2_IDENTIFIER: [u8; 12] = [
    0xAB, 0x4B, 0x54, 0x58, 0x20, 0x32, 0x30, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
];
/// Identifier, header and index; the level index follows.
const LEVEL_INDEX: usize = 80;
const LEVEL_ENTRY: usize = 24;
const MAX_DIMENSION: u32 = 8192;
const SUPERCOMPRESSION_ZSTD: u32 = 2;
pub const DDS_HEADER_BYTES: usize = 4 + 124 + 20;

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

/// The BC format a KTX2 holds, from its header's vkFormat (UNORM only).
pub fn bc_format(bytes: &[u8]) -> Result<BcFormat, String> {
    if bytes.len() < LEVEL_INDEX || bytes[..12] != KTX2_IDENTIFIER {
        return Err("not a KTX2 file".to_owned());
    }
    let vk_format = u32_at(bytes, 12);
    [BcFormat::Bc7, BcFormat::Bc1, BcFormat::Bc5]
        .into_iter()
        .find(|format| format.vk_formats().contains(&vk_format))
        .ok_or_else(|| format!("vkFormat {vk_format} is not BC1, BC5 or BC7 UNORM"))
}

/// The DDS (DX10 header, all levels largest first) a BC KTX2 of `format` holds.
pub fn bc_ktx2_to_dds(bytes: &[u8], format: BcFormat) -> Result<Vec<u8>, String> {
    if bytes.len() < LEVEL_INDEX || bytes[..12] != KTX2_IDENTIFIER {
        return Err("not a KTX2 file".to_owned());
    }
    let header = |field: usize| u32_at(bytes, 12 + 4 * field);
    let (vk_format, type_size, width, height) = (header(0), header(1), header(2), header(3));
    let (depth, layers, faces, levels, scheme) =
        (header(4), header(5), header(6), header(7), header(8));
    if !format.vk_formats().contains(&vk_format) {
        return Err(format!("vkFormat {vk_format} is not {format:?} UNORM"));
    }
    if type_size != 1 {
        return Err(format!("typeSize {type_size} for a block format"));
    }
    if width == 0 || height == 0 || width > MAX_DIMENSION || height > MAX_DIMENSION {
        return Err(format!("{width}x{height} is outside 1..={MAX_DIMENSION}"));
    }
    // wgpu refuses a BC texture whose base size is not whole 4x4 blocks, and on native that
    // error is fatal, so such a file falls back to the original here; lower mips (2x1, 1x1)
    // stay legal
    if width % 4 != 0 || height % 4 != 0 {
        return Err(format!("{width}x{height} is not whole 4x4 blocks"));
    }
    if depth != 0 || layers != 0 || faces != 1 {
        return Err("not a single 2D texture".to_owned());
    }
    let chain = 32 - width.max(height).leading_zeros();
    if levels == 0 || levels > chain {
        return Err(format!("{levels} levels for a {chain}-level mip chain"));
    }
    if scheme != SUPERCOMPRESSION_ZSTD {
        return Err(format!("supercompression scheme {scheme}, not zstd"));
    }
    if bytes.len() < LEVEL_INDEX + levels as usize * LEVEL_ENTRY {
        return Err("truncated level index".to_owned());
    }

    let mut dds = dds_header(width, height, levels, format);
    for level in 0..levels {
        let entry = LEVEL_INDEX + level as usize * LEVEL_ENTRY;
        let (offset, length, uncompressed) = (
            u64_at(bytes, entry),
            u64_at(bytes, entry + 8),
            u64_at(bytes, entry + 16),
        );
        let expected = format.level_bytes((width >> level).max(1), (height >> level).max(1));
        if uncompressed != expected {
            return Err(format!(
                "level {level} declares {uncompressed} bytes, its size is {expected}"
            ));
        }
        let data = offset
            .checked_add(length)
            .filter(|end| *end <= bytes.len() as u64)
            .map(|end| &bytes[offset as usize..end as usize])
            .ok_or_else(|| format!("level {level} lies outside the file"))?;
        let inflated = zstd_inflate(data, expected as usize)
            .ok_or_else(|| format!("level {level} does not inflate to {expected} bytes"))?;
        dds.extend_from_slice(&inflated);
    }
    Ok(dds)
}

/// One zstd frame to exactly `want` bytes, as the vendored basisu inflates KTX2 levels.
fn zstd_inflate(compressed: &[u8], want: usize) -> Option<Vec<u8>> {
    use ruzstd::io::Read;
    let max_window = (want as u64).max(8 << 20);
    let mut decoder =
        ruzstd::decoding::StreamingDecoder::new_with_max_window_size(compressed, max_window)
            .ok()?;
    let mut out = Vec::new();
    out.try_reserve_exact(want).ok()?;
    out.resize(want, 0);
    decoder.read_exact(&mut out).ok()?;
    let mut extra = [0u8; 1];
    if decoder.read(&mut extra).ok()? != 0 {
        return None;
    }
    Some(out)
}

fn dds_header(width: u32, height: u32, levels: u32, format: BcFormat) -> Vec<u8> {
    const CAPS: u32 = 0x1;
    const HEIGHT: u32 = 0x2;
    const WIDTH: u32 = 0x4;
    const PIXELFORMAT: u32 = 0x1000;
    const MIPMAPCOUNT: u32 = 0x20000;
    const LINEARSIZE: u32 = 0x80000;
    const FOURCC: u32 = 0x4;
    const CAPS_COMPLEX: u32 = 0x8;
    const CAPS_TEXTURE: u32 = 0x1000;
    const CAPS_MIPMAP: u32 = 0x40_0000;
    const DIMENSION_TEXTURE2D: u32 = 3;

    let mut header = Vec::with_capacity(DDS_HEADER_BYTES);
    let mut put = |value: u32| header.extend_from_slice(&value.to_le_bytes());
    put(u32::from_le_bytes(*b"DDS "));
    put(124);
    put(CAPS | HEIGHT | WIDTH | PIXELFORMAT | MIPMAPCOUNT | LINEARSIZE);
    put(height);
    put(width);
    // at most 8192²/16 blocks of 16 bytes: 64 MiB
    put(format.level_bytes(width, height) as u32);
    put(0); // depth
    put(levels);
    (0..11).for_each(|_| put(0));
    // pixel format: the format is in the DX10 header
    put(32);
    put(FOURCC);
    put(u32::from_le_bytes(*b"DX10"));
    (0..5).for_each(|_| put(0));
    let mipmapped = if levels > 1 {
        CAPS_COMPLEX | CAPS_MIPMAP
    } else {
        0
    };
    put(CAPS_TEXTURE | mipmapped);
    (0..4).for_each(|_| put(0)); // caps2, caps3, caps4, reserved2
    put(format.dxgi_format());
    put(DIMENSION_TEXTURE2D);
    put(0); // misc flags
    put(1); // array size
    put(0); // alpha mode unknown
    header
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// One zstd frame holding `data` as a single raw block (single segment, 1-byte size).
    pub(crate) fn zstd_raw(data: &[u8]) -> Vec<u8> {
        assert!(data.len() < 256);
        let block = 1 | ((data.len() as u32) << 3);
        let mut frame = vec![0x28, 0xB5, 0x2F, 0xFD, 0x20, data.len() as u8];
        frame.extend_from_slice(&block.to_le_bytes()[..3]);
        frame.extend_from_slice(data);
        frame
    }

    /// A KTX2 with `levels` (largest first) stored as zstd frames, smallest last in the file.
    pub(crate) fn ktx2(vk_format: u32, width: u32, height: u32, levels: &[Vec<u8>]) -> Vec<u8> {
        let mut file = KTX2_IDENTIFIER.to_vec();
        for value in [vk_format, 1, width, height, 0, 0, 1, levels.len() as u32, 2] {
            file.extend_from_slice(&value.to_le_bytes());
        }
        // dfd, kvd (u32 offset/length), sgd (u64 offset/length): all empty
        file.extend_from_slice(&[0; 32]);
        let frames: Vec<_> = levels.iter().map(|level| zstd_raw(level)).collect();
        let mut offset = (LEVEL_INDEX + levels.len() * LEVEL_ENTRY) as u64;
        for (level, frame) in levels.iter().zip(&frames) {
            file.extend_from_slice(&offset.to_le_bytes());
            file.extend_from_slice(&(frame.len() as u64).to_le_bytes());
            file.extend_from_slice(&(level.len() as u64).to_le_bytes());
            offset += frame.len() as u64;
        }
        for frame in &frames {
            file.extend_from_slice(frame);
        }
        file
    }

    /// BC7, 8x4: level 0 is 2x1 blocks, level 1 (4x2) one block.
    pub(crate) fn bc7_two_levels() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let level0: Vec<u8> = (0..32).collect();
        let level1: Vec<u8> = (100..116).collect();
        (
            ktx2(145, 8, 4, &[level0.clone(), level1.clone()]),
            level0,
            level1,
        )
    }

    #[test]
    fn a_bc7_ktx2_becomes_a_dx10_dds_with_its_levels_in_order() {
        let (file, level0, level1) = bc7_two_levels();
        let dds = bc_ktx2_to_dds(&file, BcFormat::Bc7).unwrap();
        let mut expected = Vec::new();
        for value in [
            u32::from_le_bytes(*b"DDS "),
            124,
            0xA1007,
            4,  // height
            8,  // width
            32, // level 0 bytes
            0,
            2, // levels
        ] {
            expected.extend_from_slice(&value.to_le_bytes());
        }
        expected.extend_from_slice(&[0; 44]);
        for value in [32, 4, u32::from_le_bytes(*b"DX10"), 0, 0, 0, 0, 0] {
            expected.extend_from_slice(&value.to_le_bytes());
        }
        for value in [0x40_1008u32, 0, 0, 0, 0, 98, 3, 0, 1, 0] {
            expected.extend_from_slice(&value.to_le_bytes());
        }
        assert_eq!(expected.len(), DDS_HEADER_BYTES);
        expected.extend_from_slice(&level0);
        expected.extend_from_slice(&level1);
        assert_eq!(dds, expected);
    }

    #[test]
    fn the_bc_format_is_read_from_the_header() {
        let (bc7, _, _) = bc7_two_levels();
        assert_eq!(bc_format(&bc7), Ok(BcFormat::Bc7));
        assert_eq!(
            bc_format(&ktx2(131, 4, 4, &[vec![0; 8]])),
            Ok(BcFormat::Bc1)
        );
        assert_eq!(
            bc_format(&ktx2(133, 4, 4, &[vec![0; 8]])),
            Ok(BcFormat::Bc1)
        );
        assert_eq!(
            bc_format(&ktx2(141, 4, 4, &[vec![0; 16]])),
            Ok(BcFormat::Bc5)
        );
        // sRGB BC7, and anything that is not a KTX2
        assert!(bc_format(&ktx2(146, 4, 4, &[vec![0; 16]]))
            .unwrap_err()
            .contains("vkFormat 146"));
        assert!(bc_format(b"not a ktx2").is_err());
    }

    #[test]
    fn bc1_and_bc5_take_their_own_block_size_and_dxgi_format() {
        let bc1 = ktx2(131, 4, 4, &[vec![7; 8]]);
        let dds = bc_ktx2_to_dds(&bc1, BcFormat::Bc1).unwrap();
        assert_eq!(u32_at(&dds, 128), 71);
        assert_eq!(&dds[DDS_HEADER_BYTES..], &[7; 8]);
        let bc5 = ktx2(141, 4, 4, &[vec![9; 16]]);
        assert_eq!(
            u32_at(&bc_ktx2_to_dds(&bc5, BcFormat::Bc5).unwrap(), 128),
            83
        );
    }

    #[test]
    fn malformed_files_are_rejected() {
        let (file, level0, level1) = bc7_two_levels();
        // truncated inside the last level, and inside the level index
        assert!(bc_ktx2_to_dds(&file[..file.len() - 1], BcFormat::Bc7)
            .unwrap_err()
            .contains("outside the file"));
        assert!(bc_ktx2_to_dds(&file[..LEVEL_INDEX + 10], BcFormat::Bc7).is_err());
        assert!(bc_ktx2_to_dds(&file[..40], BcFormat::Bc7).is_err());
        // a vkFormat that is not the manifest's format, or not UNORM
        assert!(bc_ktx2_to_dds(&file, BcFormat::Bc1)
            .unwrap_err()
            .contains("vkFormat"));
        let srgb = ktx2(146, 8, 4, &[level0.clone(), level1.clone()]);
        assert!(bc_ktx2_to_dds(&srgb, BcFormat::Bc7)
            .unwrap_err()
            .contains("vkFormat"));
        // a level 0 that is not whole 4x4 blocks, on either axis
        for (width, height) in [(6, 4), (8, 6), (3, 3)] {
            let unaligned = ktx2(145, width, height, std::slice::from_ref(&level0));
            assert!(bc_ktx2_to_dds(&unaligned, BcFormat::Bc7)
                .unwrap_err()
                .contains("whole 4x4 blocks"));
        }
        // oversized, and more levels than the mip chain has
        let huge = ktx2(145, 16384, 4, std::slice::from_ref(&level0));
        assert!(bc_ktx2_to_dds(&huge, BcFormat::Bc7)
            .unwrap_err()
            .contains("outside"));
        let deep = ktx2(
            145,
            4,
            4,
            &[vec![0; 16], vec![0; 16], vec![0; 16], vec![0; 16]],
        );
        assert!(bc_ktx2_to_dds(&deep, BcFormat::Bc7)
            .unwrap_err()
            .contains("mip chain"));
        // a level whose declared or inflated size is not the block bytes its size needs
        let short = ktx2(145, 8, 4, &[level0[..16].to_vec(), level1.clone()]);
        assert!(bc_ktx2_to_dds(&short, BcFormat::Bc7)
            .unwrap_err()
            .contains("declares"));
        // the index declares the right size, but the frame inflates to one byte less
        let mut lying = ktx2(145, 8, 4, &[vec![0; 31], level1.clone()]);
        lying[LEVEL_INDEX + 16] = 32;
        assert!(bc_ktx2_to_dds(&lying, BcFormat::Bc7)
            .unwrap_err()
            .contains("does not inflate"));
        // not supercompressed
        let mut plain = file.clone();
        plain[12 + 4 * 8] = 0;
        assert!(bc_ktx2_to_dds(&plain, BcFormat::Bc7)
            .unwrap_err()
            .contains("scheme"));
        assert!(bc_ktx2_to_dds(&[0x44; 100], BcFormat::Bc7).is_err());
    }
}
