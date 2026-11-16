// Captures worlds-content-server's Content-Type answers for `worlds-sniff.tsv`.
//
//   node capture-sniff.cjs <worlds-content-server checkout> > worlds-sniff.tsv
//
// Each sample is written to a file and read back as `content-file-handler.ts` does: `fromStream`
// over a file read stream (`full`), and over the stream of bytes 0-4099 that a ranged GET sniffs
// (`range`). A throw or no answer is `application/octet-stream`, the handler's fallback.
'use strict'
const fs = require('fs')
const os = require('os')
const path = require('path')

const root = process.argv[2]
if (!root) throw new Error('usage: node capture-sniff.cjs <worlds-content-server checkout>')
const { fromStream } = require(path.join(root, 'node_modules', 'file-type'))
const fileTypeVersion = require(path.join(root, 'node_modules', 'file-type', 'package.json')).version
const revision = require('child_process').execFileSync('git', ['-C', root, 'rev-parse', 'HEAD']).toString().trim()

const MIME_SNIFF_BYTES = 4100 // content-file-handler.ts
const FALLBACK = 'application/octet-stream'

const hex = (s) => Buffer.from(s.replace(/\s+/g, ''), 'hex')
const ascii = (s) => Buffer.from(s, 'latin1')
const u32be = (n) => {
  const b = Buffer.alloc(4)
  b.writeUInt32BE(n)
  return b
}
// ID3v2 tag header: the size is four 7-bit bytes.
const id3 = (size) =>
  Buffer.concat([ascii('ID3'), hex('0400 00'), Buffer.from([(size >> 21) & 0x7f, (size >> 14) & 0x7f, (size >> 7) & 0x7f, size & 0x7f])])
const pngChunk = (type, length) => Buffer.concat([u32be(length), ascii(type), Buffer.alloc(length + 4)])
const PNG = hex('89504E470D0A1A0A')
const ebml = (docType) =>
  Buffer.concat([hex('1A45DFA3'), Buffer.from([0x80 | (docType.length + 3 + 4)]), hex('4286 81 01'), hex('4282'), Buffer.from([0x80 | docType.length]), ascii(docType)])
const ftyp = (brand) => Buffer.concat([hex('00000018'), ascii('ftyp'), ascii(brand), hex('00000000'), ascii('isomiso2')])
const ogg = (codec) => Buffer.concat([ascii('OggS'), Buffer.alloc(24), ascii(codec)])

// [name, total length, the bytes it opens with (the rest is zeros)]
const samples = [
  ['empty', 0, Buffer.alloc(0)],
  ['one-byte', 1, hex('42')],
  ['zeros', 5000, Buffer.alloc(0)],
  ['text', 5000, ascii('export function main() {}\n')],
  ['json-object', 5000, ascii('{"asset":{"version":"2.0"}}')],
  ['xml', 200, ascii('<?xml version="1.0"?><svg/>')],
  ['svg', 200, ascii('<svg xmlns="http://www.w3.org/2000/svg"/>')],
  ['png', 46, Buffer.concat([PNG, pngChunk('IHDR', 13), pngChunk('IDAT', 1)])],
  ['png-large', 9000, Buffer.concat([PNG, pngChunk('IHDR', 13), pngChunk('IDAT', 8000)])],
  ['png-text-before-idat', 9000, Buffer.concat([PNG, pngChunk('IHDR', 13), pngChunk('tEXt', 5000), pngChunk('IDAT', 1)])],
  ['apng', 200, Buffer.concat([PNG, pngChunk('IHDR', 13), pngChunk('acTL', 8), pngChunk('IDAT', 1)])],
  ['apng-actl-past-4100', 9000, Buffer.concat([PNG, pngChunk('IHDR', 13), pngChunk('tEXt', 5000), pngChunk('acTL', 8)])],
  ['jpeg', 5000, hex('FFD8FFE0')],
  ['gif', 5000, ascii('GIF89a')],
  ['webp', 5000, Buffer.concat([ascii('RIFF'), hex('00000000'), ascii('WEBPVP8 ')])],
  ['bmp', 5000, ascii('BM')],
  ['tiff-le', 5000, hex('49492A00 08000000')],
  ['tiff-be', 5000, hex('4D4D002A 00000008')],
  ['ico', 5000, hex('00000100 0100')],
  ['ktx', 5000, hex('AB4B5458203131BB0D0A1A0A')],
  ['ktx2', 5000, hex('AB4B5458203230BB0D0A1A0A')],
  ['glb', 5000, Buffer.concat([ascii('glTF'), hex('02000000')])],
  ['glb-v1', 5000, Buffer.concat([ascii('glTF'), hex('01000000')])],
  ['mp3-frame-fffb', 5000, hex('FFFB9064')],
  ['mp3-frame-fff3', 5000, hex('FFF39064')],
  ['mp3-frame-ffe3', 5000, hex('FFE39064')],
  ['mp2-frame', 5000, hex('FFFD9064')],
  ['aac-adts-mpeg4', 5000, hex('FFF15080')],
  ['aac-adts-mpeg2', 5000, hex('FFF95080')],
  ['id3-short', 5, hex('4944330400')],
  ['id3-mp3', 5000, Buffer.concat([id3(20), Buffer.alloc(20), hex('FFFB9064')])],
  ['id3-flac', 5000, Buffer.concat([id3(20), Buffer.alloc(20), ascii('fLaC')])],
  ['id3-nothing-after', 5000, Buffer.concat([id3(20), Buffer.alloc(20)])],
  ['id3-tag-past-eof', 100, id3(1000)],
  ['id3-tag-past-4100', 9000, Buffer.concat([id3(5000), Buffer.alloc(5000), hex('FFFB9064')])],
  ['id3-twice', 5000, Buffer.concat([id3(10), Buffer.alloc(10), id3(10), Buffer.alloc(10), hex('FFFB9064')])],
  ['ogg-short', 6, ascii('OggS\0\x02')],
  ['ogg-opus', 5000, ogg('OpusHead')],
  ['ogg-vorbis', 5000, ogg('\x01vorbis')],
  ['ogg-theora', 5000, ogg('\x80theora')],
  ['ogg-other', 5000, ogg('xxxxxxxx')],
  ['wav', 5000, Buffer.concat([ascii('RIFF'), hex('00000000'), ascii('WAVEfmt ')])],
  ['avi', 5000, Buffer.concat([ascii('RIFF'), hex('00000000'), ascii('AVI LIST')])],
  ['aiff', 5000, Buffer.concat([ascii('FORM'), hex('00000000'), ascii('AIFF')])],
  ['flac', 5000, ascii('fLaC')],
  ['midi', 5000, Buffer.concat([ascii('MThd'), hex('00000006')])],
  ['mp4-isom', 5000, ftyp('isom')],
  ['m4a', 5000, ftyp('M4A ')],
  ['quicktime-ftyp', 5000, ftyp('qt  ')],
  ['avif', 5000, ftyp('avif')],
  ['heic', 5000, ftyp('heic')],
  ['3gp', 5000, ftyp('3gp4')],
  ['ftyp-unprintable-brand', 5000, Buffer.concat([hex('00000018'), ascii('ftyp'), hex('01020304')])],
  ['mov-moov', 5000, Buffer.concat([hex('00000008'), ascii('moov')])],
  ['mov-mdat', 5000, Buffer.concat([hex('00000008'), ascii('mdat')])],
  ['webm', 5000, ebml('webm')],
  ['matroska', 5000, ebml('matroska')],
  ['ebml-other-doctype', 5000, ebml('other')],
  ['pdf-small', 100, ascii('%PDF-1.7\n')],
  ['pdf-1350', 1350, ascii('%PDF-1.7\n')],
  ['pdf-1351', 1351, ascii('%PDF-1.7\n')],
  ['pdf', 5000, ascii('%PDF-1.7\n')],
  ['pdf-illustrator', 5000, Buffer.concat([ascii('%PDF-1.5\n'), Buffer.alloc(2000), ascii('AIPrivateData1')])],
  ['woff', 5000, Buffer.concat([ascii('wOFF'), hex('00010000')])],
  ['woff2', 5000, Buffer.concat([ascii('wOF2'), ascii('OTTO')])],
  ['otf', 5000, ascii('OTTO\0')],
  ['ttf', 5000, hex('0001000000')],
  ['wasm', 5000, hex('0061736D01000000')],
  ['mpeg-ps-mp2p', 5000, hex('000001BA 44')],
  ['mpeg-ps-mov-atom', 5000, Buffer.concat([hex('000001BA'), ascii('mdat')])],
  ['mpeg-ps-other', 5000, hex('000001BA 00')],
  ['mpeg-ts-in-mp3-frame', 5000, Buffer.concat([hex('FFFB9064 47'), Buffer.alloc(187), hex('47')])],
  ['sketchup', 5000, Buffer.concat([hex('FFFEFF0E'), Buffer.from('SketchUp Model', 'utf16le')])],
  ['ftyp-brand-nel', 5000, Buffer.concat([hex('00000018'), ascii('ftyp'), hex('71742085')])],
  ['zip', 5000, Buffer.concat([hex('504B0304'), Buffer.alloc(22), hex('0500 0000'), ascii('a.txt')])],
  ['gzip', 5000, hex('1F8B08')]
]

const bytesOf = ([, length, head]) => {
  const out = Buffer.alloc(length)
  head.copy(out, 0, 0, Math.min(head.length, length))
  return out
}

async function sniff(file, options) {
  const stream = fs.createReadStream(file, options)
  try {
    return (await fromStream(stream))?.mime ?? FALLBACK
  } catch {
    return FALLBACK
  } finally {
    stream.destroy()
  }
}

async function main() {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'worlds-sniff-'))
  const rows = []
  for (const sample of samples) {
    const [name, length, head] = sample
    const file = path.join(dir, name)
    fs.writeFileSync(file, bytesOf(sample))
    const full = await sniff(file)
    const range = await sniff(file, { start: 0, end: Math.max(0, Math.min(MIME_SNIFF_BYTES, length) - 1) })
    rows.push([name, length, head.toString('hex'), full, range].join('\t'))
  }
  fs.rmSync(dir, { recursive: true })
  process.stdout.write(
    `# worlds-content-server ${revision}, file-type@${fileTypeVersion}; generated by capture-sniff.cjs\n` +
      '# name\tlength\thead (hex; the rest is zeros)\tfull GET\tranged GET (bytes 0-4099)\n' +
      rows.join('\n') +
      '\n'
  )
}

main()
