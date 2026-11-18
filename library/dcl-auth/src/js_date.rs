//! `Date.parse` as V8 (Node) implements it: the ECMAScript Date Time String
//! Format, then V8's legacy grammar for the rest of the string (RFC 2822,
//! `toUTCString`, `toString` and the other forms it accepts). A date-time
//! without an offset is read as UTC: V8 reads it in the process time zone,
//! and the servers run in UTC. `fixtures/js-date-parse.json` pins the
//! answers against Node.

const NONE: i32 = i32::MAX;
/// V8 keeps the first nine significant digits of a number.
const MAX_SIGNIFICANT_DIGITS: usize = 9;
const MAX_TIME_MS: f64 = 8.64e15;
const MS_PER_DAY: f64 = 86_400_000.0;

/// `Date.parse(text)`: milliseconds since the epoch, `None` for NaN.
pub fn parse(text: &str) -> Option<i64> {
    let units: Vec<u16> = text.encode_utf16().collect();
    let out = parse_fields(&units)?;
    let day = make_day(out.year, out.month, out.day)?;
    let time = out.hour * 3_600_000.0 + out.minute * 60_000.0 + out.second * 1000.0 + out.ms;
    let mut date = day * MS_PER_DAY + time;
    match out.offset_secs {
        // The process time zone is UTC: local time converts unchanged.
        None if date.abs() > MAX_TIME_MS + 864e6 => return None,
        None => {}
        Some(offset) => {
            date -= offset as f64 * 1000.0;
            if date.abs() > MAX_TIME_MS {
                return None;
            }
        }
    }
    if date.abs() > MAX_TIME_MS {
        return None;
    }
    Some(date as i64)
}

/// `new Date(ms).toISOString()`; `None` outside the representable range.
pub fn to_iso_string(ms: i64) -> Option<String> {
    if (ms as f64).abs() > MAX_TIME_MS {
        return None;
    }
    let days = ms.div_euclid(86_400_000);
    let in_day = ms.rem_euclid(86_400_000);
    let (year, month, day) = civil_from_days(days);
    let year = if (0..=9999).contains(&year) {
        format!("{year:04}")
    } else if year < 0 {
        format!("-{:06}", -year)
    } else {
        format!("+{year:06}")
    };
    Some(format!(
        "{year}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:03}Z",
        in_day / 3_600_000,
        in_day / 60_000 % 60,
        in_day / 1000 % 60,
        in_day % 1000
    ))
}

struct Fields {
    year: f64,
    month: f64,
    day: f64,
    hour: f64,
    minute: f64,
    second: f64,
    ms: f64,
    offset_secs: Option<i32>,
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// ES `MakeDay` for a 0-based month in 0..12 and a day in 1..=31.
fn make_day(year: f64, month: f64, day: f64) -> Option<f64> {
    let (y, m, d) = (year as i64, month as i64, day as i64);
    Some(days_from_civil(y, m + 1, 1) as f64 + (d - 1) as f64)
}

/// JS WhiteSpace and LineTerminator.
pub(crate) fn is_js_whitespace(c: u32) -> bool {
    matches!(
        c,
        0x09 | 0x0A | 0x0B | 0x0C | 0x0D | 0x20 | 0xA0 | 0x1680 | 0x2000
            ..=0x200A | 0x2028 | 0x2029 | 0x202F | 0x205F | 0x3000 | 0xFEFF
    )
}

/// `String.prototype.trim`.
/// `String.prototype.trim`: JS whitespace, which leaves U+0085 (NEL).
pub(crate) fn js_trim(value: &str) -> &str {
    value.trim_matches(|c: char| is_js_whitespace(c as u32))
}

/// Whitespace that ends a word in the date grammar: not U+2028 or U+2029.
fn ends_word(c: u32) -> bool {
    is_js_whitespace(c) && c != 0x2028 && c != 0x2029
}

struct Reader<'a> {
    text: &'a [u16],
    pos: usize,
}

impl Reader<'_> {
    /// The current code unit; 0 past the end, which also ends the input.
    fn ch(&self) -> u32 {
        self.text.get(self.pos).map_or(0, |&c| u32::from(c))
    }

    fn next(&mut self) {
        if self.pos < self.text.len() {
            self.pos += 1;
        }
    }

    fn is_end(&self) -> bool {
        self.ch() == 0
    }

    fn is_digit(&self) -> bool {
        (0x30..=0x39).contains(&self.ch())
    }

    fn skip(&mut self, c: u8) -> bool {
        if self.ch() == u32::from(c) {
            self.next();
            true
        } else {
            false
        }
    }

    fn read_unsigned_numeral(&mut self) -> i32 {
        while self.ch() == u32::from(b'0') {
            self.next();
        }
        let mut n: i32 = 0;
        let mut i = 0;
        while self.is_digit() {
            if i < MAX_SIGNIFICANT_DIGITS {
                n = n * 10 + (self.ch() - 0x30) as i32;
            }
            i += 1;
            self.next();
        }
        n
    }

    fn is_alpha_or_above(&self) -> bool {
        self.ch() >= u32::from(b'A')
    }

    fn read_word(&mut self) -> ([u32; 3], usize) {
        let mut prefix = [0u32; 3];
        let mut len = 0;
        while self.is_alpha_or_above() && !ends_word(self.ch()) {
            if len < 3 {
                prefix[len] = self.ch() | 0x20;
            }
            len += 1;
            self.next();
        }
        (prefix, len)
    }

    fn skip_parentheses(&mut self) -> bool {
        if self.ch() != u32::from(b'(') {
            return false;
        }
        let mut balance = 0;
        loop {
            match self.ch() {
                0x29 => balance -= 1,
                0x28 => balance += 1,
                _ => {}
            }
            self.next();
            if balance <= 0 || self.ch() == 0 {
                break;
            }
        }
        true
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Keyword {
    Invalid,
    MonthName,
    TimeZoneName,
    TimeSeparator,
    AmPm,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Token {
    Invalid,
    Unknown,
    WhiteSpace,
    End,
    Number {
        value: i32,
        length: usize,
    },
    Symbol(u8),
    Keyword {
        kind: Keyword,
        value: i32,
        length: usize,
    },
}

impl Token {
    fn is_number(self) -> bool {
        matches!(self, Token::Number { .. })
    }

    fn is_fixed_length_number(self, n: usize) -> bool {
        matches!(self, Token::Number { length, .. } if length == n)
    }

    fn number(self) -> i32 {
        match self {
            Token::Number { value, .. } => value,
            _ => 0,
        }
    }

    fn is_symbol(self, c: u8) -> bool {
        self == Token::Symbol(c)
    }

    fn is_sign(self) -> bool {
        matches!(self, Token::Symbol(b'+' | b'-'))
    }

    fn sign(self) -> i32 {
        if self.is_symbol(b'-') { -1 } else { 1 }
    }

    fn is_keyword_z(self) -> bool {
        matches!(
            self,
            Token::Keyword {
                kind: Keyword::TimeZoneName,
                value: 0,
                length: 1
            }
        )
    }

    fn is_keyword(self, kind: Keyword) -> bool {
        matches!(self, Token::Keyword { kind: k, .. } if k == kind)
    }
}

const KEYWORDS: [([u8; 3], Keyword, i32); 27] = [
    (*b"jan", Keyword::MonthName, 1),
    (*b"feb", Keyword::MonthName, 2),
    (*b"mar", Keyword::MonthName, 3),
    (*b"apr", Keyword::MonthName, 4),
    (*b"may", Keyword::MonthName, 5),
    (*b"jun", Keyword::MonthName, 6),
    (*b"jul", Keyword::MonthName, 7),
    (*b"aug", Keyword::MonthName, 8),
    (*b"sep", Keyword::MonthName, 9),
    (*b"oct", Keyword::MonthName, 10),
    (*b"nov", Keyword::MonthName, 11),
    (*b"dec", Keyword::MonthName, 12),
    (*b"am\0", Keyword::AmPm, 0),
    (*b"pm\0", Keyword::AmPm, 12),
    (*b"ut\0", Keyword::TimeZoneName, 0),
    (*b"utc", Keyword::TimeZoneName, 0),
    (*b"z\0\0", Keyword::TimeZoneName, 0),
    (*b"gmt", Keyword::TimeZoneName, 0),
    (*b"cdt", Keyword::TimeZoneName, -5),
    (*b"cst", Keyword::TimeZoneName, -6),
    (*b"edt", Keyword::TimeZoneName, -4),
    (*b"est", Keyword::TimeZoneName, -5),
    (*b"mdt", Keyword::TimeZoneName, -6),
    (*b"mst", Keyword::TimeZoneName, -7),
    (*b"pdt", Keyword::TimeZoneName, -7),
    (*b"pst", Keyword::TimeZoneName, -8),
    (*b"t\0\0", Keyword::TimeSeparator, 0),
];

fn lookup_keyword(prefix: [u32; 3], len: usize) -> (Keyword, i32) {
    KEYWORDS
        .iter()
        .find(|(word, kind, _)| {
            word.iter().zip(prefix).all(|(&w, p)| u32::from(w) == p)
                && (len <= 3 || *kind == Keyword::MonthName)
        })
        .map_or((Keyword::Invalid, 0), |&(_, kind, value)| (kind, value))
}

struct Tokenizer<'a> {
    reader: Reader<'a>,
    next: Token,
}

impl<'a> Tokenizer<'a> {
    fn new(text: &'a [u16]) -> Self {
        let mut reader = Reader { text, pos: 0 };
        let next = scan(&mut reader);
        Self { reader, next }
    }

    fn next(&mut self) -> Token {
        let result = self.next;
        self.next = scan(&mut self.reader);
        result
    }

    fn peek(&self) -> Token {
        self.next
    }

    fn skip_symbol(&mut self, c: u8) -> bool {
        if self.next.is_symbol(c) {
            self.next();
            true
        } else {
            false
        }
    }
}

fn scan(reader: &mut Reader) -> Token {
    let start = reader.pos;
    if reader.is_end() {
        return Token::End;
    }
    if reader.is_digit() {
        let value = reader.read_unsigned_numeral();
        return Token::Number {
            value,
            length: reader.pos - start,
        };
    }
    for c in *b":-+.)" {
        if reader.skip(c) {
            return Token::Symbol(c);
        }
    }
    if reader.is_alpha_or_above() && !ends_word(reader.ch()) {
        let (prefix, length) = reader.read_word();
        let (kind, value) = lookup_keyword(prefix, length);
        return Token::Keyword {
            kind,
            value,
            length,
        };
    }
    if is_js_whitespace(reader.ch()) {
        reader.next();
        return Token::WhiteSpace;
    }
    if reader.skip_parentheses() {
        return Token::Unknown;
    }
    reader.next();
    Token::Unknown
}

fn between(x: i32, lo: i32, hi: i32) -> bool {
    (lo..=hi).contains(&x)
}

fn is_month(x: i32) -> bool {
    between(x, 1, 12)
}

fn is_day(x: i32) -> bool {
    between(x, 1, 31)
}

fn is_hour(x: i32) -> bool {
    between(x, 0, 23)
}

fn is_minute(x: i32) -> bool {
    between(x, 0, 59)
}

fn is_millisecond(x: i32) -> bool {
    between(x, 0, 999)
}

struct DayComposer {
    comp: [i32; 3],
    index: usize,
    named_month: i32,
    iso: bool,
}

impl DayComposer {
    fn is_empty(&self) -> bool {
        self.index == 0
    }

    fn add(&mut self, n: i32) -> bool {
        if self.index < 3 {
            self.comp[self.index] = n;
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn write(&mut self) -> Option<(i32, i32, i32)> {
        if self.index < 1 {
            return None;
        }
        while self.index < 3 {
            self.comp[self.index] = 1;
            self.index += 1;
        }
        let c = self.comp;
        let (mut year, month, day);
        if self.named_month == NONE {
            if self.iso || !is_day(c[0]) {
                (year, month, day) = (c[0], c[1], c[2]);
            } else {
                (month, day, year) = (c[0], c[1], c[2]);
            }
        } else {
            month = self.named_month;
            if !is_day(c[0]) {
                (year, day) = (c[0], c[1]);
            } else {
                (day, year) = (c[0], c[1]);
            }
        }
        if !self.iso {
            if between(year, 0, 49) {
                year += 2000;
            } else if between(year, 50, 99) {
                year += 1900;
            }
        }
        // Every i32 year is a small integer in Node's V8.
        if !is_month(month) || !is_day(day) {
            return None;
        }
        Some((year, month - 1, day))
    }
}

struct TimeComposer {
    comp: [i32; 4],
    index: usize,
    hour_offset: i32,
}

impl TimeComposer {
    fn is_empty(&self) -> bool {
        self.index == 0
    }

    fn is_expecting(&self, n: i32) -> bool {
        (self.index == 1 && is_minute(n))
            || (self.index == 2 && is_minute(n))
            || (self.index == 3 && is_millisecond(n))
    }

    fn add(&mut self, n: i32) -> bool {
        if self.index < 4 {
            self.comp[self.index] = n;
            self.index += 1;
            true
        } else {
            false
        }
    }

    fn add_final(&mut self, n: i32) -> bool {
        if !self.add(n) {
            return false;
        }
        while self.index < 4 {
            self.comp[self.index] = 0;
            self.index += 1;
        }
        true
    }

    fn write(&mut self) -> Option<[i32; 4]> {
        while self.index < 4 {
            self.comp[self.index] = 0;
            self.index += 1;
        }
        let [mut hour, minute, second, ms] = self.comp;
        if self.hour_offset != NONE {
            if !between(hour, 0, 12) {
                return None;
            }
            hour %= 12;
            hour += self.hour_offset;
        }
        if (!is_hour(hour) || !is_minute(minute) || !is_minute(second) || !is_millisecond(ms))
            && (hour != 24 || minute != 0 || second != 0 || ms != 0)
        {
            return None;
        }
        Some([hour, minute, second, ms])
    }
}

struct TimeZoneComposer {
    sign: i32,
    hour: i32,
    minute: i32,
}

impl TimeZoneComposer {
    fn set(&mut self, offset_hours: i32) {
        self.sign = if offset_hours < 0 { -1 } else { 1 };
        self.hour = offset_hours * self.sign;
        self.minute = 0;
    }

    fn is_expecting(&self, n: i32) -> bool {
        self.hour != NONE && self.minute == NONE && is_minute(n)
    }

    fn is_utc(&self) -> bool {
        self.hour == 0 && self.minute == 0
    }

    fn is_empty(&self) -> bool {
        self.hour == NONE
    }

    fn write(&mut self) -> Option<Option<i32>> {
        if self.sign == NONE {
            return Some(None);
        }
        if self.hour == NONE {
            self.hour = 0;
        }
        if self.minute == NONE {
            self.minute = 0;
        }
        let total = (self.hour as u32)
            .wrapping_mul(3600)
            .wrapping_add((self.minute as u32).wrapping_mul(60));
        if total > i32::MAX as u32 {
            return None;
        }
        let total = total as i32;
        Some(Some(if self.sign < 0 { -total } else { total }))
    }
}

/// V8's `ReadMilliseconds`: the first three significant digits, from the
/// number and its digit count.
fn read_milliseconds(token: Token) -> i32 {
    let (mut number, mut length) = match token {
        Token::Number { value, length } => (value, length),
        _ => return 0,
    };
    if length == 1 {
        number *= 100;
    } else if length == 2 {
        number *= 10;
    } else if length > 3 {
        length = length.min(MAX_SIGNIFICANT_DIGITS);
        let mut factor = 1;
        while length > 3 {
            factor *= 10;
            length -= 1;
        }
        number /= factor;
    }
    number
}

/// The ES Date Time String Format prefix of the input. `Token::End` when it
/// was the whole input, `Token::Invalid` when the input cannot be a date,
/// otherwise the token the legacy grammar continues from.
fn parse_es5(
    scanner: &mut Tokenizer,
    day: &mut DayComposer,
    time: &mut TimeComposer,
    tz: &mut TimeZoneComposer,
) -> Token {
    if scanner.peek().is_sign() {
        let sign_token = scanner.next();
        if !scanner.peek().is_fixed_length_number(6) {
            return sign_token;
        }
        let sign = sign_token.sign();
        let year = scanner.next().number();
        if sign < 0 && year == 0 {
            return sign_token;
        }
        day.add(sign * year);
    } else if scanner.peek().is_fixed_length_number(4) {
        day.add(scanner.next().number());
    } else {
        return scanner.next();
    }
    if scanner.skip_symbol(b'-') {
        if !scanner.peek().is_fixed_length_number(2) || !is_month(scanner.peek().number()) {
            return scanner.next();
        }
        day.add(scanner.next().number());
        if scanner.skip_symbol(b'-') {
            if !scanner.peek().is_fixed_length_number(2) || !is_day(scanner.peek().number()) {
                return scanner.next();
            }
            day.add(scanner.next().number());
        }
    }
    if !scanner.peek().is_keyword(Keyword::TimeSeparator) {
        if scanner.peek() != Token::End {
            return scanner.next();
        }
    } else {
        scanner.next();
        if !scanner.peek().is_fixed_length_number(2) || !between(scanner.peek().number(), 0, 24) {
            return Token::Invalid;
        }
        let hour_is_24 = scanner.peek().number() == 24;
        time.add(scanner.next().number());
        if !scanner.skip_symbol(b':') {
            return Token::Invalid;
        }
        if !scanner.peek().is_fixed_length_number(2)
            || !is_minute(scanner.peek().number())
            || (hour_is_24 && scanner.peek().number() > 0)
        {
            return Token::Invalid;
        }
        time.add(scanner.next().number());
        if scanner.skip_symbol(b':') {
            if !scanner.peek().is_fixed_length_number(2)
                || !is_minute(scanner.peek().number())
                || (hour_is_24 && scanner.peek().number() > 0)
            {
                return Token::Invalid;
            }
            time.add(scanner.next().number());
            if scanner.skip_symbol(b'.') {
                if !scanner.peek().is_number() || (hour_is_24 && scanner.peek().number() > 0) {
                    return Token::Invalid;
                }
                time.add(read_milliseconds(scanner.next()));
            }
        }
        if scanner.peek().is_keyword_z() {
            scanner.next();
            tz.set(0);
        } else if scanner.peek().is_sign() {
            tz.sign = scanner.next().sign();
            if scanner.peek().is_fixed_length_number(4) {
                let hourmin = scanner.next().number();
                let (hour, minute) = (hourmin / 100, hourmin % 100);
                if !is_hour(hour) || !is_minute(minute) {
                    return Token::Invalid;
                }
                tz.hour = hour;
                tz.minute = minute;
            } else {
                if !scanner.peek().is_fixed_length_number(2) || !is_hour(scanner.peek().number()) {
                    return Token::Invalid;
                }
                tz.hour = scanner.next().number();
                if !scanner.skip_symbol(b':') {
                    return Token::Invalid;
                }
                if !scanner.peek().is_fixed_length_number(2) || !is_minute(scanner.peek().number())
                {
                    return Token::Invalid;
                }
                tz.minute = scanner.next().number();
            }
        }
        if scanner.peek() != Token::End {
            return Token::Invalid;
        }
    }
    // Date-only forms are UTC; date-time forms without an offset are local.
    if tz.is_empty() && time.is_empty() {
        tz.set(0);
    }
    day.iso = true;
    Token::End
}

fn parse_fields(text: &[u16]) -> Option<Fields> {
    let mut scanner = Tokenizer::new(text);
    let mut day = DayComposer {
        comp: [0; 3],
        index: 0,
        named_month: NONE,
        iso: false,
    };
    let mut time = TimeComposer {
        comp: [0; 4],
        index: 0,
        hour_offset: NONE,
    };
    let mut tz = TimeZoneComposer {
        sign: NONE,
        hour: NONE,
        minute: NONE,
    };

    let mut token = parse_es5(&mut scanner, &mut day, &mut time, &mut tz);
    if token == Token::Invalid {
        return None;
    }
    let mut has_read_number = !day.is_empty();
    while token != Token::End {
        match token {
            Token::Number { value: n, .. } => {
                has_read_number = true;
                if scanner.skip_symbol(b':') {
                    if scanner.skip_symbol(b':') {
                        if !time.is_empty() {
                            return None;
                        }
                        time.add(n);
                        time.add(0);
                    } else {
                        if !time.add(n) {
                            return None;
                        }
                        if scanner.peek().is_symbol(b'.') {
                            scanner.next();
                        }
                    }
                } else if scanner.skip_symbol(b'.') && time.is_expecting(n) {
                    time.add(n);
                    if !scanner.peek().is_number() {
                        return None;
                    }
                    let ms = read_milliseconds(scanner.next());
                    if ms < 0 {
                        return None;
                    }
                    time.add_final(ms);
                } else if tz.is_expecting(n) {
                    tz.minute = n;
                } else if time.is_expecting(n) {
                    time.add_final(n);
                    let peek = scanner.peek();
                    if peek != Token::End
                        && peek != Token::WhiteSpace
                        && !peek.is_keyword_z()
                        && !peek.is_sign()
                    {
                        return None;
                    }
                } else {
                    if !day.add(n) {
                        return None;
                    }
                    scanner.skip_symbol(b'-');
                }
            }
            Token::Keyword { kind, value, .. } => {
                if kind == Keyword::AmPm && !time.is_empty() {
                    time.hour_offset = value;
                } else if kind == Keyword::MonthName {
                    day.named_month = value;
                    scanner.skip_symbol(b'-');
                } else if kind == Keyword::TimeZoneName && has_read_number {
                    tz.set(value);
                } else {
                    if has_read_number {
                        return None;
                    }
                    if scanner.peek().is_number() {
                        return None;
                    }
                }
            }
            t if t.is_sign() && (tz.is_utc() || !time.is_empty()) => {
                tz.sign = t.sign();
                let (mut n, mut length) = (0, 0);
                if let Token::Number { value, length: l } = scanner.peek() {
                    scanner.next();
                    n = value;
                    length = l;
                }
                has_read_number = true;
                if scanner.peek().is_symbol(b':') {
                    tz.hour = n;
                    tz.minute = NONE;
                } else if length == 1 || length == 2 {
                    tz.hour = n;
                    tz.minute = 0;
                } else if length == 3 || length == 4 {
                    tz.hour = n / 100;
                    tz.minute = n % 100;
                } else {
                    return None;
                }
            }
            t if (t.is_sign() || t.is_symbol(b')')) && has_read_number => return None,
            _ => {}
        }
        token = scanner.next();
    }

    let (year, month, day) = day.write()?;
    let [hour, minute, second, ms] = time.write()?;
    let offset_secs = tz.write()?;
    Some(Fields {
        year: f64::from(year),
        month: f64::from(month),
        day: f64::from(day),
        hour: f64::from(hour),
        minute: f64::from(minute),
        second: f64::from(second),
        ms: f64::from(ms),
        offset_secs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Deserialize)]
    struct Case {
        input: String,
        expected: Option<i64>,
    }

    #[derive(serde::Deserialize)]
    struct Corpus {
        cases: Vec<Case>,
    }

    /// Every row of the Node corpus (`Date.parse` under `TZ=UTC`).
    #[test]
    fn matches_node_date_parse() {
        let corpus: Corpus = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/js-date-parse.json"
        )))
        .unwrap();
        let mismatches: Vec<String> = corpus
            .cases
            .iter()
            .filter(|case| parse(&case.input) != case.expected)
            .map(|case| {
                format!(
                    "{:?}: node {:?}, rust {:?}",
                    case.input,
                    case.expected,
                    parse(&case.input)
                )
            })
            .collect();
        assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
        assert!(corpus.cases.len() > 200);
    }

    /// Node: `"a\u0085".trim()` keeps NEL, `"\u180ex".trim()` keeps U+180E.
    #[test]
    fn trim_matches_node() {
        assert_eq!(js_trim("a\u{85}"), "a\u{85}");
        assert_eq!(js_trim("\u{2028}x\u{2029}"), "x");
        assert_eq!(js_trim("\u{feff}\u{a0} x\u{3000}"), "x");
        assert_eq!(js_trim("\u{180e}x"), "\u{180e}x");
        assert_eq!(parse("\u{85}2020-02-13"), None);
    }

    #[test]
    fn iso_strings_round_trip() {
        for ms in [
            0,
            1_581_618_670_000,
            -1,
            -62_198_755_200_000,
            253_402_300_800_000,
            8_640_000_000_000_000,
            -8_640_000_000_000_000,
        ] {
            let text = to_iso_string(ms).unwrap();
            assert_eq!(parse(&text), Some(ms), "{text}");
        }
        assert_eq!(
            to_iso_string(1_581_618_670_000).unwrap(),
            "2020-02-13T18:31:10.000Z"
        );
        assert_eq!(
            to_iso_string(8_640_000_000_000_000).unwrap(),
            "+275760-09-13T00:00:00.000Z"
        );
        assert_eq!(to_iso_string(8_640_000_000_000_001), None);
    }
}
