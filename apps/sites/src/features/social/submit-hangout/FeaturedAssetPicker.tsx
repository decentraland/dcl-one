import { useEventsCopy, Notice, SitesButton as Button } from "@dcl/one-ui";
import { useEffect, useState } from "react";
import { searchFeaturedAssets, type FeaturedAsset } from "@/data/lib/catalyst/landings/hangout-options";

import styles from "./FeaturedAssetPicker.module.css";
import form from "./SubmitHangout.module.css";

export default function FeaturedAssetPicker({ value, onChange }: { value: string; onChange: (urn: string) => void }) {
  const { text, errorText } = useEventsCopy();
  const [query, setQuery] = useState(value);
  const [label, setLabel] = useState<string | null>(null);
  const [attempt, setAttempt] = useState(0);
  const [read, setRead] = useState<{ query: string; assets?: FeaturedAsset[]; error?: string } | null>(null);
  const search = query.trim();
  useEffect(() => {
    if (search.length < 3) return;
    const controller = new AbortController();
    const timer = setTimeout(() => {
      searchFeaturedAssets(search, controller.signal).then(
        (assets) => { if (!controller.signal.aborted) setRead({ query: search, assets }); },
        (error: unknown) => { if (!controller.signal.aborted) setRead({ query: search, error: error instanceof Error ? error.message : "Could not search the catalogue." }); },
      );
    }, 300);
    return () => { clearTimeout(timer); controller.abort(); };
  }, [search, attempt]);
  const current = read?.query === search ? read : null;
  const selected = current?.assets?.find((asset) => asset.urn === value);
  return <fieldset className={`${styles.root} ${form.picker}`}>
    <legend>{text("Featured wearable, emote or collection")}</legend>
    {value && <p className="submit-hangout__hint">{text("Selected: {asset}", { asset: label ?? selected?.name ?? value })} <button type="button" className="submit-hangout__btn" onClick={() => { onChange(""); setQuery(""); setLabel(null); }}>{text("Remove featured asset")}</button></p>}
    <label className={`submit-hangout__fieldlabel ${form.fieldLabel}`}>{text("Search featured items or collections")}<input className={`submit-hangout__field ${form.field}`} value={query} onChange={(event) => setQuery(event.target.value)} placeholder={text("Search the catalogue or paste an asset URN")} /></label>
    {search.length > 0 && search.length < 3 && <p className="submit-hangout__hint">{text("Enter at least three characters.")}</p>}
    {search.length >= 3 && !current && <p role="status">{text("Searching the catalogue…")}</p>}
    {current?.error && <Notice tone="error">{errorText(current.error)} <Button variant="tertiary" size="sm" onClick={() => setAttempt((previous) => previous + 1)}>{text("Retry asset search")}</Button></Notice>}
    {current?.assets?.length === 0 && <p className="submit-hangout__hint">{text("No matching assets.")}</p>}
    {current?.assets && current.assets.length > 0 && <ul className={styles.results} aria-label={text("Featured asset results")}>{current.assets.map((asset) => <li key={asset.urn}><button type="button" className={styles.asset} onClick={() => { onChange(asset.urn); setLabel(asset.name); setQuery(""); }}>
      {asset.thumbnail && <img src={asset.thumbnail} className={styles.thumbnail} alt="" width={48} height={48} />}<span className={styles.label}>{asset.name}<small className={styles.kind}>{text(asset.kind === "collection" ? "Collection" : "Item")}</small></span>
    </button></li>)}</ul>}
  </fieldset>;
}
