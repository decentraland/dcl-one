//! Finite JavaScript Number input, used only by Social's timestamp boundary.
pub(super) fn timestamp(raw: &str) -> Option<f64> {
    let raw = raw.trim_matches(|c: char| c == '\u{feff}' || (c != '\u{85}' && c.is_whitespace()));
    if raw.is_empty() {
        return Some(0.0);
    }
    let radix = [
        ("0x", 16),
        ("0X", 16),
        ("0o", 8),
        ("0O", 8),
        ("0b", 2),
        ("0B", 2),
    ]
    .into_iter()
    .find_map(|(prefix, radix)| raw.strip_prefix(prefix).map(|digits| (digits, radix)));
    let number = if let Some((digits, radix)) = radix {
        if digits.is_empty() {
            return None;
        }
        // Keep the leading 53 bits and round once with guard/sticky bits.
        // Repeated floating multiply/add would double-round long radix inputs.
        let width = match radix {
            2 => 1,
            8 => 3,
            _ => 4,
        };
        let (mut bits, mut mantissa, mut guard, mut sticky) = (0usize, 0u64, false, false);
        for digit in digits.chars() {
            let digit = digit.to_digit(radix)?;
            for shift in (0..width).rev() {
                let bit = (digit >> shift) & 1;
                if bits == 0 && bit == 0 {
                    continue;
                }
                bits += 1;
                if bits <= 53 {
                    mantissa = mantissa * 2 + bit as u64;
                } else if bits == 54 {
                    guard = bit != 0;
                } else {
                    sticky |= bit != 0;
                }
            }
        }
        if bits > 1024 {
            return None;
        }
        if guard && (sticky || mantissa & 1 != 0) {
            mantissa += 1;
        }
        if bits <= 53 {
            mantissa as f64
        } else {
            mantissa as f64 * 2.0_f64.powi((bits - 53) as i32)
        }
    } else {
        raw.parse::<f64>().ok()?
    };
    number.is_finite().then_some(number)
}
