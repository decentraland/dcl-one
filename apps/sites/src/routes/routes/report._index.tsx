import { sidLoader } from "@/core/lib/telemetry/page-loader";
import ReportForm from "@/features/social/report/ReportForm";

import type { Route } from "./+types/report._index";

export const meta = () => [{ title: "Report a player – Decentraland" }];

export async function loader({ request, context }: Route.LoaderArgs) {
  const { sid, wrap } = sidLoader(request, context);
  return wrap({ sid });
}

export default function Report({ loaderData }: Route.ComponentProps) {
  return <ReportForm trackCtx={{ sid: loaderData.sid, story: "landings/report-abuse", variant: "form" }} />;
}
