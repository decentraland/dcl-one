import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import type { ComponentType } from "react";
import { renderToString } from "react-dom/server";
import {
  createStaticHandler,
  createStaticRouter,
  Outlet,
  RouterContextProvider,
  StaticRouterProvider,
  type LoaderFunction,
} from "react-router";
import { describe, expect, it } from "vitest";

import ChromeNavBridge from "@/features/components/chrome/ChromeNavBridge";
import styles from "@/features/components/chrome/SitesLayout.module.css";

import LegalSlugRoute, { loader } from "./$slug";

function SitesTestLayout() {
  return (
    <QueryClientProvider client={new QueryClient()}>
      <ChromeNavBridge>
        <Outlet />
      </ChromeNavBridge>
    </QueryClientProvider>
  );
}

describe("when a legal document is rendered with Sites chrome", () => {
  it.each(["/privacy", "/terms", "/creator-fund-terms"])(
    "should reserve header space on desktop and keep the mobile overlay for %s",
    async (path) => {
      const handler = createStaticHandler([
        {
          Component: SitesTestLayout,
          children: [
            {
              path: ":slug",
              loader: loader as unknown as LoaderFunction,
              Component: LegalSlugRoute as ComponentType,
            },
          ],
        },
      ]);
      const context = await handler.query(
        new Request("https://sites.test" + path),
        {
          requestContext: new RouterContextProvider(),
        },
      );
      if (context instanceof Response) {
        throw new Error(`Unexpected response: ${context.status}`);
      }
      const router = createStaticRouter(handler.dataRoutes, context);
      const html = renderToString(
        <StaticRouterProvider
          context={context}
          router={router}
          hydrate={false}
        />,
      );

      expect(html).toContain('data-header-layout="native"');
      const layout = html.match(/<div[^>]*data-site-layout=""[^>]*>/)?.[0];
      expect(layout).toContain(styles.mobileOverlay);
      expect(layout).not.toContain(` ${styles.overlay}"`);
      expect(layout).not.toContain("data-header-overlay");
    },
  );
});
