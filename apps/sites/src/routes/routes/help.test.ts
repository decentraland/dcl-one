import { describe, expect, it } from "vitest";

import { DEFAULT_INTERCOM_APP_ID } from "@/data/lib/help/config.server";
import { DEFAULT_STATUS_PAGE_URL, DEFAULT_STATUS_SERVICES } from "@/data/lib/help/status";

import { loader, meta } from "./help";

describe("/help loader", () => {
  it("hands the page the production services, status page and Intercom workspace by default", async () => {
    const result = (await loader({ request: new Request("https://site.example/help") } as never)) as unknown as {
      data: Record<string, unknown>;
    };
    expect(result.data).toMatchObject({
      services: DEFAULT_STATUS_SERVICES,
      statusPageUrl: DEFAULT_STATUS_PAGE_URL,
      intercomAppId: DEFAULT_INTERCOM_APP_ID,
      wallet: null,
    });
    expect(typeof result.data.sid).toBe("string");
  });
});

describe("/help meta", () => {
  it("names the page after the help center", () => {
    expect(meta()[0]).toEqual({ title: "Decentraland - Help Center" });
  });
});
