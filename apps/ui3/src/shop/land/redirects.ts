import { landTokenId } from "../../data/catalyst/landListing";
import { landNetwork } from "./contracts";

export type LandRedirect = "lands" | "rent" | "transfer" | "operator" | "assign-ens";

type Identity = { contract: string; tokenId: string };

const IDENTITY_KEYS = ["id", "landId", "contractAddress", "contract", "tokenId", "x", "y", "estateId", "category"];

export function landIdentity(q: URLSearchParams, chainId?: number): Identity | null {
  const net = landNetwork(chainId);
  const contract = (q.get("contractAddress") ?? q.get("contract"))?.toLowerCase();
  const tokenId = q.get("tokenId");
  if (contract && /^0x[0-9a-f]{40}$/.test(contract) && tokenId && /^[0-9]+$/.test(tokenId)) return { contract, tokenId };
  const id = (q.get("id") ?? q.get("landId"))?.trim();
  if (id) {
    const pair = /^(0x[0-9a-fA-F]{40})[-:]([0-9]+)$/.exec(id);
    if (pair) return { contract: pair[1]!.toLowerCase(), tokenId: pair[2]! };
    const coords = /^(-?\d+)\s*,\s*(-?\d+)$/.exec(id);
    if (coords) return { contract: net.land, tokenId: landTokenId(Number(coords[1]), Number(coords[2])) };
    if (/^[0-9]+$/.test(id)) return { contract: net.estate, tokenId: id };
  }
  const x = q.get("x");
  const y = q.get("y");
  if (x && y && /^-?\d+$/.test(x) && /^-?\d+$/.test(y)) return { contract: net.land, tokenId: landTokenId(Number(x), Number(y)) };
  const estateId = q.get("estateId");
  if (estateId && /^[0-9]+$/.test(estateId)) return { contract: net.estate, tokenId: estateId };
  return null;
}

function rest(q: URLSearchParams, drop: readonly string[]): URLSearchParams {
  const out = new URLSearchParams();
  for (const [k, v] of q) if (!drop.includes(k)) out.append(k, v);
  return out;
}

function withQuery(path: string, q: URLSearchParams): string {
  const s = q.toString();
  return s ? `${path}${path.includes("?") ? "&" : "?"}${s}` : path;
}

export function landRedirectTarget(kind: LandRedirect, search: string, chainId?: number): string {
  const q = new URLSearchParams(search);
  if (kind === "lands") {
    const out = rest(q, ["category"]);
    const category = q.get("category");
    if ((category === "parcel" || category === "estate") && !out.has("section")) out.set("section", category === "parcel" ? "parcels" : "estates");
    return withQuery("/shop/lands", out);
  }
  const identity = landIdentity(q, chainId);
  const carried = rest(q, IDENTITY_KEYS);
  if (kind === "rent") {
    if (identity) return withQuery(`/shop/token/${identity.contract}/${identity.tokenId}/rent`, carried);
    const out = rest(q, IDENTITY_KEYS);
    out.set("onlyOnRent", "true");
    return withQuery("/shop/lands", out);
  }
  if (!identity) return withQuery("/create/land", carried);
  const page = kind === "transfer" ? "transfer" : "manage";
  return withQuery(`/shop/token/${identity.contract}/${identity.tokenId}/${page}`, carried);
}
