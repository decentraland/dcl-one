// from: catalyrst-v2 sites/src/data/lib/catalyst/hashing.ts (hashV1, the @dcl/hashing CIDv1 twin);
//       dcl-catalyst-client DeploymentBuilder.buildEntity and ContentClient.deploy (as upstream shop's saveStore uses them)
// Deploying a content entity from the browser without dcl-catalyst-client: files and the entity JSON are
// hashed exactly as the content server re-hashes them (CIDv1, raw leaves, 256 KiB UnixFS chunks behind a
// dag-pb root for anything larger), the entity id is signed by the session's ephemeral key on top of its
// auth chain, and everything goes to `POST {peer}/content/entities` as multipart form data. ui3 cannot
// import sites, so the hashing lives here too (the vectors in its test are the ones sites pins).

import type { AuthIdentity, AuthLink } from "../../../data/auth/identity";

const CHUNK = 262_144;
const MAX_CHILDREN = 174;
const B32 = "abcdefghijklmnopqrstuvwxyz234567";
const RAW = 0x55;
const DAG_PB = 0x70;

function base32(bytes: Uint8Array): string {
  let bits = 0;
  let value = 0;
  let out = "";
  for (const byte of bytes) {
    value = (value << 8) | byte;
    bits += 8;
    while (bits >= 5) {
      out += B32[(value >>> (bits - 5)) & 31];
      bits -= 5;
    }
  }
  if (bits > 0) out += B32[(value << (5 - bits)) & 31];
  return out;
}

async function sha256(bytes: Uint8Array): Promise<Uint8Array> {
  const copy = new Uint8Array(bytes.byteLength);
  copy.set(bytes);
  return new Uint8Array(await crypto.subtle.digest("SHA-256", copy.buffer));
}

function cid(codec: number, digest: Uint8Array): Uint8Array {
  const out = new Uint8Array(4 + digest.length);
  out.set([0x01, codec, 0x12, 0x20]);
  out.set(digest, 4);
  return out;
}

function varint(out: number[], value: number): void {
  let v = value;
  while (v >= 0x80) {
    out.push((v % 0x80) + 0x80);
    v = Math.floor(v / 0x80);
  }
  out.push(v);
}

type Node = { cid: Uint8Array; tsize: number; fileSize: number };

async function reduce(batch: readonly Node[]): Promise<Node> {
  const data: number[] = [0x08, 0x02, 0x18];
  let fileSize = 0;
  for (const n of batch) fileSize += n.fileSize;
  varint(data, fileSize);
  for (const n of batch) {
    data.push(0x20);
    varint(data, n.fileSize);
  }
  const block: number[] = [];
  for (const n of batch) {
    const link: number[] = [0x0a];
    varint(link, n.cid.length);
    link.push(...n.cid, 0x12, 0x00, 0x18);
    varint(link, n.tsize);
    block.push(0x12);
    varint(block, link.length);
    block.push(...link);
  }
  block.push(0x0a);
  varint(block, data.length);
  block.push(...data);
  let tsize = block.length;
  for (const n of batch) tsize += n.tsize;
  return { cid: cid(DAG_PB, await sha256(Uint8Array.from(block))), tsize, fileSize };
}

/** @dcl/hashing `hashV1`: the content hash the content server computes for `bytes`. */
export async function hashV1(bytes: Uint8Array): Promise<string> {
  if (bytes.length <= CHUNK) return `b${base32(cid(RAW, await sha256(bytes)))}`;
  let level: Node[] = [];
  for (let off = 0; off < bytes.length; off += CHUNK) {
    const chunk = bytes.subarray(off, Math.min(off + CHUNK, bytes.length));
    level.push({ cid: cid(RAW, await sha256(chunk)), tsize: chunk.length, fileSize: chunk.length });
  }
  while (level.length > 1) {
    const next: Node[] = [];
    for (let i = 0; i < level.length; i += MAX_CHILDREN) next.push(await reduce(level.slice(i, i + MAX_CHILDREN)));
    level = next;
  }
  return `b${base32(level[0]!.cid)}`;
}

export type BuiltEntity = { entityId: string; files: Map<string, Uint8Array>; entity: Record<string, unknown> };

/** DeploymentBuilder.buildEntity: `v3` entity JSON over the files' hashes; the JSON is itself a file. */
export async function buildEntity(opts: {
  type: string;
  pointers: string[];
  files: Map<string, Uint8Array>;
  metadata: unknown;
  timestamp: number;
}): Promise<BuiltEntity> {
  const files = new Map<string, Uint8Array>();
  const content: { file: string; hash: string }[] = [];
  for (const [name, bytes] of opts.files) {
    const hash = await hashV1(bytes);
    content.push({ file: name, hash });
    files.set(hash, bytes);
  }
  const entity = { version: "v3", type: opts.type, pointers: opts.pointers, timestamp: opts.timestamp, content, metadata: opts.metadata };
  const entityFile = new TextEncoder().encode(JSON.stringify(entity));
  const entityId = await hashV1(entityFile);
  files.set(entityId, entityFile);
  return { entityId, files, entity };
}

/** `Authenticator.signPayload(identity, entityId)`: the auth chain plus the ephemeral key's signature of the id. */
export async function signEntityId(identity: AuthIdentity, entityId: string): Promise<AuthLink[]> {
  const { privateKeyToAccount } = await import("viem/accounts");
  const signature = await privateKeyToAccount(identity.ephemeral.privateKey).signMessage({ message: entityId });
  return [...identity.authChain, { type: "ECDSA_SIGNED_ENTITY", payload: entityId, signature }];
}

/** ContentClient.deploy: multipart `entityId`, `authChain[i][…]` and one part per file named by its hash. */
export async function deployEntity(opts: {
  contentUrl: string;
  built: BuiltEntity;
  authChain: AuthLink[];
  fetchImpl?: typeof fetch;
}): Promise<void> {
  const form = new FormData();
  form.append("entityId", opts.built.entityId);
  opts.authChain.forEach((link, i) => {
    form.append(`authChain[${i}][type]`, link.type);
    form.append(`authChain[${i}][payload]`, link.payload);
    form.append(`authChain[${i}][signature]`, link.signature);
  });
  for (const [hash, bytes] of opts.built.files) {
    const copy = new Uint8Array(bytes.byteLength);
    copy.set(bytes);
    form.append(hash, new Blob([copy.buffer]), hash);
  }
  const res = await (opts.fetchImpl ?? fetch)(`${opts.contentUrl}/entities`, { method: "POST", body: form });
  if (!res.ok) throw new Error(`The content server refused the deployment (${res.status}): ${await res.text().catch(() => "")}`);
}
