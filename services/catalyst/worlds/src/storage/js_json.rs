//! An upsert body read as upstream's Node runtime reads it: the body decoded
//! as UTF-8 (lossy, a leading BOM dropped), `JSON.parse` (V8's grammar and
//! error messages, positions in UTF-16 units), the ajv 8 check of the route
//! schema (`strictNumbers`, so an overflowing number such as `1e400` fails
//! its `anyOf`), and the stored text `JSON.stringify` produces (ECMAScript
//! `Number::toString`, array-index keys first, a duplicate key keeping its
//! first position and its last value). Parsing, validation and serialization
//! are iterative, so any nesting depth is safe.

use std::collections::HashMap;
use std::fmt::Write as _;

use serde::Serialize;
use serde_json::{Value, json};

use super::js::{JsString, JsValue};

/// One parsed value; containers hold arena indices.
enum Item {
    Null,
    Bool(bool),
    Num(f64),
    Str(Vec<u16>),
    Arr(Vec<usize>),
    Obj(Vec<(Vec<u16>, usize)>),
}

/// A parsed document; the root is item 0.
pub struct Doc {
    items: Vec<Item>,
}

/// One ajv error object, fields in ajv's order.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AjvError {
    pub instance_path: String,
    pub schema_path: &'static str,
    pub keyword: &'static str,
    pub params: Value,
    pub message: String,
}

/// Failing chains deeper than this answer only their outermost `anyOf`
/// error: the full cascade grows with the square of the depth.
pub const MAX_ERROR_DEPTH: usize = 128;

/// The text `JSON.parse` sees: the body as UTF-8 with a leading BOM removed.
pub fn decode_body(bytes: &[u8]) -> Vec<u16> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    String::from_utf8_lossy(bytes).encode_utf16().collect()
}

/// `JSON.parse(source)`; the error is V8's message.
pub fn parse(source: &[u16]) -> Result<Doc, String> {
    Parser {
        s: source,
        pos: 0,
        items: Vec::new(),
    }
    .run()
}

/// One open container. A body can open one per byte, so an array frame
/// stays two words and the object state is boxed.
enum Frame {
    Arr(usize),
    Obj(Box<ObjFrame>),
}

struct ObjFrame {
    idx: usize,
    /// The property whose value is being parsed.
    key: Vec<u16>,
    /// Each key's entry position, for a repeated key.
    seen: HashMap<Vec<u16>, usize>,
}

#[derive(Clone, Copy)]
enum Msg {
    UnexpectedNumber,
    UnexpectedString,
    ExpectedPropNameOrRBrace,
    ExpectedCommaOrRBrack,
    ExpectedCommaOrRBrace,
    ExpectedColon,
    UnterminatedString,
    BadControlCharacter,
    BadUnicodeEscape,
    BadEscapedCharacter,
    NoNumberAfterMinus,
    ExponentMissingNumber,
    UnterminatedFraction,
    ExpectedDoubleQuotedPropertyName,
    NonWhitespaceAfter,
}

impl Msg {
    fn text(self) -> &'static str {
        match self {
            Msg::UnexpectedNumber => "Unexpected number in JSON",
            Msg::UnexpectedString => "Unexpected string in JSON",
            Msg::ExpectedPropNameOrRBrace => "Expected property name or '}' in JSON",
            Msg::ExpectedCommaOrRBrack => "Expected ',' or ']' after array element in JSON",
            Msg::ExpectedCommaOrRBrace => "Expected ',' or '}' after property value in JSON",
            Msg::ExpectedColon => "Expected ':' after property name in JSON",
            Msg::UnterminatedString => "Unterminated string in JSON",
            Msg::BadControlCharacter => "Bad control character in string literal in JSON",
            Msg::BadUnicodeEscape => "Bad Unicode escape in JSON",
            Msg::BadEscapedCharacter => "Bad escaped character in JSON",
            Msg::NoNumberAfterMinus => "No number after minus sign in JSON",
            Msg::ExponentMissingNumber => "Exponent part is missing a number in JSON",
            Msg::UnterminatedFraction => "Unterminated fractional number in JSON",
            Msg::ExpectedDoubleQuotedPropertyName => "Expected double-quoted property name in JSON",
            Msg::NonWhitespaceAfter => "Unexpected non-whitespace character after JSON",
        }
    }
}

const EOS_MESSAGE: &str = "Unexpected end of JSON input";
const CONTEXT_CHARS: usize = 10;

fn is_ws(c: u16) -> bool {
    matches!(c, 0x20 | 0x09 | 0x0a | 0x0d)
}

fn is_digit(c: u16) -> bool {
    (0x30..=0x39).contains(&c)
}

fn hex_value(c: u16) -> Option<u16> {
    match c {
        0x30..=0x39 => Some(c - 0x30),
        0x41..=0x46 => Some(c - 0x41 + 10),
        0x61..=0x66 => Some(c - 0x61 + 10),
        _ => None,
    }
}

struct Parser<'a> {
    s: &'a [u16],
    pos: usize,
    items: Vec<Item>,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u16> {
        self.s.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while self.peek().is_some_and(is_ws) {
            self.pos += 1;
        }
    }

    fn push(&mut self, item: Item) -> usize {
        self.items.push(item);
        self.items.len() - 1
    }

    /// `… at position N (line L column C)`, a CR LF counting as one break.
    fn at(&self, msg: Msg, pos: usize) -> String {
        let (mut line, mut line_start, mut i) = (1, 0, 0);
        while i < pos.min(self.s.len()) {
            match self.s[i] {
                0x0d => {
                    if self.s.get(i + 1) == Some(&0x0a) && i + 1 < pos {
                        i += 1;
                    }
                    line += 1;
                    line_start = i + 1;
                }
                0x0a => {
                    line += 1;
                    line_start = i + 1;
                }
                _ => {}
            }
            i += 1;
        }
        format!(
            "{} at position {pos} (line {line} column {})",
            msg.text(),
            pos - line_start + 1
        )
    }

    /// V8's report for an unexpected token at `pos`, keyed on its class.
    fn unexpected(&self, pos: usize) -> String {
        let Some(c) = self.s.get(pos).copied() else {
            return EOS_MESSAGE.to_string();
        };
        if is_digit(c) || c == u16::from(b'-') {
            return self.at(Msg::UnexpectedNumber, pos);
        }
        if c == u16::from(b'"') {
            return self.at(Msg::UnexpectedString, pos);
        }
        let source = String::from_utf16_lossy(self.s);
        if matches!(
            source.as_str(),
            "NaN" | "Infinity" | "undefined" | "[object Object]"
        ) {
            return format!("\"{source}\" is not valid JSON");
        }
        let token = String::from_utf16_lossy(&[c]);
        let len = self.s.len();
        let slice = |from: usize, to: usize| String::from_utf16_lossy(&self.s[from..to.min(len)]);
        if len < 2 * CONTEXT_CHARS + 1 {
            format!("Unexpected token '{token}', \"{source}\" is not valid JSON")
        } else if pos < CONTEXT_CHARS {
            format!(
                "Unexpected token '{token}', \"{}\"... is not valid JSON",
                slice(0, pos + CONTEXT_CHARS)
            )
        } else if pos < len - CONTEXT_CHARS {
            format!(
                "Unexpected token '{token}', ...\"{}\"... is not valid JSON",
                slice(pos - CONTEXT_CHARS, pos + CONTEXT_CHARS)
            )
        } else {
            format!(
                "Unexpected token '{token}', ...\"{}\" is not valid JSON",
                slice(pos - CONTEXT_CHARS, len)
            )
        }
    }

    fn string(&mut self) -> Result<Vec<u16>, String> {
        self.pos += 1;
        let mut out = Vec::new();
        loop {
            let Some(c) = self.peek() else {
                return Err(self.at(Msg::UnterminatedString, self.pos));
            };
            match c {
                0x22 => {
                    self.pos += 1;
                    return Ok(out);
                }
                0x5c => {
                    self.pos += 1;
                    let Some(e) = self.peek() else {
                        return Err(EOS_MESSAGE.to_string());
                    };
                    let unit = match e {
                        0x22 | 0x5c | 0x2f => e,
                        0x62 => 0x08,
                        0x66 => 0x0c,
                        0x6e => 0x0a,
                        0x72 => 0x0d,
                        0x74 => 0x09,
                        0x75 => {
                            let mut unit = 0u16;
                            for _ in 0..4 {
                                self.pos += 1;
                                let Some(h) = self.peek().and_then(hex_value) else {
                                    return Err(self.at(Msg::BadUnicodeEscape, self.pos));
                                };
                                unit = unit * 16 + h;
                            }
                            unit
                        }
                        _ => return Err(self.at(Msg::BadEscapedCharacter, self.pos)),
                    };
                    out.push(unit);
                    self.pos += 1;
                }
                c if c < 0x20 => return Err(self.at(Msg::BadControlCharacter, self.pos)),
                c => {
                    out.push(c);
                    self.pos += 1;
                }
            }
        }
    }

    fn digits(&mut self) {
        while self.peek().is_some_and(is_digit) {
            self.pos += 1;
        }
    }

    fn number(&mut self) -> Result<f64, String> {
        let start = self.pos;
        if self.peek() == Some(u16::from(b'-')) {
            self.pos += 1;
        }
        match self.peek() {
            Some(0x30) => {
                self.pos += 1;
                if self.peek().is_some_and(is_digit) {
                    return Err(self.at(Msg::UnexpectedNumber, self.pos));
                }
            }
            Some(c) if is_digit(c) => self.digits(),
            _ => return Err(self.at(Msg::NoNumberAfterMinus, self.pos)),
        }
        if self.peek() == Some(u16::from(b'.')) {
            self.pos += 1;
            if !self.peek().is_some_and(is_digit) {
                return Err(self.at(Msg::UnterminatedFraction, self.pos));
            }
            self.digits();
        }
        if matches!(self.peek(), Some(0x45 | 0x65)) {
            self.pos += 1;
            if matches!(self.peek(), Some(0x2b | 0x2d)) {
                self.pos += 1;
            }
            if !self.peek().is_some_and(is_digit) {
                return Err(self.at(Msg::ExponentMissingNumber, self.pos));
            }
            self.digits();
        }
        let text: String = self.s[start..self.pos]
            .iter()
            .map(|&u| char::from(u as u8))
            .collect();
        // The JSON number grammar is a subset of Rust's; overflow is infinity.
        Ok(text.parse::<f64>().unwrap_or(f64::NAN))
    }

    fn literal(&mut self, word: &[u8]) -> Result<(), String> {
        for &b in word {
            match self.peek() {
                Some(c) if c == u16::from(b) => self.pos += 1,
                _ => return Err(self.unexpected(self.pos)),
            }
        }
        Ok(())
    }

    /// After `{` or `,`: the property name and its `:`.
    fn property_name(&mut self) -> Result<Vec<u16>, String> {
        let key = self.string()?;
        self.skip_ws();
        if self.peek() != Some(u16::from(b':')) {
            return Err(self.at(Msg::ExpectedColon, self.pos));
        }
        self.pos += 1;
        self.skip_ws();
        Ok(key)
    }

    fn run(mut self) -> Result<Doc, String> {
        let mut stack: Vec<Frame> = Vec::new();
        self.skip_ws();
        'value: loop {
            let mut done = match self.peek() {
                None => return Err(EOS_MESSAGE.to_string()),
                Some(0x22) => {
                    let s = self.string()?;
                    self.push(Item::Str(s))
                }
                Some(c) if is_digit(c) || c == u16::from(b'-') => {
                    let n = self.number()?;
                    self.push(Item::Num(n))
                }
                Some(0x74) => {
                    self.literal(b"true")?;
                    self.push(Item::Bool(true))
                }
                Some(0x66) => {
                    self.literal(b"false")?;
                    self.push(Item::Bool(false))
                }
                Some(0x6e) => {
                    self.literal(b"null")?;
                    self.push(Item::Null)
                }
                Some(0x5b) => {
                    self.pos += 1;
                    let idx = self.push(Item::Arr(Vec::new()));
                    self.skip_ws();
                    if self.peek() == Some(u16::from(b']')) {
                        self.pos += 1;
                        idx
                    } else {
                        stack.push(Frame::Arr(idx));
                        continue 'value;
                    }
                }
                Some(0x7b) => {
                    self.pos += 1;
                    let idx = self.push(Item::Obj(Vec::new()));
                    self.skip_ws();
                    match self.peek() {
                        Some(0x7d) => {
                            self.pos += 1;
                            idx
                        }
                        Some(0x22) => {
                            let key = self.property_name()?;
                            stack.push(Frame::Obj(Box::new(ObjFrame {
                                idx,
                                key,
                                seen: HashMap::new(),
                            })));
                            continue 'value;
                        }
                        _ => return Err(self.at(Msg::ExpectedPropNameOrRBrace, self.pos)),
                    }
                }
                Some(_) => return Err(self.unexpected(self.pos)),
            };
            loop {
                match stack.last_mut() {
                    None => {
                        self.skip_ws();
                        if self.pos < self.s.len() {
                            return Err(self.at(Msg::NonWhitespaceAfter, self.pos));
                        }
                        return Ok(Doc { items: self.items });
                    }
                    Some(Frame::Arr(a)) => {
                        let a = *a;
                        if let Item::Arr(children) = &mut self.items[a] {
                            children.push(done);
                        }
                        self.skip_ws();
                        match self.peek() {
                            Some(0x2c) => {
                                self.pos += 1;
                                self.skip_ws();
                                continue 'value;
                            }
                            Some(0x5d) => {
                                self.pos += 1;
                                stack.pop();
                                done = a;
                            }
                            _ => return Err(self.at(Msg::ExpectedCommaOrRBrack, self.pos)),
                        }
                    }
                    Some(Frame::Obj(frame)) => {
                        let ObjFrame { idx, key, seen } = &mut **frame;
                        let o = *idx;
                        let key = std::mem::take(key);
                        if let Item::Obj(entries) = &mut self.items[o] {
                            match seen.get(&key) {
                                Some(&at) => entries[at].1 = done,
                                None => {
                                    seen.insert(key.clone(), entries.len());
                                    entries.push((key, done));
                                }
                            }
                        }
                        self.skip_ws();
                        match self.peek() {
                            Some(0x2c) => {
                                self.pos += 1;
                                self.skip_ws();
                                if self.peek() != Some(u16::from(b'"')) {
                                    return Err(
                                        self.at(Msg::ExpectedDoubleQuotedPropertyName, self.pos)
                                    );
                                }
                                let next = self.property_name()?;
                                if let Some(Frame::Obj(frame)) = stack.last_mut() {
                                    frame.key = next;
                                }
                                continue 'value;
                            }
                            Some(0x7d) => {
                                self.pos += 1;
                                stack.pop();
                                if let Item::Obj(entries) = &mut self.items[o] {
                                    entries
                                        .sort_by_key(|(k, _)| array_index(k).unwrap_or(u64::MAX));
                                }
                                done = o;
                            }
                            _ => return Err(self.at(Msg::ExpectedCommaOrRBrace, self.pos)),
                        }
                    }
                }
            }
        }
    }
}

/// An ECMAScript array index (`0` ..= 2^32 - 2 in canonical form): such
/// keys enumerate first, ascending. The sort is stable, so the others keep
/// their insertion order behind them.
fn array_index(key: &[u16]) -> Option<u64> {
    if key.is_empty() || key.len() > 10 || !key.iter().all(|&u| is_digit(u)) {
        return None;
    }
    if key.len() > 1 && key[0] == 0x30 {
        return None;
    }
    let n = key.iter().fold(0u64, |n, &u| n * 10 + u64::from(u - 0x30));
    (n <= 4_294_967_294).then_some(n)
}

/// `JSON.stringify`'s number: `Number::toString`, `null` when non-finite.
pub fn js_number(f: f64) -> String {
    if f.is_finite() {
        super::js::number_to_string(f)
    } else {
        "null".to_string()
    }
}

/// `JSON.stringify` of a string given as UTF-16 units: a lone surrogate is
/// written as a lowercase `\uXXXX` escape.
pub(super) fn write_js_string(out: &mut String, units: &[u16]) {
    out.push('"');
    for unit in char::decode_utf16(units.iter().copied()) {
        match unit {
            Ok('"') => out.push_str("\\\""),
            Ok('\\') => out.push_str("\\\\"),
            Ok('\u{8}') => out.push_str("\\b"),
            Ok('\u{c}') => out.push_str("\\f"),
            Ok('\n') => out.push_str("\\n"),
            Ok('\r') => out.push_str("\\r"),
            Ok('\t') => out.push_str("\\t"),
            Ok(c) if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            Ok(c) => out.push(c),
            Err(lone) => {
                let _ = write!(out, "\\u{:04x}", lone.unpaired_surrogate());
            }
        }
    }
    out.push('"');
}

impl Doc {
    fn object_entries(&self, idx: usize) -> Option<&[(Vec<u16>, usize)]> {
        match &self.items[idx] {
            Item::Obj(entries) => Some(entries),
            _ => None,
        }
    }

    /// `JSON.stringify` of item `idx`.
    pub fn stringify(&self, idx: usize) -> String {
        let mut out = String::new();
        // (container, next child)
        let mut stack: Vec<(usize, usize)> = Vec::new();
        let mut next = Some(idx);
        loop {
            if let Some(i) = next.take() {
                match &self.items[i] {
                    Item::Null => out.push_str("null"),
                    Item::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
                    Item::Num(n) => out.push_str(&js_number(*n)),
                    Item::Str(s) => write_js_string(&mut out, s),
                    Item::Arr(_) => {
                        out.push('[');
                        stack.push((i, 0));
                    }
                    Item::Obj(_) => {
                        out.push('{');
                        stack.push((i, 0));
                    }
                }
            }
            let Some((container, child)) = stack.last_mut() else {
                return out;
            };
            let (len, is_arr) = match &self.items[*container] {
                Item::Arr(children) => (children.len(), true),
                Item::Obj(entries) => (entries.len(), false),
                _ => unreachable!("only containers are stacked"),
            };
            if *child == len {
                out.push(if is_arr { ']' } else { '}' });
                stack.pop();
                continue;
            }
            if *child > 0 {
                out.push(',');
            }
            let at = *child;
            *child += 1;
            next = Some(match &self.items[*container] {
                Item::Arr(children) => children[at],
                Item::Obj(entries) => {
                    write_js_string(&mut out, &entries[at].0);
                    out.push(':');
                    entries[at].1
                }
                _ => unreachable!("only containers are stacked"),
            });
        }
    }

    /// The document as a [`JsValue`] tree, built bottom-up: children always
    /// follow their container in the arena.
    pub fn into_js_value(self) -> JsValue {
        let mut built: Vec<Option<JsValue>> = Vec::with_capacity(self.items.len());
        built.resize_with(self.items.len(), || None);
        fn take(built: &mut [Option<JsValue>], i: usize) -> JsValue {
            built[i].take().expect("each child has one container")
        }
        for (i, item) in self.items.into_iter().enumerate().rev() {
            let value = match item {
                Item::Null => JsValue::Null,
                Item::Bool(b) => JsValue::Bool(b),
                Item::Num(n) => JsValue::Number(n),
                Item::Str(s) => JsValue::String(JsString::from_units(s)),
                Item::Arr(children) => {
                    JsValue::array(children.into_iter().map(|c| take(&mut built, c)).collect())
                }
                Item::Obj(entries) => JsValue::object(
                    entries
                        .into_iter()
                        .map(|(k, c)| (JsString::from_units(k), take(&mut built, c)))
                        .collect(),
                ),
            };
            built[i] = Some(value);
        }
        take(&mut built, 0)
    }

    /// A string item as the UTF-8 text Node encodes it (a lone surrogate
    /// becomes U+FFFD).
    pub fn string(&self, idx: usize) -> Option<String> {
        match &self.items[idx] {
            Item::Str(s) => Some(String::from_utf16_lossy(s)),
            _ => None,
        }
    }

    /// The shared `{ value }` checks, in ajv's keyword order: `type`,
    /// `required`, `additionalProperties`. Returns the `value` item.
    fn envelope(&self) -> Result<usize, Vec<AjvError>> {
        let Some(entries) = self.object_entries(0) else {
            return Err(vec![type_error("", "#/type", "object")]);
        };
        let value_key: Vec<u16> = "value".encode_utf16().collect();
        let Some(&(_, value)) = entries.iter().find(|(k, _)| *k == value_key) else {
            return Err(vec![AjvError {
                instance_path: String::new(),
                schema_path: "#/required",
                keyword: "required",
                params: json!({ "missingProperty": "value" }),
                message: "must have required property 'value'".to_string(),
            }]);
        };
        if let Some((extra, _)) = entries.iter().find(|(k, _)| *k != value_key) {
            return Err(vec![AjvError {
                instance_path: String::new(),
                schema_path: "#/additionalProperties",
                keyword: "additionalProperties",
                params: json!({ "additionalProperty": String::from_utf16_lossy(extra) }),
                message: "must NOT have additional properties".to_string(),
            }]);
        }
        Ok(value)
    }

    /// `UpsertEnvStorageRequestSchema`: `{ value: string }`.
    pub fn validate_env_body(&self) -> Result<usize, Vec<AjvError>> {
        let value = self.envelope()?;
        if !matches!(self.items[value], Item::Str(_)) {
            return Err(vec![type_error(
                "/value",
                "#/properties/value/type",
                "string",
            )]);
        }
        Ok(value)
    }

    /// `UpsertStorageRequestSchema`: `{ value: JSONValue }`, where only a
    /// non-finite number can fail the recursive `anyOf`.
    pub fn validate_storage_body(&self) -> Result<usize, Vec<AjvError>> {
        let value = self.envelope()?;
        let mut invalid = vec![false; self.items.len()];
        // Children always follow their container in the arena.
        for i in (0..self.items.len()).rev() {
            invalid[i] = match &self.items[i] {
                Item::Num(n) => !n.is_finite(),
                Item::Arr(children) => children.iter().any(|&c| invalid[c]),
                Item::Obj(entries) => entries.iter().any(|&(_, c)| invalid[c]),
                _ => false,
            };
        }
        if !invalid[value] {
            return Ok(value);
        }
        Err(self.any_of_cascade(value, &invalid))
    }

    /// ajv's errors for the first failing path (`allErrors: false`): at each
    /// level the scalar branches fail, the matching container branch
    /// descends into its first invalid child, then the remaining branch and
    /// the `anyOf` itself fail.
    fn any_of_cascade(&self, value: usize, invalid: &[bool]) -> Vec<AjvError> {
        let mut errors = Vec::new();
        let mut unwind: Vec<(String, bool)> = Vec::new();
        let (mut cur, mut path) = (value, "/value".to_string());
        loop {
            if unwind.len() >= MAX_ERROR_DEPTH {
                return vec![any_of_error("/value")];
            }
            for t in ["string", "number", "boolean", "null"] {
                errors.push(branch_error(&path, t));
            }
            match &self.items[cur] {
                Item::Arr(children) => {
                    let (i, &child) = children
                        .iter()
                        .enumerate()
                        .find(|(_, c)| invalid[**c])
                        .expect("an invalid array has an invalid item");
                    let child_path = format!("{path}/{i}");
                    unwind.push((std::mem::replace(&mut path, child_path), true));
                    cur = child;
                }
                Item::Obj(entries) => {
                    errors.push(branch_error(&path, "array"));
                    let (key, child) = entries
                        .iter()
                        .find(|(_, c)| invalid[*c])
                        .expect("an invalid object has an invalid property");
                    let escaped = String::from_utf16_lossy(key)
                        .replace('~', "~0")
                        .replace('/', "~1");
                    let child_path = format!("{path}/{escaped}");
                    unwind.push((std::mem::replace(&mut path, child_path), false));
                    cur = *child;
                }
                _ => {
                    errors.push(branch_error(&path, "array"));
                    errors.push(branch_error(&path, "object"));
                    errors.push(any_of_error(&path));
                    break;
                }
            }
        }
        while let Some((path, was_array)) = unwind.pop() {
            if was_array {
                errors.push(branch_error(&path, "object"));
            }
            errors.push(any_of_error(&path));
        }
        errors
    }
}

fn type_error(path: &str, schema_path: &'static str, t: &str) -> AjvError {
    AjvError {
        instance_path: path.to_string(),
        schema_path,
        keyword: "type",
        params: json!({ "type": t }),
        message: format!("must be {t}"),
    }
}

fn branch_error(path: &str, t: &'static str) -> AjvError {
    let schema_path = match t {
        "string" => "#/anyOf/0/type",
        "number" => "#/anyOf/1/type",
        "boolean" => "#/anyOf/2/type",
        "null" => "#/anyOf/3/type",
        "array" => "#/anyOf/4/type",
        _ => "#/anyOf/5/type",
    };
    type_error(path, schema_path, t)
}

fn any_of_error(path: &str) -> AjvError {
    AjvError {
        instance_path: path.to_string(),
        schema_path: "#/anyOf",
        keyword: "anyOf",
        params: json!({}),
        message: "must match a schema in anyOf".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(s: &str) -> Result<Doc, String> {
        parse(&decode_body(s.as_bytes()))
    }

    fn roundtrip(s: &str) -> String {
        parse_str(s).unwrap().stringify(0)
    }

    /// Node 24 `JSON.parse` messages, captured with `node -e`.
    #[test]
    fn parse_errors_match_v8() {
        for (input, expected) in [
            ("", "Unexpected end of JSON input"),
            (" ", "Unexpected end of JSON input"),
            ("x", "Unexpected token 'x', \"x\" is not valid JSON"),
            ("xyz", "Unexpected token 'x', \"xyz\" is not valid JSON"),
            ("tru", "Unexpected end of JSON input"),
            (
                "truex",
                "Unexpected non-whitespace character after JSON at position 4 (line 1 column 5)",
            ),
            (
                "{",
                "Expected property name or '}' in JSON at position 1 (line 1 column 2)",
            ),
            (
                "{\"a\"",
                "Expected ':' after property name in JSON at position 4 (line 1 column 5)",
            ),
            ("{\"a\":", "Unexpected end of JSON input"),
            (
                "{\"a\":1",
                "Expected ',' or '}' after property value in JSON at position 6 (line 1 column 7)",
            ),
            (
                "{\"a\":1,",
                "Expected double-quoted property name in JSON at position 7 (line 1 column 8)",
            ),
            (
                "{\"a\" 1}",
                "Expected ':' after property name in JSON at position 5 (line 1 column 6)",
            ),
            (
                "{a:1}",
                "Expected property name or '}' in JSON at position 1 (line 1 column 2)",
            ),
            ("[", "Unexpected end of JSON input"),
            (
                "[1",
                "Expected ',' or ']' after array element in JSON at position 2 (line 1 column 3)",
            ),
            ("[1,", "Unexpected end of JSON input"),
            (
                "[1 2]",
                "Expected ',' or ']' after array element in JSON at position 3 (line 1 column 4)",
            ),
            ("[1,]", "Unexpected token ']', \"[1,]\" is not valid JSON"),
            (
                "{\"value\":1,}",
                "Expected double-quoted property name in JSON at position 11 (line 1 column 12)",
            ),
            (
                "\"abc",
                "Unterminated string in JSON at position 4 (line 1 column 5)",
            ),
            (
                "\"a\\x\"",
                "Bad escaped character in JSON at position 3 (line 1 column 4)",
            ),
            (
                "\"a\\u12g4\"",
                "Bad Unicode escape in JSON at position 6 (line 1 column 7)",
            ),
            (
                "\"a\u{1}\"",
                "Bad control character in string literal in JSON at position 2 (line 1 column 3)",
            ),
            (
                "-",
                "No number after minus sign in JSON at position 1 (line 1 column 2)",
            ),
            (
                "1.e3",
                "Unterminated fractional number in JSON at position 2 (line 1 column 3)",
            ),
            (
                "1e+",
                "Exponent part is missing a number in JSON at position 3 (line 1 column 4)",
            ),
            (
                "-01",
                "Unexpected number in JSON at position 2 (line 1 column 3)",
            ),
            ("\"\\", "Unexpected end of JSON input"),
            (
                "\"\\u",
                "Bad Unicode escape in JSON at position 3 (line 1 column 4)",
            ),
            (
                "tr1",
                "Unexpected number in JSON at position 2 (line 1 column 3)",
            ),
            (
                "tr\"",
                "Unexpected string in JSON at position 2 (line 1 column 3)",
            ),
            (
                "{\"a\":nulx}",
                "Unexpected token 'x', \"{\"a\":nulx}\" is not valid JSON",
            ),
            ("NaN", "\"NaN\" is not valid JSON"),
            ("[object Object]", "\"[object Object]\" is not valid JSON"),
            (" NaN", "Unexpected token 'N', \" NaN\" is not valid JSON"),
            (
                "\u{a0}{}",
                "Unexpected token '\u{a0}', \"\u{a0}{}\" is not valid JSON",
            ),
            (
                "{\"value\":1}\r\n\r\nx",
                "Unexpected non-whitespace character after JSON at position 15 (line 3 column 1)",
            ),
            (
                "{\"a\":1}\rx",
                "Unexpected non-whitespace character after JSON at position 8 (line 2 column 1)",
            ),
            (
                "{\"a\":\n1\n,x}",
                "Expected double-quoted property name in JSON at position 9 (line 3 column 2)",
            ),
            (
                "{\"value\": \"\u{1F600}\" x}",
                "Expected ',' or '}' after property value in JSON at position 15 (line 1 column 16)",
            ),
            (
                "x1111111111111111111",
                "Unexpected token 'x', \"x1111111111111111111\" is not valid JSON",
            ),
            (
                "x11111111111111111111",
                "Unexpected token 'x', \"x111111111\"... is not valid JSON",
            ),
            (
                "[1,1,1,1,1,1,1,1,1,1,x,2,2,2,2]",
                "Unexpected token 'x', ...\"1,1,1,1,1,x,2,2,2,2]\" is not valid JSON",
            ),
            (
                "[1,1,1,1,1,1,1,1,1,1,x,2,2,2,2,2]",
                "Unexpected token 'x', ...\"1,1,1,1,1,x,2,2,2,2,\"... is not valid JSON",
            ),
        ] {
            assert_eq!(
                parse_str(input).err().as_deref(),
                Some(expected),
                "{input:?}"
            );
        }
        // The body decoder drops a leading BOM, as undici's `json()` does.
        assert!(parse_str("\u{feff}{}").is_ok());
    }

    #[test]
    fn stringify_matches_node() {
        for (input, expected) in [
            ("1.0", "1"),
            ("1e2", "100"),
            ("1E+2", "100"),
            ("-0", "0"),
            ("-0.0", "0"),
            ("12345678901234567890", "12345678901234567000"),
            ("9007199254740993", "9007199254740992"),
            ("1e21", "1e+21"),
            ("1e20", "100000000000000000000"),
            ("1e-7", "1e-7"),
            ("0.000001", "0.000001"),
            ("123e-20", "1.23e-18"),
            ("5e-324", "5e-324"),
            ("1.7976931348623157e308", "1.7976931348623157e+308"),
            ("-1.25e-5", "-0.0000125"),
            ("0.1", "0.1"),
            ("1e-400", "0"),
            (
                "{\"b\":1,\"2\":2,\"1\":3,\"b\":4}",
                "{\"1\":3,\"2\":2,\"b\":4}",
            ),
            (
                "{\"4294967295\":1,\"4294967294\":2,\"01\":3,\"0\":4}",
                "{\"0\":4,\"4294967294\":2,\"4294967295\":1,\"01\":3}",
            ),
            ("[1, \"two\" , {\"a\" : [ ]}]", "[1,\"two\",{\"a\":[]}]"),
            ("\"\\ud800\"", "\"\\ud800\""),
            ("\"\\uD83D\\uDE00\"", "\"\u{1F600}\""),
            (
                "\"\\u0000\\u001f\\u007f\\u2028\\/\"",
                "\"\\u0000\\u001f\u{7f}\u{2028}/\"",
            ),
            ("\"\\b\\f\\n\\r\\t\\\"\\\\\"", "\"\\b\\f\\n\\r\\t\\\"\\\\\""),
            ("{\"__proto__\":1}", "{\"__proto__\":1}"),
        ] {
            assert_eq!(roundtrip(input), expected, "{input:?}");
        }
    }

    #[test]
    fn deep_nesting_is_iterative() {
        let depth = 200_000;
        let text = format!("{}1{}", "[".repeat(depth), "]".repeat(depth));
        let doc = parse_str(&text).unwrap();
        assert_eq!(doc.stringify(0), text);
        let text = format!("{}1{}", "{\"a\":".repeat(depth), "}".repeat(depth));
        assert_eq!(parse_str(&text).unwrap().stringify(0), text);
    }

    /// A body may open one container per byte: an open array costs a
    /// two-word frame, and a maximum-size world upsert (512 KiB value plus
    /// the envelope slack) parses whether it closes or not.
    #[test]
    fn a_maximum_size_nested_body_is_bounded() {
        assert!(std::mem::size_of::<Frame>() <= 16);
        let max_body = 524_288 + 1024;
        assert_eq!(
            parse_str(&"[".repeat(max_body)).err().as_deref(),
            Some(EOS_MESSAGE)
        );
        let depth = (max_body - "{\"value\":12}".len()) / 2;
        let text = format!("{{\"value\":{}12{}}}", "[".repeat(depth), "]".repeat(depth));
        assert_eq!(text.len(), max_body);
        let doc = parse_str(&text).unwrap();
        let value = doc.validate_storage_body().unwrap();
        assert_eq!(doc.stringify(value).len(), 2 * depth + 2);
    }

    fn storage_errors(body: &str) -> Value {
        serde_json::to_value(
            parse_str(body)
                .unwrap()
                .validate_storage_body()
                .unwrap_err(),
        )
        .unwrap()
    }

    #[test]
    fn schema_errors_match_ajv() {
        assert_eq!(
            storage_errors("[]"),
            json!([{"instancePath":"","schemaPath":"#/type","keyword":"type","params":{"type":"object"},"message":"must be object"}])
        );
        assert_eq!(
            storage_errors("{\"extra\":2}"),
            json!([{"instancePath":"","schemaPath":"#/required","keyword":"required","params":{"missingProperty":"value"},"message":"must have required property 'value'"}])
        );
        assert_eq!(
            storage_errors("{\"value\":1e400,\"x\":1}"),
            json!([{"instancePath":"","schemaPath":"#/additionalProperties","keyword":"additionalProperties","params":{"additionalProperty":"x"},"message":"must NOT have additional properties"}])
        );
        let t = |p: &str, i: u8, t: &str| json!({"instancePath":p,"schemaPath":format!("#/anyOf/{i}/type"),"keyword":"type","params":{"type":t},"message":format!("must be {t}")});
        let any = |p: &str| json!({"instancePath":p,"schemaPath":"#/anyOf","keyword":"anyOf","params":{},"message":"must match a schema in anyOf"});
        let scalars = |p: &str| {
            vec![
                t(p, 0, "string"),
                t(p, 1, "number"),
                t(p, 2, "boolean"),
                t(p, 3, "null"),
            ]
        };
        let leaf = |p: &str| {
            let mut v = scalars(p);
            v.extend([t(p, 4, "array"), t(p, 5, "object"), any(p)]);
            v
        };
        assert_eq!(storage_errors("{\"value\":1e400}"), json!(leaf("/value")));
        let mut array = scalars("/value");
        array.extend(leaf("/value/1"));
        array.extend([t("/value", 5, "object"), any("/value")]);
        assert_eq!(storage_errors("{\"value\":[1,-1e400]}"), json!(array));
        let mut object = scalars("/value");
        object.push(t("/value", 4, "array"));
        object.extend(leaf("/value/a~1b~0c"));
        object.push(any("/value"));
        assert_eq!(
            storage_errors("{\"value\":{\"ok\":1,\"a/b~c\":1e999}}"),
            json!(object)
        );

        let deep = format!(
            "{{\"value\":{}1e400{}}}",
            "[".repeat(MAX_ERROR_DEPTH + 1),
            "]".repeat(MAX_ERROR_DEPTH + 1)
        );
        assert_eq!(storage_errors(&deep), json!([any("/value")]));

        let env = parse_str("{\"value\":5}")
            .unwrap()
            .validate_env_body()
            .unwrap_err();
        assert_eq!(
            serde_json::to_value(env).unwrap(),
            json!([{"instancePath":"/value","schemaPath":"#/properties/value/type","keyword":"type","params":{"type":"string"},"message":"must be string"}])
        );
    }

    #[test]
    fn env_strings_are_node_utf8() {
        let doc = parse_str("{\"value\":\"a\\ud800b\"}").unwrap();
        let value = doc.validate_env_body().unwrap();
        assert_eq!(doc.string(value).unwrap(), "a\u{fffd}b");
    }
}
