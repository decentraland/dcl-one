// @vitest-environment jsdom
import { cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from "vitest";

vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});

import type { SubscriptionDetails } from "@/data/lib/account/notifications";

import { NotificationGroupAccordion } from "./NotificationGroupAccordion";

afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("NotificationGroupAccordion", () => {
  let details: SubscriptionDetails;
  let onToggleType: Mock<(type: string, enabled: boolean) => void>;
  let user: ReturnType<typeof userEvent.setup>;
  let header: HTMLElement;

  function show(disabled: boolean) {
    render(
      <NotificationGroupAccordion
        group="marketplace"
        details={details}
        disabled={disabled}
        onToggleType={onToggleType}
      />,
    );
    header = screen.getByRole("button", { name: "Marketplace" });
  }

  beforeEach(() => {
    details = {
      ignore_all_email: false,
      ignore_all_in_app: false,
      message_type: { item_sold: { email: true, in_app: true } },
    };
    onToggleType = vi.fn();
    user = userEvent.setup();
  });

  describe("when it is enabled", () => {
    beforeEach(() => {
      show(false);
    });

    describe("and it has just rendered", () => {
      it("should start closed, with its types out of reach", () => {
        expect(header.getAttribute("aria-expanded")).toBe("false");
        expect(screen.queryByRole("switch")).toBeNull();
        const region = document.getElementById(header.getAttribute("aria-controls")!)!;
        expect(region.hidden).toBe(true);
      });
    });

    describe("and the header is clicked", () => {
      let region: HTMLElement;

      beforeEach(async () => {
        region = document.getElementById(header.getAttribute("aria-controls")!)!;
        await user.click(header);
      });

      it("should expand and reveal its region", () => {
        expect(header.getAttribute("aria-expanded")).toBe("true");
        expect(region.hidden).toBe(false);
      });

      it("should show each type's switch with its subscription state", () => {
        expect((screen.getByRole("switch", { name: "Item Sold" }) as HTMLInputElement).checked).toBe(true);
        expect((screen.getByRole("switch", { name: "Bid Received" }) as HTMLInputElement).checked).toBe(false);
      });

      describe("and the header is clicked again", () => {
        beforeEach(async () => {
          await user.click(header);
        });

        it("should close and hide its types", () => {
          expect(header.getAttribute("aria-expanded")).toBe("false");
          expect(screen.queryByRole("switch")).toBeNull();
        });
      });
    });

    describe("and it is opened from the keyboard and a type is toggled", () => {
      beforeEach(async () => {
        header.focus();
        await user.keyboard("{Enter}");
        await user.click(screen.getByRole("switch", { name: "Bid Received" }));
      });

      it("should report the toggled type", () => {
        expect(onToggleType).toHaveBeenCalledWith("bid_received", true);
      });
    });
  });

  describe("when it is disabled and opened", () => {
    let item: HTMLInputElement;

    beforeEach(async () => {
      show(true);
      await user.click(header);
      item = screen.getByRole("switch", { name: "Item Sold" }) as HTMLInputElement;
    });

    it("should lock its types", () => {
      expect(item.disabled).toBe(true);
    });

    describe("and a locked type is clicked", () => {
      beforeEach(async () => {
        await user.click(item);
      });

      it("should not report anything", () => {
        expect(onToggleType).not.toHaveBeenCalled();
      });

      it("should keep the type's state", () => {
        expect(item.checked).toBe(true);
      });
    });
  });
});
