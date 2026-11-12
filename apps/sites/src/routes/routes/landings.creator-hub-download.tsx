import { useEffect, useRef } from "react";
import { redirect } from "react-router";

import LdCreatorHubDownloadPage from "@ui/landings/pages/LdCreatorHubDownloadPage";

import {
  loadDownloadFixture,
  resolveDownload,
  type DownloadOption,
  type OsKey,
} from "@/core/lib/landings/creator-hub-download.server";
import { pageLoader, type PageDefaults } from "@/core/lib/telemetry/page-loader";
import { track } from "@/core/lib/telemetry/track";

import type { Route } from "./+types/landings.creator-hub-download";
import type { StoryId } from "@/core/lib/telemetry/story-id";

const STORY: StoryId = "landings/creator-hub-download";

const FALLBACK: PageDefaults = {
  variant: "os_detected",
};

const SUCCESS_PATH = "/download/creator-hub-success";
// from: sites@17b3535a:src/hooks/useCreatorHubDownload.ts (REDIRECT_DELAY_MS): the installer starts, then the steps page opens.
const SUCCESS_DELAY_MS = 3000;

export function successPath(os: OsKey, arch: string): string {
  return `${SUCCESS_PATH}?${new URLSearchParams({ os, arch }).toString()}`;
}

export async function loader({ request, context }: Route.LoaderArgs) {
  const url = new URL(request.url);
  const osParam = url.searchParams.get("os");
  const archParam = url.searchParams.get("arch");

  const fixture = loadDownloadFixture();
  const resolved = resolveDownload(fixture, request.headers.get("user-agent"), {
    os: osParam,
    arch: archParam,
  });

  if (url.searchParams.get("step")?.trim() === "success") {
    throw redirect(successPath(resolved.detectedOs, resolved.detectedArch));
  }

  const { sid, wrap } = await pageLoader(request, context, FALLBACK);

  const payload = {
    sid,
    version: resolved.version,
    releaseUrl: resolved.releaseUrl,
    repo: resolved.repo,
    overridden: resolved.overridden,
    detectedOs: resolved.detectedOs,
    detectedArch: resolved.detectedArch,
    primary: resolved.installable ? resolved.primary : null,
    secondary: resolved.installable ? resolved.secondary : [],
    systemRequirements: resolved.systemRequirements,
  };

  return wrap(payload);
}

export default function CreatorHubDownloadRoute({ loaderData }: Route.ComponentProps) {
  const d = loaderData;
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useDownloadView(d.sid, d.detectedOs, d.detectedArch, d.version, d.overridden);
  useEffect(() => () => {
    if (timer.current !== null) clearTimeout(timer.current);
  }, []);

  function onDownload(option: DownloadOption) {
    track(
      "lp_creatorhub_download_clicked",
      {
        os: option.osKey,
        arch: option.arch,
        file_name: option.fileName,
        kind: option === d.primary ? "primary" : "secondary",
      },
      { sid: d.sid, story: STORY },
    );
    if (timer.current !== null) clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      window.location.assign(successPath(option.osKey, option.arch));
    }, SUCCESS_DELAY_MS);
  }

  return (
    <LdCreatorHubDownloadPage
      version={d.version}
      primary={d.primary}
      secondary={d.secondary}
      onDownload={onDownload}
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
