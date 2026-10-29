// from: shop@2d3bf62:app/src/pages/OutfitStudio.tsx, app/src/pages/OutfitStudio.styles.ts,
//       app/src/components/OutfitItemPicker/OutfitItemPicker.tsx, app/src/components/EmptyState/EmptyState.tsx
//       (assets/empty/signin-empty.svg vendored as outfit-assets/signin-empty.svg)

import { lazy, Suspense, useEffect, useMemo, useRef, useState } from "react";
import { Link, useLocation, useNavigate, useParams } from "react-router";

import { isShopApiError } from "../../api/client";
import { BASE_FEMALE, BASE_MALE } from "../../detail/lib/bodyShape";
import { itemUrn } from "../../detail/lib/urn";
import { SHOP_BASE } from "../../routes";
import { useShopWallet } from "../../wallet/context";
import { fetchCatalogByIds, fetchShopItems } from "../api";
import { cardPrice } from "../cardPrice";
import Icon from "../components/Icon";
import PriceTag, { formatCreditsFull } from "../components/PriceTag";
import { useSeo } from "../hooks";
import type { CatalogItem } from "../lib/catalogItem";
import { isWearable, playingEmote } from "../lib/outfit";
import { invalidateShopQueries, primeShopQuery, useShopQuery } from "../query";
import {
  DEFAULT_OUTFIT_GRADIENT,
  MAX_OUTFIT_ITEMS,
  MAX_THUMBNAIL_BYTES,
  MIN_OUTFIT_ITEMS,
  deleteOutfit,
  fetchAllOutfits,
  fetchOutfit,
  isHexColor,
  outfitErrorMessage,
  outfitGradient,
  outfitItemKey,
  outfitRadialGradient,
  OUTFIT_ERRORS,
  parseOutfitImport,
  saveOutfit,
  thumbnailUrl,
  toggleOutfitItem,
  uploadThumbnail,
  type Outfit,
  type OutfitBodyShape,
  type OutfitDraft,
} from "./outfits";
import signInIllustration from "./outfit-assets/signin-empty.svg";
import "./outfitstudio.css";

const WearablePreview = lazy(() => import("../../../wearable-preview/WearablePreview"));

const COPY = {
  title: "Outfit Studio",
  myOutfits: "My outfits",
  new: "New outfit",
  editTitle: "Edit outfit",
  signInTitle: "Sign in to manage your outfits",
  signInBody: "Sign in to build and save outfits from the items you own.",
  signIn: "Sign in",
  notAvailable: "This area is only for the shop's styling team.",
  backToShop: "Back to the shop",
  untitled: "Untitled outfit",
  view: "View",
  edit: "Edit",
  draft: "Draft",
  published: "Published",
  deleteTitle: "Delete this outfit?",
  deleteBody: "Unpublishing hides it from the shop but keeps it here. Deleting can't be undone.",
  confirmDelete: "Delete",
  cancel: "Cancel",
  unpublish: "Unpublish",
  saving: "Saving…",
  save: "Save",
  publish: "Publish",
  name: "Name",
  namePlaceholder: "e.g. Neon Nights",
  items: "Items",
  noItems: "No items yet — pick some from the catalog below.",
  maxItems: `An outfit can have up to ${MAX_OUTFIT_ITEMS} items.`,
  remove: "Remove",
  playsInPreview: "Plays in preview",
  needsWearable: "Add at least one wearable to publish.",
  pickerCategory: "What to browse",
  searchWearables: "Search wearables…",
  searchEmotes: "Search emotes…",
  noResults: "No results",
  gradient: "Background colors",
  gradientTop: "Top",
  gradientBottom: "Bottom",
  gradientHint: "The outfit is shown over these two colors, blending from top to bottom.",
  thumbnail: "Thumbnail",
  thumbnailHint: "PNG, JPEG or WebP, up to 1 MB. Portrait (3:4) works best — upload it with a transparent background so the colors show through.",
  upload: "Upload image",
  uploading: "Uploading…",
  bodyShape: "Body shape",
  shape: { unisex: "Both", male: "Male", female: "Female" } as Record<OutfitBodyShape, string>,
  previewEmpty: "Pick items to see them on the mannequin.",
  needName: "Give the outfit a name to save it.",
  empty: "No outfits yet — create the first one.",
  cardPreview: "Card preview",
  import: "Import from a preview link",
  importPlaceholder: "Paste an avatar preview link or code…",
  importApply: "Import",
  importHint: "Paste the preview link from the avatar editor — its items, body shape and colors fill the outfit.",
  importError: "We couldn't find outfit items in that link.",
  retry: "Try again",
  unavailable: "No longer available",
  totalPrice: "Total price",
  loadMore: "Load more",
};

const THUMB_TYPES = ["image/png", "image/jpeg", "image/webp"];

function useStudioAccess() {
  const { session } = useShopWallet();
  const signedIn = session.kind !== "signedOut";
  const q = useShopQuery(signedIn ? ["outfits-all", session.kind === "web3" ? session.wallet : session.kind === "web2" ? session.account : ""] : null, (signal) => fetchAllOutfits(signal), { staleMs: 30_000 });
  const denied = isShopApiError(q.error) && (q.error.status === 401 || q.error.status === 403 || q.error.status === 404 || q.error.code === "noLegacyAlias");
  return { signedIn, list: q, denied };
}

function SignInPanel() {
  const { host } = useShopWallet();
  return (
    <div className="shop-studio__fill">
      <div className="shop-studio__empty" data-fill="true" data-testid="outfit-studio-signin">
        <img className="shop-studio__empty-art" src={signInIllustration} alt="" aria-hidden />
        <div className="shop-studio__empty-text">
          <p className="shop-studio__empty-title">{COPY.signInTitle}</p>
          <p className="shop-studio__empty-body">{COPY.signInBody}</p>
        </div>
        <button type="button" className="shop-studio__empty-cta" onClick={() => host?.onSignIn?.()}>
          {COPY.signIn}
        </button>
      </div>
    </div>
  );
}

export default function OutfitStudio() {
  const { id } = useParams<{ id?: string }>();
  const { pathname } = useLocation();
  const access = useStudioAccess();
  useSeo({ title: COPY.title });
  const mode = pathname.endsWith("/new") ? "new" : id ? "edit" : "list";

  if (!access.signedIn) {
    return (
      <div className="shop-page shop-studio">
        <SignInPanel />
      </div>
    );
  }
  if (access.denied) {
    return (
      <div className="shop-page shop-studio">
        <div className="shop-studio__gate" data-testid="outfit-studio-unavailable">
          <h1 className="shop-studio__gate-title">{COPY.title}</h1>
          <p className="shop-studio__hint">{COPY.notAvailable}</p>
          <Link className="shop-studio__btn" data-variant="white" to={`${SHOP_BASE}/overview`}>
            {COPY.backToShop}
          </Link>
        </div>
      </div>
    );
  }
  if (access.list.isLoading && !access.list.data) {
    return (
      <div className="shop-page shop-studio">
        <div className="shop-studio__gate" aria-busy="true" data-testid="outfit-studio-loading">
          <span className="shop-studio__spinner" aria-hidden />
        </div>
      </div>
    );
  }
  if (mode === "list") return <StudioList outfits={access.list.data ?? []} error={access.list.error} onRetry={access.list.refetch} />;
  return <StudioEditor key={id ?? "new"} outfitId={mode === "edit" ? id! : null} />;
}

function RowThumb({ outfit }: { outfit: Outfit }) {
  const thumb = thumbnailUrl(outfit.thumbnailHash);
  if (!thumb) return <span className="shop-studio__rowthumb" data-empty aria-hidden />;
  return (
    <span className="shop-studio__rowthumb" style={{ background: outfitGradient(outfit) }}>
      <img src={thumb} alt="" loading="lazy" />
    </span>
  );
}

function refreshOutfits() {
  invalidateShopQueries('["outfits-all"');
  invalidateShopQueries('["outfit",');
}

function StudioList({ outfits, error, onRetry }: { outfits: Outfit[]; error: unknown; onRetry: () => void }) {
  const [confirming, setConfirming] = useState<Outfit | null>(null);
  const [busyId, setBusyId] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  async function mutate(outfit: Outfit, action: "unpublish" | "delete") {
    setBusyId(outfit.id);
    try {
      if (action === "delete") {
        await deleteOutfit(outfit.id);
        setNotice("Outfit deleted");
      } else {
        await saveOutfit({ ...toDraft(outfit), published: false }, "update");
        setNotice("Outfit unpublished");
      }
      refreshOutfits();
    } catch (e) {
      setNotice(outfitErrorMessage(e));
    } finally {
      setBusyId(null);
      setConfirming(null);
    }
  }

  return (
    <div className="shop-page shop-studio" data-testid="outfit-studio-list">
      <div className="shop-studio__head">
        <h1 className="shop-studio__title">{COPY.myOutfits}</h1>
        <Link className="shop-studio__btn" data-variant="white" to={`${SHOP_BASE}/outfits/new`} data-testid="outfit-studio-new">
          {COPY.new}
        </Link>
      </div>
      {notice ? (
        <p className="shop-studio__hint" role="status">
          {notice}
        </p>
      ) : null}
      {error && outfits.length === 0 ? (
        <div className="shop-studio__gate">
          <p className="shop-studio__error">{OUTFIT_ERRORS.generic}</p>
          <button type="button" className="shop-studio__btn" data-variant="white" onClick={onRetry}>
            {COPY.retry}
          </button>
        </div>
      ) : outfits.length === 0 ? (
        <p className="shop-studio__hint">{COPY.empty}</p>
      ) : (
        <ul className="shop-studio__list">
          {outfits.map((outfit) => (
            <li key={outfit.id} className="shop-studio__row" data-testid="outfit-studio-row" data-published={outfit.published || undefined}>
              <RowThumb outfit={outfit} />
              <span className="shop-studio__rowinfo">
                <span className="shop-studio__rowname">{outfit.name || COPY.untitled}</span>
                <span className="shop-studio__rowmeta">
                  <span className="shop-studio__state" data-state={outfit.published ? "published" : "draft"}>
                    {outfit.published ? COPY.published : COPY.draft}
                  </span>
                  {outfit.items.length} {outfit.items.length === 1 ? "item" : "items"}
                </span>
              </span>
              <span className="shop-studio__rowactions">
                {outfit.published ? (
                  <Link className="shop-studio__btn" data-variant="ghost" data-size="sm" to={`${SHOP_BASE}/items/outfits/${outfit.id}`}>
                    {COPY.view}
                  </Link>
                ) : null}
                <Link className="shop-studio__btn" data-variant="outline" data-size="sm" to={`${SHOP_BASE}/outfits/${outfit.id}/edit`} data-testid="outfit-studio-edit">
                  {COPY.edit}
                </Link>
                <button type="button" className="shop-studio__iconbtn" data-testid="outfit-studio-delete" aria-label={COPY.confirmDelete} disabled={busyId === outfit.id} onClick={() => setConfirming(outfit)}>
                  <TrashIcon />
                </button>
              </span>
            </li>
          ))}
        </ul>
      )}

      {confirming ? (
        <div className="shop-studio__modal" role="dialog" aria-modal="true" aria-label={COPY.deleteTitle} data-testid="outfit-studio-confirm">
          <div className="shop-studio__scrim" onClick={() => setConfirming(null)} />
          <div className="shop-studio__panel">
            <h2 className="shop-studio__panel-title">{COPY.deleteTitle}</h2>
            <p className="shop-studio__hint">{COPY.deleteBody}</p>
            <div className="shop-studio__panel-actions">
              {confirming.published ? (
                <button type="button" className="shop-studio__btn" data-variant="purple" data-testid="outfit-studio-confirm-unpublish" disabled={busyId === confirming.id} onClick={() => void mutate(confirming, "unpublish")}>
                  {COPY.unpublish}
                </button>
              ) : null}
              <button type="button" className="shop-studio__btn" data-variant={confirming.published ? "outline" : "purple"} data-testid="outfit-studio-confirm-delete" disabled={busyId === confirming.id} onClick={() => void mutate(confirming, "delete")}>
                {COPY.confirmDelete}
              </button>
              <button type="button" className="shop-studio__btn" data-variant="ghost" onClick={() => setConfirming(null)}>
                {COPY.cancel}
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}

function TrashIcon({ size = 18 }: { size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <path d="M3 6h18M8 6V4h8v2M19 6l-1 14H6L5 6M10 11v6M14 11v6" />
    </svg>
  );
}

function emptyDraft(): OutfitDraft {
  return {
    id: typeof crypto !== "undefined" && "randomUUID" in crypto ? crypto.randomUUID() : `${Date.now()}`,
    name: "",
    thumbnailHash: "",
    items: [],
    bodyShape: "unisex",
    gradientFrom: DEFAULT_OUTFIT_GRADIENT.from,
    gradientTo: DEFAULT_OUTFIT_GRADIENT.to,
    published: false,
  };
}

function toDraft(outfit: Outfit): OutfitDraft {
  const { id, name, thumbnailHash, items, bodyShape, gradientFrom, gradientTo, published } = outfit;
  return { id, name, thumbnailHash, items, bodyShape, gradientFrom, gradientTo, published };
}

function readStoredDraft(key: string): OutfitDraft | null {
  try {
    const raw = sessionStorage.getItem(key);
    return raw ? (JSON.parse(raw) as OutfitDraft) : null;
  } catch {
    return null;
  }
}

function StudioEditor({ outfitId }: { outfitId: string | null }) {
  const isNew = outfitId === null;
  const storageKey = `outfit-draft:${outfitId ?? "new"}`;
  const wallet = useShopWallet();
  const navigate = useNavigate();
  const [restored] = useState(() => readStoredDraft(storageKey));
  const [draft, setDraft] = useState<OutfitDraft | null>(() => restored ?? (isNew ? emptyDraft() : null));
  const [dirty, setDirty] = useState(!!restored);
  const recordQ = useShopQuery(isNew ? null : ["outfit", outfitId, "studio"], (signal) => fetchOutfit(outfitId!, signal), { staleMs: 0 });

  useEffect(() => {
    if (recordQ.data) setDraft((prev) => prev ?? toDraft(recordQ.data!));
  }, [recordQ.data]);

  function update(patch: Partial<OutfitDraft>) {
    setDraft((prev) => (prev ? { ...prev, ...patch } : prev));
    setDirty(true);
  }

  useEffect(() => {
    if (!draft || !dirty) return;
    try {
      sessionStorage.setItem(storageKey, JSON.stringify(draft));
    } catch {
    }
  }, [draft, dirty, storageKey]);
  useEffect(() => {
    if (!dirty) return;
    const handler = (e: BeforeUnloadEvent) => e.preventDefault();
    window.addEventListener("beforeunload", handler);
    return () => window.removeEventListener("beforeunload", handler);
  }, [dirty]);
  useEffect(
    () => () => {
      try {
        sessionStorage.removeItem(storageKey);
      } catch {
      }
    },
    [storageKey],
  );

  const keys = useMemo(() => [...new Set((draft?.items ?? []).map(outfitItemKey))].sort(), [draft]);
  const itemsQ = useShopQuery(keys.length ? ["outfit-items", ...keys] : null, (signal) => fetchCatalogByIds(keys, { signal }), { staleMs: 60_000 });
  const byKey = useMemo(() => new Map((itemsQ.data ?? []).map((item) => [item.id.toLowerCase(), item])), [itemsQ.data]);
  const resolving = keys.length > 0 && itemsQ.isLoading && !itemsQ.data;
  const selectedKeys = useMemo(() => new Set(keys), [keys]);

  const nextItems = (item: CatalogItem) => toggleOutfitItem(draft?.items ?? [], item, byKey);
  const canPick = (item: CatalogItem) => !!item.itemId && nextItems(item).length <= MAX_OUTFIT_ITEMS;
  const pick = (item: CatalogItem) => {
    if (draft && canPick(item)) update({ items: nextItems(item) });
  };

  const [thumbLocal, setThumbLocal] = useState<string | null>(null);
  const thumbLocalRef = useRef<string | null>(null);
  const [thumbBusy, setThumbBusy] = useState(false);
  const [thumbError, setThumbError] = useState<string | null>(null);
  const [importText, setImportText] = useState("");
  const [importError, setImportError] = useState<string | null>(null);
  const [importColors, setImportColors] = useState<{ skin?: string; hair?: string; eyes?: string }>();
  const [notice, setNotice] = useState<string | null>(null);
  useEffect(
    () => () => {
      if (thumbLocalRef.current) URL.revokeObjectURL(thumbLocalRef.current);
    },
    [],
  );

  function applyImport() {
    const parsed = parseOutfitImport(importText);
    if (!parsed) {
      setImportError(COPY.importError);
      return;
    }
    setImportError(null);
    setImportText("");
    setImportColors(parsed.colors);
    update({ items: parsed.items, ...(parsed.bodyShape ? { bodyShape: parsed.bodyShape } : {}) });
    setNotice(`${parsed.items.length} ${parsed.items.length === 1 ? "item" : "items"} imported`);
  }

  async function onThumbFile(file: File | undefined) {
    if (!file) return;
    setThumbError(null);
    if (!THUMB_TYPES.includes(file.type)) return setThumbError(OUTFIT_ERRORS.unsupported_type!);
    if (file.size > MAX_THUMBNAIL_BYTES) return setThumbError(OUTFIT_ERRORS.too_large!);
    if (thumbLocalRef.current) URL.revokeObjectURL(thumbLocalRef.current);
    const url = URL.createObjectURL(file);
    thumbLocalRef.current = url;
    setThumbLocal(url);
    setThumbBusy(true);
    try {
      update({ thumbnailHash: await uploadThumbnail(file, wallet.host?.identity ?? null) });
    } catch (e) {
      setThumbError(outfitErrorMessage(e));
      URL.revokeObjectURL(url);
      thumbLocalRef.current = null;
      setThumbLocal(null);
    } finally {
      setThumbBusy(false);
    }
  }

  const [saving, setSaving] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);
  const nameValid = !!draft && draft.name.trim().length > 0 && draft.name.length <= 60;
  const gradientValid = !!draft && isHexColor(draft.gradientFrom) && isHexColor(draft.gradientTo);
  const hasWearable =
    !!draft &&
    draft.items.some((ref) => {
      const item = byKey.get(outfitItemKey(ref));
      return !item || isWearable(item);
    });
  const canPublish = !!draft && nameValid && !!draft.thumbnailHash && gradientValid && hasWearable && draft.items.length >= MIN_OUTFIT_ITEMS && draft.items.length <= MAX_OUTFIT_ITEMS;

  async function save(published: boolean) {
    if (!draft || saving) return;
    setSaving(true);
    setSaveError(null);
    try {
      const saved = await saveOutfit({ ...draft, published }, isNew ? "create" : "update");
      try {
        sessionStorage.removeItem(storageKey);
      } catch {
      }
      setDraft(toDraft(saved));
      setDirty(false);
      refreshOutfits();
      setNotice(published && !draft.published ? "Outfit published" : "Outfit saved");
      if (isNew) {
        primeShopQuery(["outfit", saved.id, "studio"], saved);
        navigate(`${SHOP_BASE}/outfits/${saved.id}/edit`, { replace: true });
      }
    } catch (e) {
      setSaveError(outfitErrorMessage(e));
    } finally {
      setSaving(false);
    }
  }

  const resolved = useMemo(() => (draft?.items ?? []).map((ref) => byKey.get(outfitItemKey(ref))).filter((i): i is CatalogItem => !!i), [draft, byKey]);
  const urns = useMemo(() => resolved.filter(isWearable).map((i) => i.urn ?? itemUrn(i)).filter((u): u is NonNullable<typeof u> => !!u), [resolved]);
  const playing = playingEmote(resolved);
  const playingKey = playing?.itemId ? outfitItemKey({ contractAddress: playing.contractAddress, itemId: playing.itemId }) : null;
  const body = draft?.bodyShape === "female" ? BASE_FEMALE : BASE_MALE;
  const webgl = typeof window !== "undefined" && typeof (window as { WebGLRenderingContext?: unknown }).WebGLRenderingContext !== "undefined";

  if (!draft) {
    if (recordQ.error) {
      return (
        <div className="shop-page shop-studio">
          <div className="shop-studio__gate">
            <p className="shop-studio__error">{OUTFIT_ERRORS.generic}</p>
            <button type="button" className="shop-studio__btn" data-variant="white" onClick={recordQ.refetch}>
              {COPY.retry}
            </button>
          </div>
        </div>
      );
    }
    if (!recordQ.isLoading && recordQ.data === null) {
      return (
        <div className="shop-page shop-studio">
          <div className="shop-studio__gate" data-testid="outfit-studio-missing">
            <p className="shop-studio__hint">{OUTFIT_ERRORS.not_found}</p>
            <Link className="shop-studio__btn" data-variant="white" to={`${SHOP_BASE}/outfits/manage`}>
              {COPY.myOutfits}
            </Link>
          </div>
        </div>
      );
    }
    return (
      <div className="shop-page shop-studio">
        <div className="shop-studio__gate" aria-busy="true">
          <span className="shop-studio__spinner" aria-hidden />
        </div>
      </div>
    );
  }

  const thumbSrc = thumbLocal ?? thumbnailUrl(draft.thumbnailHash);
  const totalCredits = resolved.reduce((sum, i) => sum + i.priceCredits, 0);

  return (
    <div className="shop-page shop-studio" data-testid="outfit-studio-editor">
      <div className="shop-studio__head">
        <Link className="shop-studio__back" to={`${SHOP_BASE}/outfits/manage`} aria-label={COPY.myOutfits} data-testid="outfit-studio-back">
          <Icon name="arrow-left" size={18} />
        </Link>
        <h1 className="shop-studio__title">{isNew ? COPY.new : COPY.editTitle}</h1>
        <span className="shop-studio__state" data-state={draft.published ? "published" : "draft"}>
          {draft.published ? COPY.published : COPY.draft}
        </span>
      </div>

      <div className="shop-studio__grid">
        <div className="shop-studio__side">
          <div className="shop-studio__preview" data-testid="outfit-studio-preview" style={{ background: outfitRadialGradient(draft) }}>
            {webgl && urns.length > 0 ? (
              <Suspense fallback={null}>
                <WearablePreview urns={urns} body={body} spin={false} zoom={1} />
              </Suspense>
            ) : null}
            {urns.length === 0 && !resolving ? <p className="shop-studio__preview-empty">{COPY.previewEmpty}</p> : null}
            {importColors ? <span className="shop-studio__sr">Imported colors apply to this session's preview only.</span> : null}
          </div>

          <div className="shop-studio__field">
            <span className="shop-studio__label">{COPY.bodyShape}</span>
            <div className="shop-studio__shapes" role="group" aria-label={COPY.bodyShape}>
              {(["unisex", "male", "female"] as OutfitBodyShape[]).map((shape) => (
                <button key={shape} type="button" className="shop-studio__shape" data-selected={draft.bodyShape === shape || undefined} aria-pressed={draft.bodyShape === shape} onClick={() => update({ bodyShape: shape })}>
                  {COPY.shape[shape]}
                </button>
              ))}
            </div>
          </div>

          <div className="shop-studio__field">
            <span className="shop-studio__label">{COPY.gradient}</span>
            <div className="shop-studio__gradient">
              {(["gradientFrom", "gradientTo"] as const).map((field) => {
                const label = field === "gradientFrom" ? COPY.gradientTop : COPY.gradientBottom;
                return (
                  <div key={field} className="shop-studio__color">
                    <input
                      type="color"
                      className="shop-studio__swatch"
                      value={isHexColor(draft[field]) ? draft[field] : DEFAULT_OUTFIT_GRADIENT.from}
                      aria-label={label}
                      data-testid={field === "gradientFrom" ? "outfit-studio-gradient-from" : "outfit-studio-gradient-to"}
                      onChange={(e) => update({ [field]: e.target.value.toLowerCase() })}
                    />
                    <span className="shop-studio__colormeta">
                      <span>{label}</span>
                      <input
                        className="shop-studio__hex"
                        value={draft[field]}
                        maxLength={7}
                        spellCheck={false}
                        placeholder="#000000"
                        aria-label={`${label} color hex code`}
                        data-invalid={!isHexColor(draft[field]) || undefined}
                        onChange={(e) => {
                          const next = e.target.value.trim().toLowerCase();
                          update({ [field]: next.startsWith("#") || next === "" ? next : `#${next}` });
                        }}
                      />
                    </span>
                  </div>
                );
              })}
            </div>
            <p className="shop-studio__hint">{COPY.gradientHint}</p>
          </div>

          <div className="shop-studio__field">
            <span className="shop-studio__label">{COPY.thumbnail}</span>
            {thumbSrc ? (
              <span className="shop-studio__thumbpreview" data-busy={thumbBusy || undefined} style={{ background: outfitGradient(draft) }}>
                <img src={thumbSrc} alt="" />
              </span>
            ) : null}
            <label className="shop-studio__btn" data-variant="outline" data-size="sm" data-busy={thumbBusy || undefined}>
              {thumbBusy ? COPY.uploading : COPY.upload}
              <input type="file" accept={THUMB_TYPES.join(",")} hidden disabled={thumbBusy} onChange={(e) => void onThumbFile(e.target.files?.[0])} data-testid="outfit-studio-thumb-input" />
            </label>
            <p className="shop-studio__hint">{COPY.thumbnailHint}</p>
            {thumbError ? (
              <p className="shop-studio__error" data-testid="outfit-studio-thumb-error">
                {thumbError}
              </p>
            ) : null}
          </div>

          <div className="shop-studio__field">
            <span className="shop-studio__label">{COPY.cardPreview}</span>
            <div className="shop-studio__card" data-testid="outfit-studio-card-preview" style={{ background: outfitGradient(draft) }}>
              {thumbSrc ? <img src={thumbSrc} alt="" /> : null}
              <span className="shop-studio__card-info">
                <span className="shop-studio__card-name">{draft.name || COPY.untitled}</span>
                <span className="shop-studio__card-meta">
                  {draft.items.length} {draft.items.length === 1 ? "item" : "items"}
                  {resolved.length > 0 ? (
                    <span className="shop-studio__card-price">
                      <Icon name="credits" size={14} />
                      {formatCreditsFull(totalCredits)}
                    </span>
                  ) : null}
                </span>
              </span>
            </div>
          </div>
        </div>

        <div className="shop-studio__form">
          <div className="shop-studio__field">
            <label className="shop-studio__label" htmlFor="outfit-import">
              {COPY.import}
            </label>
            <div className="shop-studio__importrow">
              <input
                id="outfit-import"
                className="shop-studio__input"
                value={importText}
                spellCheck={false}
                placeholder={COPY.importPlaceholder}
                onChange={(e) => setImportText(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") applyImport();
                }}
                data-testid="outfit-studio-import"
              />
              <button type="button" className="shop-studio__btn" data-variant="outline" data-size="sm" onClick={applyImport} disabled={importText.trim() === ""} data-testid="outfit-studio-import-apply">
                {COPY.importApply}
              </button>
            </div>
            <p className="shop-studio__hint">{COPY.importHint}</p>
            {importError ? (
              <p className="shop-studio__error" data-testid="outfit-studio-import-error">
                {importError}
              </p>
            ) : null}
          </div>

          <div className="shop-studio__field">
            <label className="shop-studio__label" htmlFor="outfit-name">
              {COPY.name}
            </label>
            <input id="outfit-name" className="shop-studio__input" value={draft.name} maxLength={60} placeholder={COPY.namePlaceholder} onChange={(e) => update({ name: e.target.value })} data-testid="outfit-studio-name" />
          </div>

          <div className="shop-studio__field">
            <span className="shop-studio__label">
              {COPY.items} ({draft.items.length}/{MAX_OUTFIT_ITEMS})
            </span>
            {draft.items.length === 0 ? (
              <p className="shop-studio__hint">{COPY.noItems}</p>
            ) : (
              <ul className="shop-studio__selected">
                {draft.items.map((ref) => {
                  const key = outfitItemKey(ref);
                  const item = byKey.get(key);
                  return (
                    <li key={key} className="shop-studio__selrow" data-testid="outfit-studio-selected" data-missing={!item || undefined}>
                      {item ? <img className="shop-studio__selthumb" src={item.thumbnail} alt="" /> : <span className="shop-studio__selthumb" aria-hidden />}
                      <span className="shop-studio__selname">{item ? item.name : resolving ? " " : COPY.unavailable}</span>
                      {key === playingKey ? (
                        <span className="shop-studio__selhint" data-testid="outfit-studio-plays-in-preview">
                          {COPY.playsInPreview}
                        </span>
                      ) : null}
                      {item ? <PriceTag price={cardPrice(item, wallet.session)} compactView /> : null}
                      <button type="button" className="shop-studio__iconbtn" aria-label={COPY.remove} title={COPY.remove} onClick={() => update({ items: draft.items.filter((r) => outfitItemKey(r) !== key) })}>
                        <TrashIcon size={16} />
                      </button>
                    </li>
                  );
                })}
              </ul>
            )}
            {draft.items.length > 0 ? (
              <p className="shop-studio__seltotal" data-testid="outfit-studio-total">
                <span>{COPY.totalPrice}</span>
                <span className="shop-studio__seltotal-value">
                  <Icon name="credits" size={14} />
                  {formatCreditsFull(totalCredits)}
                </span>
              </p>
            ) : null}
            {draft.items.length >= MAX_OUTFIT_ITEMS ? <p className="shop-studio__hint">{COPY.maxItems}</p> : null}
          </div>

          <OutfitItemPicker selectedKeys={selectedKeys} onPick={pick} canPick={canPick} />
        </div>
      </div>

      <div className="shop-studio__savebar" data-testid="outfit-studio-savebar">
        {saveError ? (
          <p className="shop-studio__error" data-testid="outfit-studio-save-error">
            {saveError}
          </p>
        ) : notice ? (
          <p className="shop-studio__hint" role="status">
            {notice}
          </p>
        ) : null}
        {!nameValid ? (
          <p className="shop-studio__hint">{COPY.needName}</p>
        ) : !canPublish && !draft.published ? (
          <p className="shop-studio__hint" data-testid="outfit-studio-publish-hint">
            {!hasWearable && draft.items.length > 0 ? COPY.needsWearable : OUTFIT_ERRORS.not_publishable}
          </p>
        ) : null}
        <div className="shop-studio__saveactions">
          <button type="button" className="shop-studio__btn" data-variant="outline" data-testid="outfit-studio-save" disabled={saving || !nameValid} onClick={() => void save(draft.published)}>
            {saving ? COPY.saving : COPY.save}
          </button>
          {draft.published ? (
            <button type="button" className="shop-studio__btn" data-variant="white" data-testid="outfit-studio-unpublish" disabled={saving} onClick={() => void save(false)}>
              {COPY.unpublish}
            </button>
          ) : (
            <button type="button" className="shop-studio__btn" data-variant="white" data-testid="outfit-studio-publish" disabled={saving || !canPublish} onClick={() => void save(true)}>
              {COPY.publish}
            </button>
          )}
        </div>
      </div>
    </div>
  );
}

const PICKER_PAGE = 24;

function OutfitItemPicker({ selectedKeys, onPick, canPick }: { selectedKeys: ReadonlySet<string>; onPick: (item: CatalogItem) => void; canPick: (item: CatalogItem) => boolean }) {
  const { session } = useShopWallet();
  const [category, setCategory] = useState<"wearable" | "emote">("wearable");
  const [query, setQuery] = useState("");
  const [debounced, setDebounced] = useState("");
  const [pages, setPages] = useState(1);
  useEffect(() => {
    const timer = setTimeout(() => setDebounced(query.trim()), 300);
    return () => clearTimeout(timer);
  }, [query]);
  useEffect(() => setPages(1), [category, debounced]);

  const first = PICKER_PAGE * pages;
  const q = useShopQuery(["outfit-picker", category, debounced, first], (signal) => fetchShopItems({ search: debounced || undefined, category, onSale: true, listingType: "primary", first, sortBy: "cheapest" }, { signal }), { staleMs: 60_000, keepPrevious: true });
  const seen = new Set<string>();
  const items = (q.data?.items ?? []).filter((item) => {
    if (!item.itemId) return false;
    const key = `${item.contractAddress.toLowerCase()}-${item.itemId}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
  const total = q.data?.total ?? 0;
  const searchLabel = category === "emote" ? COPY.searchEmotes : COPY.searchWearables;

  return (
    <div className="shop-studio__picker" data-testid="outfit-picker">
      <div className="shop-studio__cats" role="group" aria-label={COPY.pickerCategory} data-testid="outfit-picker-category">
        {(["wearable", "emote"] as const).map((key) => (
          <button
            key={key}
            type="button"
            className="shop-studio__cat"
            data-selected={category === key || undefined}
            aria-pressed={category === key}
            onClick={() => {
              setCategory(key);
              setQuery("");
            }}
          >
            {key === "wearable" ? "Wearables" : "Emotes"}
          </button>
        ))}
      </div>
      <label className="shop-studio__search">
        <Icon name="search" size={16} />
        <input value={query} placeholder={searchLabel} aria-label={searchLabel} onChange={(e) => setQuery(e.target.value)} data-testid="outfit-picker-search" />
      </label>
      {q.error && !q.data ? (
        <p className="shop-studio__hint">{OUTFIT_ERRORS.generic}</p>
      ) : q.isLoading && !q.data ? (
        <div className="shop-studio__pickgrid" aria-busy="true">
          {Array.from({ length: 8 }, (_, i) => (
            <span key={i} className="shop-studio__pickph" aria-hidden />
          ))}
        </div>
      ) : items.length === 0 ? (
        <p className="shop-studio__hint">{COPY.noResults}</p>
      ) : (
        <div className="shop-studio__pickgrid">
          {items.map((item) => {
            const key = `${item.contractAddress.toLowerCase()}-${item.itemId}`;
            const selected = selectedKeys.has(key);
            return (
              <button key={key} type="button" className="shop-studio__pick" data-testid="outfit-picker-item" data-selected={selected || undefined} disabled={!selected && !canPick(item)} onClick={() => onPick(item)} title={item.name}>
                <img className="shop-studio__pickthumb" src={item.thumbnail} alt="" loading="lazy" />
                <span className="shop-studio__pickname">{item.name}</span>
                <PriceTag price={cardPrice(item, session)} compactView />
                {selected ? (
                  <span className="shop-studio__pickcheck" aria-hidden>
                    <Icon name="check" size={14} />
                  </span>
                ) : null}
              </button>
            );
          })}
        </div>
      )}
      {items.length > 0 && items.length < total ? (
        <button type="button" className="shop-studio__btn" data-variant="outline" data-size="sm" disabled={q.isLoading} onClick={() => setPages((n) => n + 1)}>
          {COPY.loadMore}
        </button>
      ) : null}
    </div>
  );
}
