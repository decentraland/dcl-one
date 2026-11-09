/** A GLB whose JSON chunk is `json`, without a binary chunk. */
export function glb(json: unknown): Uint8Array {
  const text = new TextEncoder().encode(JSON.stringify(json));
  const padded = new Uint8Array(Math.ceil(text.byteLength / 4) * 4).fill(0x20);
  padded.set(text);
  const out = new Uint8Array(12 + 8 + padded.byteLength);
  const view = new DataView(out.buffer);
  view.setUint32(0, 0x46546c67, true);
  view.setUint32(4, 2, true);
  view.setUint32(8, out.byteLength, true);
  view.setUint32(12, padded.byteLength, true);
  view.setUint32(16, 0x4e4f534a, true);
  out.set(padded, 20);
  return out;
}

/** The bytes of a .gltf file holding `json`. */
export const gltfText = (json: unknown) => new TextEncoder().encode(JSON.stringify(json));
