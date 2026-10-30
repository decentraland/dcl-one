import { flatRoutes } from "@react-router/fs-routes";
import type { RouteConfig } from "@react-router/dev/routes";

// Includes the native marketplace runtime and its screen-data resource route.
export default flatRoutes({
  ignoredRouteFiles: ["**/*.test.*", "**/*.spec.*"],
}) satisfies RouteConfig;
