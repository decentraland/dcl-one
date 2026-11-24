import type { LegacyClick } from "@/core/lib/telemetry/legacy-click";

type Send = (click: LegacyClick) => void;

/**
 * The legacy home page's non-download `Click`s (decentraland/sites `Home/CatchTheVibe`, `Home/WhatsOn`,
 * `Home/WeeklyRituals`). A What's On card names itself by its title, a ritual by its id.
 */
export function homeClicks(send: Send) {
  return {
    hangOut: () => send({ place: "Landing Catch The Vibe" }),
    viewAll: () => send({ place: "Landing Explore", section: "view_all" }),
    whatsOnCard: (card: { title: string }) => send({ place: "Landing Explore", card: card.title }),
    whatsOnJumpIn: (card: { title: string }) => send({ place: "Landing Explore", card: card.title, section: "jump_in" }),
    ritual: (card: { id: string }) => send({ place: "Landing Weekly Rituals", card: card.id }),
  };
}
