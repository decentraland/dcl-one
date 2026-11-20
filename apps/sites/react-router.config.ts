import type { Config } from "@react-router/dev/config";

export default {
  ssr: true,
  appDirectory: "src/routes",
  routeDiscovery: { mode: "initial" },
} satisfies Config;
