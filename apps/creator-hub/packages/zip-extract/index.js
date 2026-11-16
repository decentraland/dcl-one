'use strict';

// Unpacks a zip archive with `node:zlib` and no other package. It has the call shape of
// `extract-zip` (`extract(zipPath, { dir, onEntry })`), so a package that requires `extract-zip`
// can be pointed at this one.
//
// It never makes a link: a link entry is skipped. It writes below `dir` only: a name that leaves
// `dir`, and a folder of `dir` that is a link to another place, both stop the extraction.
// It reads stored and deflated entries, also in zip64 archives. Encrypted and multi-disk archives
// are refused.
//
// It reads only the end record and the central directory into memory. It sends the data of each
// entry from the archive to the target file in small parts, so the memory use does not grow with
// the size of an entry. It stops an entry as soon as it is larger than the size it states, and it
// removes the file of an entry that fails.

const fs = require('node:fs');
const path = require('node:path');
const zlib = require('node:zlib');
const { pipeline } = require('node:stream/promises');

const END_SIGNATURE = 0x06054b50;
const END_SIZE = 22;
const MAX_COMMENT_SIZE = 0xffff;
const ZIP64_END_SIGNATURE = 0x06064b50;
const ZIP64_END_SIZE = 56;
const ZIP64_LOCATOR_SIGNATURE = 0x07064b50;
const ZIP64_LOCATOR_SIZE = 20;
const ZIP64_EXTRA = 0x0001;
const UNICODE_PATH_EXTRA = 0x7075;
const FLAG_UTF8 = 0x800;
// The upper half of code page 437, the name encoding of a zip entry without the UTF-8 flag.
const CP437_HIGH =
  'ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐' +
  '└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■\u00a0';
const CENTRAL_SIGNATURE = 0x02014b50;
const CENTRAL_SIZE = 46;
const LOCAL_SIGNATURE = 0x04034b50;
const LOCAL_SIZE = 30;
const ZIP64_32 = 0xffffffff;
const FLAG_ENCRYPTED = 0x1;
const METHOD_STORED = 0;
const METHOD_DEFLATED = 8;
const HOST_UNIX = 3;
const TYPE_MASK = 0o170000;
const TYPE_LINK = 0o120000;
const TYPE_FOLDER = 0o040000;
const READ_SIZE = 1 << 20;
const INFLATE_SIZE = 1 << 16;

function readUInt64(buffer, at) {
  const value = buffer.readBigUInt64LE(at);
  if (value > BigInt(Number.MAX_SAFE_INTEGER)) {
    throw new Error('Damaged zip archive: a zip64 value is out of range');
  }
  return Number(value);
}

// Gives fewer bytes than `length` only at the end of the file.
async function readAt(handle, length, position) {
  const buffer = Buffer.alloc(length);
  let done = 0;
  while (done < length) {
    const { bytesRead } = await handle.read(buffer, done, length - done, position + done);
    if (bytesRead === 0) break;
    done += bytesRead;
  }
  return buffer.subarray(0, done);
}

async function* readRange(handle, start, length, fileName) {
  for (let done = 0; done < length; ) {
    const size = Math.min(length - done, READ_SIZE);
    const buffer = Buffer.allocUnsafe(size);
    const { bytesRead } = await handle.read(buffer, 0, size, start + done);
    if (bytesRead === 0) throw new Error(`Damaged zip archive: the data of "${fileName}" is cut`);
    done += bytesRead;
    yield buffer.subarray(0, bytesRead);
  }
}

async function findEnd(handle, fileSize) {
  const tailSize = Math.min(fileSize, ZIP64_LOCATOR_SIZE + END_SIZE + MAX_COMMENT_SIZE);
  const tailStart = fileSize - tailSize;
  const tail = await readAt(handle, tailSize, tailStart);
  const lowest = Math.max(0, tail.length - END_SIZE - MAX_COMMENT_SIZE);
  for (let at = tail.length - END_SIZE; at >= lowest; at--) {
    if (tail.readUInt32LE(at) !== END_SIGNATURE) continue;
    if (at + END_SIZE + tail.readUInt16LE(at + 20) === tail.length) {
      return { tail, at, position: tailStart + at };
    }
  }
  throw new Error('Not a zip archive: the end record is missing');
}

// Gives the place of the central directory, from the zip64 end record when the archive has one.
async function findDirectory(handle, fileSize) {
  const { tail, at, position } = await findEnd(handle, fileSize);
  const locator = at - ZIP64_LOCATOR_SIZE;
  if (locator < 0 || tail.readUInt32LE(locator) !== ZIP64_LOCATOR_SIGNATURE) {
    if (tail.readUInt16LE(at + 4) !== 0 || tail.readUInt16LE(at + 6) !== 0) {
      throw new Error('Multi-disk archives are not supported');
    }
    return {
      count: tail.readUInt16LE(at + 10),
      size: tail.readUInt32LE(at + 12),
      start: tail.readUInt32LE(at + 16),
      limit: position,
    };
  }

  if (tail.readUInt32LE(locator + 4) !== 0 || tail.readUInt32LE(locator + 16) > 1) {
    throw new Error('Multi-disk archives are not supported');
  }
  const recordAt = readUInt64(tail, locator + 8);
  const record = await readAt(handle, ZIP64_END_SIZE, recordAt);
  if (
    recordAt + ZIP64_END_SIZE > position - ZIP64_LOCATOR_SIZE ||
    record.length !== ZIP64_END_SIZE ||
    record.readUInt32LE(0) !== ZIP64_END_SIGNATURE
  ) {
    throw new Error('Damaged zip archive: the zip64 end record is missing');
  }
  if (record.readUInt32LE(16) !== 0 || record.readUInt32LE(20) !== 0) {
    throw new Error('Multi-disk archives are not supported');
  }
  return {
    count: readUInt64(record, 32),
    size: readUInt64(record, 40),
    start: readUInt64(record, 48),
    limit: recordAt,
  };
}

// The zip64 field holds, in this order, each size and the offset that its record sets to
// 0xffffffff.
function zip64Values(directory, from, to, fileName) {
  for (let at = from; at + 4 <= to; at += 4 + directory.readUInt16LE(at + 2)) {
    if (directory.readUInt16LE(at) !== ZIP64_EXTRA) continue;
    let field = at + 4;
    const end = Math.min(to, field + directory.readUInt16LE(at + 2));
    return () => {
      if (field + 8 > end)
        throw new Error(`Damaged zip archive: the zip64 field of "${fileName}" is cut`);
      field += 8;
      return readUInt64(directory, field - 8);
    };
  }
  throw new Error(`Damaged zip archive: the zip64 field of "${fileName}" is missing`);
}

function decodeCp437(bytes) {
  let name = '';
  for (const byte of bytes)
    name += byte < 0x80 ? String.fromCharCode(byte) : CP437_HIGH[byte - 0x80];
  return name;
}

// As yauzl: UTF-8 when the flag says so; else the Info-ZIP Unicode Path field when its checksum
// matches the stored name; else code page 437.
function entryName(directory, nameStart, extraStart, extraEnd, flags) {
  const raw = directory.subarray(nameStart, extraStart);
  if (flags & FLAG_UTF8) return raw.toString('utf8');
  for (let at = extraStart; at + 4 <= extraEnd; at += 4 + directory.readUInt16LE(at + 2)) {
    const size = directory.readUInt16LE(at + 2);
    if (directory.readUInt16LE(at) !== UNICODE_PATH_EXTRA || size < 5 || at + 4 + size > extraEnd) {
      continue;
    }
    if (directory[at + 4] === 1 && directory.readUInt32LE(at + 5) === zlib.crc32(raw)) {
      return directory.toString('utf8', at + 9, at + 4 + size);
    }
  }
  return decodeCp437(raw);
}

async function readEntries(handle, fileSize) {
  const { count, size, start, limit } = await findDirectory(handle, fileSize);
  if (start + size > limit) throw new Error('Damaged zip archive: the directory is out of range');
  const directory = await readAt(handle, size, start);

  const entries = [];
  let at = 0;
  for (let index = 0; index < count; index++) {
    if (at + CENTRAL_SIZE > size || directory.readUInt32LE(at) !== CENTRAL_SIGNATURE) {
      throw new Error('Damaged zip archive: a directory record is missing');
    }
    const nameSize = directory.readUInt16LE(at + 28);
    const extraStart = at + CENTRAL_SIZE + nameSize;
    const extraEnd = extraStart + directory.readUInt16LE(at + 30);
    const next = extraEnd + directory.readUInt16LE(at + 32);
    if (next > size) throw new Error('Damaged zip archive: a directory record is cut');
    const host = directory.readUInt16LE(at + 4) >> 8;
    const flags = directory.readUInt16LE(at + 8);
    const entry = {
      fileName: entryName(directory, at + CENTRAL_SIZE, extraStart, extraEnd, flags),
      flags,
      method: directory.readUInt16LE(at + 10),
      crc32: directory.readUInt32LE(at + 16),
      compressedSize: directory.readUInt32LE(at + 20),
      uncompressedSize: directory.readUInt32LE(at + 24),
      mode: host === HOST_UNIX ? directory.readUInt32LE(at + 38) >>> 16 : 0,
      localOffset: directory.readUInt32LE(at + 42),
    };
    if (
      entry.uncompressedSize === ZIP64_32 ||
      entry.compressedSize === ZIP64_32 ||
      entry.localOffset === ZIP64_32
    ) {
      const value = zip64Values(directory, extraStart, extraEnd, entry.fileName);
      if (entry.uncompressedSize === ZIP64_32) entry.uncompressedSize = value();
      if (entry.compressedSize === ZIP64_32) entry.compressedSize = value();
      if (entry.localOffset === ZIP64_32) entry.localOffset = value();
    }
    entries.push(entry);
    at = next;
  }
  return entries;
}

// A folder entry may name the target folder itself (`./`): it gives no parts.
function pathParts(fileName, folder) {
  if (fileName === '' || fileName.includes('\\') || fileName.includes('\0')) {
    throw new Error(`Invalid entry name "${fileName}"`);
  }
  if (fileName.startsWith('/') || /^[a-zA-Z]:/.test(fileName)) {
    throw new Error(`Absolute entry name "${fileName}"`);
  }
  const parts = fileName.split('/').filter(part => part !== '' && part !== '.');
  if (parts.includes('..')) throw new Error(`Entry name "${fileName}" leaves the target folder`);
  if (parts.length === 0 && !folder) throw new Error(`Invalid entry name "${fileName}"`);
  return parts;
}

function isInside(root, target) {
  return target === root || target.startsWith(root + path.sep);
}

// A folder that exists already can be a link to another place. Only the nearest existing folder
// can be one: this module makes the folders below it, and it makes no link.
async function makeFolder(root, folder, fileName) {
  let existing = folder;
  while (!fs.existsSync(existing)) existing = path.dirname(existing);
  if (!isInside(root, await fs.promises.realpath(existing))) {
    throw new Error(`Entry "${fileName}" goes through a link that leaves the target folder`);
  }
  await fs.promises.mkdir(folder, { recursive: true });
}

async function dataStart(handle, fileSize, entry) {
  const local = await readAt(handle, LOCAL_SIZE, entry.localOffset);
  if (local.length !== LOCAL_SIZE || local.readUInt32LE(0) !== LOCAL_SIGNATURE) {
    throw new Error(`Damaged zip archive: no data for "${entry.fileName}"`);
  }
  const start = entry.localOffset + LOCAL_SIZE + local.readUInt16LE(26) + local.readUInt16LE(28);
  if (start + entry.compressedSize > fileSize) {
    throw new Error(`Damaged zip archive: the data of "${entry.fileName}" is cut`);
  }
  return start;
}

async function writeContent(handle, fileSize, entry, target) {
  if (entry.flags & FLAG_ENCRYPTED) throw new Error(`Entry "${entry.fileName}" is encrypted`);
  if (entry.method !== METHOD_STORED && entry.method !== METHOD_DEFLATED) {
    throw new Error(`Entry "${entry.fileName}" uses the unsupported method ${entry.method}`);
  }
  const start = await dataStart(handle, fileSize, entry);
  const damaged = () =>
    new Error(`Damaged zip archive: "${entry.fileName}" fails its size or checksum`);

  const stages = [readRange(handle, start, entry.compressedSize, entry.fileName)];
  // Some writers give an empty entry no deflate data at all; zlib refuses that as a cut stream.
  if (entry.method === METHOD_DEFLATED && entry.compressedSize > 0) {
    stages.push(zlib.createInflateRaw({ chunkSize: INFLATE_SIZE }));
  }
  const output = await fs.promises.open(target, 'w', entry.mode & 0o777 || 0o644);
  let size = 0;
  let crc32 = 0;
  try {
    await pipeline(...stages, async chunks => {
      for await (const chunk of chunks) {
        size += chunk.length;
        if (size > entry.uncompressedSize) throw damaged();
        crc32 = zlib.crc32(chunk, crc32);
        for (let done = 0; done < chunk.length; ) {
          done += (await output.write(chunk, done)).bytesWritten;
        }
      }
    });
    // An empty entry has nothing to check its checksum against; some writers store a stray one.
    if (size !== entry.uncompressedSize || (size > 0 && crc32 !== entry.crc32)) throw damaged();
  } catch (error) {
    await output.close();
    await fs.promises.rm(target, { force: true });
    throw error;
  }
  await output.close();
}

async function extract(zipPath, options) {
  if (!options || typeof options.dir !== 'string' || !path.isAbsolute(options.dir)) {
    throw new Error('Target directory is expected to be absolute');
  }
  await fs.promises.mkdir(options.dir, { recursive: true });
  const root = await fs.promises.realpath(options.dir);
  const handle = await fs.promises.open(zipPath, 'r');
  try {
    const { size: fileSize } = await handle.stat();
    for (const entry of await readEntries(handle, fileSize)) {
      if (entry.fileName.startsWith('__MACOSX/')) continue;
      if (options.onEntry) options.onEntry(entry);

      const type = entry.mode & TYPE_MASK;
      const folder = type === TYPE_FOLDER || entry.fileName.endsWith('/');
      const parts = pathParts(entry.fileName, folder);
      if (type === TYPE_LINK || parts.length === 0) continue;
      const target = path.join(root, ...parts);

      if (folder) {
        await makeFolder(root, target, entry.fileName);
        continue;
      }

      await makeFolder(root, path.dirname(target), entry.fileName);
      const present = await fs.promises.lstat(target).catch(() => undefined);
      if (present && !present.isFile()) {
        throw new Error(`Entry "${entry.fileName}" would replace something that is not a file`);
      }
      await writeContent(handle, fileSize, entry, target);
    }
  } finally {
    await handle.close();
  }
}

module.exports = extract;
