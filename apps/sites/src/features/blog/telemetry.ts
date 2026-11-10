import { useMemo } from "react";
import { useRouteLoaderData } from "react-router";

/** The root loader owns the visit, including client navigations between blog pages. */
export function useBlogSession() {
  const root = useRouteLoaderData("root") as { sid?: string } | undefined;
  const sid = root?.sid ?? "";
  return useMemo(() => ({ sid, story: "misc/blog" as const }), [sid]);
}
