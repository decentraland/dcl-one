//! The worlds mount's Content-Type sniff: worlds-content-server runs `file-type@16.5.4`
//! `fromStream` (`core.js`, the version its `yarn.lock` pins) over the stored bytes
//! (`content-file-handler.ts`). This ports its checks, in its order, for the formats a scene
//! carries; any other file answers nothing. The stream's size is unknown to `file-type`, so a
//! check that reads past the end answers nothing too. `None` is `application/octet-stream`.
//!
//! The loops that walk a file (ID3 tags, PNG chunks, EBML header elements) stop after more turns
//! than a real file takes and answer nothing, where `file-type` walks on to the end.
//!
//! `tests/fixtures/conformance/worlds/worlds-sniff.tsv` holds upstream's answers for a sample of
//! each format; `the_worlds_sniff_answers_as_file_type` replays it.

use bytes::Bytes;

use crate::storage::{OpenedContent, StorageError};

/// `file-type`'s `minimumBytes`, the most its signature checks read at one position. It is also
/// the window a ranged GET sniffs (`MIME_SNIFF_BYTES` in `content-file-handler.ts`).
pub(crate) const SAMPLE: u64 = 4100;
/// `%PDF`: how far `file-type` reads (from byte 1350) looking for Illustrator's marker.
const PDF_SCAN: u64 = 10 * 1024 * 1024;
/// Reads past the sample are made in blocks of this size.
const BLOCK: u64 = 64 * 1024;
/// The most ID3 tags skipped before the sniff gives up.
const MAX_ID3_TAGS: usize = 16;
/// The most PNG chunks walked looking for `IDAT` or `acTL`.
const MAX_PNG_CHUNKS: usize = 256;
/// The most EBML header elements read looking for `DocType`.
const MAX_EBML_ELEMENTS: u64 = 64;
/// The longest `DocType` that can match (`matroska`); a longer one is read by `file-type` only to
/// be compared and fail.
const MAX_DOC_TYPE: u64 = 8;

/// The bytes `file-type` reads: up to `len` bytes from `start`, fewer only at the end.
pub(crate) trait Source {
    async fn read_at(&mut self, start: u64, len: u64) -> Result<Bytes, StorageError>;
}

impl Source for &[u8] {
    async fn read_at(&mut self, start: u64, len: u64) -> Result<Bytes, StorageError> {
        let from = (start.min(self.len() as u64)) as usize;
        let to = (start.saturating_add(len).min(self.len() as u64)) as usize;
        Ok(Bytes::copy_from_slice(&self[from..to]))
    }
}

/// A stored file cut at `end`, read through a cache of the last block read.
pub(crate) struct Window<'a> {
    opened: &'a mut OpenedContent,
    end: u64,
    cached: (u64, Bytes),
    /// Whether the file was read anywhere but its first block.
    moved: bool,
}

impl<'a> Window<'a> {
    pub(crate) fn new(opened: &'a mut OpenedContent, end: u64) -> Self {
        let end = end.min(opened.size);
        Self {
            opened,
            end,
            cached: (0, Bytes::new()),
            moved: false,
        }
    }

    /// The file's first `SAMPLE` bytes (or fewer), with the reader positioned after them, as
    /// `OpenedContent::into_body` expects.
    pub(crate) async fn into_head(self) -> Result<Bytes, StorageError> {
        let (start, cached) = self.cached;
        if start == 0 && !self.moved && cached.len() as u64 == SAMPLE.min(self.end) {
            return Ok(cached);
        }
        self.opened.read_range(0, SAMPLE - 1).await
    }
}

impl Source for Window<'_> {
    async fn read_at(&mut self, start: u64, len: u64) -> Result<Bytes, StorageError> {
        let end = start.saturating_add(len).min(self.end);
        if start >= end {
            return Ok(Bytes::new());
        }
        let (at, cached) = &self.cached;
        if *at <= start && end <= at + cached.len() as u64 {
            return Ok(cached.slice((start - at) as usize..(end - at) as usize));
        }
        // The first read is the sample at 0; later ones fetch a block from where they start.
        let size = if start == 0 && self.cached.1.is_empty() {
            SAMPLE.max(len)
        } else {
            self.moved = true;
            BLOCK.max(len)
        };
        let last = start.saturating_add(size).min(self.end) - 1;
        let block = self.opened.read_range(start, last).await?;
        self.cached = (start, block);
        let got = (end - start).min(self.cached.1.len() as u64) as usize;
        Ok(self.cached.1.slice(..got))
    }
}

/// `strtok3`'s tokenizer over a [`Source`]: peeks leave the position, reads and ignores move it.
struct Tokenizer<'s, S> {
    src: &'s mut S,
    pos: u64,
}

impl<S: Source> Tokenizer<'_, S> {
    /// `peekBuffer(.., {mayBeLess: true})` into a zeroed buffer of `len`.
    async fn peek(&mut self, len: u64) -> Result<Vec<u8>, StorageError> {
        let got = self.src.read_at(self.pos, len).await?;
        let mut buf = vec![0; len as usize];
        buf[..got.len()].copy_from_slice(&got);
        Ok(buf)
    }

    /// `readBuffer`/`readToken`: `None` where `file-type` throws `EndOfStreamError`.
    async fn read(&mut self, len: u64) -> Result<Option<Bytes>, StorageError> {
        let got = self.src.read_at(self.pos, len).await?;
        if (got.len() as u64) < len {
            return Ok(None);
        }
        self.pos += len;
        Ok(Some(got))
    }

    /// `ignore`: `false` where it runs past the end (`EndOfStreamError`).
    async fn ignore(&mut self, len: u64) -> Result<bool, StorageError> {
        if len > 0 && self.src.read_at(self.pos + len - 1, 1).await?.is_empty() {
            return Ok(false);
        }
        self.pos += len;
        Ok(true)
    }
}

/// `fromStream(stream)?.mime`, `None` for no answer or a throw.
pub(crate) async fn file_type(src: &mut impl Source) -> Result<Option<&'static str>, StorageError> {
    let mut tok = Tokenizer { src, pos: 0 };
    // Each turn is one `_fromTokenizer` call; an ID3 tag skips itself and starts another.
    for _ in 0..=MAX_ID3_TAGS {
        let b = tok.peek(SAMPLE).await?;
        let at = |offset: usize, sig: &[u8]| b[offset..offset + sig.len()] == *sig;
        let found = |mime| Ok(Some(mime));

        if at(0, b"BM") {
            return found("image/bmp");
        }
        if at(0, &[0xFF, 0xD8, 0xFF]) {
            return found("image/jpeg");
        }
        if at(0, &[0x1F, 0x8B, 0x08]) {
            return found("application/gzip");
        }
        if at(0, b"ID3") {
            let Some(size) = tok.skip_read(6, 4).await? else {
                return Ok(None);
            };
            // `uint32SyncSafeToken`: only the last byte is masked.
            let len = (i32::from(size[3]) & 0x7F)
                | (i32::from(size[2]) << 7)
                | (i32::from(size[1]) << 14)
                | (i32::from(size[0]) << 21);
            if !tok.ignore(len as u64).await? {
                return Ok(None);
            }
            continue;
        }
        if at(0, b"GIF") {
            return found("image/gif");
        }
        if at(8, b"WEBP") {
            return found("image/webp");
        }
        if at(0, b"FORM") {
            return found("audio/aiff");
        }
        // Office, EPUB, ODF, 3MF and XPI archives are told apart by their entries; not ported.
        if at(0, &[0x50, 0x4B, 0x03, 0x04]) {
            return found("application/zip");
        }
        if at(0, b"OggS") {
            let Some(codec) = tok.skip_read(28, 8).await? else {
                return Ok(None);
            };
            let is = |sig: &[u8]| codec.starts_with(sig);
            return found(if is(b"OpusHead") {
                "audio/opus"
            } else if is(b"\x80theora") || is(b"\x01video\0") {
                "video/ogg"
            } else if is(b"\x7FFLAC") || is(b"Speex  ") || is(b"\x01vorbis") {
                "audio/ogg"
            } else {
                "application/ogg"
            });
        }
        if at(0, b"PK") && matches!(b[2], 0x03 | 0x05 | 0x07) && matches!(b[3], 0x04 | 0x06 | 0x08)
        {
            return found("application/zip");
        }
        if at(4, b"ftyp") && b[8] & 0x60 != 0 {
            return found(iso_bmff_brand(&b[8..12]));
        }
        if at(0, b"MThd") {
            return found("audio/midi");
        }
        let sfnt = at(4, &[0x00, 0x01, 0x00, 0x00]) || at(4, b"OTTO");
        if at(0, b"wOFF") && sfnt {
            return found("font/woff");
        }
        if at(0, b"wOF2") && sfnt {
            return found("font/woff2");
        }
        if at(0, b"fLaC") {
            return found("audio/x-flac");
        }
        if at(0, b"%PDF") {
            if !tok.ignore(1350).await? {
                return Ok(None);
            }
            return Ok(illustrator(&mut tok).await?.map(|ai| {
                if ai {
                    "application/postscript"
                } else {
                    "application/pdf"
                }
            }));
        }
        if at(0, &[0x00, 0x61, 0x73, 0x6D]) {
            return found("application/wasm");
        }
        if at(0, &[0x49, 0x49, 0x2A, 0x00]) {
            return found(tiff_le(&b));
        }
        if at(0, &[0x4D, 0x4D, 0x00, 0x2A]) {
            return found("image/tiff");
        }
        if at(0, &[0x1A, 0x45, 0xDF, 0xA3]) {
            return Ok(match ebml_doc_type(&mut tok).await? {
                Some(doc) if doc == b"webm" => Some("video/webm"),
                Some(doc) if doc == b"matroska" => Some("video/x-matroska"),
                _ => None,
            });
        }
        if at(0, b"RIFF") {
            if at(8, b"AVI") {
                return found("video/vnd.avi");
            }
            if at(8, b"WAVE") {
                return found("audio/vnd.wave");
            }
            if at(8, b"QLCM") {
                return found("audio/qcelp");
            }
        }
        if at(0, b"OTTO\0") {
            return found("font/otf");
        }
        if at(0, &[0x00, 0x00, 0x01, 0xBA]) {
            if b[4] & 0xF1 == 0x21 {
                return found("video/MP1S");
            }
            if b[4] & 0xC4 == 0x44 {
                return found("video/MP2P");
            }
        }
        if at(0, b"<?xml ") {
            return found("application/xml");
        }
        if at(0, &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
            tok.pos += 8;
            return png(&mut tok).await;
        }
        if at(0, b"glTF\x02\x00\x00\x00") {
            return found("model/gltf-binary");
        }
        if at(4, b"free") || at(4, b"mdat") || at(4, b"moov") || at(4, b"wide") {
            return found("video/quicktime");
        }
        if at(
            0,
            &[
                0xAB, 0x4B, 0x54, 0x58, 0x20, 0x31, 0x31, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
            ],
        ) {
            return found("image/ktx");
        }
        if at(0, &[0x00, 0x00, 0x01, 0xBA]) || at(0, &[0x00, 0x00, 0x01, 0xB3]) {
            return found("video/mpeg");
        }
        if at(0, &[0x00, 0x01, 0x00, 0x00, 0x00]) {
            return found("font/ttf");
        }
        if at(0, &[0x00, 0x00, 0x01, 0x00]) || at(0, &[0x00, 0x00, 0x02, 0x00]) {
            return found("image/x-icon");
        }
        if b[4] == 0x47 && (b[192] == 0x47 || b[196] == 0x47) {
            return found("video/mp2t");
        }
        if at(
            0,
            b"\xFF\xFE\xFF\x0ES\0k\0e\0t\0c\0h\0U\0p\0 \0M\0o\0d\0e\0l\0",
        ) {
            return found("application/vnd.sketchup.skp");
        }
        return Ok(mpeg_audio(b[0], b[1]));
    }
    Ok(None)
}

/// The `ftyp` major brand read as Latin-1, the first NUL as a space, trimmed as JavaScript's
/// `trim()` does (which keeps U+0085, unlike Rust's).
fn iso_bmff_brand(raw: &[u8]) -> &'static str {
    let brand: String = raw.iter().map(|&c| char::from(c)).collect();
    let brand = brand.replacen('\0', " ", 1);
    let js_space = |c: char| matches!(c, '\t' | '\n' | '\x0B' | '\x0C' | '\r' | ' ' | '\u{A0}');
    match brand.trim_matches(js_space) {
        "avif" => "image/avif",
        "mif1" => "image/heif",
        "msf1" => "image/heif-sequence",
        "heic" | "heix" => "image/heic",
        "hevc" | "hevx" => "image/heic-sequence",
        "qt" => "video/quicktime",
        "M4V" | "M4VH" | "M4VP" => "video/x-m4v",
        "M4B" | "F4A" | "F4B" => "audio/mp4",
        "M4A" => "audio/x-m4a",
        "crx" => "image/x-canon-cr3",
        b if b.starts_with("3g2") => "video/3gpp2",
        b if b.starts_with("3g") => "video/3gpp",
        _ => "video/mp4",
    }
}

/// Little-endian TIFF and the camera raws built on it.
fn tiff_le(b: &[u8]) -> &'static str {
    let at = |offset: usize, sig: &[u8]| b[offset..offset + sig.len()] == *sig;
    if at(8, b"CR") {
        "image/x-canon-cr2"
    } else if at(8, &[0x1C, 0x00, 0xFE, 0x00]) || at(8, &[0x1F, 0x00, 0x0B, 0x00]) {
        "image/x-nikon-nef"
    } else if at(4, &[0x08, 0x00, 0x00, 0x00])
        && (at(8, &[0x2D, 0x00, 0xFE, 0x00]) || at(8, &[0x27, 0x00, 0xFE, 0x00]))
    {
        "image/x-adobe-dng"
    } else if (at(4, &[0x10, 0xFB, 0x86, 0x01]) || at(4, &[0x08, 0x00, 0x00, 0x00]))
        && at(
            9,
            &[
                0x00, 0xFE, 0x00, 0x04, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x03,
                0x01,
            ],
        )
    {
        "image/x-sony-arw"
    } else {
        "image/tiff"
    }
}

/// Whether the 10 MiB from the tokenizer's position carry `AIPrivateData`; `None` when nothing is
/// left to read (`readBuffer` throws).
async fn illustrator<S: Source>(tok: &mut Tokenizer<'_, S>) -> Result<Option<bool>, StorageError> {
    const MARK: &[u8] = b"AIPrivateData";
    let mut scanned = 0;
    let mut tail = Vec::new();
    while scanned < PDF_SCAN {
        let chunk = tok
            .src
            .read_at(tok.pos + scanned, BLOCK.min(PDF_SCAN - scanned))
            .await?;
        if chunk.is_empty() {
            return Ok((scanned > 0).then_some(false));
        }
        tail.extend_from_slice(&chunk);
        if tail.windows(MARK.len()).any(|w| w == MARK) {
            return Ok(Some(true));
        }
        tail.drain(..tail.len().saturating_sub(MARK.len() - 1));
        scanned += chunk.len() as u64;
    }
    Ok(Some(false))
}

/// The EBML header's `DocType`, read as `file-type` does: the root element's length is taken as
/// a count of children. A `DocType` longer than [`MAX_DOC_TYPE`] cannot match and is not read.
async fn ebml_doc_type<S: Source>(
    tok: &mut Tokenizer<'_, S>,
) -> Result<Option<Vec<u8>>, StorageError> {
    let Some((_, children)) = ebml_element(tok).await? else {
        return Ok(None);
    };
    for _ in 0..children.min(MAX_EBML_ELEMENTS) {
        let Some((id, len)) = ebml_element(tok).await? else {
            return Ok(None);
        };
        if id == 0x4282 {
            if len > MAX_DOC_TYPE {
                return Ok(None);
            }
            return Ok(tok.read(len).await?.map(|doc| doc.to_vec()));
        }
        if !tok.ignore(len).await? {
            return Ok(None);
        }
    }
    Ok(None)
}

/// One element header: its id and payload length. `None` at the end of the stream, or for an id
/// wider than six bytes (`readUIntBE` throws).
async fn ebml_element<S: Source>(
    tok: &mut Tokenizer<'_, S>,
) -> Result<Option<(u64, u64)>, StorageError> {
    let Some(id) = ebml_field(tok).await? else {
        return Ok(None);
    };
    let Some(mut len) = ebml_field(tok).await? else {
        return Ok(None);
    };
    if id.len() > 6 {
        return Ok(None);
    }
    len[0] ^= 0x80u8.checked_shr(len.len() as u32 - 1).unwrap_or(0);
    let len = &len[len.len() - len.len().min(6)..];
    let be = |bytes: &[u8]| bytes.iter().fold(0u64, |n, &b| (n << 8) | u64::from(b));
    Ok(Some((be(&id), be(len))))
}

/// A variable-length field: its width is one plus the leading zero bits of its first byte.
async fn ebml_field<S: Source>(
    tok: &mut Tokenizer<'_, S>,
) -> Result<Option<Vec<u8>>, StorageError> {
    let Some(first) = tok.peek_byte().await? else {
        return Ok(None);
    };
    let width = (first.leading_zeros() as u64 + 1).min(9);
    Ok(tok.read(width).await?.map(|field| field.to_vec()))
}

impl<S: Source> Tokenizer<'_, S> {
    /// `ignore(skip)` then `read(len)`.
    async fn skip_read(&mut self, skip: u64, len: u64) -> Result<Option<Bytes>, StorageError> {
        if !self.ignore(skip).await? {
            return Ok(None);
        }
        self.read(len).await
    }

    /// `peekNumber(UINT8)`: `None` at the end of the stream.
    async fn peek_byte(&mut self) -> Result<Option<u8>, StorageError> {
        Ok(self.src.read_at(self.pos, 1).await?.first().copied())
    }
}

/// PNG: an `acTL` chunk before the first `IDAT` is APNG. A chunk header past the end, a
/// negative length, or more than [`MAX_PNG_CHUNKS`] chunks before either answers nothing.
async fn png<S: Source>(tok: &mut Tokenizer<'_, S>) -> Result<Option<&'static str>, StorageError> {
    for _ in 0..MAX_PNG_CHUNKS {
        let Some(length) = tok.read(4).await? else {
            return Ok(None);
        };
        let Some(kind) = tok.read(4).await? else {
            return Ok(None);
        };
        let length = i32::from_be_bytes([length[0], length[1], length[2], length[3]]);
        if length < 0 {
            return Ok(None);
        }
        match &kind[..] {
            b"IDAT" => return Ok(Some("image/png")),
            b"acTL" => return Ok(Some("image/apng")),
            _ if !tok.ignore(length as u64 + 4).await? => return Ok(None),
            _ => {}
        }
    }
    Ok(None)
}

/// The MPEG audio sync word: ADTS is AAC, layers 1 to 3 are MPEG audio.
fn mpeg_audio(b0: u8, b1: u8) -> Option<&'static str> {
    if b0 != 0xFF || b1 & 0xE0 != 0xE0 {
        return None;
    }
    if b1 & 0x16 == 0x10 {
        return Some("audio/aac");
    }
    match b1 & 0x06 {
        0x02 | 0x04 | 0x06 => Some("audio/mpeg"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UPSTREAM: &str = include_str!("../../tests/fixtures/conformance/worlds/worlds-sniff.tsv");

    async fn sniffed(mut bytes: &[u8]) -> &'static str {
        file_type(&mut bytes)
            .await
            .unwrap()
            .unwrap_or("application/octet-stream")
    }

    /// Every sample `capture-sniff.cjs` ran through worlds-content-server, whole and cut at the
    /// window a ranged GET sniffs.
    #[tokio::test]
    async fn the_worlds_sniff_answers_as_file_type() {
        let mut rows = 0;
        for line in UPSTREAM.lines().filter(|l| !l.starts_with('#')) {
            let [name, length, head, whole, ranged] = line.split('\t').collect::<Vec<_>>()[..]
            else {
                panic!("a row of five fields: {line}");
            };
            let mut bytes = hex::decode(head).unwrap();
            bytes.resize(length.parse().unwrap(), 0);
            assert_eq!(sniffed(&bytes).await, whole, "{name}");
            let window = &bytes[..bytes.len().min(SAMPLE as usize)];
            assert_eq!(sniffed(window).await, ranged, "{name} ranged");
            rows += 1;
        }
        assert!(rows > 60, "{rows} rows");
    }

    /// A [`Source`] that records the largest read it served.
    struct Counting<'a> {
        bytes: &'a [u8],
        largest: u64,
        reads: usize,
    }

    impl Source for Counting<'_> {
        async fn read_at(&mut self, start: u64, len: u64) -> Result<Bytes, StorageError> {
            let got = self.bytes.read_at(start, len).await?;
            self.largest = self.largest.max(got.len() as u64);
            self.reads += 1;
            Ok(got)
        }
    }

    async fn counted(bytes: &[u8]) -> (Option<&'static str>, Counting<'_>) {
        let mut src = Counting {
            bytes,
            largest: 0,
            reads: 0,
        };
        let mime = file_type(&mut src).await.unwrap();
        (mime, src)
    }

    /// An EBML `DocType` whose length field says 2^44 bytes: nothing past the sample is read.
    #[tokio::test]
    async fn a_huge_ebml_doc_type_length_is_not_read() {
        let mut bytes = b"\x1A\x45\xDF\xA3\x84\x42\x82\x01\x00\x00\x10\x00\x00\x00\x00".to_vec();
        bytes.resize(1 << 20, b'w');
        let (mime, src) = counted(&bytes).await;
        assert_eq!(mime, None);
        assert!(src.largest <= SAMPLE, "read {} bytes at once", src.largest);
    }

    /// Chained ID3 tags are skipped up to [`MAX_ID3_TAGS`]; past that the sniff gives up.
    #[tokio::test]
    async fn chained_id3_tags_stop_at_the_cap() {
        let file = |tags: usize| {
            let mut bytes = b"ID3\x04\x00\x00\x00\x00\x00\x00".repeat(tags);
            bytes.extend_from_slice(&[0xFF, 0xFB, 0x90, 0x64]);
            bytes.resize(bytes.len() + 64, 0);
            bytes
        };
        assert_eq!(counted(&file(3)).await.0, Some("audio/mpeg"));
        assert_eq!(counted(&file(MAX_ID3_TAGS)).await.0, Some("audio/mpeg"));
        let bytes = file(10_000);
        let (mime, src) = counted(&bytes).await;
        assert_eq!(mime, None);
        assert!(src.reads < 4 * MAX_ID3_TAGS, "{} reads", src.reads);
    }

    /// PNG chunks are walked up to [`MAX_PNG_CHUNKS`]; a file of empty chunks is not walked to
    /// its end.
    #[tokio::test]
    async fn a_png_of_empty_chunks_stops_at_the_cap() {
        let file = |chunks: usize| {
            let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
            for _ in 0..chunks {
                bytes.extend_from_slice(b"\0\0\0\0tEXt\0\0\0\0");
            }
            bytes.extend_from_slice(b"\0\0\0\x01IDAT\0\0\0\0\0");
            bytes
        };
        assert_eq!(counted(&file(10)).await.0, Some("image/png"));
        let bytes = file(100_000);
        let (mime, src) = counted(&bytes).await;
        assert_eq!(mime, None);
        assert!(src.reads < 3 * MAX_PNG_CHUNKS + 3, "{} reads", src.reads);
    }
}
