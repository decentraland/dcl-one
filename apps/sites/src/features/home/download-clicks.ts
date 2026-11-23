import type {
  DownloadActionsProps,
  DownloadAlternative,
  StoreBadgeProps,
} from "@dcl/one-ui/apps/sites/download/DownloadActions";

import type { DownloadClick, DownloadClickPlace } from "../download/download-click";
import { osName } from "../download/actions";
import { installerSelection, legacySuccessHref, withCampaignParams } from "../download/installers";
import type { HomeDownloadActions } from "./actions";

type Send = (click: DownloadClick) => void;

type Section = {
  primary: DownloadClick;
  /** The `place` its main button puts on /download_success. */
  successPlace: string;
  epic: DownloadClick;
  platformSwitch: DownloadClickPlace;
};

/** What the home page's /download_success links carry besides the control: the visitor and the page's query. */
export type SuccessLinkContext = { anonUserId?: string; search: string };

// The legacy home page's attributes per control (decentraland/sites `Home/Hero/Hero.tsx`,
// `Home/ComeHangOut/ComeHangOut.tsx`). Come Hang Out's main button sent no `event`: its `data-event` was `Click`.
const HERO: Section = {
  primary: { place: "Landing Hero", event: "Download", download_target: "desktop_installer" },
  successPlace: "landing-hero",
  epic: { place: "landing-hero-epic", event: "Download", download_target: "epic" },
  platformSwitch: "landing-hero-platform-switch",
};
const HANG_OUT: Section = {
  primary: { place: "come-hang-out", download_target: "desktop_installer" },
  successPlace: "come-hang-out",
  epic: { place: "come-hang-out", event: "Download", download_target: "epic" },
  platformSwitch: "come-hang-out-platform-switch",
};

function alternativeClick(alt: DownloadAlternative, place: DownloadClickPlace): DownloadClick {
  if (alt.icon === "ios") return { place, event: "Download", download_target: "app_store", os: "iOS" };
  if (alt.icon === "android") return { place, event: "Download", download_target: "google_play", os: "Android" };
  return { place, event: "Download", download_target: "desktop_installer", os: osName(alt.icon) };
}

function storeClick(store: StoreBadgeProps, place: DownloadClickPlace): DownloadClick {
  return store.badge === "google_play"
    ? { place, event: "Download", download_target: "google_play", os: "Android" }
    : { place, event: "Download", download_target: "app_store", os: "iOS" };
}

/** The legacy link for a control: /download_success with its place, /download with the page's `utm_*`. */
function relink(href: string, place: string, { anonUserId, search }: SuccessLinkContext): string {
  if (href.startsWith("/download_success?")) {
    return legacySuccessHref(installerSelection(new URLSearchParams(href.split("?")[1])).os, place, { anonUserId, search });
  }
  return href === "/download" ? withCampaignParams(href, search) : href;
}

function withActionClicks(actions: DownloadActionsProps, section: Section, send: Send, link: SuccessLinkContext): DownloadActionsProps {
  return {
    ...actions,
    primary: actions.primary && {
      ...actions.primary,
      href: relink(actions.primary.href, section.successPlace, link),
      onClick: () => send(section.primary),
    },
    epic: actions.epic && { ...actions.epic, onClick: () => send(section.epic) },
    alternativeGroups: actions.alternativeGroups?.map((group) =>
      group.map((alt) => ({
        ...alt,
        ...(alt.href !== undefined ? { href: relink(alt.href, section.platformSwitch, link) } : {}),
        onClick: () => send(alternativeClick(alt, section.platformSwitch)),
      })),
    ),
  };
}

function withStoreClick(store: StoreBadgeProps, place: DownloadClickPlace, send: Send): StoreBadgeProps {
  return { ...store, onClick: () => send(storeClick(store, place)) };
}

/** The home page's download controls, each sending its legacy `Click` and linking as the legacy page did. */
export function withDownloadClicks(
  actions: HomeDownloadActions,
  send: Send,
  link: SuccessLinkContext = { search: "" },
): HomeDownloadActions & {
  hangOutStore: StoreBadgeProps;
} {
  return {
    hero: withActionClicks(actions.hero, HERO, send, link),
    hangOut: withActionClicks(actions.hangOut, HANG_OUT, send, link),
    store: withStoreClick(actions.store, "Landing Hero", send),
    alternateStore: actions.alternateStore && withStoreClick(actions.alternateStore, "Landing Hero", send),
    hangOutStore: withStoreClick(actions.store, "Landing Come Hang Out", send),
  };
}
