import { sidLoader } from "@/core/lib/telemetry/page-loader";
import { useAnalyticsUserId } from "@/core/lib/telemetry/page-view";
import ReportForm from "@/features/social/report/ReportForm";

import { publicBase } from "../lib/social-meta";
import { staticPageMeta } from "../lib/static-page-meta";
import type { Route } from "./+types/report._index";

export function meta({ matches }: Route.MetaArgs) {
  return staticPageMeta("/report", publicBase(matches));
}

export async function loader({ request, context }: Route.LoaderArgs) {
  const { sid, wrap } = sidLoader(request, context);
  return wrap({ sid });
}

export default function Report({ loaderData }: Route.ComponentProps) {
  const analyticsUserId = useAnalyticsUserId();
  return (
    <ReportForm
      trackCtx={{ sid: loaderData.sid, story: "landings/report-abuse", variant: "form" }}
      analyticsUserId={analyticsUserId}
    />
  );
}
