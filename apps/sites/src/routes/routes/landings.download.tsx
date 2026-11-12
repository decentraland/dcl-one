import { DownloadPage } from "@dcl/one-ui/apps/sites/download/DownloadPage";
import SitesChrome from "@ui/web/frames/SitesChrome";

import { loadDownloadsLabel } from "@/data/lib/catalyst/landings/download-counts.server";
import { downloadActions } from "@/features/download/actions";
import { detectPlatform } from "@/features/download/options";

import type { Route } from "./+types/landings.download";

export function meta() {
  return [{ title: "Download Decentraland" }];
}

export async function loader({ request }: Route.LoaderArgs) {
  const downloads = await loadDownloadsLabel();
  return {
    downloads,
    platform: detectPlatform(request.headers.get("user-agent")),
  };
}

export default function LandingsDownload({ loaderData }: Route.ComponentProps) {
  const { actions, store } = downloadActions(
    loaderData.platform,
    loaderData.downloads,
  );

  return (
    <SitesChrome active="play" overlayNav>
      <DownloadPage
        title="Download Decentraland"
        actions={actions}
        store={store}
      />
    </SitesChrome>
  );
}
