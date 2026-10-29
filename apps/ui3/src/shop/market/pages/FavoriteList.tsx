// from: marketplace@3bf3662f:webapp/src/components/ListPage/ListPage.tsx, ListPage.module.css (header: name,
//       private tag, share and edit/delete; sub-header: description, owner, last updated; the not-found,
//       could-not-load and empty panels, their SVGs vendored from webapp/src/images) and
//       webapp/src/modules/translation/locales/en.json (list_page.*)
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useState, type ReactNode } from "react";
import { Link, useNavigate, useParams } from "react-router";

import Button from "../../../atoms/Button";
import Icon from "../../discovery/components/Icon";
import SkeletonCards from "../../discovery/components/SkeletonCards";
import { useCreatorName, useSeo } from "../../discovery/hooks";
import { shopPath } from "../../routes";
import { MarketSignIn } from "../../seller/pages/SellerSignIn";
import { useShopWallet } from "../../wallet/context";
import { fetchCatalogByIds } from "../lib/catalog";
import { bulkPick, canEditList, fetchListPicks, getList, ListNotAvailableError, listPath, type FavoriteList as List } from "../lib/favorites";
import ItemCard from "../components/ItemCard";
import { DeleteList, ListEditor } from "../components/ListEditor";
import { EnsureQueryClient, shortAddr } from "../components/kit";
import couldNotLoad from "./could-not-load-list.svg?url";
import emptyList from "./empty-list.svg?url";
import notFound from "./list-not-found.svg?url";
import "../../discovery/components/browselayout.css";
import "./shopfavorites.css";

function Panel({ icon, title, subtitle, children, testId }: { icon: string; title: string; subtitle?: string; children?: ReactNode; testId?: string }) {
  return (
    <div className="shop-favlist__panel" data-testid={testId}>
      <img src={icon} alt="" aria-hidden />
      <h1>{title}</h1>
      {subtitle ? <p>{subtitle}</p> : null}
      {children}
    </div>
  );
}

function Loading() {
  return (
    <div className="shop-fav__grid" aria-busy data-testid="favorites-loading">
      <span className="u-sr-only" role="status">
        Loading the list…
      </span>
      <SkeletonCards count={8} />
    </div>
  );
}

export function timeAgo(ms: number, now = Date.now()): string {
  const s = Math.max(0, Math.round((now - ms) / 1000));
  const steps: [number, string][] = [
    [60, "second"],
    [60, "minute"],
    [24, "hour"],
    [30, "day"],
    [12, "month"],
    [Infinity, "year"],
  ];
  let n = s;
  for (const [size, unit] of steps) {
    if (n < size) return n <= 1 && unit === "second" ? "less than a minute ago" : `${n} ${unit}${n === 1 ? "" : "s"} ago`;
    n = Math.floor(n / size);
  }
  return "";
}

function ListItems({ list, viewer, editable, owner }: { list: List; viewer: string | null; editable: boolean; owner: boolean }) {
  const qc = useQueryClient();
  const items = useQuery({
    queryKey: ["favorite-list-items", list.id, viewer],
    queryFn: async () => {
      const picks = await fetchListPicks(list, viewer, { first: 100 });
      return fetchCatalogByIds(picks.results.map((p) => p.itemId));
    },
  });
  if (items.isPending) return <Loading />;
  if (items.isError)
    return (
      <Panel icon={couldNotLoad} title="Oops! This list couldn't load." subtitle="Please try again." testId="shop-list-error">
        <Button onClick={() => void items.refetch()}>Try again</Button>
      </Panel>
    );
  if (items.data.length === 0)
    return owner || editable ? (
      <Panel icon={emptyList} title="You don't have any saved items in this list" subtitle="Discover amazing items." testId="shop-empty">
        <Link className="shop-favlist__primary" to={shopPath("items")}>
          Explore collectibles
        </Link>
      </Panel>
    ) : (
      <Panel icon={emptyList} title="The list is empty" subtitle="Try with another list." testId="shop-empty" />
    );
  return (
    <div className="shop-fav__grid">
      {items.data.map((item) => (
        <ItemCard
          key={item.key}
          item={item}
          actions={
            editable ? (
              <button type="button" onClick={() => void bulkPick(item.key, [], [list.id]).then(() => qc.invalidateQueries({ queryKey: ["favorite-list-items", list.id] }))}>
                Remove from list
              </button>
            ) : null
          }
        />
      ))}
    </div>
  );
}

function Owner({ address }: { address: string }) {
  const name = useCreatorName(address);
  return (
    <Link className="shop-favlist__owner" to={shopPath("account", { address })}>
      {name || shortAddr(address)}
    </Link>
  );
}

export function FavoriteListView({ listId }: { listId: string }) {
  const { account } = useShopWallet();
  const navigate = useNavigate();
  const [editing, setEditing] = useState(false);
  const [copied, setCopied] = useState(false);
  const list = useQuery({ queryKey: ["favorite-list", listId, account], queryFn: () => getList(listId, account), retry: false });
  useSeo({ title: list.data?.name ?? "List", noindex: true });

  if (list.isPending) return <Loading />;
  if (list.isError) {
    if (list.error instanceof ListNotAvailableError && !account)
      return (
        <div className="shop-favlist__signin" data-testid="shop-list-signin">
          <MarketSignIn />
        </div>
      );
    if (list.error instanceof ListNotAvailableError)
      return (
        <Panel
          icon={notFound}
          title="This list is private or doesn't exist."
          subtitle="Try with another list."
          testId="shop-list-unavailable"
        />
      );
    return (
      <Panel icon={couldNotLoad} title="Oops! This list couldn't load." subtitle="Please try again." testId="shop-list-error">
        <Button onClick={() => void list.refetch()}>Try again</Button>
      </Panel>
    );
  }
  const data = list.data;
  const editable = canEditList(data, account);
  const owner = !!account && data.userAddress.toLowerCase() === account.toLowerCase();
  const shareUrl = typeof window !== "undefined" ? `${window.location.origin}${listPath(data.id)}` : listPath(data.id);
  return (
    <>
      <div className="shop-favlist__head">
        {owner ? (
          <Link className="shop-favlist__back" to={`${shopPath("myFavorites")}?tab=lists`} aria-label="Back to my lists">
            <Icon name="arrow-left" size={20} />
          </Link>
        ) : null}
        <div className="shop-favlist__name">
          <h1>{data.name}</h1>
          {data.isPrivate ? <span className="shop-fav__badge">Private</span> : null}
        </div>
        <div className="shop-favlist__actions">
          {!data.isPrivate ? (
            <Button
              size="sm"
              variant="secondary"
              onClick={() => {
                void navigator.clipboard?.writeText(shareUrl).then(
                  () => setCopied(true),
                  () => setCopied(false),
                );
              }}
            >
              {copied ? "Link copied" : "Copy link"}
            </Button>
          ) : null}
          {editable ? (
            <Button size="sm" variant="secondary" onClick={() => setEditing(true)}>
              Edit
            </Button>
          ) : null}
          {editable && owner ? <DeleteList list={data} onDeleted={() => navigate(`${shopPath("myFavorites")}?tab=lists`)} /> : null}
        </div>
      </div>
      <div className="shop-favlist__subhead">
        <div className="shop-favlist__desc">
          {data.description ? <span>{data.description}</span> : null}
          {!owner && data.userAddress ? <Owner address={data.userAddress} /> : null}
        </div>
        {data.updatedAt ? (
          <div data-testid="shop-list-updated">
            <b>Last updated:</b> {timeAgo(data.updatedAt)}
          </div>
        ) : null}
      </div>
      {editing ? <ListEditor list={data} onCancel={() => setEditing(false)} onSaved={() => setEditing(false)} /> : null}
      <ListItems list={data} viewer={account} editable={editable} owner={owner} />
    </>
  );
}

export default function FavoriteList() {
  const { listId } = useParams();
  return (
    <EnsureQueryClient>
      <section className="shop-page shop-fav shop-favlist" data-testid="favorite-list-page">
        {listId ? <FavoriteListView listId={listId} /> : <Panel icon={notFound} title="This list is private or doesn't exist." subtitle="Try with another list." testId="shop-list-unavailable" />}
      </section>
    </EnsureQueryClient>
  );
}
