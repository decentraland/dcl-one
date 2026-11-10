use super::*;

#[derive(Asset, TypePath)]
struct Item;

fn turn() -> ReclaimTurn {
    ReclaimTurn::bounded(10, 64)
}

#[test]
fn touches_share_one_pin_and_one_recency_record_per_content_hash() {
    let mut assets = Assets::<Item>::default();
    let mut cache = IdleCache::default();
    let first = cache
        .get_or_insert_with("hash", 1, || assets.add(Item))
        .clone();
    for frame in 2..100 {
        assert_eq!(
            cache
                .get_or_insert_with("hash", frame, || panic!("duplicate load"))
                .id(),
            first.id()
        );
    }
    assert_eq!(cache.len(), 1);
    assert_eq!(cache.order.len(), 1);
    cache.reclaim(102, &mut turn(), |_| false);
    assert_eq!(cache.len(), 1);
    cache.reclaim(103, &mut turn(), |_| false);
    assert_eq!(cache.len(), 0);
}

#[test]
fn pending_entries_do_not_starve_later_entries_and_failed_loads_can_leave() {
    let mut assets = Assets::<Item>::default();
    let mut cache = IdleCache::default();
    let pending = cache
        .get_or_insert_with("pending", 0, || assets.add(Item))
        .id();
    for n in 0..20 {
        cache.get_or_insert_with(&n.to_string(), 0, || assets.add(Item));
    }
    let mut first = ReclaimTurn::bounded(10, 4);
    cache.reclaim(4, &mut first, |id| id == pending);
    assert_eq!(cache.len(), 18);
    for _ in 0..5 {
        cache.reclaim(4, &mut ReclaimTurn::bounded(10, 4), |id| id == pending);
    }
    assert_eq!(cache.len(), 1);
    cache.reclaim(4, &mut turn(), |_| false);
    assert_eq!(cache.len(), 0);
}

#[test]
fn recency_storage_is_bounded_and_frame_wrap_keeps_the_grace() {
    let mut assets = Assets::<Item>::default();
    let mut cache = IdleCache::default();
    cache.get_or_insert_with("hash", u32::MAX - 1, || assets.add(Item));
    cache.reclaim(1, &mut turn(), |_| false);
    assert_eq!(cache.len(), 1);
    cache.reclaim(2, &mut turn(), |_| false);
    assert_eq!(cache.len(), 0);
    assert!(cache.order.is_empty());
}

#[test]
fn releasing_a_pin_preserves_an_independently_created_strong_owner() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()))
        .init_asset::<Item>();
    app.finish();
    app.cleanup();
    let mut cache = IdleCache::default();
    let handle = cache
        .get_or_insert_with("hash", 0, || {
            app.world_mut().resource_mut::<Assets<Item>>().add(Item)
        })
        .clone();
    let id = handle.id();
    let owner = app
        .world_mut()
        .resource_mut::<Assets<Item>>()
        .get_strong_handle(id)
        .unwrap();
    drop(handle);
    cache.reclaim(4, &mut turn(), |_| false);
    for _ in 0..4 {
        app.update();
    }
    assert!(app.world().resource::<Assets<Item>>().contains(id));
    drop(owner);
    for _ in 0..4 {
        app.update();
    }
    assert!(!app.world().resource::<Assets<Item>>().contains(id));
}

#[test]
fn hot_touches_cannot_strand_a_completed_idle_load_behind_the_scan_cursor() {
    let mut assets = Assets::<Item>::default();
    let mut cache = IdleCache::default();
    let pending = cache
        .get_or_insert_with("pending", 0, || assets.add(Item))
        .id();
    for n in 0..128 {
        cache.get_or_insert_with(&n.to_string(), 4, || assets.add(Item));
    }
    cache.reclaim(4, &mut turn(), |id| id == pending);
    // The first entry finishes loading and loses its requester. More than a
    // turn's worth of hot pins continue moving to the back on every frame.
    for frame in 5..15 {
        for n in 0..128 {
            cache.get_or_insert_with(&n.to_string(), frame, || {
                panic!("hot pin unexpectedly expired")
            });
        }
        cache.reclaim(frame, &mut turn(), |_| false);
    }
    assert!(!cache.entries.contains_key("pending"));
    assert_eq!(cache.len(), 128);
    assert_eq!(cache.order.len(), 128);
}

#[derive(Resource, Default)]
struct ScanFixture {
    cache: IdleCache<Item>,
    pending: bool,
}

fn scan_fixture(
    mut fixture: ResMut<ScanFixture>,
    mut budget: ResMut<common::memory_reclaim::ReclaimBudget>,
) {
    if let Some(mut turn) = budget.begin("fixture", fixture.cache.len()) {
        let pending = fixture.pending;
        fixture.cache.reclaim(10, &mut turn, |_| pending);
        budget.finish_turn(turn, fixture.cache.len());
    }
}

#[test]
fn completed_scan_outcomes_include_releases_from_previous_bounded_turns() {
    use common::memory_reclaim::{
        AddMemoryReclaimer, MemoryReclaimPlugin, MemoryReclaimSettings, ReclaimBudget,
        ReclaimDomains, ReclaimScanStatus, ReclaimSimulation, ReclaimSpec,
    };
    let mut assets = Assets::<Item>::default();
    let mut fixture = ScanFixture {
        pending: true,
        ..Default::default()
    };
    for n in 0..5 {
        fixture
            .cache
            .get_or_insert_with(&n.to_string(), 0, || assets.add(Item));
    }
    let mut app = App::new();
    app.insert_resource(fixture)
        .add_plugins(MemoryReclaimPlugin)
        .add_memory_reclaimer(
            ReclaimSpec {
                name: "fixture",
                priority: 0,
                domains: ReclaimDomains::Ram,
                max_releases: 1,
                max_inspections: 2,
            },
            scan_fixture,
        );
    app.world_mut()
        .resource_mut::<MemoryReclaimSettings>()
        .simulation = ReclaimSimulation::Ram;
    app.world_mut()
        .resource_mut::<MemoryReclaimSettings>()
        .frame_budget = std::time::Duration::from_secs(10);
    app.finish();
    app.cleanup();
    for _ in 0..2 {
        app.update();
        let stats = &app.world().resource::<ReclaimBudget>().caches()[0];
        assert_eq!(stats.scan_status, ReclaimScanStatus::InProgress);
        assert_eq!(stats.completed_scans, 0);
        assert_eq!(stats.last_scan_released, None);
    }
    app.update();
    let stats = &app.world().resource::<ReclaimBudget>().caches()[0];
    assert_eq!(stats.scan_status, ReclaimScanStatus::NoEligible);
    assert_eq!(stats.completed_scans, 1);
    assert_eq!(stats.last_scan_released, Some(0));
    app.world_mut().resource_mut::<ScanFixture>().pending = false;
    for _ in 0..5 {
        app.update();
        assert_eq!(
            app.world().resource::<ReclaimBudget>().caches()[0].scan_status,
            ReclaimScanStatus::InProgress
        );
    }
    // The final turn releases nothing itself, but completes a pass that released five pins.
    app.update();
    let stats = &app.world().resource::<ReclaimBudget>().caches()[0];
    assert_eq!(stats.last_released, 0);
    assert_eq!(stats.scan_status, ReclaimScanStatus::Released);
    assert_eq!(stats.last_scan_released, Some(5));
    assert_eq!(stats.completed_scans, 2);
    app.update();
    let stats = &app.world().resource::<ReclaimBudget>().caches()[0];
    assert_eq!(stats.scan_status, ReclaimScanStatus::NoEligible);
    assert_eq!(stats.completed_scans, 3);
    app.world_mut()
        .resource_mut::<MemoryReclaimSettings>()
        .simulation = ReclaimSimulation::Off;
    app.update();
    let stats = &app.world().resource::<ReclaimBudget>().caches()[0];
    assert_eq!(stats.scan_status, ReclaimScanStatus::NotRun);
    assert_eq!(stats.completed_scans, 3);
}
