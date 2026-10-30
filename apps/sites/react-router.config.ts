import type { Config } from "@react-router/dev/config";

export default {
  ssr: true,
  appDirectory: "src/routes",
  routeDiscovery: { mode: "lazy", manifestPath: "/assets/__manifest" },
} satisfies Config;
