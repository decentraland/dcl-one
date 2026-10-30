// from: shop@2d3bf62:app/src/pages/NotFound.tsx, app/src/pages/NotFound.styles.ts, app/src/styles/index.css (.page),
//       app/src/components/Button/Button.styles.ts (variant="white");
//       marketplace@f3956826:webapp/src/components/Routes/Routes.tsx (the `*` redirect)
// Licence of the Marketplace parts: Apache-2.0, Copyright 2020 Metaverse Holdings Ltd.

import { Link } from "react-router";

import { SHOP_BASE } from "../../routes";
import Icon from "../components/Icon";
import { useSeo } from "../hooks";
import { t } from "../intl";
import "./notfound.css";

const SEO_TITLE = "Page Not Found";
const BODY = "The page you’re looking for isn’t here. Let’s get you back to the shop.";

export default function NotFound() {
  useSeo({ title: SEO_TITLE, noindex: true });
  return (
    <div className="shop-notfound-page">
      <div className="shop-notfound" data-testid="notfound">
        <Icon name="cart" size={44} className="shop-notfound__icon" />
        <h1 className="shop-notfound__title">{t("notFound.title")}</h1>
        <p className="shop-notfound__body">{BODY}</p>
        <Link className="shop-notfound__cta" to={`${SHOP_BASE}/items`}>
          {t("notFound.cta")}
        </Link>
      </div>
    </div>
  );
}
