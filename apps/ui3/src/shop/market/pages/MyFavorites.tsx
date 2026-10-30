// from: shop@2d3bf62:app/src/pages/MyFavorites.tsx, app/src/pages/MyFavorites.styles.ts,
//       app/src/components/EmptyState/EmptyState.tsx (favorites-empty.svg vendored from app/src/assets/empty);
//       marketplace@3bf3662f:webapp/src/components/ListsPage (the signed-in "My lists" tab, which upstream Shop lacks)
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.

import { useQuery } from "@tanstack/react-query";
import { useEffect, useMemo, useState, type ReactNode } from "react";
import { Link, useNavigate, useSearchParams } from "react-router";

import Button from "../../../atoms/Button";
import LoadMore from "../../discovery/components/LoadMore";
import SkeletonCards from "../../discovery/components/SkeletonCards";
import SuggestedForYouRow from "../../discovery/components/SuggestedForYouRow";
import { useSeo } from "../../discovery/hooks";
import { shopPath } from "../../routes";
import { useShopWallet } from "../../wallet/context";
import { fetchLists, type FavoriteList } from "../lib/favorites";
import { shopFavorites, useFavorites } from "../store/favorites";
import ItemCard from "../components/ItemCard";
import { ListEditor } from "../components/ListEditor";
import SaveToLists from "../components/SaveToLists";
import { EnsureQueryClient, LoadFailure } from "../components/kit";
import emptyIllustration from "./favorites-empty.svg?url";
import "../../discovery/components/browselayout.css";
import "./shopfavorites.css";

const PAGE_SIZE = 24;

const SUGGESTED_EXCLUDE_CAP = 20;

const TITLE = "My Favorites";
const SUGGESTED_TITLE = "More like what you saved";

type EmptyCta = { label: string; to: string } | { label: string; onClick: () => void };

export function FavoritesEmpty({ icon, title, body, cta, testId }: { icon: string; title: string; body?: string; cta?: EmptyCta; testId?: string }) {
  return (
    <div className="shop-fav-empty" data-testid={testId}>
      <img className="shop-fav-empty__img" src={icon} alt="" aria-hidden />
      <div className="shop-fav-empty__text">
        <p className="shop-fav-empty__title">{title}</p>
        {body ? <p className="shop-fav-empty__body">{body}</p> : null}
      </div>
      {cta ? (
        "to" in cta ? (
          <Link className="shop-fav-empty__cta" to={cta.to}>
            {cta.label}
          </Link>
        ) : (
          <button type="button" className="shop-fav-empty__cta" onClick={cta.onClick}>
            {cta.label}
          </button>
        )
      ) : null}
    </div>
  );
}

function CountHead({ count }: { count: number | null }) {
  return (
    <div className="shop-fav__head">
      <h1 className="shop-fav__title">{TITLE}</h1>
      {count !== null ? <span className="shop-fav__count">{count === 1 ? "1 item" : `${count} items`}</span> : null}
    </div>
  );
}

export function SavedItems({ head }: { head?: ReactNode }) {
  const { items, status, error, account } = useFavorites();
  const [visible, setVisible] = useState(PAGE_SIZE);
  const [picking, setPicking] = useState<{ key: string; name: string } | null>(null);
  const list = useMemo(() => Object.values(items), [items]);
  const exclude = useMemo(() => list.slice(0, SUGGESTED_EXCLUDE_CAP).map((item) => item.key), [list]);

  if (status === "error") return <LoadFailure error={error} what="Your favorites" onRetry={() => shopFavorites.retry()} />;
  const loading = status === "loading";
  if (!loading && list.length === 0)
    return (
      <>
        {head}
        <div className="shop-fav__emptywrap">
          <FavoritesEmpty
            testId="shop-empty"
            icon={emptyIllustration}
            title="Nothing here... yet!"
            body="Start exploring and save the items you love to keep them close for later."
            cta={{ label: "Start exploring", to: shopPath("items") }}
          />
          <div className="shop-fav__suggested">
            <SuggestedForYouRow title={SUGGESTED_TITLE} />
          </div>
        </div>
      </>
    );
  return (
    <>
      {head ?? <CountHead count={loading ? null : list.length} />}
      {error ? (
        <p className="shop-fav__note" role="alert">
          Couldn't update favorites. Please try again.
        </p>
      ) : null}
      {picking ? <SaveToLists itemKey={picking.key} itemName={picking.name} onClose={() => setPicking(null)} /> : null}
      <div className="shop-fav__grid" data-testid={loading ? "favorites-loading" : undefined}>
        {loading ? (
          <SkeletonCards count={8} />
        ) : (
          list.slice(0, visible).map((item) => (
            <ItemCard
              key={item.key}
              item={item}
              saved
              onToggleSave={() => shopFavorites.toggle(item)}
              actions={
                account ? (
                  <button type="button" onClick={() => setPicking({ key: item.key, name: item.name })}>
                    Add to a list
                  </button>
                ) : null
              }
            />
          ))
        )}
      </div>
      <LoadMore hasNextPage={!loading && visible < list.length} isFetching={false} onLoadMore={() => setVisible((v) => v + PAGE_SIZE)} />
      {!loading ? (
        <div className="shop-fav__suggested">
          <SuggestedForYouRow title={SUGGESTED_TITLE} exclude={exclude} />
        </div>
      ) : null}
    </>
  );
}

export function ListsOverview({ account }: { account: string }) {
  const navigate = useNavigate();
  const [creating, setCreating] = useState(false);
  const lists = useQuery({ queryKey: ["favorite-lists", account], queryFn: () => fetchLists({ first: 100, sortBy: "updatedAt", sortDirection: "desc" }) });
  const own = (lists.data?.results ?? []).filter((l) => !l.isDefaultList);
  return (
    <div className="shop-fav__lists" aria-label="Lists">
      <div>
        <Button size="sm" onClick={() => setCreating(true)} disabled={creating}>
          Create a list
        </Button>
      </div>
      {creating ? <ListEditor onCancel={() => setCreating(false)} onSaved={(list) => navigate(shopPath("favoriteList", { listId: list.id }))} /> : null}
      {lists.isPending ? (
        <div className="shop-fav__grid" aria-busy>
          <SkeletonCards count={4} />
        </div>
      ) : lists.isError ? (
        <LoadFailure error={lists.error} what="Your lists" onRetry={() => void lists.refetch()} />
      ) : own.length === 0 ? (
        <FavoritesEmpty testId="shop-empty" icon={emptyIllustration} title="No lists yet" body="Group items into lists; keep them private or share them with a link." />
      ) : (
        own.map((list: FavoriteList) => (
          <div key={list.id} className="shop-fav__listrow" data-testid="shop-list-row">
            <div className="shop-fav__listmain">
              <Link to={shopPath("favoriteList", { listId: list.id })}>{list.name}</Link>
              <span className="shop-fav__sub">{list.description || "No description"}</span>
            </div>
            <span className="shop-fav__badge">{list.isPrivate ? "Private" : "Shared"}</span>
            <span className="shop-fav__sub">
              {list.itemsCount} {list.itemsCount === 1 ? "item" : "items"}
            </span>
          </div>
        ))
      )}
    </div>
  );
}

export default function MyFavorites() {
  const { account } = useShopWallet();
  const [params, setParams] = useSearchParams();
  const tab = params.get("tab") === "lists" && account ? "lists" : "saved";
  useSeo({ title: TITLE, noindex: true });
  useEffect(() => {
    shopFavorites.reloadFor(account);
  }, [account]);
  const head = account ? (
    <div className="shop-fav__head">
      <h1 className="shop-fav__title">{TITLE}</h1>
      <div className="shop-fav__tabs" role="tablist" aria-label="Favorites">
        <button type="button" role="tab" className="shop-fav__tab" aria-selected={tab === "saved"} onClick={() => setParams({}, { replace: true })}>
          Saved items
        </button>
        <button type="button" role="tab" className="shop-fav__tab" aria-selected={tab === "lists"} onClick={() => setParams({ tab: "lists" }, { replace: true })}>
          My lists
        </button>
      </div>
    </div>
  ) : undefined;
  return (
    <EnsureQueryClient>
      <section className="shop-page shop-fav" data-testid="favorites-page">
        {tab === "lists" && account ? (
          <>
            {head}
            <ListsOverview account={account} />
          </>
        ) : (
          <SavedItems head={head} />
        )}
      </section>
    </EnsureQueryClient>
  );
}
