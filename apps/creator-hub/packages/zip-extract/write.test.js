'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const zlib = require('node:zlib');
const { afterEach, beforeEach, describe, it } = require('node:test');

const extract = require('./index.js');
const { zip } = require('./write.js');

const LOCAL_VERSION = 4;
const LOCAL_FLAGS = 6;
const LOCAL_CRC = 14;
const LOCAL_COMPRESSED_SIZE = 18;
const LOCAL_SIZE = 22;
const LOCAL_NAME_SIZE = 26;
const LOCAL_EXTRA_SIZE = 28;
const LOCAL_HEADER = 30;
const FLAG_DESCRIPTOR = 0x8;
const FLAG_UTF8 = 0x800;
const STORED = 0;
const DEFLATED = 8;
const ZIP64_VERSION = 45;
const ZIP64_EXTRA = 0x0001;
const ZIP64_END_SIGNATURE = 0x06064b50;
const ZIP64_LOCATOR_SIGNATURE = 0x07064b50;
const END = 22;
const LOCATOR = 20;
const DATE = new Date(2024, 4, 17, 13, 45, 58);

async function collect(chunks) {
  const all = [];
  for await (const chunk of chunks) all.push(chunk);
  return Buffer.concat(all);
}

describe('zip', () => {
  let folder;
  let target;
  let archive;

  async function unpack(bytes) {
    const entries = new Map();
    fs.writeFileSync(archive, bytes);
    await extract(archive, { dir: target, onEntry: entry => entries.set(entry.fileName, entry) });
    return entries;
  }

  function source(name, content) {
    const file = path.join(folder, name);
    fs.writeFileSync(file, content);
    return file;
  }

  beforeEach(() => {
    folder = fs.realpathSync(fs.mkdtempSync(path.join(os.tmpdir(), 'zip-write-')));
    target = path.join(folder, 'target');
    archive = path.join(folder, 'archive.zip');
  });

  afterEach(() => {
    fs.rmSync(folder, { recursive: true, force: true });
  });

  it('should write content and file entries that the reader gives back', async () => {
    const long = Buffer.from('scene '.repeat(50000));
    const image = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0, 1, 2, 3]);

    const entries = await unpack(
      await collect(
        zip([
          { name: 'scene/scene.json', content: '{"main":"bin/index.js"}' },
          { name: 'scene/notes.txt', content: Buffer.from('stored text'), store: true },
          { name: 'scene/src/index.ts', file: source('index.ts', long) },
          { name: 'scene/images/a.png', file: source('a.png', image), store: true },
          { name: 'scene/empty.txt', file: source('empty.txt', '') },
        ]),
      ),
    );

    const read = name => fs.readFileSync(path.join(target, name));
    assert.equal(read('scene/scene.json').toString(), '{"main":"bin/index.js"}');
    assert.equal(read('scene/notes.txt').toString(), 'stored text');
    assert.deepEqual(read('scene/src/index.ts'), long);
    assert.deepEqual(read('scene/images/a.png'), image);
    assert.equal(read('scene/empty.txt').length, 0);
    assert.deepEqual(
      [...entries.values()].map(entry => entry.method),
      [DEFLATED, STORED, DEFLATED, STORED, DEFLATED],
    );
    assert.ok(entries.get('scene/src/index.ts').compressedSize < long.length / 10);
    assert.equal(entries.get('scene/images/a.png').compressedSize, image.length);
  });

  it('should state the sizes of a stored entry before its data', async () => {
    const content = await collect(
      zip([{ name: 'a.txt', content: 'text', store: true }], { date: DATE }),
    );
    const file = await collect(
      zip([{ name: 'a.txt', file: source('a.txt', 'text'), store: true }], { date: DATE }),
    );

    for (const bytes of [content, file]) {
      assert.equal(bytes.readUInt16LE(LOCAL_FLAGS) & FLAG_DESCRIPTOR, 0);
      assert.equal(bytes.readUInt32LE(LOCAL_CRC), zlib.crc32('text'));
      assert.equal(bytes.readUInt32LE(LOCAL_COMPRESSED_SIZE), 4);
      assert.equal(bytes.readUInt32LE(LOCAL_SIZE), 4);
    }
    assert.deepEqual(file, content);
  });

  it('should state the sizes of a deflated file entry after its data', async () => {
    const content = await collect(zip([{ name: 'a.txt', content: 'text' }], { date: DATE }));
    const file = await collect(
      zip([{ name: 'a.txt', file: source('a.txt', 'text') }], { date: DATE }),
    );

    assert.equal(content.readUInt16LE(LOCAL_FLAGS) & FLAG_DESCRIPTOR, 0);
    assert.equal(file.readUInt16LE(LOCAL_FLAGS) & FLAG_DESCRIPTOR, FLAG_DESCRIPTOR);
    assert.equal(file.readUInt32LE(LOCAL_CRC), 0);
    assert.equal(file.readUInt32LE(LOCAL_SIZE), 0);
    assert.equal(file.length, content.length + 16);
  });

  it('should refuse a stored file that changes between its two reads', async () => {
    for (const next of ['text', 'longer text']) {
      const file = source('a.txt', 'TEXT');
      const chunks = zip([{ name: 'a.txt', file, store: true }]);

      const header = await chunks.next();
      fs.writeFileSync(file, next);

      assert.equal(header.value.readUInt32LE(LOCAL_SIZE), 4);
      await assert.rejects(collect(chunks), /Entry "a.txt" changed while it was read/);
    }
  });

  it('should write zip64 records with forceZip64 that the reader gives back', async () => {
    const long = Buffer.from('scene '.repeat(50000));
    const image = Buffer.from([0x89, 0x50, 0x4e, 0x47, 0, 1, 2, 3]);
    const bytes = await collect(
      zip(
        [
          { name: 'scene/scene.json', content: '{"main":"bin/index.js"}' },
          { name: 'scene/notes.txt', content: 'stored text', store: true },
          { name: 'scene/src/index.ts', file: source('index.ts', long) },
          { name: 'scene/images/a.png', file: source('a.png', image), store: true },
          { name: 'scene/empty.txt', file: source('empty.txt', '') },
        ],
        { forceZip64: true },
      ),
    );

    const entries = await unpack(bytes);

    const read = name => fs.readFileSync(path.join(target, name));
    assert.equal(read('scene/scene.json').toString(), '{"main":"bin/index.js"}');
    assert.equal(read('scene/notes.txt').toString(), 'stored text');
    assert.deepEqual(read('scene/src/index.ts'), long);
    assert.deepEqual(read('scene/images/a.png'), image);
    assert.equal(read('scene/empty.txt').length, 0);
    assert.equal(entries.get('scene/images/a.png').compressedSize, image.length);
    assert.equal(entries.get('scene/src/index.ts').uncompressedSize, long.length);

    let at = 0;
    for (const entry of entries.values()) {
      assert.equal(entry.localOffset, at);
      assert.equal(bytes.readUInt16LE(at + LOCAL_VERSION), ZIP64_VERSION);
      const extra = at + LOCAL_HEADER + bytes.readUInt16LE(at + LOCAL_NAME_SIZE);
      assert.equal(bytes.readUInt16LE(extra), ZIP64_EXTRA);
      assert.equal(bytes.readUInt16LE(at + LOCAL_EXTRA_SIZE), 20);
      // With a descriptor too: a streaming reader then takes the 64-bit descriptor.
      assert.equal(bytes.readUInt32LE(at + LOCAL_COMPRESSED_SIZE), 0xffffffff);
      assert.equal(bytes.readUInt32LE(at + LOCAL_SIZE), 0xffffffff);
      const descriptor = bytes.readUInt16LE(at + LOCAL_FLAGS) & FLAG_DESCRIPTOR ? 24 : 0;
      at = extra + 20 + entry.compressedSize + descriptor;
    }
    const end = bytes.length - END;
    assert.equal(bytes.readUInt16LE(end + 10), 0xffff);
    assert.equal(bytes.readUInt32LE(end + 16), 0xffffffff);
    assert.equal(bytes.readUInt32LE(end - LOCATOR), ZIP64_LOCATOR_SIGNATURE);
    assert.equal(bytes.readUInt32LE(end - LOCATOR - 56), ZIP64_END_SIGNATURE);
  });

  it('should write a name that is not ASCII as UTF-8', async () => {
    const bytes = await collect(zip([{ name: 'escena/año.txt', content: 'text' }]));

    const entries = await unpack(bytes);

    assert.equal(bytes.readUInt16LE(LOCAL_FLAGS) & FLAG_UTF8, FLAG_UTF8);
    assert.deepEqual([...entries.keys()], ['escena/año.txt']);
  });

  it('should give each entry and the count of written entries to onEntry', async () => {
    const seen = [];

    await collect(
      zip(
        [
          { name: 'a.txt', content: 'a' },
          { name: 'b.txt', file: source('b.txt', 'b') },
        ],
        { onEntry: (entry, count) => seen.push([entry.name, count]) },
      ),
    );

    assert.deepEqual(seen, [
      ['a.txt', 1],
      ['b.txt', 2],
    ]);
  });

  it('should write the date of the archive on each entry', async () => {
    const bytes = await collect(
      zip([{ name: 'a.txt', content: 'a' }], { date: new Date(2024, 4, 17, 13, 45, 58) }),
    );

    assert.equal(bytes.readUInt16LE(10), (13 << 11) | (45 << 5) | 29);
    assert.equal(bytes.readUInt16LE(12), ((2024 - 1980) << 9) | (5 << 5) | 17);
  });

  it('should write an archive with no entry', async () => {
    const bytes = await collect(zip([]));

    const entries = await unpack(bytes);

    assert.equal(bytes.length, 22);
    assert.equal(entries.size, 0);
  });

  it('should take the entries from an async source', async () => {
    async function* entries() {
      yield { name: 'a.txt', content: 'a' };
      yield { name: 'b.txt', content: 'b' };
    }

    const read = await unpack(await collect(zip(entries())));

    assert.deepEqual([...read.keys()], ['a.txt', 'b.txt']);
  });

  it('should stop when a file entry cannot be read', async () => {
    const missing = path.join(folder, 'missing.txt');

    for (const store of [true, false]) {
      await assert.rejects(
        collect(
          zip([
            { name: 'a.txt', file: missing, store },
            { name: 'b.txt', content: 'b' },
          ]),
        ),
        { code: 'ENOENT' },
      );
    }
  });

  it('should refuse an entry with no name', async () => {
    await assert.rejects(collect(zip([{ name: '', content: 'a' }])), /Invalid entry name/);
  });

  it('should write zip64 end records only above the entry count of the plain format', async () => {
    const entries = length =>
      Array.from({ length }, (_, index) => ({ name: String(index), content: '', store: true }));

    const plain = await collect(zip(entries(65534)));
    const zip64 = await collect(zip(entries(65535)));

    assert.equal(plain.readUInt16LE(plain.length - 12), 65534);
    assert.notEqual(plain.readUInt32LE(plain.length - END - LOCATOR), ZIP64_LOCATOR_SIGNATURE);
    assert.equal(zip64.readUInt16LE(zip64.length - 12), 0xffff);
    assert.equal(zip64.readUInt32LE(zip64.length - END - LOCATOR), ZIP64_LOCATOR_SIGNATURE);
    assert.equal(zip64.readBigUInt64LE(zip64.length - END - LOCATOR - 56 + 32), 65535n);
  });

  // The test needs a file with a hole. On Windows such a file takes its full size on the disk.
  const sparse = process.platform !== 'win32';

  it(
    'should state zip64 for a file entry that is larger than 4 GiB',
    { skip: !sparse },
    async () => {
      const large = path.join(folder, 'large.bin');
      fs.closeSync(fs.openSync(large, 'w'));
      fs.truncateSync(large, 2 ** 32);

      // The local header of a deflated file entry comes before its data is read.
      const chunks = zip([{ name: 'large.bin', file: large }]);
      const { value: header } = await chunks.next();
      await chunks.return();

      assert.equal(header.readUInt16LE(LOCAL_VERSION), ZIP64_VERSION);
      assert.equal(header.readUInt16LE(LOCAL_FLAGS) & FLAG_DESCRIPTOR, FLAG_DESCRIPTOR);
      assert.equal(header.readUInt16LE(LOCAL_HEADER + 'large.bin'.length), ZIP64_EXTRA);
      assert.equal(header.readUInt32LE(LOCAL_COMPRESSED_SIZE), 0xffffffff);
      assert.equal(header.readUInt32LE(LOCAL_SIZE), 0xffffffff);
    },
  );
});
