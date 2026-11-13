import * as zlib from 'zlib'

export type ZipEntry = {
  /** The archive path, `/`-separated; a directory ends with `/`. */
  name: string
  /** The uncompressed size the central directory declares. */
  size: number
  /** Unix permission bits when the archive records them, else 0. */
  mode: number
  /** Decodes the entry's bytes; throws when they do not match the declared size or checksum. */
  read(): Buffer
}

const LOCAL_HEADER = 0x04034b50
const CENTRAL_HEADER = 0x02014b50
const END_OF_CENTRAL_DIRECTORY = 0x06054b50
const STORED = 0
const DEFLATED = 8
/** Far more than a template holds: the ceiling on what an archive may write in total. */
export const MAX_TOTAL_SIZE = 1 << 30

/**
 * The entries of a zip archive held in memory: stored and deflated ones, which is all a GitHub
 * branch archive carries. Zip64, encryption, symbolic links and other compression methods are
 * refused. The central directory is checked whole before anything is decoded: every record
 * names its own local header and data (none may alias or overlap another's), no name repeats,
 * and the declared sizes add up to at most MAX_TOTAL_SIZE, which bounds the output since each
 * entry is read against its declared size. Each entry is decoded when it is read, so only one
 * lives in memory at a time. (The archive is read whole and decoded synchronously: streaming it
 * through extract-zip (yauzl 2 over fd-slicer) stalled on stored entries under Node 24 and later.)
 */
export function readZip(archive: Buffer): ZipEntry[] {
  const end = findEndOfCentralDirectory(archive)
  const count = archive.readUInt16LE(end + 10)
  const directoryOffset = archive.readUInt32LE(end + 16)
  if (count === 0xffff || directoryOffset === 0xffffffff) throw new Error('zip64 archives are not supported')
  const entries: ZipEntry[] = []
  const names = new Set<string>()
  const ranges: { name: string; start: number; end: number }[] = []
  let total = 0
  let offset = directoryOffset
  for (let i = 0; i < count; i++) {
    if (offset + 46 > end || archive.readUInt32LE(offset) !== CENTRAL_HEADER) {
      throw new Error(`corrupt zip: central directory entry ${i} is missing`)
    }
    const flags = archive.readUInt16LE(offset + 8)
    const method = archive.readUInt16LE(offset + 10)
    const crc = archive.readUInt32LE(offset + 16)
    const compressedSize = archive.readUInt32LE(offset + 20)
    const size = archive.readUInt32LE(offset + 24)
    const nameLength = archive.readUInt16LE(offset + 28)
    const extraLength = archive.readUInt16LE(offset + 30)
    const commentLength = archive.readUInt16LE(offset + 32)
    const externalAttributes = archive.readUInt32LE(offset + 38)
    const localOffset = archive.readUInt32LE(offset + 42)
    if (offset + 46 + nameLength + extraLength + commentLength > end) {
      throw new Error(`corrupt zip: central directory entry ${i} is truncated`)
    }
    const name = archive.toString('utf8', offset + 46, offset + 46 + nameLength)
    offset += 46 + nameLength + extraLength + commentLength
    if (flags & 0x1) throw new Error(`encrypted zip entry: ${name}`)
    if (((externalAttributes >>> 16) & 0o170000) === 0o120000) throw new Error(`symbolic link zip entry: ${name}`)
    if (compressedSize === 0xffffffff || size === 0xffffffff) throw new Error('zip64 archives are not supported')
    if (method !== STORED && method !== DEFLATED) {
      throw new Error(`unsupported zip compression method ${method}: ${name}`)
    }
    if (names.has(name)) throw new Error(`corrupt zip: ${name} is listed twice`)
    names.add(name)
    total += size
    if (total > MAX_TOTAL_SIZE) throw new Error(`zip inflates past ${MAX_TOTAL_SIZE} bytes in total`)
    if (localOffset + 30 > archive.length || archive.readUInt32LE(localOffset) !== LOCAL_HEADER) {
      throw new Error(`corrupt zip: local header of ${name} is missing`)
    }
    const dataOffset =
      localOffset + 30 + archive.readUInt16LE(localOffset + 26) + archive.readUInt16LE(localOffset + 28)
    if (dataOffset + compressedSize > archive.length) throw new Error(`corrupt zip: ${name} is truncated`)
    ranges.push({ name, start: localOffset, end: dataOffset + compressedSize })
    const raw = archive.subarray(dataOffset, dataOffset + compressedSize)
    entries.push({
      name,
      size,
      mode: (externalAttributes >>> 16) & 0o777,
      read: () => decode(name, raw, method, size, crc)
    })
  }
  ranges.sort((a, b) => a.start - b.start)
  for (let i = 1; i < ranges.length; i++) {
    if (ranges[i].start < ranges[i - 1].end) {
      throw new Error(`corrupt zip: ${ranges[i].name} overlaps ${ranges[i - 1].name}`)
    }
  }
  return entries
}

function decode(name: string, raw: Buffer, method: number, size: number, crc: number): Buffer {
  let data = raw
  if (method === DEFLATED) {
    try {
      // Inflation stops at the declared size: a stream claiming to be small cannot expand without bound.
      data = zlib.inflateRawSync(raw, { maxOutputLength: Math.max(size, 1) })
    } catch (error: any) {
      if (error.code === 'ERR_BUFFER_TOO_LARGE') throw new Error(`corrupt zip: ${name} inflates past its ${size} bytes`)
      throw new Error(`corrupt zip: ${name} does not inflate: ${error.message}`)
    }
  }
  if (data.length !== size) throw new Error(`corrupt zip: ${name} inflates to ${data.length} bytes, not ${size}`)
  if (zlib.crc32(data) !== crc) throw new Error(`corrupt zip: ${name} fails its checksum`)
  return data
}

/** The record's offset: it closes the archive, after a comment of up to 65535 bytes. */
function findEndOfCentralDirectory(archive: Buffer): number {
  const last = archive.length - 22
  for (let i = last; i >= 0 && i >= last - 0xffff; i--) {
    if (archive.readUInt32LE(i) === END_OF_CENTRAL_DIRECTORY && archive.readUInt16LE(i + 20) === last - i) return i
  }
  throw new Error('not a zip archive')
}
