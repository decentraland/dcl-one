import * as zlib from 'zlib'

export type ZipEntry = {
  /** The archive path, `/`-separated; a directory ends with `/`. */
  name: string
  data: Buffer
  /** The Unix mode, file type included, when a Unix tool wrote the entry, else 0. */
  mode: number
}

/** The most bytes the entries of one archive may hold: all of them stay in memory. */
export const MAX_ARCHIVE_SIZE = 1024 * 1024 * 1024

const LOCAL_HEADER = 0x04034b50
const CENTRAL_HEADER = 0x02014b50
const END_OF_CENTRAL_DIRECTORY = 0x06054b50
const STORED = 0
const DEFLATED = 8
const HOST_UNIX = 3

/**
 * The entries of a zip archive held in memory: stored and deflated ones, which is all a GitHub
 * branch archive carries. Zip64, encryption and other compression methods are refused. The
 * archive is read whole and decoded synchronously: streaming it through extract-zip (yauzl 2 over
 * fd-slicer) stalled on stored entries under Node 24 and later.
 */
export function readZip(archive: Buffer): ZipEntry[] {
  const end = findEndOfCentralDirectory(archive)
  const count = archive.readUInt16LE(end + 10)
  const directoryOffset = archive.readUInt32LE(end + 16)
  if (count === 0xffff || directoryOffset === 0xffffffff) throw new Error('zip64 archives are not supported')
  const entries: ZipEntry[] = []
  let total = 0
  let offset = directoryOffset
  for (let i = 0; i < count; i++) {
    if (offset + 46 > archive.length || archive.readUInt32LE(offset) !== CENTRAL_HEADER) {
      throw new Error(`corrupt zip: central directory entry ${i} is missing`)
    }
    const host = archive.readUInt8(offset + 5)
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
    const name = archive.toString('utf8', offset + 46, offset + 46 + nameLength)
    offset += 46 + nameLength + extraLength + commentLength
    if (flags & 0x1) throw new Error(`encrypted zip entry: ${name}`)
    if (compressedSize === 0xffffffff || size === 0xffffffff) throw new Error('zip64 archives are not supported')
    total += size
    if (total > MAX_ARCHIVE_SIZE) throw new Error(`zip archive holds more than ${MAX_ARCHIVE_SIZE} bytes`)
    if (localOffset + 30 > archive.length || archive.readUInt32LE(localOffset) !== LOCAL_HEADER) {
      throw new Error(`corrupt zip: local header of ${name} is missing`)
    }
    const dataOffset =
      localOffset + 30 + archive.readUInt16LE(localOffset + 26) + archive.readUInt16LE(localOffset + 28)
    if (dataOffset + compressedSize > archive.length) throw new Error(`corrupt zip: ${name} is truncated`)
    const raw = archive.subarray(dataOffset, dataOffset + compressedSize)
    let data: Buffer
    switch (method) {
      case STORED:
        data = raw
        break
      case DEFLATED:
        // Some writers give an empty entry no deflate data, which zlib refuses as a cut stream.
        data = compressedSize === 0 ? Buffer.alloc(0) : inflate(raw, size, name)
        break
      default:
        throw new Error(`unsupported zip compression method ${method}: ${name}`)
    }
    if (data.length !== size) throw new Error(`corrupt zip: ${name} inflates to ${data.length} bytes, not ${size}`)
    if (zlib.crc32(data) !== crc) throw new Error(`corrupt zip: ${name} fails its checksum`)
    entries.push({ name, data, mode: host === HOST_UNIX ? externalAttributes >>> 16 : 0 })
  }
  return entries
}

/** Inflates no more than the size that the central directory records. */
function inflate(raw: Buffer, size: number, name: string): Buffer {
  try {
    return zlib.inflateRawSync(raw, { maxOutputLength: Math.max(size, 1) })
  } catch (error) {
    if ((error as { code?: string }).code !== 'ERR_BUFFER_TOO_LARGE') throw error
    throw new Error(`corrupt zip: ${name} inflates to more than ${size} bytes`)
  }
}

/** The record's offset: it closes the archive, after a comment of up to 65535 bytes. */
function findEndOfCentralDirectory(archive: Buffer): number {
  const last = archive.length - 22
  for (let i = last; i >= 0 && i >= last - 0xffff; i--) {
    if (archive.readUInt32LE(i) === END_OF_CENTRAL_DIRECTORY && archive.readUInt16LE(i + 20) === last - i) return i
  }
  throw new Error('not a zip archive')
}
