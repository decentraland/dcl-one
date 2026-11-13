// /discord: decentraland.org's hCaptcha-gated invite generator (sites: pages/discord/DiscordPage.tsx) runs on a
// Foundation lambda this node does not have; the reference's public link (config/publicLinks.json) serves instead.
import { redirect } from "react-router";

export const DISCORD_URL = "https://dcl.gg/discord";

export function loader() {
  return redirect(DISCORD_URL);
}
