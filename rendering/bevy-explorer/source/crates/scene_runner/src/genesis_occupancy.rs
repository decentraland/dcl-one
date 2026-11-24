//! Genesis City's occupancy as Decentraland's places pipeline publishes it (`WorldManifest.json`,
//! the file unity-explorer's `WorldManifestProvider` downloads), embedded as a snapshot. The
//! landscape reads it until the realm resolves a parcel's pointer, so the terrain and the trees
//! of the whole city are known from the start, as in Unity, instead of filling in as the pointers
//! around the player resolve. The published file cannot be fetched from the web build: its bucket
//! answers CORS for one origin only.
//!
//! A snapshot is `OCC1`, then min x, min y, width and height as little-endian i32, then one byte
//! per parcel, rows from min y upwards with x ascending: 0 unlisted, 1 empty, 2 occupied (roads
//! are occupied). `crates/assets/landscape_tools/genesis_occupancy.py` writes them.

use bevy::math::IVec2;
use common::{base_domain, terrain::Occupancy};

static ORG: &[u8] = include_bytes!("genesis_occupancy/org.occupancy");
static ZONE: &[u8] = include_bytes!("genesis_occupancy/zone.occupancy");
const MAGIC: &[u8; 4] = b"OCC1";
const EMPTY: u8 = 1;
const OCCUPIED: u8 = 2;

#[derive(Clone, Copy, Debug)]
pub struct GenesisOccupancy {
    min: IVec2,
    size: IVec2,
    cells: &'static [u8],
}

impl GenesisOccupancy {
    /// None for anything that is not a whole snapshot.
    pub fn parse(bytes: &'static [u8]) -> Option<Self> {
        let (magic, mut rest) = bytes.split_first_chunk::<4>()?;
        if magic != MAGIC {
            return None;
        }
        let mut header = [0i32; 4];
        for value in &mut header {
            let (chunk, tail) = rest.split_first_chunk::<4>()?;
            *value = i32::from_le_bytes(*chunk);
            rest = tail;
        }
        let [x, y, width, height] = header;
        if width <= 0 || height <= 0 || rest.len() != width as usize * height as usize {
            return None;
        }
        Some(Self {
            min: IVec2::new(x, y),
            size: IVec2::new(width, height),
            cells: rest,
        })
    }

    /// The snapshot of the city a realm at `about_url` serves: decentraland.zone's for a realm
    /// there, decentraland.org's for any other (a dcl-one node mirrors .org's city).
    pub fn for_realm(about_url: &str) -> Option<Self> {
        match base_domain::decentraland_domain(about_url) {
            Some("decentraland.zone") => Self::parse(ZONE),
            _ => Self::parse(ORG),
        }
    }

    /// The parcel's occupancy, None when the snapshot does not list it.
    pub fn get(&self, parcel: IVec2) -> Option<Occupancy> {
        let offset = parcel - self.min;
        if offset.cmplt(IVec2::ZERO).any() || offset.cmpge(self.size).any() {
            return None;
        }
        match self.cells[(offset.y * self.size.x + offset.x) as usize] {
            EMPTY => Some(Occupancy::Empty),
            OCCUPIED => Some(Occupancy::Occupied),
            _ => None,
        }
    }

    /// Every parcel the snapshot lists as occupied.
    pub fn occupied(&self) -> impl Iterator<Item = IVec2> + '_ {
        let (min, width) = (self.min, self.size.x);
        self.cells
            .iter()
            .enumerate()
            .filter(|(_, cell)| **cell == OCCUPIED)
            .map(move |(index, _)| {
                let index = index as i32;
                min + IVec2::new(index % width, index / width)
            })
    }
}

#[cfg(test)]
mod tests;
