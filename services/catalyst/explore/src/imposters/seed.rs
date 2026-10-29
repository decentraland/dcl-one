//! `catalyst bvimposters seed <realm-cache-dir>`: imports a renderer's
//! on-disk imposter cache (`{level}/{x},{y}-spec.json` with its `.boimp`
//! and `-floor.boimp`) into the store as the stored zips the explorer asks
//! for (`{level}/{x},{y}.{crc}.zip`, the spec's crc). A spec with crc 0 is
//! skipped (nothing baked there), a tile missing a member or with an
//! unreadable spec is counted incomplete, and a zip already in the store is
//! left alone.

use std::path::Path;

use anyhow::{Context, Result};

use crate::filecache::FileCache;
use crate::imposters::key::{ImposterKey, MAX_LEVEL};
use crate::imposters::zips;

#[derive(Default, Debug, PartialEq, Eq)]
pub struct SeedCounts {
    pub imported: u64,
    pub skipped: u64,
    pub crc0: u64,
    pub incomplete: u64,
}

pub fn seed_into(store: &FileCache, source: &Path) -> Result<SeedCounts> {
    let mut counts = SeedCounts::default();
    for level in 0..=MAX_LEVEL {
        let dir = source.join(level.to_string());
        let Ok(rd) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(coords) = name.strip_suffix("-spec.json") else {
                continue;
            };
            let Some((x, y)) = crate::auth::pointer::parse_pointer(coords) else {
                continue;
            };
            let (Ok(x), Ok(y)) = (i32::try_from(x), i32::try_from(y)) else {
                continue;
            };
            let Ok(spec_bytes) = std::fs::read(entry.path()) else {
                counts.incomplete += 1;
                continue;
            };
            let Ok(spec) = serde_json::from_slice::<serde_json::Value>(&spec_bytes) else {
                counts.incomplete += 1;
                continue;
            };
            let Some(crc) = spec.get("crc").and_then(|v| v.as_u64()) else {
                counts.incomplete += 1;
                continue;
            };
            if crc == 0 {
                counts.crc0 += 1;
                continue;
            }
            let Some(key) = u32::try_from(crc)
                .ok()
                .and_then(|crc| ImposterKey::new(level, x, y, crc))
            else {
                counts.incomplete += 1;
                continue;
            };
            let boimp_name = format!("{coords}.boimp");
            let floor_name = format!("{coords}-floor.boimp");
            let boimp_path = dir.join(&boimp_name);
            let floor_path = dir.join(&floor_name);
            if !boimp_path.is_file() || !floor_path.is_file() {
                counts.incomplete += 1;
                continue;
            }
            let Some(target) = store.path(&key.rel()) else {
                counts.incomplete += 1;
                continue;
            };
            if target.exists() {
                counts.skipped += 1;
                continue;
            }
            let members = [
                (name.clone(), spec_bytes),
                (
                    boimp_name,
                    std::fs::read(&boimp_path)
                        .with_context(|| format!("reading {}", boimp_path.display()))?,
                ),
                (
                    floor_name,
                    std::fs::read(&floor_path)
                        .with_context(|| format!("reading {}", floor_path.display()))?,
                ),
            ];
            let tmp = store
                .tmp_dir()
                .join(format!("seed-{}", uuid::Uuid::new_v4()));
            std::fs::write(&tmp, zips::stored_zip_bytes(&members)?)
                .with_context(|| format!("writing {}", tmp.display()))?;
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::rename(&tmp, &target)
                .with_context(|| format!("landing {}", target.display()))?;
            counts.imported += 1;
        }
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::filecache::tests::tempdir;
    use std::io::Read;

    fn write_triple(dir: &Path, x: i32, y: i32, crc: u64) {
        std::fs::create_dir_all(dir).unwrap();
        let spec = serde_json::json!({"imposters": {}, "crc": crc});
        std::fs::write(
            dir.join(format!("{x},{y}-spec.json")),
            serde_json::to_vec(&spec).unwrap(),
        )
        .unwrap();
        std::fs::write(dir.join(format!("{x},{y}.boimp")), vec![7u8; 128]).unwrap();
        std::fs::write(dir.join(format!("{x},{y}-floor.boimp")), vec![8u8; 64]).unwrap();
    }

    #[test]
    fn seeds_synthetic_corpus() {
        let dir = tempdir("imposters-seed");
        let corpus = dir.0.join("corpus");
        write_triple(&corpus.join("0"), 0, 100, 3504527830);
        write_triple(&corpus.join("2"), -64, -128, 42);
        write_triple(&corpus.join("1"), 4, 6, 0);
        let l0 = corpus.join("0");
        let spec = serde_json::json!({"imposters": {}, "crc": 9});
        std::fs::write(l0.join("5,5-spec.json"), serde_json::to_vec(&spec).unwrap()).unwrap();

        let store = FileCache::new(dir.0.join("root"), u64::MAX);
        store.init().unwrap();
        let counts = seed_into(&store, &corpus).unwrap();
        assert_eq!(
            counts,
            SeedCounts {
                imported: 2,
                skipped: 0,
                crc0: 1,
                incomplete: 1
            }
        );

        let target = store.store_dir().join("0").join("0,100.3504527830.zip");
        assert!(target.exists());
        assert!(store.store_dir().join("2").join("-64,-128.42.zip").exists());

        let bytes = std::fs::read(&target).unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(&bytes[..])).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().unwrap().to_string())
            .collect();
        assert_eq!(
            names,
            vec!["0,100-spec.json", "0,100.boimp", "0,100-floor.boimp"]
        );
        let mut member = archive.by_name("0,100.boimp").unwrap();
        assert_eq!(member.compression(), zip::CompressionMethod::Stored);
        let mut buf = Vec::new();
        member.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, vec![7u8; 128]);

        let again = seed_into(&store, &corpus).unwrap();
        assert_eq!(
            (again.imported, again.skipped, again.crc0, again.incomplete),
            (0, 2, 1, 1)
        );
    }
}
