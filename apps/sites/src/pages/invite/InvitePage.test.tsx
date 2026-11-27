// @vitest-environment jsdom
import type { ReactNode } from "react";
import { renderToString } from "react-dom/server";
import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, beforeAll, describe, expect, it, vi } from "vitest";
import { docsUrl } from "@web-core/docs";

import { InvitePage } from "./InvitePage";
import { referralUrl } from "./referral-url";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

const layout = vi.hoisted(() => vi.fn());
vi.mock("@/features/components/chrome/SitesLayout", () => ({
  default: ({ children, ...props }: { children?: ReactNode; hideNav?: boolean }) => {
    layout(props);
    return <>{children}</>;
  },
}));
vi.mock("@ui/wearable-preview/WearablePreview", () => ({
  default: ({ profile, controls }: { profile: string; controls?: boolean }) => (
    <canvas data-testid="avatar" data-profile={profile} data-controls={String(controls)} />
  ),
}));
vi.mock("@dcl/one-ui", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@dcl/one-ui")>()),
  AnimatedBackground: () => null,
}));

const ADDRESS = "0xAbCdEf0123456789aBcDeF0123456789AbCdEf01";

// Loaded once up front, so a page that asks for the renderer gets it within a tick in every test.
beforeAll(async () => {
  await import("@ui/wearable-preview/WearablePreview");
});

afterEach(() => {
  cleanup();
  layout.mockReset();
});

const avatarSlots = (container: HTMLElement) =>
  Array.from(container.querySelectorAll("[data-invite-avatar]"), (slot) => slot.childElementCount);

describe("when the invite page renders", () => {
  describe("and the inviter resolved to a wallet", () => {
    it("should show the landing without the site navigation", () => {
      render(<InvitePage referrerName="Jane" referrerAddress={ADDRESS} />);
      expect(layout).toHaveBeenCalledWith(expect.objectContaining({ hideNav: true }));
      expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("Jane invited you to join Decentraland");
    });

    it("should send both calls to action to the sign-in with the lowercased wallet as referrer", () => {
      render(<InvitePage referrerName="Jane" referrerAddress={ADDRESS} />);
      const expected = referralUrl(ADDRESS.toLowerCase());
      expect(screen.getByRole("link", { name: "JOIN NOW" }).getAttribute("href")).toBe(expected);
      expect(screen.getByRole("link", { name: "JOIN FREE" }).getAttribute("href")).toBe(expected);
    });

    it("should draw the inviter's avatar, without controls, once mounted in the browser", async () => {
      render(<InvitePage referrerName="Jane" referrerAddress={ADDRESS} />);
      const avatar = await screen.findByTestId("avatar");
      expect(avatar.getAttribute("data-profile")).toBe(ADDRESS.toLowerCase());
      expect(avatar.getAttribute("data-controls")).toBe("false");
    });

    it("should leave the avatar out of the server's HTML", () => {
      const html = renderToString(<InvitePage referrerName="Jane" referrerAddress={ADDRESS} />);
      expect(html).toContain("Jane");
      expect(html).not.toContain('data-testid="avatar"');
    });

    it("should link the questions to the player FAQs in the docs", () => {
      render(<InvitePage referrerName="Jane" referrerAddress={ADDRESS} />);
      expect(screen.getByRole("link", { name: "see more" }).getAttribute("href")).toBe(docsUrl("player/faqs"));
    });
  });

  describe("and the inviter's wallet has no name", () => {
    it("should show the title alone and still draw the avatar", async () => {
      render(<InvitePage referrerName={null} referrerAddress={ADDRESS} />);
      expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("invited you to join Decentraland");
      expect((await screen.findByTestId("avatar")).getAttribute("data-profile")).toBe(ADDRESS.toLowerCase());
    });
  });

  describe("and the inviter lookup failed", () => {
    it("should show the title alone and send the calls to action to the sign-in without a referrer", () => {
      render(<InvitePage referrerName={null} referrerAddress={null} />);
      expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("invited you to join Decentraland");
      expect(screen.getByRole("link", { name: "JOIN NOW" }).getAttribute("href")).toBe(referralUrl(""));
    });

    it("should draw no avatar", async () => {
      const { container } = render(<InvitePage referrerName={null} referrerAddress={null} />);
      // Long enough for the preloaded renderer to have mounted, had the page asked for it.
      await act(() => new Promise((resolve) => setTimeout(resolve, 20)));
      expect(avatarSlots(container)).toEqual([0, 0]);
    });
  });
});
