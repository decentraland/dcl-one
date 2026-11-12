import { describe, expect, it } from "vitest";

import { DEFAULT_INTERCOM_APP_ID, helpConfig } from "./config.server";
import { DEFAULT_STATUS_PAGE_URL, DEFAULT_STATUS_SERVICES } from "./status";

describe("helpConfig", () => {
  it("uses the production services, status page and Intercom workspace when nothing is configured", () => {
    expect(helpConfig({})).toEqual({
      services: DEFAULT_STATUS_SERVICES,
      statusPageUrl: DEFAULT_STATUS_PAGE_URL,
      intercomAppId: DEFAULT_INTERCOM_APP_ID,
    });
  });

  it("takes each value a deployment configures", () => {
    const config = helpConfig({
      HELP_STATUS_SERVICES: JSON.stringify([{ name: "Node", url: "https://node.example/about" }]),
      HELP_STATUS_PAGE_URL: "https://status.example/",
      INTERCOM_APP_ID: " app-example123 ",
    });
    expect(config).toEqual({
      services: [{ name: "Node", url: "https://node.example/about" }],
      statusPageUrl: "https://status.example/",
      intercomAppId: "app-example123",
    });
  });

  it("turns the chat widget off when INTERCOM_APP_ID is set empty", () => {
    expect(helpConfig({ INTERCOM_APP_ID: "" }).intercomAppId).toBe("");
  });
});
