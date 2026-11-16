use bevy::prelude::*;
use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};

/// The colour of a name with no profile to derive one from.
pub const UNCLAIMED_NAME_COLOR: Color = Color::srgb(0.6, 0.6, 0.6);

/// A name's colour: a hue hashed from the name as displayed (its letters and digits, plus `#`
/// and the last four characters of the address when the name isn't claimed), at 75%
/// saturation and full value. The HUD derives the same colour (`nameColor.ts`), so a name has
/// one colour everywhere.
pub fn name_color(name: &str, address: &str, claimed: bool) -> Color {
    let base = name
        .split('#')
        .next()
        .unwrap_or_default()
        .chars()
        // letters and numbers only, as the HUD's `\p{L}\p{N}`: no combining marks
        .filter(|c| {
            matches!(
                c.general_category_group(),
                GeneralCategoryGroup::Letter | GeneralCategoryGroup::Number
            )
        })
        .collect::<String>();
    if base.is_empty() {
        return Color::WHITE;
    }
    let display = match address.char_indices().rev().nth(3) {
        Some((last4, _)) if !claimed && address.chars().count() > 4 => {
            format!("{base}#{}", &address[last4..])
        }
        _ => base,
    };

    // FNV-1a over the UTF-16 code units
    let hash = display.encode_utf16().fold(2166136261u32, |hash, unit| {
        (hash ^ unit as u32).wrapping_mul(16777619)
    });
    let hue = hash as f64 / u32::MAX as f64 * 6.0;
    let sector = hue.floor();
    let f = (hue - sector) as f32;
    let (p, q, t) = (0.25, 1.0 - f * 0.75, 1.0 - (1.0 - f) * 0.75);
    let [r, g, b] = match sector as u32 % 6 {
        0 => [1.0, t, p],
        1 => [q, 1.0, p],
        2 => [p, 1.0, t],
        3 => [p, q, 1.0],
        4 => [t, p, 1.0],
        _ => [1.0, p, q],
    };
    Color::srgb(r, g, b)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(color: Color) -> String {
        color.to_srgba().to_hex().to_lowercase()
    }

    /// The colours `userNameColor` in `library/explorer-core/engine/nameColor.ts` gives; its test
    /// checks the same vectors.
    #[test]
    fn matches_the_hud() {
        for (name, address, claimed, expected) in [
            (
                "Ada",
                "0x00000000000000000000000000000000000012ab",
                true,
                "#4087ff",
            ),
            (
                "Ada",
                "0x00000000000000000000000000000000000012ab",
                false,
                "#4078ff",
            ),
            (
                "Grace Hopper!",
                "0xAbCdEf0000000000000000000000000000009F3c",
                false,
                "#40dfff",
            ),
            (
                "名前😀",
                "0x0000000000000000000000000000000000000001",
                true,
                "#4f40ff",
            ),
            (
                "",
                "0x0000000000000000000000000000000000000001",
                true,
                "#ffffff",
            ),
            (
                "x#1234",
                "0x00000000000000000000000000000000deadbeef",
                false,
                "#40daff",
            ),
            // a combining vowel sign is neither a letter nor a number
            (
                "राम",
                "0x0000000000000000000000000000000000000001",
                true,
                "#ff4092",
            ),
        ] {
            assert_eq!(hex(name_color(name, address, claimed)), expected, "{name}");
        }
    }
}
