// /discord: decentraland.org's hCaptcha-gated invite generator runs on a
// Foundation lambda this node does not have; the reference's public link serves instead.
import { redirect } from "react-router";

const DISCORD_URL = "https://dcl.gg/discord";

export function loader() {
  return redirect(DISCORD_URL);
}
