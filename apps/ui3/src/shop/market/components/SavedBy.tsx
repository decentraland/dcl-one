// from: marketplace@784c4af4:webapp/src/components/Modals/FavoritesModal/FavoritesModal.tsx (who saved an item, opened
//       from the item page's favorites counter; the first page on open, more as the list is read) and
//       webapp/src/modules/translation/locales/en.json (favorites_modal.title, favorites_modal.empty)
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.
// Upstream's disclaimer ("Only accounts with more than 1 VP are counted") is left out: the Shop's picks route has no
// voting table and lists every account that saved the item, the same set its counter counts.

import { useInfiniteQuery } from "@tanstack/react-query";
import { Link } from "react-router";

import Modal from "../../../components/Modal";
import { pdpText } from "../../detail/copy";
import LoadMore from "../../discovery/components/LoadMore";
import { useProfiles } from "../../discovery/hooks";
import { shortAddress } from "@shop/model/address";
import { shopPath } from "../../routes";
import { useMarketplace } from "../access";
import { fetchWhoPicked } from "../lib/favorites";
import { EnsureQueryClient, LoadFailure, Loading } from "./kit";
import styles from "./SavedBy.module.css";

export const SAVED_BY_PAGE = 50;

type Props = { itemKey: string; name: string; onClose: () => void };

export default function SavedBy(props: Props) {
  return (
    <EnsureQueryClient>
      <SavedByDialog {...props} />
    </EnsureQueryClient>
  );
}

function SavedByDialog({ itemKey, name, onClose }: Props) {
  // An account's page is the Marketplace's (market/access.tsx): without it the names are not links.
  const marketplace = useMarketplace();
  const q = useInfiniteQuery({
    queryKey: ["saved-by", itemKey],
    queryFn: ({ pageParam }) => fetchWhoPicked(itemKey, { first: SAVED_BY_PAGE, skip: pageParam }),
    initialPageParam: 0,
    // A failed read says so, with its retry: the reader chooses to ask again.
    retry: false,
    // Read afresh on each open: a save made since the last open belongs in the list.
    refetchOnMount: "always",
    getNextPageParam: (last, pages) => {
      const read = pages.reduce((n, p) => n + p.addresses.length, 0);
      return last.addresses.length > 0 && read < last.total ? read : undefined;
    },
  });
  // Offset pages over a list that grows can repeat an address at a page edge: each account is listed once.
  const addresses = [...new Set(q.data?.pages.flatMap((p) => p.addresses) ?? [])];
  const profiles = useProfiles(addresses);
  return (
    <Modal onClose={onClose} width={440} className={styles.modal}>
      <h2>{pdpText("savedBy.title")}</h2>
      <p className={styles.item}>{name}</p>
      {q.isPending ? (
        <Loading label={pdpText("savedBy.loading")} />
      ) : q.isError && addresses.length === 0 ? (
        <LoadFailure error={q.error} what={pdpText("savedBy.what")} onRetry={() => void q.refetch()} />
      ) : addresses.length === 0 ? (
        <p className={styles.empty}>{pdpText("savedBy.empty")}</p>
      ) : (
        <>
          <ul className={styles.list} data-testid="saved-by-list">
            {addresses.map((address) => {
              const named = profiles[address.toLowerCase()]?.name;
              return (
                <li key={address}>
                  {marketplace ? (
                    <Link to={shopPath("account", { address })} title={address} onClick={onClose}>
                      <strong>{named || shortAddress(address)}</strong>
                      {named ? <span>{shortAddress(address)}</span> : null}
                    </Link>
                  ) : (
                    <div className={styles.account} title={address}>
                      <strong>{named || shortAddress(address)}</strong>
                      {named ? <span>{shortAddress(address)}</span> : null}
                    </div>
                  )}
                </li>
              );
            })}
          </ul>
          <LoadMore hasNextPage={q.hasNextPage} isFetching={q.isFetchingNextPage} isError={q.isFetchNextPageError} onLoadMore={() => void q.fetchNextPage()} auto={false} />
        </>
      )}
    </Modal>
  );
}
