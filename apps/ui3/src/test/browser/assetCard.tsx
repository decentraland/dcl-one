import { createRoot } from "react-dom/client";
import { toCatalogItem } from "@shop/discovery/lib/catalogItem";
import AssetCard from "../../shop/discovery/components/AssetCard";
import { catalogRow, PRIMARY_WEARABLE, ShopHarness } from "../../shop/discovery/fixtures";
import "../../shop/discovery/pages/items.css";

const item = { ...toCatalogItem(catalogRow(PRIMARY_WEARABLE)), priceCredits: 29, manaWei: "50000000000000000000" };
document.body.style.margin = "0";

createRoot(document.getElementById("root")!).render(
  <ShopHarness entry="/shop/items" session="web3">
    <main className="shop-page shop-items">
      <div className="shop-grid" style={{ display: "grid" }}>
        <AssetCard item={item} source="grid" creatorName="Test creator" />
      </div>
    </main>
  </ShopHarness>
);
