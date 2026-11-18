import { useEffect, useRef } from "react";
import { redirect } from "react-router";

import LdCreatorHubDownloadPage from "@ui/landings/pages/LdCreatorHubDownloadPage";

import {
  CREATOR_HUB_DOWNLOAD_STORY as STORY,
  successPath,
  useCreatorHubDownloadClick,
} from "@/data/lib/download/creator-hub-click";
import { loadLatestRelease, resolveDownload, type OsKey } from "@/data/lib/download/creator-hub.server";
import { sidLoader } from "@/core/lib/telemetry/page-loader";
import { track } from "@/core/lib/telemetry/track";

import type { Route } from "./+types/download_.creator-hub";

export async function loader({ request, context }: Route.LoaderArgs) {
  const url = new URL(request.url);
  const osParam = url.searchParams.get("os");
  const archParam = url.searchParams.get("arch");

  const resolved = resolveDownload(await loadLatestRelease(), request.headers.get("user-agent"), {
    os: osParam,
    arch: archParam,
  });

  if (url.searchParams.get("step")?.trim() === "success") {
    throw redirect(successPath(resolved.detectedOs, resolved.detectedArch));
  }

  const { sid, wrap } = sidLoader(request, context);

  const payload = {
    sid,
    version: resolved.version,
    overridden: resolved.overridden,
    detectedOs: resolved.detectedOs,
    detectedArch: resolved.detectedArch,
    primary: resolved.installable ? resolved.primary : null,
    secondary: resolved.installable ? resolved.secondary : [],
  };

  return wrap(payload);
}

export default function CreatorHubDownloadRoute({ loaderData }: Route.ComponentProps) {
  const d = loaderData;
  const onDownload = useCreatorHubDownloadClick(d.sid);

  useDownloadView(d.sid, d.detectedOs, d.detectedArch, d.version, d.overridden);
  return (
    <LdCreatorHubDownloadPage
      version={d.version}
      primary={d.primary}
      secondary={d.secondary}
      onDownload={(option, event) => onDownload(option, option === d.primary ? "primary" : "secondary", event)}
    />
  );
}

function useDownloadView(
  sid: string,
  os: OsKey,
  arch: string,
  version: string,
  overridden: boolean,
) {
  const lastKey = useRef<string | null>(null);
  useEffect(() => {
    const key = `${os}|${arch}`;
    if (lastKey.current === key) return;
    lastKey.current = key;
    track(
      "lp_creatorhub_download_viewed",
      { os, arch, version, overridden },
      { sid, story: STORY },
    );
  }, [sid, os, arch, version, overridden]);
}
