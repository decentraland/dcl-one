export { default } from "@/features/social/report/ReportSuccess";

import { publicBase } from "../lib/social-meta";
import { staticPageMeta } from "../lib/static-page-meta";
import type { Route } from "./+types/report.success";

export function meta({ matches }: Route.MetaArgs) {
  return staticPageMeta("/report/success", publicBase(matches));
}
