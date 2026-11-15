'use strict';

// Writes a zip archive with `node:zlib` and no other package. `zip(entries)` gives the bytes of
// the archive in order, so the caller can send them to a file with `stream.pipeline`. It reads
// one entry at a time and never holds a file entry in memory: the sizes and the checksum of a
// file entry go in a data descriptor after its data.
//
// It writes no zip64 record. Above the limits of the plain format (65534 entries, 4 GiB for an
// entry or for the archive) it stops with an error.

const fs = require('node:fs');
const zlib = require('node:zlib');
const { pipeline } = require('node:stream');
const { promisify } = require('node:util');

const deflateRaw = promisify(zlib.deflateRaw);

const LOCAL_SIGNATURE = 0x04034b50;
const DESCRIPTOR_SIGNATURE = 0x08074b50;
const CENTRAL_SIGNATURE = 0x02014b50;
const END_SIGNATURE = 0x06054b50;
const VERSION = 20;
const HOST_UNIX = 3;
const FILE_MODE = 0o100644;
const FLAG_DESCRIPTOR = 0x8;
const FLAG_UTF8 = 0x800;
const METHOD_STORED = 0;
const METHOD_DEFLATED = 8;
const MAX_ENTRIES = 0xfffe;
const MAX_SIZE = 0xfffffffe;
const MAX_NAME_SIZE = 0xffff;
const READ_SIZE = 1 << 20;

function tooLarge(subject) {
  return new Error(`${subject} is larger than 4 GiB, and zip64 is not supported`);
}

function dosTime(date) {
  const year = Math.max(date.getFullYear(), 1980);
  return {
    time: (date.getHours() << 11) | (date.getMinutes() << 5) | (date.getSeconds() >> 1),
    date: ((year - 1980) << 9) | ((date.getMonth() + 1) << 5) | date.getDate(),
  };
}

function localHeader(record) {
  const header = Buffer.alloc(30);
  header.writeUInt32LE(LOCAL_SIGNATURE, 0);
  header.writeUInt16LE(VERSION, 4);
  header.writeUInt16LE(record.flags, 6);
  header.writeUInt16LE(record.method, 8);
  header.writeUInt16LE(record.time, 10);
  header.writeUInt16LE(record.date, 12);
  if (!(record.flags & FLAG_DESCRIPTOR)) {
    header.writeUInt32LE(record.crc32, 14);
    header.writeUInt32LE(record.compressedSize, 18);
    header.writeUInt32LE(record.size, 22);
  }
  header.writeUInt16LE(record.name.length, 26);
  return Buffer.concat([header, record.name]);
}

function descriptor(record) {
  const data = Buffer.alloc(16);
  data.writeUInt32LE(DESCRIPTOR_SIGNATURE, 0);
  data.writeUInt32LE(record.crc32, 4);
  data.writeUInt32LE(record.compressedSize, 8);
  data.writeUInt32LE(record.size, 12);
  return data;
}

function centralHeader(record) {
  const header = Buffer.alloc(46);
  header.writeUInt32LE(CENTRAL_SIGNATURE, 0);
  header.writeUInt16LE((HOST_UNIX << 8) | VERSION, 4);
  header.writeUInt16LE(VERSION, 6);
  header.writeUInt16LE(record.flags, 8);
  header.writeUInt16LE(record.method, 10);
  header.writeUInt16LE(record.time, 12);
  header.writeUInt16LE(record.date, 14);
  header.writeUInt32LE(record.crc32, 16);
  header.writeUInt32LE(record.compressedSize, 20);
  header.writeUInt32LE(record.size, 24);
  header.writeUInt16LE(record.name.length, 28);
  header.writeUInt32LE((FILE_MODE << 16) >>> 0, 38);
  header.writeUInt32LE(record.offset, 42);
  return Buffer.concat([header, record.name]);
}

function endRecord(count, size, offset) {
  const end = Buffer.alloc(22);
  end.writeUInt32LE(END_SIGNATURE, 0);
  end.writeUInt16LE(count, 8);
  end.writeUInt16LE(count, 10);
  end.writeUInt32LE(size, 12);
  end.writeUInt32LE(offset, 16);
  return end;
}

async function* counted(chunks, record) {
  for await (const chunk of chunks) {
    record.crc32 = zlib.crc32(chunk, record.crc32);
    record.size += chunk.length;
    if (record.size > MAX_SIZE) throw tooLarge(`Entry "${record.name}"`);
    yield chunk;
  }
}

async function* fileData(file, record) {
  const source = fs.createReadStream(file, { highWaterMark: READ_SIZE });
  let data = counted(source, record);
  let deflate;
  if (record.method === METHOD_DEFLATED) {
    deflate = zlib.createDeflateRaw();
    // A failure destroys `deflate` with the error, and the loop below throws it.
    pipeline(data, deflate, () => {});
    data = deflate;
  }
  try {
    for await (const chunk of data) {
      record.compressedSize += chunk.length;
      yield chunk;
    }
  } finally {
    source.destroy();
    deflate?.destroy();
  }
}

async function* entryBytes(entry, record) {
  if (typeof entry.file === 'string') {
    record.flags |= FLAG_DESCRIPTOR;
    yield localHeader(record);
    yield* fileData(entry.file, record);
    yield descriptor(record);
    return;
  }
  const content = Buffer.from(entry.content);
  if (content.length > MAX_SIZE) throw tooLarge(`Entry "${entry.name}"`);
  const data = record.method === METHOD_DEFLATED ? await deflateRaw(content) : content;
  record.crc32 = zlib.crc32(content);
  record.size = content.length;
  record.compressedSize = data.length;
  yield localHeader(record);
  yield data;
}

async function* zip(entries, options = {}) {
  const stamp = dosTime(options.date ?? new Date());
  const records = [];
  let offset = 0;

  for await (const entry of entries) {
    const name = Buffer.from(String(entry.name), 'utf8');
    if (name.length === 0 || name.length > MAX_NAME_SIZE) {
      throw new Error(`Invalid entry name "${entry.name}"`);
    }
    if (records.length === MAX_ENTRIES) {
      throw new Error(`A zip archive holds at most ${MAX_ENTRIES} entries without zip64`);
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
    };
    for await (const chunk of entryBytes(entry, record)) {
      offset += chunk.length;
      if (offset > MAX_SIZE) throw tooLarge('The zip archive');
      yield chunk;
    }
    records.push(record);
    if (options.onEntry) options.onEntry(entry, records.length);
  }

  const directory = Buffer.concat(records.map(centralHeader));
  if (offset + directory.length > MAX_SIZE) throw tooLarge('The zip archive');
  yield directory;
  yield endRecord(records.length, directory.length, offset);
}

module.exports = { zip };
