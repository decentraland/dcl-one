import { renderToString } from "react-dom/server";
import { MemoryRouter } from "react-router";
import { describe, expect, it } from "vitest";
import type { EventProfile } from "@/data/lib/catalyst/places/event-management";
import { EventNavigation } from "./EventManagement";

describe("when a moderator who may edit any schedule opens the hangout navigation", () => {
  const profile = { user: "0x1111111111111111111111111111111111111111", permissions: ["approve_any_event", "edit_any_schedule"] } as EventProfile;
  const html = renderToString(<MemoryRouter><EventNavigation profile={profile} canManageUsers /></MemoryRouter>);

  it("should offer the moderation and user pages", () => {
    expect(html).toContain('href="/events/admin/pending-events"');
    expect(html).toContain('href="/events/admin/users"');
  });

  it("should offer no schedules page", () => {
    expect(html).not.toContain("Schedules");
    expect(html).not.toContain("/landings/");
  });
});
