'use strict';

const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { afterEach, beforeEach, describe, it } = require('node:test');

const extract = require('./index.js');
const { zip } = require('./write.js');

const LOCAL_FLAGS = 6;
const LOCAL_SIZE = 22;
const FLAG_DESCRIPTOR = 0x8;
const FLAG_UTF8 = 0x800;
const STORED = 0;
const DEFLATED = 8;

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

  it('should state the sizes of a content entry before its data and of a file entry after it', async () => {
    const content = await collect(zip([{ name: 'a.txt', content: 'text', store: true }]));
    const file = await collect(
      zip([{ name: 'a.txt', file: source('a.txt', 'text'), store: true }]),
    );

    assert.equal(content.readUInt16LE(LOCAL_FLAGS) & FLAG_DESCRIPTOR, 0);
    assert.equal(content.readUInt32LE(LOCAL_SIZE), 4);
    assert.equal(file.readUInt16LE(LOCAL_FLAGS) & FLAG_DESCRIPTOR, FLAG_DESCRIPTOR);
    assert.equal(file.readUInt32LE(LOCAL_SIZE), 0);
    assert.equal(file.length, content.length + 16);
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

  it('should refuse more entries than an archive without zip64 holds', async () => {
    const entries = Array.from({ length: 65535 }, (_, index) => ({
      name: String(index),
      content: '',
      store: true,
    }));

    await assert.rejects(collect(zip(entries)), /at most 65534 entries without zip64/);
  });

  it('should write as many entries as an archive without zip64 holds', async () => {
    const entries = Array.from({ length: 65534 }, (_, index) => ({
      name: String(index),
      content: '',
      store: true,
    }));

    const bytes = await collect(zip(entries));

    assert.equal(bytes.readUInt16LE(bytes.length - 12), 65534);
  });

  // The test needs a file with a hole. On Windows such a file takes its full size on the disk.
  const sparse = process.platform !== 'win32';

  it('should refuse a file entry that is larger than 4 GiB', { skip: !sparse }, async () => {
    const large = path.join(folder, 'large.bin');
    fs.closeSync(fs.openSync(large, 'w'));
    fs.truncateSync(large, 2 ** 32);
    let written = 0;

    await assert.rejects(async () => {
      for await (const chunk of zip([{ name: 'large.bin', file: large, store: true }])) {
        written += chunk.length;
      }
    }, /Entry "large.bin" is larger than 4 GiB, and zip64 is not supported/);

    assert.ok(written < 2 ** 32);
  });
});
