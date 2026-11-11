// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { renderToString } from "react-dom/server";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Hero } from "@dcl/one-ui/apps/sites/home/Hero";
import { ComeHangOut } from "@dcl/one-ui/apps/sites/home/ComeHangOut";
import { WhatsOn } from "@dcl/one-ui/apps/sites/home/WhatsOn";
import { Carousel } from "@dcl/one-ui/apps/sites/shared/Carousel";
import { downloadActions } from "../download/actions";

vi.hoisted(() =>
  Object.assign(window, { __vite_plugin_react_preamble_installed__: true }),
);
const labels = {
  previous: "Previous",
  next: "Next",
  goTo: (n: number) => `Slide ${n}`,
  position: (n: number, total: number) => `${n} of ${total}`,
};

describe("home sections", () => {
  beforeEach(() =>
    vi.stubGlobal(
      "matchMedia",
      vi.fn(() => ({
        matches: true,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      })),
    ),
  );
  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
  });
  it("renders both counters with the single solid white product badge", () => {
    const { actions, store } = downloadActions({ kind: "unknown" }, "+438K");
    const html = renderToString(
      <>
        <Hero
          title="Come Hang Out"
          imageAlt="Decentraland"
          actions={actions}
          mobile={{ title: "Anywhere", subtitle: "Mobile", store }}
        />
        <ComeHangOut title="Come Hang Out" actions={actions} store={store} />
      </>,
    );
    expect(html.match(/Total Downloads: \+438K/g)).toHaveLength(2);
    expect(html.match(/viewBox="0 0 18 18"/g)).toHaveLength(2);
    expect(html).not.toContain("linearGradient");
    expect(html).not.toContain("/download_success");
    expect(html).toContain("Decentraland_installer.exe");
    expect(html).toContain("Download on the App Store");
  });
  it("does not invent event cards when the backend has none", () => {
    expect(
      renderToString(
        <WhatsOn
          title="Live"
          cards={[]}
          labels={{ live: "LIVE", by: "by" }}
          viewAll={{ label: "All", href: "/whats-on" }}
        />,
      ),
    ).toBe("");
  });
  it("does not invent a player count when an event has no count", () => {
    const html = renderToString(
      <WhatsOn
        title="Live"
        cards={[
          {
            id: "event",
            title: "Real event",
            image: null,
            href: "/events?id=event",
          },
        ]}
        labels={{ live: "LIVE", by: "by" }}
        viewAll={{ label: "All", href: "/whats-on" }}
      />,
    );
    expect(html).toContain("Real event");
    expect(html).not.toContain(">0<");
  });
  it("moves with controls while keeping inactive links inert", () => {
    render(
      <>
        <h2 id="rituals">Rituals</h2>
        <Carousel
          labelledBy="rituals"
          items={["one", "two", "three"]}
          getKey={(s) => s}
          labels={labels}
          renderSlide={(s) => <a href={`/${s}`}>{s}</a>}
        />
      </>,
    );
    fireEvent.click(screen.getByRole("button", { name: "Next" }));
    expect(screen.getByRole("group", { name: "2 of 3" })).toBeTruthy();
    expect(document.querySelectorAll("[inert]").length).toBeGreaterThan(0);
  });
  it("does not emit invalid arithmetic for an empty carousel", () => {
    const html = renderToString(
      <Carousel
        labelledBy="empty"
        items={[]}
        getKey={(s) => String(s)}
        labels={labels}
        renderSlide={() => null}
      />,
    );
    expect(html).not.toContain("NaN");
    expect(html).not.toContain("Infinity");
  });
});
