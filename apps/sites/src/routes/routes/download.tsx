import { redirect } from "react-router";

import { loadDownloadCounts } from "@/data/lib/download/counts.server";
import { downloadOptions } from "@/data/lib/download/installers";
import { sidLoader } from "@/core/lib/telemetry/page-loader";
import DownloadPage from "@/features/download/DownloadPage";

import { canonicalLink } from "../lib/canonical";
import type { Route } from "./+types/download";

export function meta({ matches }: Route.MetaArgs) {
  return [
    { title: "Download | Decentraland" },
    { name: "description", content: "Download Decentraland and start exploring the metaverse — events, communities, voice chat and more." },
    ...canonicalLink(matches[0]?.loaderData?.publicPageBase, "/download"),
  ];
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
  return wrap({ sid, ...downloadOptions(userAgent), android: /android/i.test(userAgent), downloads: await loadDownloadCounts() });
}

export default function Download({ loaderData }: Route.ComponentProps) {
  const { primary, secondary, android, downloads } = loaderData;
  return <DownloadPage primary={primary} secondary={secondary} android={android} downloads={downloads} />;
}
