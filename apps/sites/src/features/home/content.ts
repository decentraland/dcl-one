import alanImage from "@dcl/one-ui/apps/sites/assets/catch_the_vibe/alan.webp";
import alanVideo from "@dcl/one-ui/apps/sites/assets/catch_the_vibe/alan.mp4";
import roustanImage from "@dcl/one-ui/apps/sites/assets/catch_the_vibe/roustan.webp";
import roustanVideo from "@dcl/one-ui/apps/sites/assets/catch_the_vibe/roustan.mp4";
import mondayMobile from "@dcl/one-ui/apps/sites/assets/weekly-rituals/mobile/Monday-Mobile.webp";
import tuesdayMobile from "@dcl/one-ui/apps/sites/assets/weekly-rituals/mobile/Tuesday-Mobile.webp";
import wednesdayMobile from "@dcl/one-ui/apps/sites/assets/weekly-rituals/mobile/Wednesday-Mobile.webp";
import monday from "@dcl/one-ui/apps/sites/assets/weekly-rituals/monday.webp";
import tuesday from "@dcl/one-ui/apps/sites/assets/weekly-rituals/tuesday.webp";
import wednesday from "@dcl/one-ui/apps/sites/assets/weekly-rituals/wednesday.webp";

// Home media and recurring events from decentraland/sites at cfdfa1a51216f9dc90c5b148686a95e7ee9c2862 (src/data/static-content.ts).

/** Two creators' clips; names and wallets are public profile data, as on source Sites. */
export const VIBE_CLIPS = [
  {
    id: "alan",
    name: "AlanHowick",
    address: "0xd82d005e8f8d5385db40ba23884a5c967bb1e8af",
    image: alanImage,
    video: alanVideo,
  },
  {
    id: "roustan",
    name: "Roustan",
    address: "0x54e93609eb454a1f152edefdf022480794ce2130",
    image: roustanImage,
    video: roustanVideo,
  },
] as const;

export const CAREER_MONDAYS_EVENT_ID = "0795dec0-4862-4f19-a4d5-d0f3c3c3b559";
export const TAKEOVER_TUESDAYS_EVENT_ID =
  "efc2bf0f-bf0d-447a-82a2-fe7a5650ca19";
export const WATCH_PARTY_WEDNESDAYS_EVENT_ID =
  "e22333e4-4277-4b59-b99b-ba3174e1dd29";

/** The recurring events and their supplied desktop and phone artwork. */
export const WEEKLY_RITUALS = [
  {
    id: "career-mondays",
    title: "Career Mondays",
    eventId: CAREER_MONDAYS_EVENT_ID,
    image: monday,
    mobileImage: mondayMobile,
  },
  {
    id: "takeover-tuesdays",
    title: "Takeover Tuesdays",
    eventId: TAKEOVER_TUESDAYS_EVENT_ID,
    image: tuesday,
    mobileImage: tuesdayMobile,
  },
  {
    id: "watch-party",
    title: "Watch Party Wednesdays",
    eventId: WATCH_PARTY_WEDNESDAYS_EVENT_ID,
    image: wednesday,
    mobileImage: wednesdayMobile,
  },
] as const;
