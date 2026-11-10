import type { ReactNode } from "react";

import type {
  DownloadActionsProps,
  StoreBadgeProps,
} from "@dcl/one-ui/apps/sites/download/DownloadActions";

import type { DownloadOffer, DownloadOption } from "./options";
import type { DownloadPlace } from "./params";
import { APP_STORE_URL, EPIC_STORE_URL, googlePlayUrl } from "./stores";
import type { useDownloadCtas } from "./useDownloadCtas";

type Ctas = Pick<
  ReturnType<typeof useDownloadCtas>,
  "installerHref" | "chooseInstaller" | "leaveForStore"
>;
type Translate = {
  t: (key: string, values?: Record<string, ReactNode>) => ReactNode;
  text: (key: string, values?: Record<string, string | number>) => string;
};

export type ClickLike = {
  preventDefault: () => void;
  defaultPrevented?: boolean;
  button?: number;
  metaKey?: boolean;
  ctrlKey?: boolean;
  shiftKey?: boolean;
  altKey?: boolean;
};

/**
 * A click that is not ours to take over: the browser turns it into a new tab, window or download,
 * or another handler already claimed it.
 */
export function isModifiedClick(event: ClickLike): boolean {
  return !!(
    event.defaultPrevented ||
    event.metaKey ||
    event.ctrlKey ||
    event.shiftKey ||
    event.altKey ||
    (event.button ?? 0) !== 0
  );
}

/**
 * Where each kind of click comes from on a surface, as source Sites names them: `installer` and
 * `platformSwitch` also go on the /download_success URL; the others only name the Click.
 */
export type ActionPlaces = {
  installer: DownloadPlace;
  installerClick: string;
  platformSwitch: DownloadPlace;
  epic: string;
  store: string;
};

/**
 * The installer, Epic and alternative-platform CTAs for one surface. Phone platforms are store
 * links on /download, or open the QR dialog on the home (`openQr`).
 */
export function downloadActionsProps(input: {
  offer: DownloadOffer;
  ctas: Ctas;
  i18n: Translate;
  places: ActionPlaces;
  downloadsLabel: string;
  campaign: Record<string, string>;
  openQr?: (os: "ios" | "android") => void;
}): DownloadActionsProps {
  const { offer, ctas, i18n, places, campaign, openQr } = input;
  if (!offer.primary) return {};
  // The href works without JS; with JS the click is recorded before navigating. A modified click
  // (new tab or window) is recorded and left to the browser.
  const installer = (
    option: DownloadOption,
    place: DownloadPlace,
    clickPlace: string = place,
  ) => ({
    href: ctas.installerHref(option, place),
    onClick: (event: ClickLike) => {
      const href = ctas.chooseInstaller(option, place, clickPlace);
      if (isModifiedClick(event)) return;
      event.preventDefault();
      window.location.assign(href);
    },
  });
  const osName = (option: DownloadOption) =>
    i18n.text(`page.download.os.${option.os}`);
  const phone = (icon: "ios" | "android") => {
    const target = icon === "ios" ? "app_store" : "google_play";
    const label = i18n.text(
      icon === "ios"
        ? "page.download.store.ios"
        : "page.download.store.google_play",
    );
    if (openQr)
      return {
        icon,
        label,
        onClick: () => {
          ctas.leaveForStore(target, places.platformSwitch);
          openQr(icon);
        },
      };
    return {
      icon,
      label,
      href: icon === "ios" ? APP_STORE_URL : googlePlayUrl(campaign),
      external: true,
      onClick: () => ctas.leaveForStore(target, places.store),
    };
  };

  return {
    primary: {
      label: i18n.t("page.download.download_for_short"),
      os: offer.primary.os,
      osName: osName(offer.primary),
      ...installer(offer.primary, places.installer, places.installerClick),
    },
    epic: {
      label: i18n.t("page.download.download_on"),
      iconLabel: i18n.text("page.download.epic_label"),
      href: EPIC_STORE_URL,
      onClick: () => ctas.leaveForStore("epic", places.epic),
    },
    downloads: i18n.t("page.download.total_downloads", {
      downloads: input.downloadsLabel,
    }),
    alternatives: [
      ...(offer.secondary
        ? [
            {
              icon: offer.secondary.os,
              label: osName(offer.secondary),
              ...installer(offer.secondary, places.platformSwitch),
            },
          ]
        : []),
      phone("ios"),
      phone("android"),
    ],
  };
}

/** The one store badge phones see instead of installers. */
export function storeBadgeProps(input: {
  offer: DownloadOffer;
  ctas: Ctas;
  i18n: Translate;
  place: string;
  campaign: Record<string, string>;
}): StoreBadgeProps | undefined {
  const store = input.offer.store;
  if (!store) return undefined;
  return {
    badge: store,
    label: input.i18n.text(
      store === "app_store"
        ? "page.download.store.app_store_badge"
        : "page.download.store.google_play_badge",
    ),
    href: store === "app_store" ? APP_STORE_URL : googlePlayUrl(input.campaign),
    onClick: () => input.ctas.leaveForStore(store, input.place),
  };
}
