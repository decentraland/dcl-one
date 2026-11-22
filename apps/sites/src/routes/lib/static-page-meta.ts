import type { MetaDescriptor } from "react-router";

import { absoluteUrl, socialMeta } from "./social-meta";

type StaticCard = { title: string; description: string };

/**
 * decentraland.org's cards for its marketing and legal pages (sites-deployer `OpenGraphStaticPageRoute`),
 * by served path. The document title is `{title} | Decentraland`.
 */
export const STATIC_PAGE_CARDS = {
  "/download": {
    title: "Download Decentraland",
    description: "Download Decentraland and start exploring the metaverse — events, communities, voice chat and more.",
  },
  "/download/creator-hub": {
    title: "Download Decentraland Creator Hub",
    description: "Download the Creator Hub to start building immersive scenes and interactive experiences in Decentraland.",
  },
  "/download/creator-hub-success": {
    title: "Creator Hub installed",
    description: "Open the Creator Hub and start building for Decentraland.",
  },
  "/download_success": {
    title: "You're almost done — finish installing Decentraland",
    description: "Open the file you just downloaded to finish installing Decentraland and start exploring.",
  },
  "/help": {
    title: "Help & Support",
    description: "Find answers, troubleshoot issues, and get the most out of Decentraland.",
  },
  "/report": {
    title: "Report a player",
    description: "Report disruptive behaviour and help keep Decentraland a safe community.",
  },
  "/report/success": {
    title: "Report submitted",
    description: "Your report has been submitted. Thanks for helping keep Decentraland safe.",
  },
  "/brand": { title: "Brand Guidelines", description: "Logos, colors and usage rules for the Decentraland brand." },
  "/security": { title: "Security", description: "How Decentraland protects your wallet, identity and content." },
  "/privacy": { title: "Privacy Policy", description: "How Decentraland handles your personal data." },
  "/terms": { title: "Terms of Use", description: "The rules for using Decentraland." },
  "/ethics": { title: "Code of Ethics", description: "The values that guide the Decentraland community." },
  "/content": { title: "Content Policy", description: "What is and isn’t allowed in Decentraland." },
  "/rewards-terms": { title: "Rewards Terms", description: "Terms and conditions for Decentraland rewards programs." },
  "/referral-terms": {
    title: "Referral Program Terms",
    description: "Terms and conditions for the Decentraland referral program.",
  },
} as const satisfies Record<string, StaticCard>;

export type StaticPagePath = keyof typeof STATIC_PAGE_CARDS;

export function isStaticPagePath(path: string): path is StaticPagePath {
  return Object.hasOwn(STATIC_PAGE_CARDS, path);
}

/** The head of a static page: its card, with the canonical at `base + path` when the base is known. */
export function staticPageMeta(path: StaticPagePath, base: string | undefined, opts: { noindex?: boolean } = {}): MetaDescriptor[] {
  const card = STATIC_PAGE_CARDS[path];
  return socialMeta({
    title: `${card.title} | Decentraland`,
    description: card.description,
    url: absoluteUrl(base, path),
    noindex: opts.noindex,
  });
}
