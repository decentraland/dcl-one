/** Byte-wise comparison; skips the read when the sizes already differ. */
export async function blobsAreEqual(a: Blob, b: Blob): Promise<boolean> {
  if (a === b) return true
  if (a.size !== b.size) return false
  const [x, y] = await Promise.all([a.arrayBuffer(), b.arrayBuffer()])
  const v = new Uint8Array(y)
  return new Uint8Array(x).every((byte, i) => byte === v[i])
}
