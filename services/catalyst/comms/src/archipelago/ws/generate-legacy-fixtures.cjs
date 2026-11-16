// NODE_PATH must select the authenticated locked packages. Output is deterministic.
// Usage: node generate-legacy-fixtures.cjs <output.json>
const fs = require('node:fs');
const crypto = require('node:crypto');
const pins = {
  '@dcl/protocol/out-js/decentraland/kernel/comms/v3/archipelago.gen.js': '3366350dc329d3aa8a56b10e31b6723c480efd8b0a7ec8b48805f22e1e5a5b9e',
  'protobufjs/src/reader.js': '7d511b51e60bfc47df4b2dda3bd87cfc411b0d5f89f3ce92c748bee2af4dc99c',
  'protobufjs/src/reader_buffer.js': 'b942cc88d4dd75b3ca378ea53c7b7037909cc388e31d35fcdfb76ea5d8c5d096',
};
for (const [name, expected] of Object.entries(pins)) {
  const actual = crypto.createHash('sha256').update(fs.readFileSync(require.resolve(name))).digest('hex');
  if (actual !== expected) throw new Error(`Unauthenticated decoder: ${name}`);
}
const { ClientPacket } = require('@dcl/protocol/out-js/decentraland/kernel/comms/v3/archipelago.gen');
function project(bytes) {
  try {
    const message = ClientPacket.decode(Buffer.from(bytes)).message;
    if (!message) return null;
    const arm = message.$case;
    if (arm === 'challengeRequest') return { arm, text: message[arm].address };
    if (arm === 'signedChallenge') return { arm, text: message[arm].authChainJson };
    const hb = message.heartbeat;
    const bits = value => { const b = Buffer.alloc(4); b.writeFloatLE(value); return b.readUInt32LE(); };
    return { arm, position: hb.position ? [hb.position.x, hb.position.y, hb.position.z].map(bits) : null,
      desiredRoom: hb.desiredRoom ?? null };
  } catch { return { error: true }; }
}
const heartbeat = [26, 7, 10, 5, 13, 0, 0, 128, 63];
const address = Array.from(Buffer.from('0x0000000000000000000000000000000000000001'));
const challenge = [10, 44, 10, 42, ...address];
const inputs = [[], [0], [28], [24, 0], heartbeat, [26, 9, 10, 5, 13, 0, 0, 128, 63, 10, 0],
  challenge, [...challenge, 10, 0], [10, 46, 10, 42, ...address, 18, 0],
  [10, 3, 10, 1, 255], [10, 3, 10, 9, 97], [26, 1, 10], [255], [11, 20], [11, 16, 0, 12]];
for (const suffix of [[26, 0], [34, 0], [42, 0], [0], [28], [24, 0], [255], [11, 12], [11, 20]]) {
  inputs.push([...heartbeat, ...suffix], [...challenge, ...suffix], [...suffix, ...heartbeat]);
}
for (let length = 1; length <= 12; length++) inputs.push([...Array(length).fill(128), 0]);
let seed = 0x125abc;
for (let i = 0; i < 512; i++) {
  const bytes = [];
  for (let j = 0; j <= i % 20; j++) { seed ^= seed << 13; seed ^= seed >>> 17; seed ^= seed << 5; bytes.push(seed & 255); }
  inputs.push(bytes);
}
const output = { source: 'decentraland/archipelago-workers@64dc515987175c679e4009fe557ea60fdf50bc6c', pins,
  cases: inputs.map(bytes => ({ bytes, expected: project(bytes) })) };
if (!process.argv[2]) throw new Error('Output path required');
fs.writeFileSync(process.argv[2], JSON.stringify(output, null, 2) + '\n');
