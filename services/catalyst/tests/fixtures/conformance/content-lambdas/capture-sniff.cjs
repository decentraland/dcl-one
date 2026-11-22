// Captures the catalyst content server's `?includeMimeType` answers for `content-sniff.tsv`.
//
//   node capture-sniff.cjs <directory holding node_modules/file-type> > content-sniff.tsv
//
// The catalyst pins `file-type` 21.1.1 (with `strtok3` 10.3.4, `token-types` 6.1.1 and
// `@tokenizer/inflate` 0.4.1, as its `yarn.lock` resolves them). `createContentFileHeaders`
// (`src/controllers/utils.ts`) runs `new FileTypeParser().fromStream(stream)` over the stored file's
// Node stream, or over the ranged stream for a `Range` request. Each sample is written to a file
// and read back the same way, through `fs.createReadStream` as `@dcl/catalyst-storage` 5.0.0 opens it
// (so `strtok3` stats the path and knows the whole file's size): whole (`full`) and as bytes 0-4099
// (`range`). No answer is
// `application/octet-stream`, the handler's fallback; a throw is `error` (the handler rethrows it).
'use strict'
const fs = require('fs')
const os = require('os')
const path = require('path')
const zlib = require('zlib')

const root = process.argv[2]
if (!root) throw new Error('usage: node capture-sniff.cjs <directory holding node_modules/file-type>')
const modules = path.join(root, 'node_modules')
const version = (name) => require(path.join(modules, name, 'package.json')).version

const FALLBACK = 'application/octet-stream'
const RANGE = 4100

const hex = (s) => Buffer.from(s.replace(/\s+/g, ''), 'hex')
const ascii = (s) => Buffer.from(s, 'latin1')
const cat = (...parts) => Buffer.concat(parts.map((p) => (typeof p === 'string' ? ascii(p) : p)))
const u16le = (n) => {
  const b = Buffer.alloc(2)
  b.writeUInt16LE(n)
  return b
}
const u32le = (n) => {
  const b = Buffer.alloc(4)
  b.writeUInt32LE(n)
  return b
}
const u32be = (n) => {
  const b = Buffer.alloc(4)
  b.writeUInt32BE(n)
  return b
}
const id3 = (size) =>
  cat('ID3', hex('0400 00'), Buffer.from([(size >> 21) & 0x7f, (size >> 14) & 0x7f, (size >> 7) & 0x7f, size & 0x7f]))
const pngChunk = (type, length) => cat(u32be(length), type, Buffer.alloc(length + 4))
const PNG = hex('89504E470D0A1A0A')
const ebml = (docType) =>
  cat(hex('1A45DFA3'), Buffer.from([0x80 | (docType.length + 3 + 4)]), hex('4286 81 01'), hex('4282'), Buffer.from([0x80 | docType.length]), docType)
const ftyp = (brand) => cat(hex('00000018'), 'ftyp', brand, hex('00000000'), 'isomiso2')
const ogg = (codec) => cat('OggS', Buffer.alloc(24), codec)
const riff = (form, rest) => cat('RIFF', hex('00000000'), form, rest)
// A ZIP local file header and its data; `flags` bit 3 announces a data descriptor.
const zipEntry = (name, data, { method = 0, flags = 0, size = data.length } = {}) =>
  cat(hex('504B0304 1400'), u16le(flags), u16le(method), hex('0000 0000 00000000'), u32le(size), u32le(size), u16le(name.length), u16le(0), name, data)
const CENTRAL = hex('504B0102')
const deflated = (text) => zlib.deflateRawSync(ascii(text))
const contentTypes = (main) =>
  `<?xml version="1.0"?><Types><Override PartName="/x" ContentType="${main}.main+xml"/></Types>`
const tiff = (le, entries) => {
  const u16 = (n) => (le ? u16le(n) : Buffer.from([n >> 8, n & 0xff]))
  const head = le ? cat('II', u16(42), u32le(8)) : cat('MM', u16(42), u32be(8))
  return cat(head, u16(entries.length), ...entries.map((tag) => cat(u16(tag), Buffer.alloc(10))))
}
// A ustar-less tar header whose checksum matches.
const tarV7 = () => {
  const h = Buffer.alloc(512)
  h.write('a.txt', 0)
  h.write('0000644\0', 100)
  let sum = 8 * 0x20
  for (let i = 0; i < 512; i++) if (i < 148 || i >= 156) sum += h[i]
  h.write(sum.toString(8).padStart(6, '0') + '\0 ', 148)
  return h
}

// [name, total length, the bytes it opens with (the rest is zeros)]
const samples = [
  ['empty', 0, Buffer.alloc(0)],
  ['one-byte', 1, hex('42')],
  ['zeros', 5000, Buffer.alloc(0)],
  ['text', 5000, ascii('export function main() {}\n')],
  ['json-object', 5000, ascii('{"asset":{"version":"2.0"}}')],
  ['json-bom', 5000, cat(hex('EFBBBF'), '[1]')],
  ['xml', 200, ascii('<?xml version="1.0"?><svg/>')],
  ['xml-bom', 200, cat(hex('EFBBBF'), '<?xml version="1.0"?>')],
  ['xml-utf16be', 200, cat(hex('FEFF'), Buffer.from('<?xml ', 'utf16le').swap16())],
  ['xml-utf16le', 200, cat(hex('FFFE'), Buffer.from('<?xml ', 'utf16le'))],
  ['utf16le-text', 200, cat(hex('FFFE'), Buffer.from('hello', 'utf16le'))],
  ['svg', 200, ascii('<svg xmlns="http://www.w3.org/2000/svg"/>')],
  ['bom-gif', 5000, cat(hex('EFBBBF'), 'GIF89a')],
  ['bom-mp3-frame', 5000, cat(hex('EFBBBF'), hex('FFFB9064'))],
  ['png', 46, cat(PNG, pngChunk('IHDR', 13), pngChunk('IDAT', 1))],
  ['png-large', 9000, cat(PNG, pngChunk('IHDR', 13), pngChunk('IDAT', 8000))],
  ['png-text-before-idat', 9000, cat(PNG, pngChunk('IHDR', 13), pngChunk('tEXt', 5000), pngChunk('IDAT', 1))],
  ['png-no-idat', 100, cat(PNG, pngChunk('IHDR', 13))],
  ['png-negative-chunk', 100, cat(PNG, hex('FFFFFFFF'), 'IHDR')],
  ['apng', 200, cat(PNG, pngChunk('IHDR', 13), pngChunk('acTL', 8), pngChunk('IDAT', 1))],
  ['jpeg', 5000, hex('FFD8FFE0')],
  ['jpeg-ls', 5000, hex('FFD8FFF7')],
  ['gif', 5000, ascii('GIF89a')],
  ['webp', 5000, riff('WEBP', 'VP8 ')],
  ['bmp', 5000, ascii('BM')],
  ['ico', 5000, hex('00000100 0100')],
  ['cur', 5000, hex('00000200 0100')],
  ['psd', 5000, ascii('8BPS')],
  ['jxr', 5000, hex('4949BC')],
  ['jxl-codestream', 5000, hex('FF0A')],
  ['jxl-container', 5000, hex('0000000C 4A584C20 0D0A870A')],
  ['jp2', 5000, cat(hex('0000000C 6A502020 0D0A870A'), hex('00000014'), 'ftyp', 'jp2 ')],
  ['jpx', 5000, cat(hex('0000000C 6A502020 0D0A870A'), hex('00000014'), 'ftyp', 'jpx ')],
  ['jp2-other', 5000, cat(hex('0000000C 6A502020 0D0A870A'), hex('00000014'), 'ftyp', 'abcd')],
  ['j2c', 5000, hex('FF4FFF51')],
  ['bpg', 5000, hex('425047FB')],
  ['flif', 5000, ascii('FLIF')],
  ['icns', 5000, ascii('icns')],
  ['xcf', 5000, ascii('gimp xcf v001')],
  ['ktx', 5000, hex('AB4B5458203131BB0D0A1A0A')],
  ['ktx2', 5000, hex('AB4B5458203230BB0D0A1A0A')],
  ['tiff-le', 5000, tiff(true, [256, 257])],
  ['tiff-be', 5000, tiff(false, [256, 257])],
  ['tiff-le-no-ifd', 5000, hex('49492A00 08000000')],
  ['tiff-ifd-past-eof', 20, hex('49492A00 FF000000')],
  ['bigtiff', 5000, hex('49492B00 08000000')],
  ['tiff-other-version', 5000, hex('49492C00 08000000')],
  ['arw', 5000, tiff(true, [50341])],
  ['dng-second-tag', 5000, tiff(true, [256, 50706])],
  ['dng-be', 5000, tiff(false, [50706])],
  ['cr2', 5000, cat(hex('49492A00 10000000'), 'CR')],
  ['nef', 5000, cat(hex('49492A00 08000000'), hex('1C00FE00'))],
  ['orf', 5000, hex('4949524F 08000000 18')],
  ['rw2', 5000, hex('49495500 18000000 88E774D8')],
  ['raf', 5000, ascii('FUJIFILMCCD-RAW 0201')],
  ['cr3', 5000, ftyp('crx ')],
  ['glb', 5000, cat('glTF', hex('02000000'))],
  ['glb-v1', 5000, cat('glTF', hex('01000000'))],
  ['draco', 5000, ascii('DRACO')],
  ['stl-ascii', 5000, ascii('solid cube\n')],
  ['fbx', 5000, cat('Kaydara FBX Binary  ', hex('00'))],
  ['blend', 5000, ascii('BLENDER-v300')],
  ['3mf', 5000, cat(zipEntry('[Content_Types].xml', ascii('<Types><Default ContentType="application/vnd.ms-package.3dmanufacturing-3dmodel+xml"/></Types>')), CENTRAL)],
  ['mp3-frame-fffb', 5000, hex('FFFB9064')],
  ['mp3-frame-ffe3', 5000, hex('FFE39064')],
  ['mp2-frame', 5000, hex('FFFD9064')],
  ['mp1-frame', 5000, hex('FFFF9064')],
  ['aac-adts-mpeg4', 5000, hex('FFF15080')],
  ['aac-adts-mpeg2', 5000, hex('FFF95080')],
  ['id3-short', 5, hex('4944330400')],
  ['id3-mp3', 5000, cat(id3(20), Buffer.alloc(20), hex('FFFB9064'))],
  ['id3-flac', 5000, cat(id3(20), Buffer.alloc(20), 'fLaC')],
  ['id3-nothing-after', 5000, cat(id3(20), Buffer.alloc(20))],
  ['id3-tag-past-eof', 100, id3(1000)],
  ['id3-tag-past-4100', 9000, cat(id3(5000), Buffer.alloc(5000), hex('FFFB9064'))],
  ['id3-twice', 5000, cat(id3(10), Buffer.alloc(10), id3(10), Buffer.alloc(10), hex('FFFB9064'))],
  ['ogg-short', 6, ascii('OggS\0\x02')],
  ['ogg-opus', 5000, ogg('OpusHead')],
  ['ogg-vorbis', 5000, ogg('\x01vorbis')],
  ['ogg-flac', 5000, ogg('\x7FFLAC\x01\x00\x00')],
  ['ogg-speex', 5000, ogg('Speex   ')],
  ['ogg-theora', 5000, ogg('\x80theora')],
  ['ogg-video', 5000, ogg('\x01video\0')],
  ['ogg-other', 5000, ogg('xxxxxxxx')],
  ['wav', 5000, riff('WAVE', 'fmt ')],
  ['avi', 5000, riff('AVI ', 'LIST')],
  ['qcp', 5000, riff('QLCM', 'fmt ')],
  ['riff-other', 5000, riff('ABCD', 'efgh')],
  ['aiff', 5000, cat('FORM', hex('00000000'), 'AIFF')],
  ['flac', 5000, ascii('fLaC')],
  ['midi', 5000, cat('MThd', hex('00000006'))],
  ['amr', 5000, ascii('#!AMR\n')],
  ['ac3', 5000, hex('0B77')],
  ['ape', 5000, ascii('MAC ')],
  ['wavpack', 5000, ascii('wvpk')],
  ['dsf', 5000, ascii('DSD ')],
  ['musepack-sv7', 5000, ascii('MP+')],
  ['musepack-sv8', 5000, ascii('MPCK')],
  ['it', 5000, ascii('IMPM')],
  ['xm', 5000, ascii('Extended Module: x')],
  ['s3m', 5000, cat(Buffer.alloc(44), 'SCRM')],
  ['voc', 5000, ascii('Creative Voice File\x1A')],
  ['mp4-isom', 5000, ftyp('isom')],
  ['mp4-mp42', 5000, ftyp('mp42')],
  ['mp4-dash', 5000, ftyp('dash')],
  ['m4a', 5000, ftyp('M4A ')],
  ['m4b', 5000, ftyp('M4B ')],
  ['m4p', 5000, ftyp('M4P ')],
  ['m4v', 5000, ftyp('M4V ')],
  ['f4v', 5000, ftyp('F4V ')],
  ['f4a', 5000, ftyp('F4A ')],
  ['quicktime-ftyp', 5000, ftyp('qt  ')],
  ['avif', 5000, ftyp('avif')],
  ['avis', 5000, ftyp('avis')],
  ['heic', 5000, ftyp('heic')],
  ['heix', 5000, ftyp('heix')],
  ['mif1', 5000, ftyp('mif1')],
  ['msf1', 5000, ftyp('msf1')],
  ['hevc', 5000, ftyp('hevc')],
  ['3gp', 5000, ftyp('3gp4')],
  ['3g2', 5000, ftyp('3g2a')],
  ['ftyp-brand-nul', 5000, cat(hex('00000018'), 'ftyp', 'qt\0 ')],
  ['ftyp-brand-nbsp', 5000, cat(hex('00000018'), 'ftyp', hex('A0717420'))],
  ['ftyp-brand-nel', 5000, cat(hex('00000018'), 'ftyp', hex('71742085'))],
  ['ftyp-unprintable-brand', 5000, cat(hex('00000018'), 'ftyp', hex('01020304'))],
  ['mov-moov', 5000, cat(hex('00000008'), 'moov')],
  ['mov-free', 5000, cat(hex('00000008'), 'free')],
  ['webm', 5000, ebml('webm')],
  ['matroska', 5000, ebml('matroska')],
  ['webm-nul-padded', 5000, ebml('webm\0\0')],
  ['ebml-other-doctype', 5000, ebml('other')],
  ['ebml-truncated', 6, hex('1A45DFA3 8542')],
  ['flv', 5000, hex('464C5601')],
  ['mpeg-ps-mp1s', 5000, hex('000001BA 21')],
  ['mpeg-ps-mp2p', 5000, hex('000001BA 44')],
  ['mpeg-ps-other', 5000, hex('000001BA 00')],
  ['mpeg-video', 5000, hex('000001B3')],
  ['mts', 5000, cat(hex('47'), Buffer.alloc(187), hex('47'))],
  ['mts-bdav', 5000, cat(hex('00000000 47'), Buffer.alloc(191), hex('47'))],
  ['mpeg-ts-in-mp3-frame', 5000, cat(hex('FFFB9064 47'), Buffer.alloc(187), hex('47'))],
  ['asf-audio', 5000, cat(hex('3026B2758E66CF11A6D900AA0062CE6C'), Buffer.alloc(14), hex('9107DCB7B7A9CF118EE600C00C205365'), u32le(80), u32le(0), hex('409E69F84D5BCF11A8FD00805F5C442B'))],
  ['asf-video', 5000, cat(hex('3026B2758E66CF11A6D900AA0062CE6C'), Buffer.alloc(14), hex('9107DCB7B7A9CF118EE600C00C205365'), u32le(80), u32le(0), hex('C0EF19BC4D5BCF11A8FD00805F5C442B'))],
  ['asf-skip-then-audio', 5000, cat(hex('3026B2758E66CF11A6D900AA0062CE6C'), Buffer.alloc(14), Buffer.alloc(16, 1), u32le(30), u32le(0), Buffer.alloc(6), hex('9107DCB7B7A9CF118EE600C00C205365'), u32le(80), u32le(0), hex('409E69F84D5BCF11A8FD00805F5C442B'))],
  ['asf-huge-object', 5000, cat(hex('3026B2758E66CF11A6D900AA0062CE6C'), Buffer.alloc(14), Buffer.alloc(16, 1), hex('FFFFFFFFFFFFFFFF'))],
  ['asf-other-stream', 5000, cat(hex('3026B2758E66CF11A6D900AA0062CE6C'), Buffer.alloc(14), hex('9107DCB7B7A9CF118EE600C00C205365'), u32le(80), u32le(0), Buffer.alloc(16, 2))],
  ['mxf', 5000, hex('060E2B34020501010D0102010102')],
  ['rm', 5000, ascii('.RMF')],
  ['woff', 5000, cat('wOFF', hex('00010000'))],
  ['woff2', 5000, cat('wOF2', 'OTTO')],
  ['otf', 5000, ascii('OTTO\0')],
  ['ttf', 5000, hex('0001000000')],
  ['ttc', 5000, ascii('ttcf')],
  ['eot', 5000, cat(Buffer.alloc(8), hex('010002'), Buffer.alloc(23), 'LP')],
  ['wasm', 5000, hex('0061736D01000000')],
  ['pdf', 5000, ascii('%PDF-1.7\n')],
  ['pdf-illustrator', 5000, cat('%PDF-1.5\n', Buffer.alloc(2000), 'AIPrivateData1')],
  ['ps', 5000, ascii('%!PS-Adobe-3.0\n')],
  ['eps', 5000, ascii('%!PS-Adobe-3.0 EPSF-3.0\n')],
  ['eps-binary', 5000, hex('C5D0D3C6')],
  ['rtf', 5000, ascii('{\\rtf1')],
  ['vtt', 5000, ascii('WEBVTT\n')],
  ['vtt-eof', 6, ascii('WEBVTT')],
  ['vcard', 5000, ascii('BEGIN:VCARD\r\n')],
  ['ics', 5000, ascii('BEGIN:VCALENDAR\r\n')],
  ['pgp', 5000, ascii('-----BEGIN PGP MESSAGE-----\n')],
  ['icc', 5000, cat(Buffer.alloc(36), 'acsp')],
  ['dicom', 5000, cat(Buffer.alloc(128), 'DICM')],
  ['mobi', 5000, cat(Buffer.alloc(60), 'BOOKMOBI')],
  ['sqlite', 5000, ascii('SQLite format 3\0')],
  ['zip-empty-entry', 5000, cat(zipEntry('a.txt', Buffer.alloc(0)), CENTRAL)],
  ['zip-entry-past-eof', 100, zipEntry('a.txt', Buffer.alloc(0), { size: 1000 })],
  ['zip-header-only', 5000, hex('504B0304')],
  ['zip-bad-signature', 5000, cat(zipEntry('a.txt', ascii('hi')), hex('DEADBEEF'))],
  ['zip-encrypted-signature', 5000, cat(zipEntry('a.txt', ascii('hi')), hex('D0CF11E0'))],
  ['zip-empty-archive', 5000, hex('504B0506')],
  ['zip-spanned', 5000, hex('504B0708')],
  ['jar', 5000, cat(zipEntry('META-INF/', Buffer.alloc(0)), zipEntry('META-INF/MANIFEST.MF', ascii('Manifest-Version: 1.0\n')), CENTRAL)],
  ['xpi', 5000, cat(zipEntry('META-INF/mozilla.rsa', ascii('x')), CENTRAL)],
  ['apk', 5000, cat(zipEntry('AndroidManifest.xml', ascii('x')), zipEntry('classes2.dex', ascii('dex')), CENTRAL)],
  ['epub', 5000, cat(zipEntry('mimetype', ascii('application/epub+zip')), CENTRAL)],
  ['odt-padded', 5000, cat(zipEntry('mimetype', ascii('  application/vnd.oasis.opendocument.TEXT\n')), CENTRAL)],
  ['mimetype-unknown', 5000, cat(zipEntry('mimetype', ascii('application/x-other')), CENTRAL)],
  ['docx-stored', 5000, cat(zipEntry('[Content_Types].xml', ascii(contentTypes('application/vnd.openxmlformats-officedocument.wordprocessingml.document'))), CENTRAL)],
  ['xlsx-deflated', 5000, cat(zipEntry('[Content_Types].xml', deflated(contentTypes('application/vnd.openxmlformats-officedocument.spreadsheetml.sheet')), { method: 8 }), CENTRAL)],
  ['xlsm-macro', 5000, cat(zipEntry('[Content_Types].xml', ascii(contentTypes('application/vnd.ms-excel.sheet.macroEnabled'))), CENTRAL)],
  ['vsdx', 5000, cat(zipEntry('[Content_Types].xml', ascii(contentTypes('application/vnd.ms-visio.drawing'))), CENTRAL)],
  ['pptx-after-other', 5000, cat(zipEntry('_rels/.rels', ascii('rels')), zipEntry('[Content_Types].xml', ascii(contentTypes('application/vnd.openxmlformats-officedocument.presentationml.presentation'))), CENTRAL)],
  ['docx-data-descriptor', 5000, cat(zipEntry('[Content_Types].xml', deflated(contentTypes('application/vnd.openxmlformats-officedocument.wordprocessingml.document')), { method: 8, flags: 8, size: 0 }), hex('504B0708'), Buffer.alloc(12), CENTRAL)],
  ['zip-data-descriptor-skip', 5000, cat(zipEntry('a.txt', ascii('hello'), { flags: 8, size: 0 }), hex('504B0708'), Buffer.alloc(12), zipEntry('mimetype', ascii('application/epub+zip')), CENTRAL)],
  ['zip-data-descriptor-missing', 5000, zipEntry('a.txt', ascii('hello'), { flags: 8, size: 0 })],
  ['zip-unsupported-method', 5000, cat(zipEntry('mimetype', ascii('x'), { method: 12 }), CENTRAL)],
  ['zip-bad-deflate', 5000, cat(zipEntry('mimetype', hex('FFFFFFFF'), { method: 8 }), CENTRAL)],
  ['zip-truncated-deflate', 5000, cat(zipEntry('mimetype', deflated('application/epub+zip').subarray(0, 5), { method: 8 }), CENTRAL)],
  ['gzip', 5000, hex('1F8B08')],
  ['tar-gz', 5000, zlib.gzipSync(cat('a.txt', Buffer.alloc(252), 'ustar\x0000'))],
  ['bz2', 5000, ascii('BZh91AY')],
  ['xz', 5000, hex('FD377A585A00')],
  ['7z', 5000, hex('377ABCAF271C')],
  ['rar', 5000, hex('526172211A0700')],
  ['rar5', 5000, hex('526172211A070100')],
  ['zstd', 5000, hex('28B52FFD')],
  ['lz4', 5000, hex('04224D18')],
  ['lzip', 5000, ascii('LZIP')],
  ['compress', 5000, hex('1F9D90')],
  ['lzh', 5000, cat(hex('0000'), '-lh5-')],
  ['cab', 5000, ascii('MSCF')],
  ['tar-ustar', 5000, cat('a.txt', Buffer.alloc(252), 'ustar\x0000')],
  ['tar-ustar-gnu', 5000, cat('a.txt', Buffer.alloc(252), 'ustar  \0')],
  ['tar-v7', 5000, tarV7()],
  ['ar', 5000, cat('!<arch>\n', 'other.txt/    ')],
  ['deb', 5000, cat('!<arch>\n', 'debian-binary   ')],
  ['ar-ascii-high-bit', 5000, cat('!<arch>\n', Buffer.from(ascii('debian-binary').map((b) => b | 0x80)))],
  ['cpio', 5000, ascii('070707')],
  ['rpm', 5000, hex('EDABEEDB')],
  ['dmg', 5000, hex('7801')],
  ['exe', 5000, ascii('MZ')],
  ['elf', 5000, hex('7F454C46')],
  ['macho', 5000, hex('CFFAEDFE')],
  ['class', 5000, hex('CAFEBABE')],
  ['swf', 5000, ascii('FWS')],
  ['crx', 5000, ascii('Cr24')],
  ['nes', 5000, hex('4E45531A')],
  ['parquet', 5000, ascii('PAR1')],
  ['arrow', 5000, ascii('ARROW1\0\0')],
  ['avro', 5000, hex('4F626A01')],
  ['cfb', 5000, hex('D0CF11E0A1B11AE1')],
  ['pst', 5000, ascii('!BDN')],
  ['chm', 5000, ascii('ITSF')],
  ['pcap', 5000, hex('D4C3B2A1')],
  ['dwg', 5000, ascii('AC1018')],
  ['dwg-exponent', 5000, ascii('AC1e3 ')],
  ['dwg-out-of-range', 5000, ascii('AC1051')],
  ['reg', 5000, ascii('REGEDIT4\r\n')],
  ['reg-utf16', 5000, cat(hex('FFFE'), Buffer.from('Windows Registry Editor Version 5.00\r\n', 'utf16le'))],
  ['sketchup', 5000, cat(hex('FFFEFF0E'), Buffer.from('SketchUp Model', 'utf16le'))],
  ['lnk', 5000, hex('4C0000000114020000000000C000000000000046')],
  ['alias', 5000, hex('626F6F6B000000006D61726B00000000')],
  ['indd', 5000, hex('0606EDF5D81D46E5BD31EFE7FE74B71D')],
  ['mie', 5000, cat(hex('7E1004'), hex('00'), '0MIE')],
  ['shp', 5000, hex('0000270A000000000000000000000000')],
  ['regf', 5000, ascii('regf')],
  ['ace', 5000, cat(Buffer.alloc(7), '**ACE**')],
  ['asar', 5000, cat(hex('04000000'), u32le(0), u32le(0), u32le(13), '{"files":{} }')],
  ['asar-no-files', 5000, cat(hex('04000000'), u32le(0), u32le(0), u32le(14), '{"other":true}')],
  ['asar-files-null', 5000, cat(hex('04000000'), u32le(0), u32le(0), u32le(14), '{"files":null}')]
]

const bytesOf = ([, length, head]) => {
  const out = Buffer.alloc(length)
  head.copy(out, 0, 0, Math.min(head.length, length))
  return out
}

async function main() {
  const { FileTypeParser } = await import(path.join(modules, 'file-type', 'index.js'))
  async function sniff(file, options) {
    const stream = fs.createReadStream(file, options)
    try {
      return (await new FileTypeParser().fromStream(stream))?.mime ?? FALLBACK
    } catch {
      return 'error'
    } finally {
      stream.destroy()
    }
  }
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'content-sniff-'))
  const rows = []
  for (const sample of samples) {
    const [name, length, head] = sample
    const file = path.join(dir, name)
    fs.writeFileSync(file, bytesOf(sample))
    const full = await sniff(file)
    const range = await sniff(file, { start: 0, end: Math.max(0, Math.min(RANGE, length) - 1) })
    rows.push([name, length, head.subarray(0, length).toString('hex'), full, range].join('\t'))
  }
  fs.rmSync(dir, { recursive: true })
  const pins = ['file-type', 'strtok3', 'token-types', '@tokenizer/inflate'].map((n) => `${n}@${version(n)}`).join(', ')
  process.stdout.write(
    `# catalyst content server ?includeMimeType, ${pins}; generated by capture-sniff.cjs\n` +
      '# name\tlength\thead (hex; the rest is zeros)\tfull GET\tranged GET (bytes 0-4099)\n' +
      rows.join('\n') +
      '\n'
  )
}

main()
