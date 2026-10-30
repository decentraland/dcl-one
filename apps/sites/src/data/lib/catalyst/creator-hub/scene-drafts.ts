export type DraftMeta = {
  id: string;
  hash: string;
  version: number;
  updatedAt: number;
  title: string;
};

export type Draft = DraftMeta & { blob: unknown };

export function hashComposite(text: string): string {
  let h = 5381;
  for (let i = 0; i < text.length; i++) h = ((h << 5) + h + text.charCodeAt(i)) | 0;
  return (h >>> 0).toString(16).padStart(8, "0") + "-" + (text.length >>> 0).toString(16);
}

export function renameComposite(text: string, title: string): string {
  const object = (value: unknown): value is Record<string, unknown> => Boolean(value && typeof value === "object" && !Array.isArray(value));
  let composite;
  try { composite = JSON.parse(text); } catch { return text; }
  if (!Array.isArray(composite?.components)) return text;
  let changed = false;
  for (const component of composite.components) {
    if (component?.name !== "inspector::SceneMetadata-v3" || !object(component.data?.["0"])) continue;
    const root = component.data["0"];
    const key = "json" in root ? "json" : "value" in root ? "value" : null;
    let metadata = key ? root[key] : root;
    const encoded = typeof metadata === "string";
    if (typeof metadata === "string") { try { metadata = JSON.parse(metadata); } catch { continue; } }
    if (!object(metadata)) continue;
    const renamed = { ...metadata, name: title };
    component.data["0"] = key ? { ...root, [key]: encoded ? JSON.stringify(renamed) : renamed } : renamed;
    changed = true;
  }
  return changed ? JSON.stringify(composite) : text;
}

const WALLET_RE = /^0x[0-9a-f]{40}$/;

const DRAFT_ID_RE = /^[a-z0-9_.,-]{1,200}$/i;

export function normalizeWallet(wallet: string): string {
  return typeof wallet === "string" ? wallet.trim().toLowerCase() : "";
}

export function isSafeWallet(wallet: string): boolean {
  return WALLET_RE.test(normalizeWallet(wallet));
}

export function isSafeDraftId(id: string): boolean {
  return (
    typeof id === "string" &&
    DRAFT_ID_RE.test(id) &&
    !id.includes("..") &&
    id !== "."
  );
}
