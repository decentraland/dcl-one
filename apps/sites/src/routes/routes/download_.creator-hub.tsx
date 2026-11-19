import { redirect } from "react-router";

import { loadLatestRelease, resolveDownload } from "@/data/lib/download/creator-hub.server";
import { sidLoader } from "@/core/lib/telemetry/page-loader";
import CreatorHubDownloadPage from "@/features/download/CreatorHubDownloadPage";
import { successPath } from "@/features/download/creator-hub-click";

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
    macChipUnknown: resolved.macChipUnknown,
    detectedOs: resolved.detectedOs,
    detectedArch: resolved.detectedArch,
    primary: resolved.installable ? resolved.primary : null,
    secondary: resolved.installable ? resolved.secondary : [],
  };

  return wrap(payload);
}

export default function CreatorHubDownloadRoute({ loaderData }: Route.ComponentProps) {
  return <CreatorHubDownloadPage {...loaderData} />;
}
