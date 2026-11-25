// The request pipeline that `react-router-serve` runs (`createRequestHandler`), over a test build of route modules.
import { createRequestHandler, type ServerBuild } from "react-router";

export type TestRoute = {
  id: string;
  parentId?: string;
  path?: string;
  module: object;
  /** What the browser manifest says: a server loader (true by default) and a client loader (false by default). */
  loader?: boolean;
  clientLoader?: boolean;
};

/** The handler of a production build with `entry` as its server entry module and `routes` as its route tree. */
export function testRequestHandler(entry: object, routes: TestRoute[]) {
  const build = {
    entry: { module: entry },
    routes: Object.fromEntries(routes.map(({ id, parentId, path, module }) => [id, { id, parentId, path, index: undefined, caseSensitive: undefined, module }])),
    assets: {
      entry: { module: "/entry.js", imports: [] },
      url: "/manifest.js",
      version: "test",
      routes: Object.fromEntries(routes.map(({ id, parentId, path, loader = true, clientLoader = false }) => [id, {
        id, parentId, path, module: `/${id}.js`, imports: [], css: [], hasAction: false, hasLoader: loader, hasClientAction: false,
        hasClientLoader: clientLoader, hasClientMiddleware: false, hasErrorBoundary: id === "root",
      }])),
    },
    publicPath: "/",
    assetsBuildDirectory: "build/client",
    basename: "/",
    future: {},
    ssr: true,
    isSpaMode: false,
    prerender: [],
    routeDiscovery: { mode: "initial" },
  } as unknown as ServerBuild;
  return createRequestHandler(build, "production");
}
