import { describe, expect, it } from "vitest";

import { loader as landingLoader } from "./landings.report-abuse";
import { loader as playersLoader } from "./report.players";

const args = (url: string) => ({ request: new Request(url), params: {}, context: {} }) as never;

describe("report redirects", () => {
  it("sends /report/players to /report with its query", () => {
    const response = playersLoader(args("https://sites.test/report/players?player_address=0xa&reported_address=0xb")) as Response;
    expect(response.status).toBe(302);
    expect(response.headers.get("Location")).toBe("/report?player_address=0xa&reported_address=0xb");
  });

  it("sends /landings/report-abuse to /report", () => {
    const response = landingLoader(args("https://sites.test/landings/report-abuse")) as Response;
    expect(response.status).toBe(308);
    expect(response.headers.get("Location")).toBe("/report");
  });
});
