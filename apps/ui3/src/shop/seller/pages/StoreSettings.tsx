// from: shop@2d3bf62:app/src/pages/StoreSettings.tsx, app/src/pages/StoreSettings.styles.ts, app/src/styles/field.styles.ts,
//       app/src/lib/creator-covers.ts (the template covers, vendored in ./assets/creator-covers), app/src/intl/en.json
//       (storeSettings.*); marketplace@3bf3662f:webapp/src/components/StoreSettings,
//       webapp/src/modules/store/{actions,sagas}.ts (FETCH_STORE, UPDATE_STORE); catalyrst-v2 ui3/src/marketplace/pages/MkStoreSettingsEditor.tsx
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.
import { useQueryClient } from "@tanstack/react-query";
import { useEffect, useId, useMemo, useRef, useState, type ChangeEvent, type ReactNode } from "react";
import { Link, useLocation, useNavigate } from "react-router";

import Icon from "../../discovery/components/Icon";
import { shopPath } from "../../routes";
import { useShopWallet } from "../../wallet/context";
import { EnsureQueryClient, LoadFailure, Loading, useWrite, WriteStatus } from "../../market/components/kit";
import { useStore } from "../hooks";
import { ShopSignInPanel } from "./SellerSignIn";
import uploadIcon from "./assets/upload.svg?url";
import "./sellerpages.css";
import "./storesettings.css";
import { draftFromStore, emptyStore, isValidLink, LINK_PREFIX, LINK_TYPES, saveStore, templateHash, type LinkType, type StoreDraft } from "../lib/store";

const MAX_COVER_BYTES = 1_000_000;
const ALLOWED_IMAGE_TYPES = ["image/png", "image/jpeg", "image/webp"];

const TEMPLATE_MODULES: Record<string, string> = import.meta.glob("./assets/creator-covers/template-cover-*.{jpeg,jpg,png,webp}", { eager: true, query: "?url", import: "default" });
export const COVER_TEMPLATES = Object.entries(TEMPLATE_MODULES)
  .sort(([a], [b]) => a.localeCompare(b))
  .map(([path, url]) => ({ name: path.split("/").pop()!, url }));

const LABEL: Record<LinkType, string> = { website: "Website", twitter: "Twitter handle", discord: "Discord invite ID", facebook: "Facebook handle" };

function mb(bytes: number): string {
  return `${(bytes / 1_000_000).toLocaleString("en-US", { maximumFractionDigits: 2 })}MB`;
}

export function StoreSettingsForm({ address, initial, templates = COVER_TEMPLATES }: { address: string; initial: StoreDraft; templates?: { name: string; url: string }[] }) {
  const id = useId();
  const qc = useQueryClient();
  const navigate = useNavigate();
  const { host } = useShopWallet();
  const [draft, setDraft] = useState<StoreDraft>(initial);
  const [base, setBase] = useState<StoreDraft>(initial);
  const [coverSize, setCoverSize] = useState<number>();
  const [customCover, setCustomCover] = useState<{ url: string; name: string; hash: string } | null>(initial.cover ? { url: initial.cover, name: initial.coverName, hash: initial.coverHash } : null);
  const [templateHashes, setTemplateHashes] = useState<Record<string, string>>({});
  const fileInput = useRef<HTMLInputElement>(null);
  const write = useWrite();

  useEffect(() => {
    let cancelled = false;
    Promise.all(templates.map(async (tpl) => [tpl.name, await templateHash(tpl.url)] as const)).then(
      (pairs) => !cancelled && setTemplateHashes(Object.fromEntries(pairs)),
      () => {},
    );
    return () => {
      cancelled = true;
    };
  }, [templates]);

  const selectedTemplate = useMemo(() => {
    const byName = templates.find((tpl) => draft.coverName === `cover/${tpl.name}`);
    if (byName) return byName.name;
    return draft.coverHash ? templates.find((tpl) => templateHashes[tpl.name] === draft.coverHash)?.name : undefined;
  }, [draft.coverName, draft.coverHash, templateHashes, templates]);

  const oversize = coverSize !== undefined && coverSize > MAX_COVER_BYTES;
  const linkErrors = LINK_TYPES.filter((type) => !isValidLink(type, draft.links[type]));
  const dirty = JSON.stringify(draft) !== JSON.stringify(base);
  const canSave = dirty && !oversize && linkErrors.length === 0 && !write.busy;

  useEffect(() => {
    if (!dirty) return;
    const handler = (e: BeforeUnloadEvent) => e.preventDefault();
    window.addEventListener("beforeunload", handler);
    return () => window.removeEventListener("beforeunload", handler);
  }, [dirty]);

  function onUpload(e: ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    if (!file || !ALLOWED_IMAGE_TYPES.includes(file.type)) return;
    setCoverSize(file.size);
    const name = `cover/${file.name}`;
    const reader = new FileReader();
    reader.onload = () => {
      const url = reader.result as string;
      setCustomCover({ url, name, hash: "" });
      setDraft((d) => ({ ...d, cover: url, coverName: name, coverHash: "" }));
    };
    reader.readAsDataURL(file);
  }

  const setLink = (type: LinkType, handle: string) =>
    setDraft((d) => ({ ...d, links: { ...d.links, [type]: handle ? (LINK_PREFIX[type] + handle).replace(/\s/g, "") : "" } }));

  return (
    <form
      className="seller-settings__form"
      aria-label="Store settings"
      onSubmit={(e) => {
        e.preventDefault();
        if (!canSave || !host?.identity) return;
        void write.run("Saving your store…", "Store saved.", async () => {
          await saveStore(address, draft, host.identity!);
          setBase(draft);
          setCoverSize(undefined);
          await qc.invalidateQueries({ queryKey: ["store", address.toLowerCase()] });
          navigate(shopPath("creator", { address }));
          return null;
        });
      }}
    >
      <div className="seller-settings__field">
        <span className="seller-settings__label" id={`${id}-cover`}>
          Store cover
        </span>
        <div className="seller-settings__picker" role="group" aria-labelledby={`${id}-cover`}>
          {templates.map((tpl) => {
            const selected = selectedTemplate === tpl.name;
            return (
              <button
                key={tpl.name}
                type="button"
                className="seller-settings__tile"
                data-testid="cover-picker-tile"
                data-selected={selected}
                aria-pressed={selected}
                onClick={() => {
                  setCoverSize(undefined);
                  setDraft((d) => ({ ...d, cover: tpl.url, coverName: `cover/${tpl.name}`, coverHash: "" }));
                }}
              >
                <img src={tpl.url} alt="" loading="lazy" />
              </button>
            );
          })}
          {customCover ? (
            <button
              type="button"
              className="seller-settings__tile"
              data-testid="cover-picker-tile"
              data-variant="custom"
              data-selected={!selectedTemplate && draft.cover === customCover.url}
              aria-pressed={!selectedTemplate && draft.cover === customCover.url}
              onClick={() => {
                setCoverSize(undefined);
                setDraft((d) => ({ ...d, cover: customCover.url, coverName: customCover.name, coverHash: customCover.hash }));
              }}
            >
              <img src={customCover.url} alt="Your cover" />
            </button>
          ) : null}
          <button type="button" className="seller-settings__tile" data-testid="cover-picker-tile" data-variant="upload" onClick={() => fileInput.current?.click()}>
            <span className="seller-settings__upload-ico" aria-hidden="true" style={{ WebkitMaskImage: `url("${uploadIcon}")`, maskImage: `url("${uploadIcon}")` }} />
            <span>Upload your own</span>
          </button>
          <input ref={fileInput} className="seller-settings__file" type="file" accept="image/png, image/jpeg, image/webp" tabIndex={-1} aria-hidden="true" data-testid="cover-picker-input" onChange={onUpload} />
        </div>
        {oversize ? (
          <FieldError>
            Maximum cover size is {mb(MAX_COVER_BYTES)}; the image you chose is {mb(coverSize!)}.
          </FieldError>
        ) : null}
      </div>
      <label className="seller-settings__field" htmlFor={`${id}-description`}>
        <span className="seller-settings__label">Description</span>
        <textarea id={`${id}-description`} rows={4} value={draft.description} disabled={write.busy} onChange={(e) => setDraft((d) => ({ ...d, description: e.target.value }))} />
      </label>
      <div className="seller-settings__field">
        <label className="seller-settings__label" htmlFor={`${id}-website`}>
          {LABEL.website}
        </label>
        <input id={`${id}-website`} type="url" value={draft.links.website} placeholder={LINK_PREFIX.website} disabled={write.busy} onChange={(e) => setDraft((d) => ({ ...d, links: { ...d.links, website: e.target.value.replace(/\s/g, "") } }))} />
        {!isValidLink("website", draft.links.website) ? <FieldError>Link should start with {LINK_PREFIX.website}</FieldError> : null}
      </div>
      {(["twitter", "discord", "facebook"] as const).map((type) => (
        <div className="seller-settings__field" key={type}>
          <label className="seller-settings__label" htmlFor={`${id}-${type}`}>
            {LABEL[type]}
          </label>
          <span className="seller-settings__prefixed">
            <span className="seller-settings__prefix" aria-hidden="true">
              {LINK_PREFIX[type]}
            </span>
            <input id={`${id}-${type}`} type="text" value={draft.links[type].replace(LINK_PREFIX[type], "")} disabled={write.busy} onChange={(e) => setLink(type, e.target.value)} />
          </span>
        </div>
      ))}
      <div className="seller-settings__actions">
        <button type="submit" className="seller-settings__save" data-testid="store-settings-save" disabled={!canSave}>
          {write.busy ? "Saving…" : "Save"}
        </button>
      </div>
      <WriteStatus state={write.state} />
    </form>
  );
}

function FieldError({ children }: { children: ReactNode }) {
  return (
    <p className="seller-settings__error" role="alert">
      {children}
    </p>
  );
}

export function StoreSettingsView({ address }: { address: string }) {
  const store = useStore(address);
  const location = useLocation();
  const cameFrom = (location.state as { from?: unknown } | null)?.from;
  const backTo = typeof cameFrom === "string" && cameFrom.startsWith("/") && !cameFrom.startsWith("//") ? cameFrom : shopPath("creator", { address });
  return (
    <section className="seller-settings" aria-label="Store settings">
      <div className="seller-settings__card">
        <div className="seller-settings__inner">
          <div className="seller-settings__head">
            <div className="seller-settings__heading">
              <Link className="seller-settings__back" to={backTo} title="Back to your store" aria-label="Back to your store" data-testid="store-settings-back">
                <Icon name="arrow-left" size={22} />
              </Link>
              <h1 className="seller-settings__title">Store settings</h1>
            </div>
            <a className="seller-settings__guest" href={shopPath("creator", { address })} target="_blank" rel="noopener noreferrer">
              See store as guest
              <Icon name="external-link" size={15} />
            </a>
          </div>
          {store.isPending ? (
            <div className="seller-settings__loading">
              <Loading label="Loading your store…" />
            </div>
          ) : store.isError ? (
            <LoadFailure error={store.error} what="Your store" onRetry={() => void store.refetch()} />
          ) : (
            <StoreSettingsForm key={JSON.stringify(store.data)} address={address} initial={draftFromStore(store.data ?? emptyStore())} />
          )}
        </div>
      </div>
    </section>
  );
}

export default function StoreSettings() {
  const { account } = useShopWallet();
  if (!account) return <ShopSignInPanel page="Store settings" title="Sign in to set up your store" body="Add a cover, a description and your links, so shoppers know whose store they are in." />;
  return (
    <EnsureQueryClient>
      <div className="seller-page">
        <StoreSettingsView address={account} />
      </div>
    </EnsureQueryClient>
  );
}
