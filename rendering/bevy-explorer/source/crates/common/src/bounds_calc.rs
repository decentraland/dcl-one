use bevy::{math::IVec2, platform::collections::HashSet};

#[derive(Debug)]
pub struct Region {
    pub min: IVec2,
    pub max: IVec2,
    pub count: u32,
}

const NEIGHBOURS: [IVec2; 4] = [IVec2::X, IVec2::Y, IVec2::NEG_X, IVec2::NEG_Y];

/// Splits a parcel set into its contiguous regions, each decomposed into rects. `count` on every
/// rect is the parcel count of the contiguous region it belongs to. Regions come out in the order
/// of their first parcel in `parcels`; duplicate parcels are ignored. Linear in the parcel count
/// (plus a sort per non-rectangular region).
pub fn scene_regions(parcels: impl Iterator<Item = IVec2>) -> Vec<Region> {
    let parcels: Vec<IVec2> = parcels.collect();
    let mut unvisited: HashSet<IVec2> = parcels.iter().copied().collect();
    let mut regions = Vec::default();
    let mut stack = Vec::default();

    for &seed in &parcels {
        if !unvisited.remove(&seed) {
            continue;
        }
        // flood fill one contiguous region
        let mut region = vec![seed];
        stack.push(seed);
        while let Some(parcel) = stack.pop() {
            for offset in NEIGHBOURS {
                let next = parcel + offset;
                if unvisited.remove(&next) {
                    region.push(next);
                    stack.push(next);
                }
            }
        }
        split_into_rects(region, &mut regions);
    }

    regions
}

fn split_into_rects(mut region: Vec<IVec2>, regions: &mut Vec<Region>) {
    let min = region.iter().fold(IVec2::MAX, |a, b| a.min(*b));
    let max = region.iter().fold(IVec2::MIN, |a, b| a.max(*b));
    let count = region.len() as u32;

    // fast path for rectangular regions
    let size = max - min;
    if count as i32 == (size.x + 1) * (size.y + 1) {
        regions.push(Region { min, max, count });
        return;
    }

    // each rect grows from the lowest remaining parcel by (x, y)
    region.sort_unstable_by_key(|p| (p.x, p.y));
    let mut remaining: HashSet<IVec2> = region.iter().copied().collect();
    for rect_base in region {
        if !remaining.remove(&rect_base) {
            continue;
        }

        // gather vertical
        let mut extent = IVec2::ZERO;
        while remaining.remove(&(rect_base + extent + IVec2::Y)) {
            extent.y += 1;
        }

        // gather horizontal
        loop {
            let next_col = || (0..=extent.y).map(|y| rect_base + IVec2::new(extent.x + 1, y));
            if next_col().all(|p| remaining.contains(&p)) {
                for p in next_col() {
                    remaining.remove(&p);
                }
                extent.x += 1;
            } else {
                break;
            }
        }

        regions.push(Region {
            min: rect_base,
            max: rect_base + extent,
            count,
        });
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use itertools::Itertools;
    use rand::{seq::SliceRandom, thread_rng, Rng};

    #[test]
    fn test() {
        let test_fn = |test: Vec<&IVec2>| {
            let regions = scene_regions(test.iter().map(|i| **i));
            // println!("{test:?} -> {regions:?}");
            let total = regions
                .iter()
                .map(|r| (r.max.x + 1 - r.min.x) * (r.max.y + 1 - r.min.y))
                .sum::<i32>() as usize;
            assert_eq!(total, test.len());
            for item in test {
                let matching_regions = regions
                    .iter()
                    .filter(|r| r.min.cmple(*item).all() && r.max.cmpge(*item).all())
                    .count();
                assert_eq!(matching_regions, 1);
            }
        };

        for test in [
            vec![(1, 1)],
            vec![(1, 1), (1, 2)],
            vec![(1, 1), (2, 1)],
            vec![(1, 1), (1, 2), (2, 1), (2, 2), (3, 4)],
            vec![(1, 1), (1, 2), (2, 1), (3, 4)],
        ] {
            let test = test
                .into_iter()
                .map(|(x, y)| IVec2::new(x, y))
                .collect::<Vec<_>>();
            for perm in test.iter().permutations(test.len()) {
                test_fn(perm);
            }
        }

        #[allow(clippy::single_element_loop)]
        for test in [vec![
            IVec2::new(0, 0),
            IVec2::new(1, 1),
            IVec2::new(0, 1),
            IVec2::new(0, 2),
            IVec2::new(2, 2),
            IVec2::new(2, 1),
            IVec2::new(1, 2),
        ]] {
            test_fn(test.iter().collect());
        }

        // is this fuzzing? butterfly meme
        let mut rng = thread_rng();
        for _ in 0..10_000 {
            let x_dim = rng.gen_range(1..10);
            let y_dim = rng.gen_range(1..10);
            let fill = rng.gen_range(0.0..1.0);
            let count = (x_dim as f32 * y_dim as f32 * fill) as usize;
            let mut parcels: HashSet<IVec2> = HashSet::default();
            while parcels.len() < count {
                parcels.insert(IVec2::new(rng.gen_range(0..x_dim), rng.gen_range(0..y_dim)));
            }
            test_fn(parcels.iter().collect())
        }
    }

    /// Quadratic merge-based reference for the output contract.
    fn reference_regions(parcels: impl Iterator<Item = IVec2>) -> Vec<Region> {
        // break into contiguous regions
        let mut contiguous_regions: Vec<HashSet<IVec2>> = Vec::default();

        for parcel in parcels.into_iter() {
            let mut adjoining_regions = Vec::default();
            for (ix, region) in contiguous_regions.iter().enumerate() {
                if [IVec2::X, IVec2::Y, IVec2::NEG_X, IVec2::NEG_Y]
                    .into_iter()
                    .any(|offset| region.contains(&(parcel + offset)))
                {
                    adjoining_regions.push(ix)
                }
            }

            match adjoining_regions.len() {
                0 => {
                    contiguous_regions.push(HashSet::from_iter([parcel]));
                }
                1 => {
                    contiguous_regions[adjoining_regions[0]].insert(parcel);
                }
                _ => {
                    adjoining_regions.sort_by_key(|ix| usize::MAX - ix);
                    let target = adjoining_regions.pop().unwrap();
                    contiguous_regions[target].insert(parcel);
                    for source in adjoining_regions.into_iter() {
                        let consumed_region = contiguous_regions.remove(source);
                        contiguous_regions[target].extend(consumed_region);
                    }
                }
            }
        }

        // break regions into rects
        let mut regions = Vec::default();

        for mut region in contiguous_regions {
            let min = region.iter().fold(IVec2::MAX, |a, b| a.min(*b));
            let max = region.iter().fold(IVec2::MIN, |a, b| a.max(*b));
            let count = region.len() as u32;

            // fast path for rectangular regions
            let size = max - min;
            if count as i32 == (size.x + 1) * (size.y + 1) {
                regions.push(Region { min, max, count });
                continue;
            }

            while !region.is_empty() {
                let rect_base = *region
                    .iter()
                    .min_by(|a, b| match a.x.cmp(&b.x) {
                        std::cmp::Ordering::Equal => a.y.cmp(&b.y),
                        diff => diff,
                    })
                    .unwrap();
                region.remove(&rect_base);

                // gather vertical
                let mut extent = IVec2::ZERO;
                while region.remove(&(rect_base + extent + IVec2::Y)) {
                    extent.y += 1;
                }

                // gather horizontal
                loop {
                    let next_col =
                        || (0..=extent.y).map(|y| rect_base + IVec2::new(extent.x + 1, y));
                    if next_col().all(|p| region.contains(&p)) {
                        for p in next_col() {
                            region.remove(&p);
                        }
                        extent.x += 1;
                    } else {
                        break;
                    }
                }

                regions.push(Region {
                    min: rect_base,
                    max: rect_base + extent,
                    count,
                });
            }
        }

        regions
    }

    fn rects(regions: &[Region]) -> Vec<(IVec2, IVec2, u32)> {
        regions.iter().map(|r| (r.min, r.max, r.count)).collect()
    }

    #[test]
    fn matches_reference_in_any_order() {
        let mut rng = thread_rng();
        for _ in 0..2_000 {
            let x_dim = rng.gen_range(1..16);
            let y_dim = rng.gen_range(1..16);
            let fill = rng.gen_range(0.0..1.0);
            let mut parcels: Vec<IVec2> = (0..x_dim)
                .flat_map(|x| (0..y_dim).map(move |y| IVec2::new(x, y)))
                .filter(|_| rng.gen_bool(fill))
                .collect();
            parcels.shuffle(&mut rng);
            assert_eq!(
                rects(&scene_regions(parcels.iter().copied())),
                rects(&reference_regions(parcels.iter().copied())),
                "{parcels:?}"
            );
        }
    }

    /// ~21k parcels: one region with a grid of holes, fed checkerboard-first so a quadratic
    /// merge would first build ~10k single-parcel regions.
    fn large_parcels() -> Vec<IVec2> {
        let mut parcels: Vec<IVec2> = (0..160)
            .flat_map(|x| (0..160).map(move |y| IVec2::new(x, y)))
            .filter(|p| !(p.x % 10 >= 7 && p.y % 10 >= 4))
            .collect();
        parcels.sort_by_key(|p| ((p.x + p.y) % 2, p.x, p.y));
        parcels
    }

    #[test]
    fn large_scene_is_fast() {
        let parcels = large_parcels();
        let start = std::time::Instant::now();
        let regions = scene_regions(parcels.iter().copied());
        let elapsed = start.elapsed();

        let total: i32 = regions
            .iter()
            .map(|r| (r.max.x + 1 - r.min.x) * (r.max.y + 1 - r.min.y))
            .sum();
        assert_eq!(total as usize, parcels.len());
        assert!(regions.iter().all(|r| r.count as usize == parcels.len()));
        // linear runs in milliseconds; the quadratic merge took seconds on this input
        assert!(
            elapsed.as_secs_f64() < 1.0,
            "{} parcels took {elapsed:?}",
            parcels.len()
        );
    }

    #[test]
    #[ignore = "measures the quadratic reference against the bound above"]
    fn large_scene_reference_is_slow() {
        let parcels = large_parcels();
        let start = std::time::Instant::now();
        reference_regions(parcels.iter().copied());
        println!("reference: {:?}", start.elapsed());
    }
}
