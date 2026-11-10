import type { ReactNode } from "react";
import {
  HydrationBoundary,
  useQuery,
  type DehydratedState,
} from "@tanstack/react-query";
import { useRouteLoaderData } from "react-router";
import { toLocale } from "../locale/locales";
import { blogPublicBase } from "./route-kit";
import { categoriesQuery, messagesQuery, type BlogBase } from "./loaders";

function Ready({ children }: { children: (base: BlogBase) => ReactNode }) {
  const root = useRouteLoaderData("root") as { locale?: string } | undefined;
  const chosen =
    typeof document === "undefined"
      ? undefined
      : document.cookie
          .split(";")
          .map((part) => part.trim())
          .find((part) => part.startsWith("dcl-locale="))
          ?.slice("dcl-locale=".length);
  const locale = toLocale(chosen) ?? toLocale(root?.locale) ?? "en";
  const categories = useQuery({ ...categoriesQuery(), throwOnError: true });
  const messages = useQuery({ ...messagesQuery(locale), throwOnError: true });
  if (!categories.data || !messages.data)
    return (
      <main aria-busy="true">
        <p role="status">Loading blog…</p>
      </main>
    );
  return children({
    categories: categories.data,
    messages: messages.data,
    publicBase: blogPublicBase(),
  });
}

export function BlogRoute({
  data,
  children,
}: {
  data: { dehydratedState?: DehydratedState } | null | undefined;
  children: (base: BlogBase) => ReactNode;
}) {
  return (
    <HydrationBoundary state={data?.dehydratedState}>
      <Ready>{children}</Ready>
    </HydrationBoundary>
  );
}
