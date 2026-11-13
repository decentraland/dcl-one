// The download pages: the Explorer's /download (DownloadLayout) and /download_success (DownloadSuccess), the Creator
// Hub's /download/creator-hub and /download/creator-hub-success (sites: App.tsx, pages/download*, pages/DownloadSuccess).
import { both, signedIn, type PageSpec } from "../../lib/spec.mts";
import { mainText, SITES_UNRECORDED, sitesFrameRules } from "./frame.mts";

// DownloadStepsLayout renders no <main> landmark, so its title is found by its words alone: the reference's h3, dcl-one's
// page heading (library/ui apps/sites/download/DownloadSuccess.tsx).
const STARTED = "xpath=//*[self::h1 or self::h3][normalize-space()='Your download has started!']";
// The title's apostrophe rules out mainText's single-quoted literal.
const ALMOST_DONE = `xpath=//main//h3[normalize-space()="You're almost done!"]`;
// The backdrop shows while the installer starts and closes once it has (DownloadSuccess's showBackdrop): the page is
// compared after that.
const BACKDROP = "text=Downloading Decentraland...";
const H = (tag: string, text: string) => `xpath=//main//${tag}[starts-with(normalize-space(),'${text}')]`;
const TITLE = H("h2", "Download Decentraland");
const CREATOR_HUB = mainText("Decentraland Creator Hub");

const frame = sitesFrameRules();

const download: PageSpec = {
  id: "download",
  title: "Download Decentraland (the Explorer)",
  reference: "sites",
  path: "/download",
  // dcl-one renders the desktop and the phone title and CSS shows one: the ready title is the visible one.
  ready: { reference: TITLE, "dcl-one": `${TITLE} >> visible=true` },
  keys: [
    { name: "title", selector: TITLE },
    { name: "footer", selector: "footer" },
  ],
  ...frame,
  unrecorded: SITES_UNRECORDED,
};

const downloadSuccess: PageSpec = {
  id: "download-success",
  title: "Download Decentraland: the download has started",
  reference: "sites",
  path: "/download_success",
  ready: both(STARTED),
  setup: async (page) => {
    await page.locator(BACKDROP).first().waitFor({ state: "hidden", timeout: 20_000 });
  },
  keys: [
    { name: "title", selector: STARTED },
    { name: "footer", selector: "footer" },
  ],
  ...frame,
  unrecorded: [
    ...SITES_UNRECORDED,
    { match: /^POST auth \/identities /, why: "the reference posts a new ephemeral identity to auth-api each session (sites: DownloadSuccess.tsx useGetIdentityId), so its body never repeats and cannot be recorded; dcl-one makes no such call" },
  ],
};

const creatorHub: PageSpec = {
  id: "download-creator-hub",
  title: "Download the Creator Hub",
  reference: "sites",
  path: "/download/creator-hub",
  ready: CREATOR_HUB,
  keys: [
    { name: "title", selector: CREATOR_HUB },
    { name: "footer", selector: "footer" },
  ],
  ...frame,
  unrecorded: SITES_UNRECORDED,
};

const creatorHubSuccess: PageSpec = {
  id: "download-creator-hub-success",
  title: "Download the Creator Hub: almost done",
  reference: "sites",
  path: "/download/creator-hub-success",
  ready: both(ALMOST_DONE),
  keys: [
    { name: "title", selector: ALMOST_DONE },
    { name: "footer", selector: "footer" },
  ],
  ...frame,
  unrecorded: SITES_UNRECORDED,
};

export default [
  download, signedIn(download, "Download Decentraland, signed in as the test wallet"),
  downloadSuccess, signedIn(downloadSuccess, "Download Decentraland: started, signed in as the test wallet"),
  creatorHub, signedIn(creatorHub, "Download the Creator Hub, signed in as the test wallet"),
  creatorHubSuccess, signedIn(creatorHubSuccess, "Download the Creator Hub: almost done, signed in as the test wallet"),
] satisfies PageSpec[];
