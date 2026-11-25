use super::*;

/// A 3x2 snapshot from (-1, 5): rows y = 5 then y = 6.
fn snapshot(cells: [u8; 6]) -> &'static [u8] {
    GenesisOccupancy::encode(IVec2::new(-1, 5), IVec2::new(3, 2), &cells)
}

#[test]
fn reads_each_parcel_from_its_row() {
    let occupancy = GenesisOccupancy::parse(snapshot([1, 2, 0, 2, 1, 1])).unwrap();
    assert_eq!(occupancy.get(IVec2::new(-1, 5)), Some(Occupancy::Empty));
    assert_eq!(occupancy.get(IVec2::new(0, 5)), Some(Occupancy::Occupied));
    assert_eq!(occupancy.get(IVec2::new(1, 5)), None);
    assert_eq!(occupancy.get(IVec2::new(-1, 6)), Some(Occupancy::Occupied));
    assert_eq!(occupancy.get(IVec2::new(1, 6)), Some(Occupancy::Empty));
}

#[test]
fn parcels_outside_the_snapshot_are_unlisted() {
    let occupancy = GenesisOccupancy::parse(snapshot([2; 6])).unwrap();
    for parcel in [
        IVec2::new(-2, 5),
        IVec2::new(2, 5),
        IVec2::new(0, 4),
        IVec2::new(0, 7),
    ] {
        assert_eq!(occupancy.get(parcel), None, "{parcel}");
    }
}

#[test]
fn lists_the_occupied_parcels() {
    let occupancy = GenesisOccupancy::parse(snapshot([1, 2, 0, 2, 1, 1])).unwrap();
    let occupied: Vec<_> = occupancy.occupied().collect();
    assert_eq!(occupied, [IVec2::new(0, 5), IVec2::new(-1, 6)]);
}

#[test]
fn rejects_what_is_not_a_whole_snapshot() {
    let whole = snapshot([0; 6]);
    // six parcels take two bytes
    assert_eq!(whole.len(), 4 + 16 + 2);
    assert!(GenesisOccupancy::parse(&whole[..whole.len() - 1]).is_none());
    assert!(GenesisOccupancy::parse(&whole[..10]).is_none());
    let mut wrong_magic = whole.to_vec();
    wrong_magic[0] = b'X';
    assert!(GenesisOccupancy::parse(wrong_magic.leak()).is_none());
}

#[test]
fn the_embedded_snapshots_cover_genesis_city() {
    for bytes in [ORG, ZONE] {
        let occupancy = GenesisOccupancy::parse(bytes).unwrap();
        // Genesis Plaza, and the city's south-west corner
        assert_eq!(occupancy.get(IVec2::ZERO), Some(Occupancy::Occupied));
        assert!(occupancy.get(IVec2::new(-150, -150)).is_some());
        assert!(occupancy.occupied().count() > 10_000);
    }
}

#[test]
fn a_realm_takes_its_environments_snapshot() {
    let cells = |occupancy: GenesisOccupancy| occupancy.cells.as_ptr();
    let org = GenesisOccupancy::parse(ORG).unwrap();
    let zone = GenesisOccupancy::parse(ZONE).unwrap();
    let realm = |url| GenesisOccupancy::for_realm(url).unwrap();
    assert_eq!(
        cells(realm("https://peer.decentraland.zone/about")),
        cells(zone)
    );
    assert_eq!(
        cells(realm(
            "https://realm-provider-ea.decentraland.org/main/about"
        )),
        cells(org)
    );
    assert_eq!(cells(realm("https://peer.example.com/about")), cells(org));
}
