// from: shop@2d3bf62:app/src/components/SaleTag/SaleTag.tsx, app/src/components/SaleTag/SaleTag.styles.ts
import { t } from "../intl";
import "./saletag.css";

export default function SaleTag({ pct, size = "sm", testId }: { pct: number; size?: "sm" | "lg"; testId?: string }) {
  return (
    <span className="shop-saletag" data-size={size} data-testid={testId}>
      <span aria-hidden>🔥</span>
      <b className="shop-saletag__pct">{pct > 0 ? t("assetCard.saleTagPct", { pct }) : t("assetCard.sale")}</b>
    </span>
  );
}
