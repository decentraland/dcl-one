use anyhow::{Result, ensure};

/// Pixels a generation may announce: 4096 x 4096, DCI 4K and a little more.
pub const MAX_PIXELS: u64 = 4096 * 4096;

/// Largest edge a generation may announce.
pub const MAX_DIMENSION: u32 = 16_384;

/// Refuses an edge outside `1..=MAX_DIMENSION`, or more than `MAX_PIXELS` pixels.
pub fn check_announced_size(width: u32, height: u32) -> Result<()> {
    ensure!(
        (1..=MAX_DIMENSION).contains(&width) && (1..=MAX_DIMENSION).contains(&height),
        "announced an unpresentable size {width}x{height}"
    );
    ensure!(
        u64::from(width).saturating_mul(u64::from(height)) <= MAX_PIXELS,
        "announced {width}x{height}, past the {MAX_PIXELS} pixel cap"
    );
    Ok(())
}