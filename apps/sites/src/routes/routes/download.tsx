import { redirect } from "react-router";

import { loadDownloadCounts } from "@/data/lib/download/counts.server";
import { downloadOptions } from "@/data/lib/download/installers";
import { sidLoader } from "@/core/lib/telemetry/page-loader";
import { useAnalyticsUserId } from "@/core/lib/telemetry/page-view";
import DownloadPage from "@/features/download/DownloadPage";
import { googlePlayHref } from "@/features/download/google-play";
import { campaignSearch } from "@/features/download/installers";

import { publicBase } from "../lib/social-meta";
import { staticPageMeta } from "../lib/static-page-meta";
import type { Route } from "./+types/download";

export function meta({ matches }: Route.MetaArgs) {
  return staticPageMeta("/download", publicBase(matches));
}

// A deep link (`?position=` or `?realm=`) opens that place in the web Explorer at /play; without one, the page
// offers the desktop installers.
export async function loader({ request, context }: Route.LoaderArgs) {
  const url = new URL(request.url);
  const params = new URLSearchParams();
  for (const key of ["position", "realm"]) {
    const value = url.searchParams.get(key);
    if (value) params.set(key, value);
  }
  if (params.size > 0) throw redirect(`/play/?${params.toString()}`);
  const userAgent = request.headers.get("user-agent") ?? "";
  const { sid, wrap } = sidLoader(request, context);
  return wrap({
    sid,
    ...downloadOptions(userAgent),
    android: /android/i.test(userAgent),
    downloads: await loadDownloadCounts(),
    googlePlay: googlePlayHref(url.search),
    search: campaignSearch(url.search),
  });
}

export default function Download({ loaderData }: Route.ComponentProps) {
  const { sid, primary, secondary, android, downloads, googlePlay, search } = loaderData;
  const userId = useAnalyticsUserId();
  return (
    <DownloadPage
      primary={primary}
      secondary={secondary}
      android={android}
      downloads={downloads}
      sid={sid}
      userId={userId}
      googlePlay={googlePlay}
      search={search}
    />
  );
}
