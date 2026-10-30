// from: shop@2d3bf62:app/src/intl/en.json (assetCard.buyNow, itemDetail.becauseYouOwn, itemDetail.savedCount,
//       itemDetail.buyResale, notifyMe.*)

import { hasMessage, t } from "../discovery/intl";

const LOCAL: Readonly<Record<string, string>> = {
  "assetCard.buyNow": "Buy now",
  "itemDetail.becauseYouOwn": "Because of what you own",
  "itemDetail.savedCount": "{count, plural, one {# person saved this} other {# people saved this}}",
  "itemDetail.favorite": "Save {name}",
  "itemDetail.unfavorite": "Remove {name} from favorites",
  "itemDetail.favoriteFailed": "Couldn't update favorites.",
  "itemDetail.favoriteRetry": "Try again",
  "itemDetail.buyResale": "Buy resale",
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
