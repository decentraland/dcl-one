'use strict';

// Unpacks a zip archive with `node:zlib` and no other package. It has the call shape of
// `extract-zip` (`extract(zipPath, { dir, onEntry })`), so a package that requires `extract-zip`
// can be pointed at this one.
//
// It never makes a link: a link entry is skipped. It writes below `dir` only: a name that leaves
// `dir`, and a folder of `dir` that is a link to another place, both stop the extraction.
// It reads stored and deflated entries. Zip64, encrypted and multi-disk archives are refused.

const fs = require('node:fs');
const path = require('node:path');
const zlib = require('node:zlib');
const { promisify } = require('node:util');

const inflateRaw = promisify(zlib.inflateRaw);

const END_SIGNATURE = 0x06054b50;
const END_SIZE = 22;
const MAX_COMMENT_SIZE = 0xffff;
const CENTRAL_SIGNATURE = 0x02014b50;
const CENTRAL_SIZE = 46;
const LOCAL_SIGNATURE = 0x04034b50;
const LOCAL_SIZE = 30;
const ZIP64_16 = 0xffff;
const ZIP64_32 = 0xffffffff;
const FLAG_ENCRYPTED = 0x1;
const METHOD_STORED = 0;
const METHOD_DEFLATED = 8;
const HOST_UNIX = 3;
const TYPE_MASK = 0o170000;
const TYPE_LINK = 0o120000;
const TYPE_FOLDER = 0o040000;

function findEnd(buffer) {
  const lowest = Math.max(0, buffer.length - END_SIZE - MAX_COMMENT_SIZE);
  for (let at = buffer.length - END_SIZE; at >= lowest; at--) {
    if (buffer.readUInt32LE(at) !== END_SIGNATURE) continue;
    if (at + END_SIZE + buffer.readUInt16LE(at + 20) === buffer.length) return at;
  }
  throw new Error('Not a zip archive: the end record is missing');
}

function readEntries(buffer) {
  const end = findEnd(buffer);
  const count = buffer.readUInt16LE(end + 10);
  const size = buffer.readUInt32LE(end + 12);
  const start = buffer.readUInt32LE(end + 16);
  if (count === ZIP64_16 || size === ZIP64_32 || start === ZIP64_32) {
    throw new Error('Zip64 archives are not supported');
  }
  if (buffer.readUInt16LE(end + 4) !== 0 || buffer.readUInt16LE(end + 6) !== 0) {
    throw new Error('Multi-disk archives are not supported');
  }
  if (start + size > end) throw new Error('Damaged zip archive: the directory is out of range');

  const entries = [];
  let at = start;
  for (let index = 0; index < count; index++) {
    if (at + CENTRAL_SIZE > end || buffer.readUInt32LE(at) !== CENTRAL_SIGNATURE) {
      throw new Error('Damaged zip archive: a directory record is missing');
    }
    const nameSize = buffer.readUInt16LE(at + 28);
    const next =
      at + CENTRAL_SIZE + nameSize + buffer.readUInt16LE(at + 30) + buffer.readUInt16LE(at + 32);
    if (next > end) throw new Error('Damaged zip archive: a directory record is cut');
    const host = buffer.readUInt16LE(at + 4) >> 8;
    entries.push({
      fileName: buffer.toString('utf8', at + CENTRAL_SIZE, at + CENTRAL_SIZE + nameSize),
      flags: buffer.readUInt16LE(at + 8),
      method: buffer.readUInt16LE(at + 10),
      crc32: buffer.readUInt32LE(at + 16),
      compressedSize: buffer.readUInt32LE(at + 20),
      uncompressedSize: buffer.readUInt32LE(at + 24),
      mode: host === HOST_UNIX ? buffer.readUInt32LE(at + 38) >>> 16 : 0,
      localOffset: buffer.readUInt32LE(at + 42),
    });
    at = next;
  }
  return entries;
}

function pathParts(fileName) {
  if (fileName === '' || fileName.includes('\\') || fileName.includes('\0')) {
    throw new Error(`Invalid entry name "${fileName}"`);
  }
  if (fileName.startsWith('/') || /^[a-zA-Z]:/.test(fileName)) {
    throw new Error(`Absolute entry name "${fileName}"`);
  }
  const parts = fileName.split('/').filter(part => part !== '' && part !== '.');
  if (parts.includes('..')) throw new Error(`Entry name "${fileName}" leaves the target folder`);
  if (parts.length === 0) throw new Error(`Invalid entry name "${fileName}"`);
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

async function readContent(buffer, entry) {
  if (entry.flags & FLAG_ENCRYPTED) throw new Error(`Entry "${entry.fileName}" is encrypted`);
  if (entry.compressedSize === ZIP64_32 || entry.uncompressedSize === ZIP64_32) {
    throw new Error('Zip64 archives are not supported');
  }
  const local = entry.localOffset;
  if (local + LOCAL_SIZE > buffer.length || buffer.readUInt32LE(local) !== LOCAL_SIGNATURE) {
    throw new Error(`Damaged zip archive: no data for "${entry.fileName}"`);
  }
  const start =
    local + LOCAL_SIZE + buffer.readUInt16LE(local + 26) + buffer.readUInt16LE(local + 28);
  if (start + entry.compressedSize > buffer.length) {
    throw new Error(`Damaged zip archive: the data of "${entry.fileName}" is cut`);
  }
  const data = buffer.subarray(start, start + entry.compressedSize);

  let content;
  if (entry.method === METHOD_STORED) {
    content = data;
  } else if (entry.method === METHOD_DEFLATED) {
    // The limit stops an entry that grows above the size it states.
    content = await inflateRaw(data, { maxOutputLength: Math.max(entry.uncompressedSize, 1) });
  } else {
    throw new Error(`Entry "${entry.fileName}" uses the unsupported method ${entry.method}`);
  }
  if (content.length !== entry.uncompressedSize || zlib.crc32(content) !== entry.crc32) {
    throw new Error(`Damaged zip archive: "${entry.fileName}" fails its size or checksum`);
  }
  return content;
}

async function extract(zipPath, options) {
  if (!options || typeof options.dir !== 'string' || !path.isAbsolute(options.dir)) {
    throw new Error('Target directory is expected to be absolute');
  }
  await fs.promises.mkdir(options.dir, { recursive: true });
  const root = await fs.promises.realpath(options.dir);
  const buffer = await fs.promises.readFile(zipPath);

  for (const entry of readEntries(buffer)) {
    if (entry.fileName.startsWith('__MACOSX/')) continue;
    if (options.onEntry) options.onEntry(entry);

    const target = path.join(root, ...pathParts(entry.fileName));
    const type = entry.mode & TYPE_MASK;
    if (type === TYPE_LINK) continue;

    if (type === TYPE_FOLDER || entry.fileName.endsWith('/')) {
      await makeFolder(root, target, entry.fileName);
      continue;
    }

    await makeFolder(root, path.dirname(target), entry.fileName);
    const present = await fs.promises.lstat(target).catch(() => undefined);
    if (present && !present.isFile()) {
      throw new Error(`Entry "${entry.fileName}" would replace something that is not a file`);
    }
    const content = await readContent(buffer, entry);
    await fs.promises.writeFile(target, content, { mode: entry.mode & 0o777 || 0o644 });
  }
}

module.exports = extract;
