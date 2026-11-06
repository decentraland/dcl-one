// from: shop@2d3bf62:app/src/pages/ItemDetail.tsx (S.Fav / S.FavCount: the title-row heart and the mobile
//       preview heart), app/src/pages/ItemDetail.styles.ts (Fav, FavCount);
//       marketplace@784c4af4:webapp/src/components/FavoritesCounter/FavoritesCounter.tsx (the heart toggles; the count,
//       once above zero, opens who saved the item). The pill keeps the Shop's look, holding the two as separate buttons.

import { useCallback, useEffect, useState } from "react";

import Icon from "../../discovery/components/Icon";
import { pdpText } from "../../detail/copy";
import { useShopQuery } from "../../discovery/query";
import { useShopWallet } from "../../wallet/context";
import { fetchCatalogByIds } from "../lib/catalog";
import { fetchFavoriteStats } from "../lib/favorites";
import { shopFavorites, useFavorites } from "../store/favorites";
import SavedBy from "./SavedBy";

export default function FavoriteButton({
  contractAddress,
  itemId,
  name,
  placement = "title",
}: {
  contractAddress: string;
  itemId: string;
  name: string;
  placement?: "title" | "preview";
}) {
  const { account } = useShopWallet();
  const favorites = useFavorites();
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);
  const [delta, setDelta] = useState(0);
  const [showSavedBy, setShowSavedBy] = useState(false);
  // Stable, so the open dialog keeps its focus when this button re-renders (useDialogKeys depends on onClose).
  const closeSavedBy = useCallback(() => setShowSavedBy(false), []);
  const key = `${contractAddress.toLowerCase()}-${itemId}`;
  const saved = !!favorites.items[key];
  useEffect(() => {
    shopFavorites.reloadFor(account);
  }, [account]);
  const stats = useShopQuery(["fav-stats", key, account ?? ""], () => fetchFavoriteStats(key, account), { staleMs: 60_000 });
  const count = stats.data ? Math.max(0, stats.data.count + delta) : undefined;

  async function toggle() {
    setBusy(true);
    setFailed(false);
    const owner = shopFavorites.getState().account;
    const was = saved;
    try {
      const item = shopFavorites.getState().items[key] ?? (await fetchCatalogByIds([key]))[0];
      if (!item) throw new Error("Item unavailable");
      if (shopFavorites.getState().account === owner) {
        await shopFavorites.toggle(item);
        setDelta((d) => d + (was ? -1 : 1));
      }
    } catch {
      setFailed(true);
    } finally {
      setBusy(false);
    }
  }

  const error = failed || !!favorites.error;
  const syncing = favorites.status === "loading" || favorites.account !== (account?.toLowerCase() ?? null);
  return (
    <span className="shop-fav-wrap" data-placement={placement}>
      <span className="shop-fav" data-on={saved || undefined} data-count={count !== undefined ? count : undefined} data-failed={error || undefined} data-busy={busy || syncing || undefined}>
        <button
          type="button"
          className="shop-fav__heart"
          aria-pressed={saved}
          aria-label={saved ? pdpText("itemDetail.unfavorite", { name }) : pdpText("itemDetail.favorite", { name })}
          disabled={busy || syncing}
          onClick={() => void toggle()}
        >
          <Icon name={saved ? "heart-solid" : "heart"} size={18} />
          {count === 0 ? (
            <span className="shop-fav__count" title={pdpText("itemDetail.savedCount", { count })} data-testid={placement === "title" ? "fav-count" : "fav-count-preview"}>
              0
            </span>
          ) : null}
        </button>
        {count !== undefined && count > 0 ? (
          <button
            type="button"
            className="shop-fav__count"
            title={pdpText("itemDetail.savedCount", { count })}
            aria-label={`${pdpText("itemDetail.savedCount", { count })}. ${pdpText("savedBy.open", { name })}`}
            aria-haspopup="dialog"
            data-testid={placement === "title" ? "fav-count" : "fav-count-preview"}
            onClick={() => setShowSavedBy(true)}
          >
            {count.toLocaleString("en-US")}
          </button>
        ) : null}
      </span>
      {error ? (
        <span className="shop-fav__error" role="alert">
          {pdpText("itemDetail.favoriteFailed")}{" "}
          <button type="button" disabled={busy} onClick={favorites.status === "error" ? () => shopFavorites.retry() : () => void toggle()}>
            {pdpText("itemDetail.favoriteRetry")}
          </button>
        </span>
      ) : null}
      {showSavedBy ? <SavedBy itemKey={key} name={name} onClose={closeSavedBy} /> : null}
    </span>
  );
}
