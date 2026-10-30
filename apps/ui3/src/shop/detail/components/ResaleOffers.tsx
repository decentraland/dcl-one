// from: shop@94584a5c:app/src/components/ResellersModal/ResellersModal.tsx (the resale list, its sort and paging,
//       the seller and mint number resolved for the visible rows only, the classic-order section)

import { useMemo, useState } from "react";
import { Link } from "react-router";

import ManaMark from "../../../atoms/ManaMark";
import { shortAddress } from "../../model/address";
import { parseManaWei } from "../../model/money";
import type { UnifiedListing } from "../../discovery/api";
import { ManaAmount } from "../../discovery/components/PriceTag";
import { t } from "../../discovery/intl";
import { tokenRoute } from "../../discovery/lib/routes";
import { useShopQuery } from "../../discovery/query";
import IndexNotice from "../../discovery/components/IndexNotice";
import { useListingsFreshness } from "../../discovery/hooks";
import { fetchResaleTokenInfos, type ClassicOrder } from "../api";
import "./resaleoffers.css";

const PAGE = 8;

export default function ResaleOffers({
  contractAddress,
  resales,
  classic = [],
  network,
  currentTokenId,
  viewer,
}: {
  contractAddress: string;
  resales: UnifiedListing[];
  classic?: ClassicOrder[];
  network: "polygon" | "ethereum";
  currentTokenId?: string;
  viewer?: string | null;
}) {
  const [page, setPage] = useState(0);
  const pages = Math.max(1, Math.ceil(resales.length / PAGE));
  const visible = useMemo(() => resales.slice(page * PAGE, page * PAGE + PAGE), [resales, page]);
  const freshness = useListingsFreshness();
  const missing = visible.filter((r) => r.tokenId && (!r.seller || !r.issuedId)).map((r) => r.tokenId!);
  const { data: infos } = useShopQuery(missing.length ? ["resale-infos", contractAddress, ...missing] : null, (signal) => fetchResaleTokenInfos(contractAddress, missing, { signal }), {
    staleMs: 60_000,
  });

  return (
    <section className="shop-resales" id="shop-resales" data-testid="resale-offers" aria-labelledby="shop-resales-title">
      <header className="shop-resales__head">
        <h2 id="shop-resales-title">{t("itemDetail.resaleOffers")}</h2>
        <span>{t("itemDetail.resaleCount", { count: resales.length + classic.length })}</span>
      </header>
      <p className="shop-resales__note">{t("itemDetail.resaleOffersNote")}</p>
      <IndexNotice freshness={freshness} what={t("itemDetail.resaleOffers")} stamp />
      {resales.length === 0 && classic.length === 0 ? <p className="shop-resales__empty">{t("itemDetail.resaleEmpty")}</p> : null}
      <ul className="shop-resales__list">
        {visible.map((r) => {
          const info = r.tokenId ? infos?.[r.tokenId] : undefined;
          const seller = r.seller ?? info?.seller;
          const issued = r.issuedId ?? info?.issuedId;
          const mana = parseManaWei(r.manaWei ?? null);
          const own = !!viewer && !!seller && viewer.toLowerCase() === seller.toLowerCase();
          return (
            <li key={r.tradeId ?? r.tokenId} className="shop-resales__row" data-current={r.tokenId === currentTokenId || undefined} data-testid="resale-row">
              <span className="shop-resales__who">
                {issued ? <strong>{t("itemDetail.resaleIssued", { issued })}</strong> : null}
                {seller ? <span>{own ? t("resales.yourListing") : t("itemDetail.resaleSeller", { seller: shortAddress(seller) })}</span> : null}
              </span>
              <span className="shop-resales__price" data-testid="resale-price">
                {mana ? (
                  <ManaAmount wei={mana} network={network} />
                ) : (
                  <span className="shop-price" data-kind="mana-at-checkout" title={t("assetCard.manaPriceOnPage")}>
                    <ManaMark size={14} network={network} />
                    <span>{t("assetCard.manaOnly")}</span>
                  </span>
                )}
              </span>
              {r.tokenId ? (
                <Link className="shop-resales__view" to={tokenRoute(contractAddress, r.tokenId)} aria-label={`${t("itemDetail.viewResale")} #${issued ?? r.tokenId}`}>
                  {t("itemDetail.viewResale")}
                </Link>
              ) : null}
            </li>
          );
        })}
      </ul>
      {pages > 1 ? (
        <nav className="shop-resales__pager" aria-label={t("itemDetail.resaleOffers")}>
          <button type="button" onClick={() => setPage((p) => Math.max(0, p - 1))} disabled={page === 0}>
            {t("overview.previous")}
          </button>
          <span>
            {page + 1} / {pages}
          </span>
          <button type="button" onClick={() => setPage((p) => Math.min(pages - 1, p + 1))} disabled={page >= pages - 1}>
            {t("overview.next")}
          </button>
        </nav>
      ) : null}
      {classic.length > 0 ? (
        <div className="shop-resales__classic" data-testid="classic-orders">
          <h3>{t("itemDetail.classicOrders")}</h3>
          <ul className="shop-resales__list">
            {classic.map((o) => {
              const mana = parseManaWei(o.priceWei);
              return (
                <li key={o.tokenId} className="shop-resales__row">
                  <span className="shop-resales__who">
                    {o.issuedId ? <strong>{t("itemDetail.resaleIssued", { issued: o.issuedId })}</strong> : null}
                    <span>{t("itemDetail.resaleSeller", { seller: shortAddress(o.seller) })}</span>
                  </span>
                  <span className="shop-resales__price">{mana ? <ManaAmount wei={mana} network={network} /> : null}</span>
                  <Link className="shop-resales__view" to={tokenRoute(o.contractAddress, o.tokenId)}>
                    {t("itemDetail.viewResale")}
                  </Link>
                </li>
              );
            })}
          </ul>
        </div>
      ) : null}
    </section>
  );
}
