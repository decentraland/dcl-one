// from: shop@94584a5c:app/src/components/FittingRoom/FittingRoom.tsx
// Try the item on together with the wearables already in the cart: one per slot (equipping one swaps
// out what held its slot), emotes never worn, the body chosen so nothing renders invisible. The 3D
// preview loads lazily; the list works without it.

import { lazy, Suspense, useMemo, useState } from "react";

import Modal from "../../../components/Modal";
import { t } from "../../discovery/intl";
import type { CatalogItem } from "../../discovery/lib/catalogItem";
import { conflictingIds, defaultWorn, isWearable, toggleWorn, wornUrns } from "../../discovery/lib/outfit";
import { BASE_MALE, dominantShape, isCompatible, itemShapes, requiredShape, shapeLabel } from "../lib/bodyShape";
import "./fittingroom.css";

const WearablePreview = lazy(() => import("../../../wearable-preview/WearablePreview"));

export default function FittingRoom({ items, onClose, portal = true }: { items: CatalogItem[]; onClose: () => void; portal?: boolean }) {
  const target = useMemo(() => requiredShape(items) ?? dominantShape(items), [items]);
  const [worn, setWorn] = useState(() => defaultWorn(items, target));
  const conflicts = useMemo(() => conflictingIds(items), [items]);
  const urns = wornUrns(items, worn, target);
  const body = target ?? BASE_MALE;

  return (
    <Modal onClose={onClose} width={880} portal={portal} ariaLabel={t("fittingRoom.title")}>
      <div className="shop-fitting" data-testid="fitting-room">
        <div className="shop-fitting__stage">
          {urns.length === 0 ? (
            <div className="shop-fitting__empty" role="status">
              <strong>{t("fittingRoom.emptyStageTitle")}</strong>
              <span>{t("fittingRoom.emptyStageBody")}</span>
            </div>
          ) : (
            <Suspense fallback={<div className="shop-fitting__empty">{t("itemDetail.previewLoading")}</div>}>
              <WearablePreview urns={urns} body={body} spin={false} zoom={1} background="#161318" />
            </Suspense>
          )}
        </div>
        <div className="shop-fitting__list">
          <h2>{t("fittingRoom.title")}</h2>
          <p>{t("fittingRoom.sub")}</p>
          <ul>
            {items.map((item) => {
              const compatible = isCompatible(item, target);
              const shape = itemShapes(item);
              return (
                <li key={item.id}>
                  <label data-conflict={conflicts.has(item.id) || undefined}>
                    <input
                      type="checkbox"
                      checked={worn.has(item.id)}
                      disabled={!isWearable(item) || !compatible}
                      onChange={() => setWorn((w) => toggleWorn(w, item, items))}
                      aria-label={t("fittingRoom.wear", { name: item.name })}
                    />
                    {item.thumbnail ? <img src={item.thumbnail} alt="" width={40} height={40} /> : null}
                    <span className="shop-fitting__name">{item.name}</span>
                    {conflicts.has(item.id) ? (
                      <span className="shop-fitting__tag" title={t("fittingRoom.conflictTooltip")}>
                        {t("fittingRoom.onePerSlot")}
                      </span>
                    ) : null}
                    {!compatible && shape.length === 1 ? <span className="shop-fitting__tag">{t("fittingRoom.shapeOnly", { shape: shapeLabel(shape[0]!) })}</span> : null}
                  </label>
                </li>
              );
            })}
          </ul>
        </div>
      </div>
    </Modal>
  );
}
