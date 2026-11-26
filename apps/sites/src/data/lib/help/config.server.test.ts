import { describe, expect, it } from "vitest";

import { DEFAULT_INTERCOM_APP_ID, helpConfig } from "./config.server";
import { DEFAULT_STATUS_PAGE_URL, DEFAULT_STATUS_SERVICES } from "./status";

describe("helpConfig", () => {
  describe("when nothing is configured", () => {
    it("should use the production services, status page and Intercom workspace", () => {
      expect(helpConfig({})).toEqual({
        services: DEFAULT_STATUS_SERVICES,
        statusPageUrl: DEFAULT_STATUS_PAGE_URL,
        intercomAppId: DEFAULT_INTERCOM_APP_ID,
      });
    });
  });

  describe("when a deployment configures each value", () => {
    it("should take each configured value", () => {
      expect(helpConfig({
        HELP_STATUS_SERVICES: JSON.stringify([{ name: "Node", url: "https://node.example/about" }]),
        HELP_STATUS_PAGE_URL: "https://status.example/",
        INTERCOM_APP_ID: " app-example123 ",
      })).toEqual({
        services: [{ name: "Node", url: "https://node.example/about" }],
        statusPageUrl: "https://status.example/",
        intercomAppId: "app-example123",
      });
    });
  });

  describe("when INTERCOM_APP_ID is set empty", () => {
    it("should turn the chat widget off", () => {
      expect(helpConfig({ INTERCOM_APP_ID: "" }).intercomAppId).toBe("");
    });
  });
});
