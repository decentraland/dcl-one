'use strict';

// Writes a zip archive with `node:zlib` and no other package. `zip(entries)` gives the bytes of
// the archive in order, so the caller can send them to a file with `stream.pipeline`. It reads
// one entry at a time and never holds a file entry in memory.
//
// A stored file entry is read two times: first for its checksum and size, which go in the local
// header, then for its data. If the file changes between the two reads, it stops with an error.
// A deflated file entry is read one time: its sizes and checksum go in a data descriptor after its
// data.
//
// It writes zip64 records only where the plain format is too small: for an entry that is larger
// than 4 GiB or starts after 4 GiB, and for an archive with 65535 entries or more, or with a
// central directory that is larger than 4 GiB or starts after 4 GiB. The option `forceZip64`
// writes them for each entry and for the archive.

const fs = require('node:fs');
const zlib = require('node:zlib');
const { pipeline } = require('node:stream');
const { promisify } = require('node:util');

const deflateRaw = promisify(zlib.deflateRaw);

const LOCAL_SIGNATURE = 0x04034b50;
const DESCRIPTOR_SIGNATURE = 0x08074b50;
const CENTRAL_SIGNATURE = 0x02014b50;
const END_SIGNATURE = 0x06054b50;
const ZIP64_END_SIGNATURE = 0x06064b50;
const ZIP64_LOCATOR_SIGNATURE = 0x07064b50;
const ZIP64_EXTRA = 0x0001;
const VERSION = 20;
const VERSION_ZIP64 = 45;
const HOST_UNIX = 3;
const FILE_MODE = 0o100644;
const FLAG_DESCRIPTOR = 0x8;
const FLAG_UTF8 = 0x800;
const METHOD_STORED = 0;
const METHOD_DEFLATED = 8;
// The plain format keeps these values for zip64, so a count or a size must be smaller.
const ZIP64_16 = 0xffff;
const ZIP64_32 = 0xffffffff;
const MAX_NAME_SIZE = 0xffff;
const READ_SIZE = 1 << 20;

function dosTime(date) {
  const year = Math.max(date.getFullYear(), 1980);
  return {
    time: (date.getHours() << 11) | (date.getMinutes() << 5) | (date.getSeconds() >> 1),
    date: ((year - 1980) << 9) | ((date.getMonth() + 1) << 5) | date.getDate(),
  };
}

function zip64Extra(values) {
  const extra = Buffer.alloc(4 + 8 * values.length);
  extra.writeUInt16LE(ZIP64_EXTRA, 0);
  extra.writeUInt16LE(8 * values.length, 2);
  values.forEach((value, index) => extra.writeBigUInt64LE(BigInt(value), 4 + 8 * index));
  return extra;
}

// The local header of a zip64 entry has the zip64 field, and a data descriptor of 64-bit sizes.
function localHeader(record) {
  const known = !(record.flags & FLAG_DESCRIPTOR);
  const extra = record.zip64
    ? zip64Extra(known ? [record.size, record.compressedSize] : [0, 0])
    : Buffer.alloc(0);
  const header = Buffer.alloc(30);
  header.writeUInt32LE(LOCAL_SIGNATURE, 0);
  header.writeUInt16LE(record.zip64 ? VERSION_ZIP64 : VERSION, 4);
  header.writeUInt16LE(record.flags, 6);
  header.writeUInt16LE(record.method, 8);
  header.writeUInt16LE(record.time, 10);
  header.writeUInt16LE(record.date, 12);
  if (known) {
    header.writeUInt32LE(record.crc32, 14);
    header.writeUInt32LE(record.zip64 ? ZIP64_32 : record.compressedSize, 18);
    header.writeUInt32LE(record.zip64 ? ZIP64_32 : record.size, 22);
  }
  header.writeUInt16LE(record.name.length, 26);
  header.writeUInt16LE(extra.length, 28);
  return Buffer.concat([header, record.name, extra]);
}

function descriptor(record) {
  const data = Buffer.alloc(record.zip64 ? 24 : 16);
  data.writeUInt32LE(DESCRIPTOR_SIGNATURE, 0);
  data.writeUInt32LE(record.crc32, 4);
  if (record.zip64) {
    data.writeBigUInt64LE(BigInt(record.compressedSize), 8);
    data.writeBigUInt64LE(BigInt(record.size), 16);
  } else {
    data.writeUInt32LE(record.compressedSize, 8);
    data.writeUInt32LE(record.size, 12);
  }
  return data;
}

function centralHeader(record) {
  const zip64 = record.zip64 || record.offset >= ZIP64_32;
  const extra = zip64
    ? zip64Extra([record.size, record.compressedSize, record.offset])
    : Buffer.alloc(0);
  const version = zip64 ? VERSION_ZIP64 : VERSION;
  const header = Buffer.alloc(46);
  header.writeUInt32LE(CENTRAL_SIGNATURE, 0);
  header.writeUInt16LE((HOST_UNIX << 8) | version, 4);
  header.writeUInt16LE(version, 6);
  header.writeUInt16LE(record.flags, 8);
  header.writeUInt16LE(record.method, 10);
  header.writeUInt16LE(record.time, 12);
  header.writeUInt16LE(record.date, 14);
  header.writeUInt32LE(record.crc32, 16);
  header.writeUInt32LE(zip64 ? ZIP64_32 : record.compressedSize, 20);
  header.writeUInt32LE(zip64 ? ZIP64_32 : record.size, 24);
  header.writeUInt16LE(record.name.length, 28);
  header.writeUInt16LE(extra.length, 30);
  header.writeUInt32LE((FILE_MODE << 16) >>> 0, 38);
  header.writeUInt32LE(zip64 ? ZIP64_32 : record.offset, 42);
  return Buffer.concat([header, record.name, extra]);
}

function endRecords(count, size, offset, force) {
  const zip64 = force || count >= ZIP64_16 || size >= ZIP64_32 || offset >= ZIP64_32;
  const end = Buffer.alloc(22);
  end.writeUInt32LE(END_SIGNATURE, 0);
  end.writeUInt16LE(zip64 ? ZIP64_16 : count, 8);
  end.writeUInt16LE(zip64 ? ZIP64_16 : count, 10);
  end.writeUInt32LE(zip64 ? ZIP64_32 : size, 12);
  end.writeUInt32LE(zip64 ? ZIP64_32 : offset, 16);
  if (!zip64) return end;

  const record = Buffer.alloc(56);
  record.writeUInt32LE(ZIP64_END_SIGNATURE, 0);
  record.writeBigUInt64LE(BigInt(record.length - 12), 4);
  record.writeUInt16LE((HOST_UNIX << 8) | VERSION_ZIP64, 12);
  record.writeUInt16LE(VERSION_ZIP64, 14);
  record.writeBigUInt64LE(BigInt(count), 24);
  record.writeBigUInt64LE(BigInt(count), 32);
  record.writeBigUInt64LE(BigInt(size), 40);
  record.writeBigUInt64LE(BigInt(offset), 48);
  const locator = Buffer.alloc(20);
  locator.writeUInt32LE(ZIP64_LOCATOR_SIGNATURE, 0);
  locator.writeBigUInt64LE(BigInt(offset + size), 8);
  locator.writeUInt32LE(1, 16);
  return Buffer.concat([record, locator, end]);
}

// Reads the file from its start to its end. A chunk that is given is not used again.
async function* fileChunks(handle, reuse) {
  let buffer;
  for (let position = 0; ; ) {
    if (!reuse || !buffer) buffer = Buffer.allocUnsafe(READ_SIZE);
    const { bytesRead } = await handle.read(buffer, 0, READ_SIZE, position);
    if (bytesRead === 0) return;
    position += bytesRead;
    yield bytesRead === READ_SIZE || reuse
      ? buffer.subarray(0, bytesRead)
      : Buffer.from(buffer.subarray(0, bytesRead));
  }
}

function changed(record) {
  return new Error(`Entry "${record.name}" changed while it was read`);
}

async function* storedFile(handle, record) {
  for await (const chunk of fileChunks(handle, true)) {
    record.crc32 = zlib.crc32(chunk, record.crc32);
    record.size += chunk.length;
  }
  record.compressedSize = record.size;
  record.zip64 ||= record.size >= ZIP64_32;
  yield localHeader(record);

  let crc32 = 0;
  let size = 0;
  for await (const chunk of fileChunks(handle, false)) {
    size += chunk.length;
    if (size > record.size) throw changed(record);
    crc32 = zlib.crc32(chunk, crc32);
    yield chunk;
  }
  if (size !== record.size || crc32 !== record.crc32) throw changed(record);
}

async function* counted(chunks, record) {
  for await (const chunk of chunks) {
    record.crc32 = zlib.crc32(chunk, record.crc32);
    record.size += chunk.length;
    if (!record.zip64 && record.size >= ZIP64_32) throw changed(record);
    yield chunk;
  }
}

async function* deflatedFile(handle, record) {
  // The local header comes before the data, so the size of the file before the read decides on
  // zip64. The bound of deflate is near 0.03% more than the data. If the file then grows above
  // the limit of the plain format, the entry stops with an error.
  const { size } = await handle.stat();
  record.flags |= FLAG_DESCRIPTOR;
  record.zip64 ||= size + size / 1024 + 64 >= ZIP64_32;
  yield localHeader(record);

  const deflate = zlib.createDeflateRaw();
  // A failure destroys `deflate` with the error, and the loop below throws it.
  pipeline(counted(fileChunks(handle, false), record), deflate, () => {});
  try {
    for await (const chunk of deflate) {
      record.compressedSize += chunk.length;
      if (!record.zip64 && record.compressedSize >= ZIP64_32) throw changed(record);
      yield chunk;
    }
  } finally {
    deflate.destroy();
  }
  yield descriptor(record);
}

async function* entryBytes(entry, record) {
  if (typeof entry.file === 'string') {
    const handle = await fs.promises.open(entry.file, 'r');
    try {
      if (record.method === METHOD_STORED) yield* storedFile(handle, record);
      else yield* deflatedFile(handle, record);
    } finally {
      await handle.close();
    }
    return;
  }
  const content = Buffer.from(entry.content);
  const data = record.method === METHOD_DEFLATED ? await deflateRaw(content) : content;
  record.crc32 = zlib.crc32(content);
  record.size = content.length;
  record.compressedSize = data.length;
  record.zip64 ||= record.size >= ZIP64_32 || record.compressedSize >= ZIP64_32;
  yield localHeader(record);
  yield data;
}

async function* zip(entries, options = {}) {
  const stamp = dosTime(options.date ?? new Date());
  const force = Boolean(options.forceZip64);
  const records = [];
  let offset = 0;

  for await (const entry of entries) {
    const name = Buffer.from(String(entry.name), 'utf8');
    if (name.length === 0 || name.length > MAX_NAME_SIZE) {
      throw new Error(`Invalid entry name "${entry.name}"`);
    }
    const record = {
      name,
      flags: FLAG_UTF8,
      method: entry.store ? METHOD_STORED : METHOD_DEFLATED,
      ...stamp,
      crc32: 0,
      compressedSize: 0,
      size: 0,
      offset,
      zip64: force,
    };
    for await (const chunk of entryBytes(entry, record)) {
      offset += chunk.length;
      yield chunk;
    }
    records.push(record);
    if (options.onEntry) options.onEntry(entry, records.length);
  }

  const directory = Buffer.concat(records.map(centralHeader));
  yield directory;
  yield endRecords(records.length, directory.length, offset, force);
}

module.exports = { zip };
