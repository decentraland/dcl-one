//! The parcels Genesis City holds: the ±150 square plus three blocks past its
//! north and east edges. This is `isInsideWorldLimits` in `@dcl/schemas`
//! 26.5.0 (`dist/dapps/world.js`), and the same set `Parcel.validate` accepts
//! in that release (`dist/core/parcel-validation.js`, `parcel-exceptions.js`),
//! the check lamb2 runs on parcel coordinates.

/// `(x_min, y_min, x_max, y_max)`, inclusive, in `getWorld().validWorldRanges` order.
const WORLD_RANGES: [(i64, i64, i64, i64); 4] = [
    (-150, -150, 150, 150),
    (62, 151, 162, 158),
    (151, 144, 162, 150),
    (151, 59, 163, 143),
];

pub fn is_inside_world_limits(x: impl Into<i64>, y: impl Into<i64>) -> bool {
    let (x, y) = (x.into(), y.into());
    WORLD_RANGES.iter().any(|&(x_min, y_min, x_max, y_max)| {
        (x_min..=x_max).contains(&x) && (y_min..=y_max).contains(&y)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `Parcel.validate` as `@dcl/schemas` 26.5.0 writes it: the standard
    /// square or one of the three exception blocks.
    fn schemas_parcel_validate(x: i64, y: i64) -> bool {
        let standard = (-150..=150).contains(&x) && (-150..=150).contains(&y);
        let block1 = (62..=150).contains(&x) && (151..=158).contains(&y);
        let block2 = (151..=162).contains(&x) && (144..=158).contains(&y);
        let block3 = (151..=163).contains(&x) && (59..=143).contains(&y);
        standard || block1 || block2 || block3
    }

    #[test]
    fn world_limits_are_the_set_parcel_validate_accepts() {
        for x in -170..=170 {
            for y in -170..=170 {
                assert_eq!(
                    is_inside_world_limits(x, y),
                    schemas_parcel_validate(x, y),
                    "({x},{y})"
                );
            }
        }
    }

    #[test]
    fn the_exception_blocks_and_their_edges() {
        for (x, y) in [
            (-150, -150),
            (150, 150),
            (62, 158),
            (162, 158),
            (151, 144),
            (163, 59),
            (163, 143),
        ] {
            assert!(is_inside_world_limits(x, y), "({x},{y})");
        }
        for (x, y) in [
            (151, 0),
            (0, 151),
            (-151, 0),
            (61, 151),
            (163, 144),
            (164, 100),
            (151, 58),
            (100, 159),
        ] {
            assert!(!is_inside_world_limits(x, y), "({x},{y})");
        }
        assert!(!is_inside_world_limits(i64::MAX, 0));
        assert!(is_inside_world_limits(160_i32, 155_i32));
    }
}
