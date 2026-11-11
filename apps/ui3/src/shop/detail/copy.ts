// from: shop@2d3bf62:app/src/intl/en.json (assetCard.buyNow, itemDetail.becauseYouOwn, itemDetail.savedCount,
//       itemDetail.buyResale, notifyMe.*); marketplace@784c4af4:webapp/src/modules/translation/locales/en.json
//       (favorites_modal.title, favorites_modal.empty as savedBy.*)

import { hasMessage, t } from "@shop/discovery/intl";

const LOCAL: Readonly<Record<string, string>> = {
  "assetCard.buyNow": "Buy now",
  "itemDetail.becauseYouOwn": "Because of what you own",
  "itemDetail.savedCount": "{count, plural, one {# person saved this} other {# people saved this}}",
  "itemDetail.favorite": "Save {name}",
  "itemDetail.unfavorite": "Remove {name} from favorites",
  "itemDetail.favoriteFailed": "Couldn't update favorites.",
  "itemDetail.favoriteRetry": "Try again",
  "itemDetail.buyResale": "Buy resale",
  "savedBy.title": "Saved by",
  "savedBy.open": "See who saved {name}",
  "savedBy.loading": "Loading who saved it",
  "savedBy.what": "Who saved this item",
  "savedBy.empty": "Nobody has saved this item yet.",
  "notifyMe.signInCta": "Sign in to get notified",
  "notifyMe.cta": "Notify me when available",
  "notifyMe.emailLabel": "Email",
  "notifyMe.emailPlaceholder": "you@example.com",
  "notifyMe.subscribedBody": "We'll email you when it's available.",
  "notifyMe.error": "Couldn't save your request. Try again.",
};

export function pdpText(key: string, vars?: Record<string, string | number>): string {
  if (hasMessage(key)) return t(key, vars);
  const local = LOCAL[key];
  if (local == null) return t(key, vars);
  return local.replace(/\{(\w+), plural, one \{([^}]*)\} other \{([^}]*)\}\}/g, (_m, name: string, one: string, other: string) => {
    const n = Number(vars?.[name] ?? 0);
    return (n === 1 ? one : other).replace(/#/g, n.toLocaleString("en-US"));
  }).replace(/\{(\w+)\}/g, (_m, name: string) => String(vars?.[name] ?? ""));
}
