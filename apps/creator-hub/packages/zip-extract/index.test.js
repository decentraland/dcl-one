'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const zlib = require('node:zlib');
const { pipeline } = require('node:stream/promises');
const { afterEach, beforeEach, describe, it } = require('node:test');

const extract = require('./index.js');

const UNIX = 3;
const FILE = 0o100644;
const PROGRAM = 0o100755;
const FOLDER = 0o040755;
const LINK = 0o120777;
const unix = process.platform !== 'win32';

// Writes the archive by hand, so that a test can state a name, a type or a size that no zip
// tool writes.
function zip(entries) {
  const locals = [];
  const centrals = [];
  let offset = 0;
  for (const entry of entries) {
    const name = Buffer.from(entry.name);
    const content = Buffer.from(entry.content ?? '');
    const method = entry.method ?? 8;
    const data = entry.data ?? (method === 8 ? zlib.deflateRawSync(content) : content);
    const crc = entry.crc32 ?? zlib.crc32(content);
    const size = entry.size ?? content.length;

    const local = Buffer.alloc(30);
    local.writeUInt32LE(0x04034b50, 0);
    local.writeUInt16LE(20, 4);
    local.writeUInt16LE(entry.flags ?? 0, 6);
    local.writeUInt16LE(method, 8);
    local.writeUInt32LE(crc, 14);
    local.writeUInt32LE(data.length, 18);
    local.writeUInt32LE(size, 22);
    local.writeUInt16LE(name.length, 26);
    locals.push(local, name, data);

    const central = Buffer.alloc(46);
    central.writeUInt32LE(0x02014b50, 0);
    central.writeUInt16LE(((entry.host ?? UNIX) << 8) | 20, 4);
    central.writeUInt16LE(20, 6);
    central.writeUInt16LE(entry.flags ?? 0, 8);
    central.writeUInt16LE(method, 10);
    central.writeUInt32LE(crc, 16);
    central.writeUInt32LE(data.length, 20);
    central.writeUInt32LE(size, 24);
    central.writeUInt16LE(name.length, 28);
    central.writeUInt32LE(((entry.mode ?? FILE) << 16) >>> 0, 38);
    central.writeUInt32LE(offset, 42);
    centrals.push(central, name);

    offset += local.length + name.length + data.length;
  }
  const directory = Buffer.concat(centrals);
  const end = Buffer.alloc(22);
  end.writeUInt32LE(0x06054b50, 0);
  end.writeUInt16LE(entries.length, 8);
  end.writeUInt16LE(entries.length, 10);
  end.writeUInt32LE(directory.length, 12);
  end.writeUInt32LE(offset, 16);
  return Buffer.concat([...locals, directory, end]);
}

// Gives the deflated data of `size` zero bytes, and never holds all the zero bytes in memory.
async function deflatedZeros(size) {
  const zero = Buffer.alloc(2 ** 20);
  const parts = [];
  let crc32 = 0;
  await pipeline(
    function* () {
      for (let done = 0; done < size; done += zero.length) {
        crc32 = zlib.crc32(zero, crc32);
        yield zero;
      }
    },
    zlib.createDeflateRaw(),
    async chunks => {
      for await (const chunk of chunks) parts.push(chunk);
    },
  );
  return { data: Buffer.concat(parts), crc32, size };
}

describe('extract', () => {
  let folder;
  let target;
  let outside;
  let archive;

  function write(entries) {
    fs.writeFileSync(archive, zip(entries));
  }

  beforeEach(() => {
    folder = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'zip-extract-')));
    target = path.join(folder, 'target');
    outside = path.join(folder, 'outside');
    archive = path.join(folder, 'archive.zip');
    fs.mkdirSync(outside);
  });

  afterEach(() => {
    fs.rmSync(folder, { recursive: true, force: true });
  });

  it('should write stored and deflated files and the folders of the archive', async () => {
    const long = 'scene '.repeat(5000);
    write([
      { name: 'repo-main/', mode: FOLDER, method: 0 },
      { name: 'repo-main/scene.json', content: '{"main":"bin/index.js"}', method: 0 },
      { name: 'repo-main/src/index.ts', content: long },
      { name: 'repo-main/empty/', mode: FOLDER, method: 0 },
      { name: 'repo-main/empty.txt', content: '' },
    ]);

    await extract(archive, { dir: target });

    assert.equal(
      fs.readFileSync(path.join(target, 'repo-main/scene.json'), 'utf8'),
      '{"main":"bin/index.js"}',
    );
    assert.equal(fs.readFileSync(path.join(target, 'repo-main/src/index.ts'), 'utf8'), long);
    assert.equal(fs.statSync(path.join(target, 'repo-main/empty')).isDirectory(), true);
    assert.equal(fs.readFileSync(path.join(target, 'repo-main/empty.txt'), 'utf8'), '');
  });

  it('should give each entry to onEntry', async () => {
    write([
      { name: 'repo-main/', mode: FOLDER, method: 0 },
      { name: 'repo-main/a.txt', content: 'a' },
      { name: '__MACOSX/repo-main/._a.txt', content: 'x' },
    ]);
    const names = [];

    await extract(archive, { dir: target, onEntry: entry => names.push(entry.fileName) });

    assert.deepEqual(names, ['repo-main/', 'repo-main/a.txt']);
    assert.equal(fs.existsSync(path.join(target, '__MACOSX')), false);
  });

  it('should keep the permission bits of a file', { skip: !unix }, async () => {
    write([
      { name: 'run.sh', content: '#!/bin/sh\n', mode: PROGRAM },
      { name: 'plain.txt', content: 'text' },
      { name: 'other-system.txt', content: 'text', host: 0, mode: 0 },
    ]);

    await extract(archive, { dir: target });

    assert.equal(fs.statSync(path.join(target, 'run.sh')).mode & 0o111, 0o111);
    assert.equal(fs.statSync(path.join(target, 'plain.txt')).mode & 0o111, 0);
    assert.equal(fs.statSync(path.join(target, 'other-system.txt')).mode & 0o111, 0);
  });

  for (const name of [
    '../escape.txt',
    'a/../../escape.txt',
    '/escape.txt',
    'C:/escape.txt',
    'a\\..\\escape.txt',
  ]) {
    it(`should refuse the entry name ${name}`, async () => {
      write([
        { name: 'first.txt', content: 'first' },
        { name, content: 'escape' },
      ]);

      await assert.rejects(extract(archive, { dir: target }), /entry name/i);

      assert.deepEqual(fs.readdirSync(folder).sort(), ['archive.zip', 'outside', 'target']);
      assert.deepEqual(fs.readdirSync(target), ['first.txt']);
      assert.deepEqual(fs.readdirSync(outside), []);
    });
  }

  it('should not make a link, so a later entry cannot go through it', async () => {
    write([
      { name: 'link', content: outside, mode: LINK, method: 0 },
      { name: 'link/escape.txt', content: 'escape' },
    ]);

    await extract(archive, { dir: target });

    assert.deepEqual(fs.readdirSync(outside), []);
    assert.equal(fs.lstatSync(path.join(target, 'link')).isDirectory(), true);
    assert.equal(fs.readFileSync(path.join(target, 'link/escape.txt'), 'utf8'), 'escape');
  });

  it('should refuse a folder that is a link to another place', { skip: !unix }, async () => {
    fs.mkdirSync(target);
    fs.symlinkSync(outside, path.join(target, 'link'));
    write([{ name: 'link/deep/escape.txt', content: 'escape' }]);

    await assert.rejects(extract(archive, { dir: target }), /goes through a link/);

    assert.deepEqual(fs.readdirSync(outside), []);
  });

  it('should refuse to write through a file link', { skip: !unix }, async () => {
    const victim = path.join(outside, 'victim.txt');
    fs.writeFileSync(victim, 'kept');
    fs.mkdirSync(target);
    fs.symlinkSync(victim, path.join(target, 'config.json'));
    write([{ name: 'config.json', content: 'replaced' }]);

    await assert.rejects(extract(archive, { dir: target }), /not a file/);

    assert.equal(fs.readFileSync(victim, 'utf8'), 'kept');
  });

  it('should replace a file that is there', async () => {
    fs.mkdirSync(target);
    fs.writeFileSync(path.join(target, 'a.txt'), 'old and longer');
    write([{ name: 'a.txt', content: 'new' }]);

    await extract(archive, { dir: target });

    assert.equal(fs.readFileSync(path.join(target, 'a.txt'), 'utf8'), 'new');
  });

  it('should refuse an entry that is larger than the size it states', async () => {
    write([{ name: 'bomb.txt', content: 'a'.repeat(100000), size: 10 }]);

    await assert.rejects(extract(archive, { dir: target }));

    assert.deepEqual(fs.readdirSync(target), []);
  });

  it('should not hold a large entry in memory', async () => {
    const size = 256 * 2 ** 20;
    write([{ name: 'zeros.bin', ...(await deflatedZeros(size)) }]);
    const before = process.memoryUsage.rss();

    await extract(archive, { dir: target });

    // The highest memory use of the process, so that a short peak is also seen.
    const growth = process.resourceUsage().maxRSS * 1024 - before;
    assert.equal(fs.statSync(path.join(target, 'zeros.bin')).size, size);
    assert.ok(growth < 128 * 2 ** 20, `memory use grew by ${Math.round(growth / 2 ** 20)} MiB`);
  });

  it('should refuse an entry with a wrong checksum', async () => {
    write([{ name: 'a.txt', content: 'text', crc32: 1 }]);

    await assert.rejects(extract(archive, { dir: target }), /checksum/);

    assert.deepEqual(fs.readdirSync(target), []);
  });

  it('should remove the written part of an entry that fails its checksum', async () => {
    const content = 'scene '.repeat(600000);
    for (const method of [0, 8]) {
      fs.mkdirSync(target, { recursive: true });
      fs.writeFileSync(path.join(target, 'a.txt'), 'old');
      write([{ name: 'a.txt', content, method, crc32: (zlib.crc32(content) ^ 1) >>> 0 }]);

      await assert.rejects(extract(archive, { dir: target }), /checksum/);

      assert.deepEqual(fs.readdirSync(target), []);
    }
  });

  it('should refuse an encrypted entry and an unknown method', async () => {
    write([{ name: 'a.txt', content: 'text', flags: 1 }]);
    await assert.rejects(extract(archive, { dir: target }), /encrypted/);

    write([{ name: 'a.txt', content: 'text', method: 12 }]);
    await assert.rejects(extract(archive, { dir: target }), /unsupported method 12/);
  });

  it('should refuse a file that is not a zip archive', async () => {
    fs.writeFileSync(archive, 'not a zip archive, only some text that is long enough');

    await assert.rejects(extract(archive, { dir: target }), /Not a zip archive/);
  });

  it('should refuse a cut archive', async () => {
    const whole = zip([{ name: 'a.txt', content: 'text' }]);
    fs.writeFileSync(archive, whole.subarray(0, whole.length - 30));

    await assert.rejects(extract(archive, { dir: target }));
  });

  it('should refuse a target folder that is not absolute', async () => {
    write([{ name: 'a.txt', content: 'text' }]);

    await assert.rejects(extract(archive, { dir: 'relative/target' }), /absolute/);
  });
});
