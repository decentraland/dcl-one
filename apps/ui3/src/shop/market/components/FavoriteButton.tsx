// from: shop@2d3bf62:app/src/pages/ItemDetail.tsx (S.Fav / S.FavCount: the title-row heart and the mobile
//       preview heart), app/src/pages/ItemDetail.styles.ts (Fav, FavCount)

import { useEffect, useState } from "react";

import Icon from "../../discovery/components/Icon";
import { pdpText } from "../../detail/copy";
import { useShopQuery } from "../../discovery/query";
import { useShopWallet } from "../../wallet/context";
import { fetchCatalogByIds } from "../lib/catalog";
import { fetchFavoriteStats } from "../lib/favorites";
import { shopFavorites, useFavorites } from "../store/favorites";

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
      <button
        type="button"
        className="shop-fav"
        data-on={saved || undefined}
        data-count={count !== undefined ? count : undefined}
        data-failed={error || undefined}
        aria-pressed={saved}
        aria-label={saved ? pdpText("itemDetail.unfavorite", { name }) : pdpText("itemDetail.favorite", { name })}
        disabled={busy || syncing}
        onClick={() => void toggle()}
        data-testid={placement === "title" ? "fav-button" : "fav-button-preview"}
      >
        <Icon name={saved ? "heart-solid" : "heart"} size={18} />
        {count !== undefined ? (
          <span className="shop-fav__count" title={pdpText("itemDetail.savedCount", { count })} data-testid={placement === "title" ? "fav-count" : "fav-count-preview"}>
            {count.toLocaleString("en-US")}
          </span>
        ) : null}
      </button>
      {error ? (
        <span className="shop-fav__error" role="alert">
          {pdpText("itemDetail.favoriteFailed")}{" "}
          <button type="button" disabled={busy} onClick={favorites.status === "error" ? () => shopFavorites.retry() : () => void toggle()}>
            {pdpText("itemDetail.favoriteRetry")}
          </button>
        </span>
      ) : null}
    </span>
  );
}
