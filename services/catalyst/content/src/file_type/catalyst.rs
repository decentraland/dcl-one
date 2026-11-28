//! The content server's Content-Type sniff: the catalyst runs `file-type@21.1.1`
//! (`new FileTypeParser().fromStream(stream)`, `core.js`, with `strtok3` 10.3.4 and
//! `@tokenizer/inflate` 0.4.1 as its `yarn.lock` pins them) over the stored file's read stream, or
//! over the ranged stream for a `Range` request (`createContentFileHeaders`,
//! `src/controllers/utils.ts`). This ports every check, in its order and with its staged reads.
//!
//! The stream is an `fs.ReadStream`, so `strtok3` stats its path: the tokenizer knows the whole
//! file's size even when the stream carries a range. Where upstream throws (the stream ends before
//! a read, a ZIP entry it cannot inflate), this answers nothing; the catalyst rethrows into a 500.
//!
//! The loops that walk a file (ID3 tags and BOMs, PNG chunks, EBML and ASF elements, ZIP entries)
//! stop after more turns than a real file takes, where `file-type` walks on to the end; a stopped
//! walk answers nothing, except a ZIP one, which answers `application/zip`. A ZIP walk also stops
//! once it has fetched 16 MiB of the file, and at a `mimetype` entry over 1 KiB or a
//! `[Content_Types].xml` entry over 1 MiB, stored or inflated.
//!
//! `tests/fixtures/conformance/content-lambdas/content-sniff.tsv` holds upstream's answers for a
//! sample of each format; `the_content_sniff_answers_as_file_type` replays it.

use std::borrow::Cow;
use std::io;

use super::{SAMPLE, Source, Tokenizer};
use crate::shared::casts::to_usize;
use crate::storage::StorageError;

/// `fromTokenizer` restarts after an ID3 tag and `detectConfident` after a UTF-8 BOM.
const MAX_PREFIXES: usize = 64;
const MAX_PNG_CHUNKS: usize = 256;
const MAX_EBML_ELEMENTS: u64 = 64;
/// The longest EBML `DocType` read; a longer one answers nothing.
const MAX_DOC_TYPE: u64 = 64 * 1024;
const MAX_ASF_OBJECTS: usize = 64;
const MAX_ZIP_ENTRIES: usize = 1024;
/// The most bytes a ZIP walk fetches from storage in all.
const MAX_ZIP_READ: u64 = 16 * 1024 * 1024;
/// The largest `mimetype` entry read, stored and inflated.
const MAX_MIMETYPE: u64 = 1024;
/// The largest `[Content_Types].xml` entry read, stored and inflated.
const MAX_CONTENT_TYPES: u64 = 1024 * 1024;
/// `@tokenizer/inflate`'s `syncBufferSize`: the window a data-descriptor scan peeks at once.
const ZIP_SYNC: u64 = 256 * 1024;

const ZIP: &str = "application/zip";

/// What one `detectConfident` pass found.
enum Pass {
    Found(&'static str),
    /// Nothing, with the position left where `fromTokenizer` can try `detectImprecise`.
    Nothing,
    /// Nothing, and `fromTokenizer` answers nothing (a moved position or a throw).
    Stop,
    /// A UTF-8 BOM skipped: `detectConfident` runs again.
    Bom,
    /// An ID3 tag skipped: `fromTokenizer` runs again.
    Id3,
}

/// `(await new FileTypeParser().fromStream(stream))?.mime` over `src`, a file of `size` bytes:
/// `None` for no answer or a throw.
pub(crate) async fn file_type(
    src: &mut impl Source,
    size: u64,
) -> Result<Option<&'static str>, StorageError> {
    let mut tok = Tokenizer { src, pos: 0 };
    let mut start = 0;
    for _ in 0..MAX_PREFIXES {
        match confident(&mut tok, size).await? {
            Pass::Found(mime) => return Ok(Some(mime)),
            Pass::Stop => return Ok(None),
            Pass::Bom => continue,
            Pass::Id3 => {
                start = tok.pos;
                continue;
            }
            Pass::Nothing if tok.pos != start => return Ok(None),
            Pass::Nothing => return imprecise(&mut tok).await,
        }
    }
    Ok(None)
}

/// `peekBuffer(this.buffer, {length, mayBeLess: true})` into a fresh 4100-byte buffer.
async fn sample<S: Source>(tok: &mut Tokenizer<'_, S>, len: u64) -> Result<Vec<u8>, StorageError> {
    let mut b = tok.peek(len).await?;
    b.resize(to_usize(SAMPLE), 0);
    Ok(b)
}

fn has(b: &[u8], offset: usize, sig: &[u8]) -> bool {
    b[offset..offset + sig.len()] == *sig
}

/// `checkString(s, {offset, encoding: 'utf-16le' | 'utf-16be'})` for an ASCII `s`.
fn has_utf16(b: &[u8], offset: usize, s: &str, big_endian: bool) -> bool {
    s.bytes().enumerate().all(|(i, c)| {
        let pair = [b[offset + 2 * i], b[offset + 2 * i + 1]];
        pair == if big_endian { [0, c] } else { [c, 0] }
    })
}

/// `detectConfident`. `None` from a read is upstream's `EndOfStreamError`, which escapes.
async fn confident<S: Source>(tok: &mut Tokenizer<'_, S>, size: u64) -> Result<Pass, StorageError> {
    let b = sample(tok, 32).await?;
    let at = |offset: usize, sig: &[u8]| has(&b, offset, sig);
    let found = |mime| Ok(Pass::Found(mime));

    // -- 2-byte signatures --
    if at(0, b"BM") {
        return found("image/bmp");
    }
    if at(0, &[0x0B, 0x77]) {
        return found("audio/vnd.dolby.dd-raw");
    }
    if at(0, &[0x78, 0x01]) {
        return found("application/x-apple-diskimage");
    }
    if at(0, b"MZ") {
        return found("application/x-msdownload");
    }
    if at(0, b"%!") {
        return found(if at(2, b"PS-Adobe-") && at(14, b" EPSF-") {
            "application/eps"
        } else {
            "application/postscript"
        });
    }
    if at(0, &[0x1F, 0xA0]) || at(0, &[0x1F, 0x9D]) {
        return found("application/x-compress");
    }
    if at(0, &[0xC7, 0x71]) {
        return found("application/x-cpio");
    }
    if at(0, &[0x60, 0xEA]) {
        return found("application/x-arj");
    }

    // -- 3-byte signatures --
    if at(0, &[0xEF, 0xBB, 0xBF]) {
        tok.pos += 3;
        return Ok(Pass::Bom);
    }
    if at(0, b"GIF") {
        return found("image/gif");
    }
    if at(0, &[0x49, 0x49, 0xBC]) {
        return found("image/vnd.ms-photo");
    }
    // A gzip is `tar.gz` or `gz`; both are `application/gzip`, so the inflated head is not read.
    if at(0, &[0x1F, 0x8B, 0x08]) {
        return found("application/gzip");
    }
    if at(0, b"BZh") {
        return found("application/x-bzip2");
    }
    if at(0, b"ID3") {
        let Some(raw) = tok.skip_read(6, 4).await? else {
            return Ok(Pass::Stop);
        };
        // `uint32SyncSafeToken`: only the last byte is masked.
        let len = (u64::from(raw[3]) & 0x7F)
            | (u64::from(raw[2]) << 7)
            | (u64::from(raw[1]) << 14)
            | (u64::from(raw[0]) << 21);
        if tok.pos + len > size {
            return found("audio/mpeg");
        }
        if !tok.ignore(len).await? {
            return Ok(Pass::Stop);
        }
        return Ok(Pass::Id3);
    }
    if at(0, b"MP+") {
        return found("audio/x-musepack");
    }
    if matches!(b[0], 0x43 | 0x46) && at(1, b"WS") {
        return found("application/x-shockwave-flash");
    }

    // -- 4-byte signatures --
    if at(0, &[0xFF, 0xD8, 0xFF]) {
        return found(if b[3] == 0xF7 {
            "image/jls"
        } else {
            "image/jpeg"
        });
    }
    if at(0, &[0x4F, 0x62, 0x6A, 0x01]) {
        return found("application/avro");
    }
    if at(0, b"FLIF") {
        return found("image/flif");
    }
    if at(0, b"8BPS") {
        return found("image/vnd.adobe.photoshop");
    }
    if at(0, b"MPCK") {
        return found("audio/x-musepack");
    }
    if at(0, b"FORM") {
        return found("audio/aiff");
    }
    if at(0, b"icns") {
        return found("image/icns");
    }
    if at(0, &[0x50, 0x4B, 0x03, 0x04]) {
        return Ok(zip(tok).await?.map_or(Pass::Stop, Pass::Found));
    }
    if at(0, b"OggS") {
        let Some(codec) = tok.skip_read(28, 8).await? else {
            return Ok(Pass::Stop);
        };
        let is = |sig: &[u8]| codec.starts_with(sig);
        return found(if is(b"OpusHead") {
            "audio/ogg; codecs=opus"
        } else if is(b"\x80theora") || is(b"\x01video\0") {
            "video/ogg"
        } else if is(b"\x7FFLAC") || is(b"Speex  ") || is(b"\x01vorbis") {
            "audio/ogg"
        } else {
            "application/ogg"
        });
    }
    if at(0, b"PK") && matches!(b[2], 0x03 | 0x05 | 0x07) && matches!(b[3], 0x04 | 0x06 | 0x08) {
        return found(ZIP);
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
    if at(0, &[0xD4, 0xC3, 0xB2, 0xA1]) || at(0, &[0xA1, 0xB2, 0xC3, 0xD4]) {
        return found("application/vnd.tcpdump.pcap");
    }
    if at(0, b"DSD ") {
        return found("audio/x-dsf");
    }
    if at(0, b"LZIP") {
        return found("application/x-lzip");
    }
    if at(0, b"fLaC") {
        return found("audio/flac");
    }
    if at(0, &[0x42, 0x50, 0x47, 0xFB]) {
        return found("image/bpg");
    }
    if at(0, b"wvpk") {
        return found("audio/wavpack");
    }
    if at(0, b"%PDF") {
        return found("application/pdf");
    }
    if at(0, &[0x00, 0x61, 0x73, 0x6D]) {
        return found("application/wasm");
    }
    for (sig, big_endian) in [(b"II", false), (b"MM", true)] {
        if at(0, sig) {
            match tiff(tok, &b, big_endian).await? {
                Pass::Nothing => {}
                pass => return Ok(pass),
            }
        }
    }
    if at(0, b"MAC ") {
        return found("audio/ape");
    }
    if at(0, &[0x1A, 0x45, 0xDF, 0xA3]) {
        return Ok(match ebml_doc_type(tok).await?.as_deref() {
            Some("webm") => Pass::Found("video/webm"),
            Some("matroska") => Pass::Found("video/matroska"),
            _ => Pass::Stop,
        });
    }
    if at(0, b"SQLi") {
        return found("application/x-sqlite3");
    }
    if at(0, &[0x4E, 0x45, 0x53, 0x1A]) {
        return found("application/x-nintendo-nes-rom");
    }
    if at(0, b"Cr24") {
        return found("application/x-google-chrome-extension");
    }
    if at(0, b"MSCF") || at(0, b"ISc(") {
        return found("application/vnd.ms-cab-compressed");
    }
    if at(0, &[0xED, 0xAB, 0xEE, 0xDB]) {
        return found("application/x-rpm");
    }
    if at(0, &[0xC5, 0xD0, 0xD3, 0xC6]) {
        return found("application/eps");
    }
    if at(0, &[0x28, 0xB5, 0x2F, 0xFD]) {
        return found("application/zstd");
    }
    if at(0, &[0x7F, 0x45, 0x4C, 0x46]) {
        return found("application/x-elf");
    }
    if at(0, b"!BDN") {
        return found("application/vnd.ms-outlook");
    }
    if at(0, b"PAR1") || at(0, b"PARE") {
        return found("application/vnd.apache.parquet");
    }
    if at(0, b"ttcf") {
        return found("font/collection");
    }
    if at(0, &[0xCF, 0xFA, 0xED, 0xFE]) {
        return found("application/x-mach-binary");
    }
    if at(0, &[0x04, 0x22, 0x4D, 0x18]) {
        return found("application/x-lz4");
    }
    if at(0, b"regf") {
        return found("application/x-ft-windows-registry-hive");
    }

    // -- 5-byte signatures --
    if at(0, b"OTTO\0") {
        return found("font/otf");
    }
    if at(0, b"#!AMR") {
        return found("audio/amr");
    }
    if at(0, b"{\\rtf") {
        return found("application/rtf");
    }
    if at(0, &[0x46, 0x4C, 0x56, 0x01]) {
        return found("video/x-flv");
    }
    if at(0, b"IMPM") {
        return found("audio/x-it");
    }
    let lzh = [
        "-lh0-", "-lh1-", "-lh2-", "-lh3-", "-lh4-", "-lh5-", "-lh6-", "-lh7-", "-lzs-", "-lz4-",
        "-lz5-", "-lhd-",
    ];
    if lzh.iter().any(|m| at(2, m.as_bytes())) {
        return found("application/x-lzh-compressed");
    }
    if at(0, &[0x00, 0x00, 0x01, 0xBA]) {
        if b[4] & 0xF1 == 0x21 {
            return found("video/MP1S");
        }
        if b[4] & 0xC4 == 0x44 {
            return found("video/MP2P");
        }
    }
    if at(0, b"ITSF") {
        return found("application/vnd.ms-htmlhelp");
    }
    if at(0, &[0xCA, 0xFE, 0xBA, 0xBE]) {
        return found("application/java-vm");
    }
    if at(0, b".RMF") {
        return found("application/vnd.rn-realmedia");
    }
    if at(0, b"DRACO") {
        return found("application/vnd.google.draco");
    }

    // -- 6-byte signatures --
    if at(0, &[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00]) {
        return found("application/x-xz");
    }
    if at(0, b"<?xml ") {
        return found("application/xml");
    }
    if at(0, &[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]) {
        return found("application/x-7z-compressed");
    }
    if at(0, &[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07]) && matches!(b[6], 0x00 | 0x01) {
        return found("application/x-rar-compressed");
    }
    if at(0, b"solid ") {
        return found("model/stl");
    }
    // `version >= 1000 && version <= 1050` compares the Latin-1 string as a JavaScript number.
    if at(0, b"AC") {
        let version: String = b[2..6].iter().map(|&c| char::from(c)).collect();
        if js_trim(&version)
            .parse::<f64>()
            .is_ok_and(|v| (1000.0..=1050.0).contains(&v))
        {
            return found("image/vnd.dwg");
        }
    }
    if at(0, b"070707") {
        return found("application/x-cpio");
    }

    // -- 7-byte signatures --
    if at(0, b"BLENDER") {
        return found("application/x-blender");
    }
    if at(0, b"!<arch>") {
        let Some(name) = tok.skip_read(8, 13).await? else {
            return Ok(Pass::Stop);
        };
        // `StringType(13, 'ascii')` drops each byte's high bit.
        let debian = name
            .iter()
            .map(|c| c & 0x7F)
            .eq(b"debian-binary".iter().copied());
        return found(if debian {
            "application/x-deb"
        } else {
            "application/x-unix-archive"
        });
    }
    if at(0, b"WEBVTT") && matches!(b[6], b'\n' | b'\r' | b'\t' | b' ' | 0) {
        return found("text/vtt");
    }

    // -- 8-byte signatures --
    if at(0, &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) {
        tok.pos += 8;
        return png(tok, size).await;
    }
    if at(0, b"ARROW1\0\0") {
        return found("application/vnd.apache.arrow.file");
    }
    if at(0, b"glTF\x02\0\0\0") {
        return found("model/gltf-binary");
    }
    if at(4, b"free") || at(4, b"mdat") || at(4, b"moov") || at(4, b"wide") {
        return found("video/quicktime");
    }

    // -- 9-byte signatures --
    if at(0, &[0x49, 0x49, 0x52, 0x4F, 0x08, 0x00, 0x00, 0x00, 0x18]) {
        return found("image/x-olympus-orf");
    }
    if at(0, b"gimp xcf ") {
        return found("image/x-xcf");
    }
    if at(4, b"ftyp") && b[8] & 0x60 != 0 {
        return found(iso_bmff_brand(&b[8..12]));
    }

    // -- 10-byte signatures --
    if at(0, b"REGEDIT4\r\n") {
        return found("application/x-ms-regedit");
    }

    // -- 12-byte signatures --
    if at(0, b"RIFF") {
        if at(8, b"WEBP") {
            return found("image/webp");
        }
        if at(8, b"AVI") {
            return found("video/vnd.avi");
        }
        if at(8, b"WAVE") {
            return found("audio/wav");
        }
        if at(8, b"QLCM") {
            return found("audio/qcelp");
        }
    }
    if at(
        0,
        &[
            0x49, 0x49, 0x55, 0x00, 0x18, 0x00, 0x00, 0x00, 0x88, 0xE7, 0x74, 0xD8,
        ],
    ) {
        return found("image/x-panasonic-rw2");
    }
    if at(
        0,
        &[0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9],
    ) {
        return asf(tok, size).await;
    }
    if at(
        0,
        &[
            0xAB, 0x4B, 0x54, 0x58, 0x20, 0x31, 0x31, 0xBB, 0x0D, 0x0A, 0x1A, 0x0A,
        ],
    ) {
        return found("image/ktx");
    }
    if (at(0, &[0x7E, 0x10, 0x04]) || at(0, &[0x7E, 0x18, 0x04])) && at(4, b"0MIE") {
        return found("application/x-mie");
    }
    if at(2, &[0x27, 0x0A, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]) {
        return found("application/x-esri-shape");
    }
    if at(0, &[0xFF, 0x4F, 0xFF, 0x51]) {
        return found("image/j2c");
    }
    if at(
        0,
        &[
            0x00, 0x00, 0x00, 0x0C, 0x6A, 0x50, 0x20, 0x20, 0x0D, 0x0A, 0x87, 0x0A,
        ],
    ) {
        let Some(brand) = tok.skip_read(20, 4).await? else {
            return Ok(Pass::Stop);
        };
        let brand: Vec<u8> = brand.iter().map(|c| c & 0x7F).collect();
        return Ok(match &brand[..] {
            b"jp2 " => Pass::Found("image/jp2"),
            b"jpx " => Pass::Found("image/jpx"),
            b"jpm " => Pass::Found("image/jpm"),
            b"mjp2" => Pass::Found("image/mj2"),
            _ => Pass::Stop,
        });
    }
    if at(0, &[0xFF, 0x0A])
        || at(
            0,
            &[
                0x00, 0x00, 0x00, 0x0C, 0x4A, 0x58, 0x4C, 0x20, 0x0D, 0x0A, 0x87, 0x0A,
            ],
        )
    {
        return found("image/jxl");
    }
    if at(0, &[0xFE, 0xFF]) {
        if has_utf16(&b, 2, "<?xml ", true) {
            return found("application/xml");
        }
        return Ok(Pass::Nothing);
    }
    if at(0, &[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]) {
        return found("application/x-cfb");
    }

    // Increase sample size from 32 to 256.
    let b = sample(tok, 256).await?;
    let at = |offset: usize, sig: &[u8]| has(&b, offset, sig);
    if at(36, b"acsp") {
        return found("application/vnd.iccprofile");
    }
    if at(7, b"**ACE") && at(12, b"**") {
        return found("application/x-ace-compressed");
    }
    if at(0, b"BEGIN:") {
        if at(6, b"VCARD") {
            return found("text/vcard");
        }
        if at(6, b"VCALENDAR") {
            return found("text/calendar");
        }
    }
    if at(0, b"FUJIFILMCCD-RAW") {
        return found("image/x-fujifilm-raf");
    }
    if at(0, b"Extended Module:") {
        return found("audio/x-xm");
    }
    if at(0, b"Creative Voice File") {
        return found("audio/x-voc");
    }
    if at(0, &[0x04, 0x00, 0x00, 0x00]) && is_asar(&b) {
        return found("application/x-asar");
    }
    if at(
        0,
        &[
            0x06, 0x0E, 0x2B, 0x34, 0x02, 0x05, 0x01, 0x01, 0x0D, 0x01, 0x02, 0x01, 0x01, 0x02,
        ],
    ) {
        return found("application/mxf");
    }
    if at(44, b"SCRM") {
        return found("audio/x-s3m");
    }
    if b[0] == 0x47 && b[188] == 0x47 {
        return found("video/mp2t");
    }
    if b[4] == 0x47 && b[196] == 0x47 {
        return found("video/mp2t");
    }
    if at(60, b"BOOKMOBI") {
        return found("application/x-mobipocket-ebook");
    }
    if at(128, b"DICM") {
        return found("application/dicom");
    }
    if at(
        0,
        &[
            0x4C, 0x00, 0x00, 0x00, 0x01, 0x14, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0xC0, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x46,
        ],
    ) {
        return found("application/x.ms.shortcut");
    }
    if at(0, b"book\0\0\0\0mark\0\0\0\0") {
        return found("application/x.apple.alias");
    }
    if at(0, b"Kaydara FBX Binary  \0") {
        return found("application/x.autodesk.fbx");
    }
    if at(34, b"LP") && (at(8, &[0, 0, 1]) || at(8, &[1, 0, 2]) || at(8, &[2, 0, 2])) {
        return found("application/vnd.ms-fontobject");
    }
    if at(
        0,
        &[
            0x06, 0x06, 0xED, 0xF5, 0xD8, 0x1D, 0x46, 0xE5, 0xBD, 0x31, 0xEF, 0xE7, 0xFE, 0x74,
            0xB7, 0x1D,
        ],
    ) {
        return found("application/x-indesign");
    }

    // Increase sample size from 256 to 512.
    let b = sample(tok, 512).await?;
    let at = |offset: usize, sig: &[u8]| has(&b, offset, sig);
    if (at(257, b"ustar") && matches!(b[262], 0 | b' '))
        || (at(257, &[0; 6]) && tar_checksum_matches(&b))
    {
        return found("application/x-tar");
    }
    if at(0, &[0xFF, 0xFE]) {
        if has_utf16(&b, 2, "<?xml ", false) {
            return found("application/xml");
        }
        if at(2, &[0xFF, 0x0E]) && has_utf16(&b, 4, "SketchUp Model", false) {
            return found("application/vnd.sketchup.skp");
        }
        if has_utf16(&b, 2, "Windows Registry Editor Version 5.00\r\n", false) {
            return found("application/x-ms-regedit");
        }
        return Ok(Pass::Nothing);
    }
    if at(0, b"-----BEGIN PGP MESSAGE-----") {
        return found("application/pgp-encrypted");
    }
    Ok(Pass::Nothing)
}

/// `detectImprecise`, at the tokenizer's position.
async fn imprecise<S: Source>(
    tok: &mut Tokenizer<'_, S>,
) -> Result<Option<&'static str>, StorageError> {
    let b = sample(tok, 8).await?;
    let at = |offset: usize, sig: &[u8]| has(&b, offset, sig);
    if at(0, &[0x00, 0x00, 0x01, 0xBA]) || at(0, &[0x00, 0x00, 0x01, 0xB3]) {
        return Ok(Some("video/mpeg"));
    }
    if at(0, &[0x00, 0x01, 0x00, 0x00, 0x00]) {
        return Ok(Some("font/ttf"));
    }
    if at(0, &[0x00, 0x00, 0x01, 0x00]) || at(0, &[0x00, 0x00, 0x02, 0x00]) {
        return Ok(Some("image/x-icon"));
    }
    // `scanMpeg(0)`: ADTS is AAC, layers 1 to 3 are MPEG audio.
    if b[0] != 0xFF || b[1] & 0xE0 != 0xE0 {
        return Ok(None);
    }
    if b[1] & 0x16 == 0x10 {
        return Ok(Some("audio/aac"));
    }
    Ok(matches!(b[1] & 0x06, 0x02 | 0x04 | 0x06).then_some("audio/mpeg"))
}

/// The `ftyp` major brand read as Latin-1, the first NUL as a space, trimmed as JavaScript does.
fn iso_bmff_brand(raw: &[u8]) -> &'static str {
    let brand: String = raw.iter().map(|&c| char::from(c)).collect();
    match js_trim(&brand.replacen('\0', " ", 1)) {
        "avif" | "avis" => "image/avif",
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
        // M4P, F4V, F4P and every other brand.
        _ => "video/mp4",
    }
}

/// JavaScript's `String.prototype.trim`, which keeps U+0085 and strips U+FEFF, unlike Rust's.
fn js_trim(s: &str) -> &str {
    s.trim_matches(|c: char| (c.is_whitespace() && c != '\u{85}') || c == '\u{FEFF}')
}

/// `TextDecoder('utf-8').decode`: lossy, with a leading BOM dropped.
fn utf8(bytes: &[u8]) -> Cow<'_, str> {
    String::from_utf8_lossy(bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes))
}

/// `s.replace(/\0.*$/, '')`: the first NUL with no line terminator after it, and the rest, cut.
fn cut_at_nul(s: &str) -> &str {
    let terminator = |c| matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}');
    s.match_indices('\0')
        .find(|(i, _)| !s[i + 1..].contains(terminator))
        .map_or(s, |(i, _)| &s[..i])
}

/// `readTiffHeader`: `Nothing` lets the checks go on.
async fn tiff<S: Source>(
    tok: &mut Tokenizer<'_, S>,
    b: &[u8],
    big_endian: bool,
) -> Result<Pass, StorageError> {
    let u16_at = |raw: &[u8]| {
        let pair = [raw[0], raw[1]];
        if big_endian {
            u16::from_be_bytes(pair)
        } else {
            u16::from_le_bytes(pair)
        }
    };
    let ifd_offset = {
        let quad = [b[4], b[5], b[6], b[7]];
        if big_endian {
            u32::from_be_bytes(quad)
        } else {
            u32::from_le_bytes(quad)
        }
    };
    match u16_at(&b[2..]) {
        42 => {}
        43 => return Ok(Pass::Found("image/tiff")),
        _ => return Ok(Pass::Nothing),
    }
    if ifd_offset >= 6 {
        if has(b, 8, b"CR") {
            return Ok(Pass::Found("image/x-canon-cr2"));
        }
        if ifd_offset >= 8 {
            let ids = (u16_at(&b[8..]), u16_at(&b[10..]));
            if ids == (0x1C, 0xFE) || ids == (0x1F, 0x0B) {
                return Ok(Pass::Found("image/x-nikon-nef"));
            }
        }
    }
    if !tok.ignore(u64::from(ifd_offset)).await? {
        return Ok(Pass::Stop);
    }
    let Some(count) = tok.read(2).await? else {
        return Ok(Pass::Stop);
    };
    let count = u16_at(&count);
    for n in 1..=count {
        let Some(tag) = tok.read(2).await? else {
            return Ok(Pass::Stop);
        };
        match u16_at(&tag) {
            50_341 => return Ok(Pass::Found("image/x-sony-arw")),
            50_706 => return Ok(Pass::Found("image/x-adobe-dng")),
            _ => {}
        }
        // Upstream does not await this `ignore`: past the end, only a next tag's read throws.
        if !tok.ignore(10).await? {
            return Ok(if n == count {
                Pass::Found("image/tiff")
            } else {
                Pass::Stop
            });
        }
    }
    Ok(Pass::Found("image/tiff"))
}

/// The EBML header's `DocType`, read as `file-type` does: the root element's length is taken as
/// a count of children. `None` where upstream throws or finds none.
async fn ebml_doc_type<S: Source>(
    tok: &mut Tokenizer<'_, S>,
) -> Result<Option<String>, StorageError> {
    let Some((_, children)) = ebml_element(tok).await? else {
        return Ok(None);
    };
    for _ in 0..children.min(MAX_EBML_ELEMENTS) {
        let Some((id, len)) = ebml_element(tok).await? else {
            return Ok(None);
        };
        if id == Some(0x4282) {
            if len > MAX_DOC_TYPE {
                return Ok(None);
            }
            return Ok(tok
                .read(len)
                .await?
                .map(|doc| cut_at_nul(&utf8(&doc)).to_string()));
        }
        if !tok.ignore(len).await? {
            return Ok(None);
        }
    }
    Ok(None)
}

/// One element header: its id (`None` past six bytes, as `getUintBE` answers) and payload length.
async fn ebml_element<S: Source>(
    tok: &mut Tokenizer<'_, S>,
) -> Result<Option<(Option<u64>, u64)>, StorageError> {
    let Some(id) = ebml_field(tok).await? else {
        return Ok(None);
    };
    let Some(mut len) = ebml_field(tok).await? else {
        return Ok(None);
    };
    len[0] ^= u32::try_from(len.len() - 1)
        .ok()
        .and_then(|shift| 0x80u8.checked_shr(shift))
        .unwrap_or(0);
    let len = &len[len.len() - len.len().min(6)..];
    let be = |bytes: &[u8]| bytes.iter().fold(0u64, |n, &b| (n << 8) | u64::from(b));
    Ok(Some(((id.len() <= 6).then(|| be(&id)), be(len))))
}

/// A variable-length field: its width is one plus the leading zero bits of its first byte.
async fn ebml_field<S: Source>(
    tok: &mut Tokenizer<'_, S>,
) -> Result<Option<Vec<u8>>, StorageError> {
    let Some(first) = tok.peek_byte().await? else {
        return Ok(None);
    };
    let width = u64::from(first.leading_zeros()) + 1;
    Ok(tok.read(width).await?.map(|field| field.to_vec()))
}

/// PNG: an `acTL` chunk before the first `IDAT` is APNG; reaching the file's size first is PNG.
async fn png<S: Source>(tok: &mut Tokenizer<'_, S>, size: u64) -> Result<Pass, StorageError> {
    for _ in 0..MAX_PNG_CHUNKS {
        let Some(header) = tok.read(8).await? else {
            return Ok(Pass::Stop);
        };
        let length = i32::from_be_bytes([header[0], header[1], header[2], header[3]]);
        if length < 0 {
            return Ok(Pass::Stop);
        }
        match &header[4..] {
            b"IDAT" => return Ok(Pass::Found("image/png")),
            b"acTL" => return Ok(Pass::Found("image/apng")),
            _ if !tok.ignore(u64::from(length.unsigned_abs()) + 4).await? => return Ok(Pass::Stop),
            _ => {}
        }
        if tok.pos + 8 >= size {
            return Ok(Pass::Found("image/png"));
        }
    }
    Ok(Pass::Stop)
}

/// ASF: the first Stream Properties object's stream type, after the header object's 30 bytes.
async fn asf<S: Source>(tok: &mut Tokenizer<'_, S>, size: u64) -> Result<Pass, StorageError> {
    const STREAM_PROPERTIES: [u8; 16] = [
        0x91, 0x07, 0xDC, 0xB7, 0xB7, 0xA9, 0xCF, 0x11, 0x8E, 0xE6, 0x00, 0xC0, 0x0C, 0x20, 0x53,
        0x65,
    ];
    const AUDIO: [u8; 16] = [
        0x40, 0x9E, 0x69, 0xF8, 0x4D, 0x5B, 0xCF, 0x11, 0xA8, 0xFD, 0x00, 0x80, 0x5F, 0x5C, 0x44,
        0x2B,
    ];
    const VIDEO: [u8; 16] = [
        0xC0, 0xEF, 0x19, 0xBC, 0x4D, 0x5B, 0xCF, 0x11, 0xA8, 0xFD, 0x00, 0x80, 0x5F, 0x5C, 0x44,
        0x2B,
    ];
    if !tok.ignore(30).await? {
        return Ok(Pass::Stop);
    }
    for _ in 0..MAX_ASF_OBJECTS {
        if tok.pos + 24 >= size {
            return Ok(Pass::Found("application/vnd.ms-asf"));
        }
        let Some(header) = tok.read(24).await? else {
            return Ok(Pass::Stop);
        };
        if header[..16] == STREAM_PROPERTIES {
            let Some(kind) = tok.read(16).await? else {
                return Ok(Pass::Stop);
            };
            return Ok(Pass::Found(if kind[..] == AUDIO {
                "audio/x-ms-asf"
            } else if kind[..] == VIDEO {
                "video/x-ms-asf"
            } else {
                "application/vnd.ms-asf"
            }));
        }
        let object = u64::from_le_bytes(
            header[16..24]
                .try_into()
                .expect("an 8-byte slice is a [u8; 8]"),
        );
        // `ignore` of a negative payload throws.
        if object < 24 || !tok.ignore(object - 24).await? {
            return Ok(Pass::Stop);
        }
    }
    Ok(Pass::Stop)
}

/// The Pickle header of an ASAR archive: a JSON object with a truthy `files`.
fn is_asar(b: &[u8]) -> bool {
    let json_size = u32::from_le_bytes([b[12], b[13], b[14], b[15]]) as usize;
    if json_size <= 12 || b.len() < json_size + 16 {
        return false;
    }
    let Ok(serde_json::Value::Object(json)) =
        serde_json::from_str::<serde_json::Value>(&utf8(&b[16..json_size + 16]))
    else {
        return false;
    };
    match json.get("files") {
        None | Some(serde_json::Value::Null) => false,
        Some(serde_json::Value::Bool(b)) => *b,
        Some(serde_json::Value::Number(n)) => n.as_f64() != Some(0.0),
        Some(serde_json::Value::String(s)) => !s.is_empty(),
        Some(_) => true,
    }
}

/// `tarHeaderChecksumMatches`: the header's octal checksum against the byte sum, the checksum
/// field counted as spaces.
fn tar_checksum_matches(b: &[u8]) -> bool {
    let field = utf8(&b[148..154]);
    let field = js_trim(cut_at_nul(&field));
    // `parseInt(field, 8)`: an optional sign, then the octal digits up to the first other character.
    let (negative, digits) = match field.as_bytes().first() {
        Some(b'-') => (true, &field[1..]),
        Some(b'+') => (false, &field[1..]),
        _ => (false, field),
    };
    let octal: String = digits
        .chars()
        .take_while(|c| ('0'..='7').contains(c))
        .collect();
    if octal.is_empty() {
        return false;
    }
    let sum = 8 * 0x20
        + b[..148].iter().map(|&c| u64::from(c)).sum::<u64>()
        + b[156..512].iter().map(|&c| u64::from(c)).sum::<u64>();
    !negative && u64::from_str_radix(&octal, 8).is_ok_and(|read| read == sum)
}

/// What an entry's name decides (`file-type`'s `unzip` callback).
#[derive(Clone, Copy)]
enum Entry {
    Skip,
    Is(&'static str),
    MimeType,
    ContentTypes,
}

fn zip_entry(name: &str) -> Entry {
    match name {
        "META-INF/mozilla.rsa" => Entry::Is("application/x-xpinstall"),
        "META-INF/MANIFEST.MF" => Entry::Is("application/java-archive"),
        "mimetype" => Entry::MimeType,
        "[Content_Types].xml" => Entry::ContentTypes,
        _ if is_dex(name) => Entry::Is("application/vnd.android.package-archive"),
        _ => Entry::Skip,
    }
}

/// `/classes\d*\.dex/` anywhere in the name.
fn is_dex(name: &str) -> bool {
    name.match_indices("classes").any(|(i, m)| {
        let rest = name[i + m.len()..].trim_start_matches(|c: char| c.is_ascii_digit());
        rest.starts_with(".dex")
    })
}

/// `getFileTypeFromMimeType`: the document types a ZIP's own MIME declaration names.
fn zip_document(declared: &str) -> Option<&'static str> {
    Some(match declared.to_lowercase().as_str() {
        "application/epub+zip" => "application/epub+zip",
        "application/vnd.oasis.opendocument.text" => "application/vnd.oasis.opendocument.text",
        "application/vnd.oasis.opendocument.text-template" => {
            "application/vnd.oasis.opendocument.text-template"
        }
        "application/vnd.oasis.opendocument.spreadsheet" => {
            "application/vnd.oasis.opendocument.spreadsheet"
        }
        "application/vnd.oasis.opendocument.spreadsheet-template" => {
            "application/vnd.oasis.opendocument.spreadsheet-template"
        }
        "application/vnd.oasis.opendocument.presentation" => {
            "application/vnd.oasis.opendocument.presentation"
        }
        "application/vnd.oasis.opendocument.presentation-template" => {
            "application/vnd.oasis.opendocument.presentation-template"
        }
        "application/vnd.oasis.opendocument.graphics" => {
            "application/vnd.oasis.opendocument.graphics"
        }
        "application/vnd.oasis.opendocument.graphics-template" => {
            "application/vnd.oasis.opendocument.graphics-template"
        }
        "application/vnd.openxmlformats-officedocument.presentationml.slideshow" => {
            "application/vnd.openxmlformats-officedocument.presentationml.slideshow"
        }
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => {
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
        }
        "application/vnd.ms-excel.sheet.macroenabled" => {
            "application/vnd.ms-excel.sheet.macroenabled.12"
        }
        "application/vnd.openxmlformats-officedocument.spreadsheetml.template" => {
            "application/vnd.openxmlformats-officedocument.spreadsheetml.template"
        }
        "application/vnd.ms-excel.template.macroenabled" => {
            "application/vnd.ms-excel.template.macroenabled.12"
        }
        "application/vnd.ms-powerpoint.slideshow.macroenabled" => {
            "application/vnd.ms-powerpoint.slideshow.macroenabled.12"
        }
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        }
        "application/vnd.ms-word.document.macroenabled" => {
            "application/vnd.ms-word.document.macroenabled.12"
        }
        "application/vnd.openxmlformats-officedocument.wordprocessingml.template" => {
            "application/vnd.openxmlformats-officedocument.wordprocessingml.template"
        }
        "application/vnd.ms-word.template.macroenabledtemplate" => {
            "application/vnd.ms-word.template.macroenabled.12"
        }
        "application/vnd.openxmlformats-officedocument.presentationml.template" => {
            "application/vnd.openxmlformats-officedocument.presentationml.template"
        }
        "application/vnd.ms-powerpoint.template.macroenabled" => {
            "application/vnd.ms-powerpoint.template.macroenabled.12"
        }
        "application/vnd.openxmlformats-officedocument.presentationml.presentation" => {
            "application/vnd.openxmlformats-officedocument.presentationml.presentation"
        }
        "application/vnd.ms-powerpoint.presentation.macroenabled" => {
            "application/vnd.ms-powerpoint.presentation.macroenabled.12"
        }
        "application/vnd.ms-visio.drawing" => "application/vnd.visio",
        "application/vnd.ms-package.3dmanufacturing-3dmodel+xml" => "model/3mf",
        _ => return None,
    })
}

/// The `[Content_Types].xml` handler: the part type ending in `.main+xml`, else 3MF's model type.
fn content_types(xml: &str) -> Option<&'static str> {
    const MODEL: &str = "application/vnd.ms-package.3dmanufacturing-3dmodel+xml";
    match xml.find(".main+xml\"") {
        None => xml
            .contains(&format!("ContentType=\"{MODEL}\""))
            .then_some("model/3mf"),
        Some(end) => {
            let head = &xml[..end];
            zip_document(&head[head.rfind('"').map_or(0, |i| i + 1)..])
        }
    }
}

/// Why a ZIP walk ended without walking on.
enum ZipEnd {
    /// `EndOfStreamError`, which `file-type` swallows.
    EndOfStream,
    /// Any other throw, which escapes.
    Error,
    /// A walk past this port's bounds.
    Bounded,
}

/// The ZIP walk of `file-type`, through `@tokenizer/inflate`'s `unzip` over a stream: local file
/// headers in order, until the central directory, an entry that decides, or the stream's end.
/// `None` where it throws.
async fn zip<S: Source>(tok: &mut Tokenizer<'_, S>) -> Result<Option<&'static str>, StorageError> {
    let mut decided = None;
    match zip_walk(tok, &mut decided).await? {
        Ok(()) | Err(ZipEnd::EndOfStream) => Ok(Some(decided.unwrap_or(ZIP))),
        Err(ZipEnd::Bounded) => Ok(Some(ZIP)),
        Err(ZipEnd::Error) => Ok(None),
    }
}

async fn zip_walk<S: Source>(
    tok: &mut Tokenizer<'_, S>,
    decided: &mut Option<&'static str>,
) -> Result<Result<(), ZipEnd>, StorageError> {
    const LOCAL: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];
    const CENTRAL: [u8; 4] = [0x50, 0x4B, 0x01, 0x02];
    const DESCRIPTOR: [u8; 4] = [0x50, 0x4B, 0x07, 0x08];
    let eos = Err(ZipEnd::EndOfStream);
    for _ in 0..MAX_ZIP_ENTRIES {
        if tok.src.fetched() > MAX_ZIP_READ {
            return Ok(Err(ZipEnd::Bounded));
        }
        let signature = tok.src.read_at(tok.pos, 4).await?;
        if signature.len() < 4 {
            return Ok(eos);
        }
        if signature[..] == CENTRAL {
            return Ok(Ok(()));
        }
        if signature[..] != LOCAL {
            // 'Encrypted ZIP' or 'Unexpected signature': both throw.
            return Ok(Err(ZipEnd::Error));
        }
        let Some(header) = tok.read(30).await? else {
            return Ok(eos);
        };
        let u16_at = |i: usize| u64::from(u16::from_le_bytes([header[i], header[i + 1]]));
        let u32_at = |i: usize| {
            u64::from(u32::from_le_bytes(
                header[i..i + 4]
                    .try_into()
                    .expect("a 4-byte slice is a [u8; 4]"),
            ))
        };
        let descriptor = u16_at(6) & 0x0008 != 0;
        let method = u16_at(8);
        let compressed = u32_at(18);
        let Some(name) = tok.read(u16_at(26)).await? else {
            return Ok(eos);
        };
        let entry = zip_entry(&utf8(&name));
        let stop = !matches!(entry, Entry::Skip);
        if let Entry::Is(mime) = entry {
            *decided = Some(mime);
        }
        let limit = match entry {
            Entry::MimeType => MAX_MIMETYPE,
            Entry::ContentTypes => MAX_CONTENT_TYPES,
            _ => 0,
        };
        let handled = limit > 0;
        if !tok.ignore(u16_at(28)).await? {
            return Ok(eos);
        }
        let data = if descriptor && compressed == 0 {
            // The size is unknown: the data runs to the next data-descriptor signature.
            let mut data = Vec::new();
            loop {
                if tok.src.fetched() > MAX_ZIP_READ {
                    return Ok(Err(ZipEnd::Bounded));
                }
                let window = tok.src.read_at(tok.pos, ZIP_SYNC).await?;
                let next = window.windows(4).position(|w| w == DESCRIPTOR);
                let len = next.unwrap_or(window.len());
                if handled {
                    if (data.len() + len) as u64 > limit {
                        return Ok(Err(ZipEnd::Bounded));
                    }
                    data.extend_from_slice(&window[..len]);
                }
                tok.pos += len as u64;
                if next.is_some() || (window.len() as u64) < ZIP_SYNC {
                    break;
                }
            }
            handled.then(|| data.into())
        } else if handled {
            if compressed > limit {
                return Ok(Err(ZipEnd::Bounded));
            }
            let Some(data) = tok.read(compressed).await? else {
                return Ok(eos);
            };
            Some(data)
        } else {
            if !tok.ignore(compressed).await? {
                return Ok(eos);
            }
            None
        };
        if let Some(data) = data {
            let inflated;
            let raw = match method {
                0 => &data[..],
                8 => match inflate_raw(&data, limit) {
                    Ok(Some(text)) => {
                        inflated = text;
                        &inflated[..]
                    }
                    Ok(None) => return Ok(Err(ZipEnd::Bounded)),
                    Err(_) => return Ok(Err(ZipEnd::Error)),
                },
                _ => return Ok(Err(ZipEnd::Error)),
            };
            let text = utf8(raw);
            *decided = match entry {
                Entry::MimeType => zip_document(js_trim(&text)),
                _ => content_types(&text),
            };
        }
        if descriptor {
            let Some(record) = tok.read(16).await? else {
                return Ok(eos);
            };
            if record[..4] != DESCRIPTOR {
                return Ok(Err(ZipEnd::Error));
            }
        }
        if stop {
            return Ok(Ok(()));
        }
    }
    Ok(Err(ZipEnd::Bounded))
}

/// `DecompressionStream('deflate-raw')` over a whole entry: a truncated stream or bytes after its
/// end are errors. `Ok(None)` when the output passes `limit`.
fn inflate_raw(data: &[u8], limit: u64) -> io::Result<Option<Vec<u8>>> {
    use flate2::{Decompress, FlushDecompress, Status};
    let mut inflater = Decompress::new(false);
    let mut out = Vec::new();
    loop {
        out.reserve(4 * 1024);
        let before = (inflater.total_in(), inflater.total_out());
        let consumed = to_usize(inflater.total_in());
        let status = inflater
            .decompress_vec(&data[consumed..], &mut out, FlushDecompress::None)
            .map_err(io::Error::other)?;
        if out.len() as u64 > limit {
            return Ok(None);
        }
        if status == Status::StreamEnd {
            return if to_usize(inflater.total_in()) == data.len() {
                Ok(Some(out))
            } else {
                Err(io::Error::other("data after the deflate stream"))
            };
        }
        if (inflater.total_in(), inflater.total_out()) == before {
            return Err(io::Error::other("truncated deflate stream"));
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::cast_possible_truncation,
    reason = "test fixtures use small literal values"
)]
mod tests {
    use super::*;

    const UPSTREAM: &str =
        include_str!("../../../tests/fixtures/conformance/content-lambdas/content-sniff.tsv");

    async fn sniffed(mut bytes: &[u8], size: u64) -> &'static str {
        file_type(&mut bytes, size)
            .await
            .unwrap()
            .unwrap_or("application/octet-stream")
    }

    /// Upstream's answer, with a throw (a 500 there) answered as no type.
    fn expected(answer: &str) -> &str {
        if answer == "error" {
            "application/octet-stream"
        } else {
            answer
        }
    }

    /// Every sample `capture-sniff.cjs` ran through `file-type` 21.1.1 as the catalyst does,
    /// whole and as the stream of bytes 0-4099 of the same file.
    #[tokio::test]
    async fn the_content_sniff_answers_as_file_type() {
        let mut rows = 0;
        for line in UPSTREAM.lines().filter(|l| !l.starts_with('#')) {
            let [name, length, head, whole, ranged] = line.split('\t').collect::<Vec<_>>()[..]
            else {
                panic!("a row of five fields: {line}");
            };
            let mut bytes = hex::decode(head).unwrap();
            bytes.resize(length.parse().unwrap(), 0);
            let size = bytes.len() as u64;
            assert_eq!(sniffed(&bytes, size).await, expected(whole), "{name}");
            let window = &bytes[..bytes.len().min(SAMPLE as usize)];
            assert_eq!(
                sniffed(window, size).await,
                expected(ranged),
                "{name} ranged"
            );
            rows += 1;
        }
        assert!(rows > 200, "{rows} rows");
    }

    fn ftyp(brand: &[u8; 4]) -> Vec<u8> {
        let mut bytes = b"\0\0\0\x18ftyp".to_vec();
        bytes.extend_from_slice(brand);
        bytes.extend_from_slice(b"\0\0\0\0isomiso2");
        bytes
    }

    /// The formats a content server stores, each with the exact string the catalyst answers.
    #[tokio::test]
    async fn content_formats_get_file_types_strings() {
        let png = [
            &b"\x89PNG\r\n\x1a\n\0\0\0\x0dIHDR"[..],
            &[0; 17],
            b"\0\0\0\x01IDAT\0\0\0\0\0",
        ]
        .concat();
        let cases: Vec<(&str, Vec<u8>, &str)> = vec![
            ("png", png, "image/png"),
            ("jpeg", b"\xFF\xD8\xFF\xE0".to_vec(), "image/jpeg"),
            ("gif", b"GIF89a".to_vec(), "image/gif"),
            ("webp", b"RIFF\0\0\0\0WEBPVP8 ".to_vec(), "image/webp"),
            ("avif", ftyp(b"avif"), "image/avif"),
            ("avis", ftyp(b"avis"), "image/avif"),
            ("heic", ftyp(b"heic"), "image/heic"),
            ("heix", ftyp(b"heix"), "image/heic"),
            ("mif1", ftyp(b"mif1"), "image/heif"),
            ("msf1", ftyp(b"msf1"), "image/heif-sequence"),
            ("mp4", ftyp(b"isom"), "video/mp4"),
            ("mp42", ftyp(b"mp42"), "video/mp4"),
            ("m4a", ftyp(b"M4A "), "audio/x-m4a"),
            ("bmp", b"BM".to_vec(), "image/bmp"),
            ("tiff", b"MM\0\x2a\0\0\0\x08\0\0".to_vec(), "image/tiff"),
            ("ico", b"\0\0\x01\0\x01\0".to_vec(), "image/x-icon"),
            ("glb", b"glTF\x02\0\0\0".to_vec(), "model/gltf-binary"),
            ("ktx", b"\xABKTX 11\xBB\r\n\x1A\n".to_vec(), "image/ktx"),
            ("mp3", b"\xFF\xFB\x90\x64".to_vec(), "audio/mpeg"),
            (
                "ogg",
                [&b"OggS"[..], &[0; 24], b"\x01vorbis"].concat(),
                "audio/ogg",
            ),
            ("wav", b"RIFF\0\0\0\0WAVEfmt ".to_vec(), "audio/wav"),
            (
                "webm",
                b"\x1A\x45\xDF\xA3\x8B\x42\x86\x81\x01\x42\x82\x84webm".to_vec(),
                "video/webm",
            ),
            (
                "zip",
                [
                    &b"PK\x03\x04"[..],
                    &[0; 22],
                    b"\x05\0\0\0a.txt",
                    b"PK\x01\x02",
                ]
                .concat(),
                "application/zip",
            ),
            ("wasm", b"\0asm\x01\0\0\0".to_vec(), "application/wasm"),
            ("pdf", b"%PDF-1.7\n".to_vec(), "application/pdf"),
            ("flac", b"fLaC".to_vec(), "audio/flac"),
            ("woff2", b"wOF2OTTO".to_vec(), "font/woff2"),
            ("ttf", b"\0\x01\0\0\0".to_vec(), "font/ttf"),
            ("gzip", b"\x1F\x8B\x08".to_vec(), "application/gzip"),
            (
                "7z",
                b"7z\xBC\xAF\x27\x1C".to_vec(),
                "application/x-7z-compressed",
            ),
            (
                "xml",
                b"<?xml version=\"1.0\"?>".to_vec(),
                "application/xml",
            ),
        ];
        for (name, mut bytes, mime) in cases {
            bytes.resize(5000, 0);
            assert_eq!(sniffed(&bytes, 5000).await, mime, "{name}");
        }
        // Text formats, JSON included, are not recognised.
        let mut json = b"{\"version\":\"v3\"}".to_vec();
        json.resize(5000, b' ');
        assert_eq!(sniffed(&json, 5000).await, "application/octet-stream");
    }

    /// A read that comes back short is not taken for the file's end when the file goes on: the
    /// range answers as upstream's stream would, and a file cut where a check reads answers nothing.
    #[tokio::test]
    async fn a_short_stream_does_not_mislabel() {
        let mut ogg = [&b"OggS"[..], &[0; 24], b"\x01vorbis"].concat();
        assert_eq!(sniffed(&ogg[..20], 20).await, "application/octet-stream");
        ogg.resize(5000, 0);
        assert_eq!(sniffed(&ogg[..36], 5000).await, "audio/ogg");
        assert_eq!(sniffed(&ogg[..30], 5000).await, "application/octet-stream");
    }

    #[test]
    fn javascript_string_helpers() {
        assert_eq!(js_trim("\u{FEFF} a\u{A0}"), "a");
        assert_eq!(js_trim("a\u{85}"), "a\u{85}");
        assert_eq!(cut_at_nul("webm\0\0"), "webm");
        assert_eq!(cut_at_nul("we\0b\nm\0x"), "we\0b\nm");
        assert!(is_dex("lib/classes12.dex"));
        assert!(!is_dex("classes.dx"));
        assert_eq!(
            content_types("<x ContentType=\"application/vnd.ms-visio.drawing.main+xml\"/>"),
            Some("application/vnd.visio")
        );
    }

    /// An entry whose stored size runs past the bound answers `application/zip`.
    #[tokio::test]
    async fn a_huge_zip_entry_is_not_read() {
        let mut bytes = b"PK\x03\x04\x14\0\0\0\0\0\0\0\0\0\0\0\0\0\xFF\xFF\xFF\x7F\xFF\xFF\xFF\x7F\x08\0\0\0mimetype".to_vec();
        bytes.resize(1 << 16, 0);
        assert_eq!(sniffed(&bytes, 1 << 16).await, "application/zip");
    }

    /// A ZIP local file header and its data: `flags` 0x0008 is a data descriptor, after which a
    /// `compressed` of 0 means the size is unknown.
    fn zip_local(name: &[u8], flags: u16, method: u16, compressed: u32, data: &[u8]) -> Vec<u8> {
        let mut entry = b"PK\x03\x04\x14\0".to_vec();
        entry.extend_from_slice(&flags.to_le_bytes());
        entry.extend_from_slice(&method.to_le_bytes());
        entry.extend_from_slice(&[0; 8]);
        entry.extend_from_slice(&compressed.to_le_bytes());
        entry.extend_from_slice(&[0; 4]);
        entry.extend_from_slice(&(name.len() as u16).to_le_bytes());
        entry.extend_from_slice(&[0; 2]);
        entry.extend_from_slice(name);
        entry.extend_from_slice(data);
        entry
    }

    async fn sniffed_zip(entry: Vec<u8>) -> &'static str {
        let mut bytes = entry;
        bytes.extend_from_slice(b"PK\x01\x02");
        bytes.resize(bytes.len().max(5000), 0);
        let size = bytes.len() as u64;
        sniffed(&bytes, size).await
    }

    /// `TextDecoder` drops a leading BOM from an entry's name.
    #[tokio::test]
    async fn a_bom_prefixed_mimetype_entry_decides() {
        let epub = b"application/epub+zip";
        let entry = zip_local(b"\xEF\xBB\xBFmimetype", 0, 0, epub.len() as u32, epub);
        assert_eq!(sniffed_zip(entry).await, "application/epub+zip");
    }

    /// A `mimetype` entry is read up to 1 KiB and a `[Content_Types].xml` up to 1 MiB, by its
    /// stored size, by a data-descriptor scan, or inflated; past that the walk answers
    /// `application/zip`.
    #[tokio::test]
    async fn handled_zip_entries_are_read_up_to_their_cap() {
        let epub = |len: usize| {
            let mut text = b"application/epub+zip".to_vec();
            text.resize(len, b' ');
            text
        };
        for (len, mime) in [(1024, "application/epub+zip"), (1025, ZIP)] {
            let stored = zip_local(b"mimetype", 0, 0, len as u32, &epub(len));
            assert_eq!(sniffed_zip(stored).await, mime, "stored {len}");
            let mut scanned = zip_local(b"mimetype", 0x0008, 0, 0, &epub(len));
            scanned.extend_from_slice(b"PK\x07\x08");
            scanned.extend_from_slice(&[0; 12]);
            assert_eq!(sniffed_zip(scanned).await, mime, "scanned {len}");
        }

        let docx = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
        let deflated = |len: usize| {
            use std::io::Write;
            let mut xml = format!("<Override ContentType=\"{docx}.main+xml\"/>").into_bytes();
            xml.resize(len, b' ');
            let mut encoder =
                flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(&xml).unwrap();
            encoder.finish().unwrap()
        };
        for (len, mime) in [
            (MAX_CONTENT_TYPES as usize, docx),
            (MAX_CONTENT_TYPES as usize + 1, ZIP),
        ] {
            let data = deflated(len);
            let entry = zip_local(b"[Content_Types].xml", 0, 8, data.len() as u32, &data);
            assert_eq!(sniffed_zip(entry).await, mime, "inflated {len}");
        }
    }

    /// An ASF object smaller than its own header stops the walk, where upstream's `ignore` of a
    /// negative length throws.
    #[tokio::test]
    async fn an_asf_object_under_24_bytes_stops_the_walk() {
        let audio = hex::decode(
            "3026b2758e66cf11a6d900aa0062ce6c00000000000000000000000000009107dcb7b7a9cf118ee600c00c2053655000000000000000409e69f84d5bcf11a8fd00805f5c442b",
        )
        .unwrap();
        let object = |size: u64| {
            let mut bytes = audio[..30].to_vec();
            bytes.extend_from_slice(&[1; 16]);
            bytes.extend_from_slice(&size.to_le_bytes());
            bytes.extend_from_slice(&[0; 24][..(size as usize).saturating_sub(24)]);
            bytes.extend_from_slice(&audio[30..]);
            bytes.resize(5000, 0);
            bytes
        };
        assert_eq!(sniffed(&object(24), 5000).await, "audio/x-ms-asf");
        assert_eq!(sniffed(&object(30), 5000).await, "audio/x-ms-asf");
        assert_eq!(sniffed(&object(0), 5000).await, "application/octet-stream");
        assert_eq!(sniffed(&object(23), 5000).await, "application/octet-stream");
    }
}
