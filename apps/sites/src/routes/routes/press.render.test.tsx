// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { createRoutesStub } from "react-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { SitesLocaleContext, type SitesLocale } from "@dcl/one-ui/apps/sites";
import Press from "./press";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

const PressStub = createRoutesStub([{ path: "/press", Component: Press }]);

function PressRouter() {
  const [client] = useState(() => new QueryClient());
  return (
    <QueryClientProvider client={client}>
      <PressStub initialEntries={["/press"]} />
    </QueryClientProvider>
  );
}

function PressWithLanguagePicker() {
  const [locale, onSelect] = useState<SitesLocale>("en");

  return (
    <SitesLocaleContext.Provider value={{ locale, onSelect }}>
      <PressRouter />
    </SitesLocaleContext.Provider>
  );
}

const FOOTER_CHANGE_LANGUAGE: Record<string, string> = {
  English: "Change language",
  Español: "Cambiar idioma",
  Français: "Changer de langue",
  中文: "更改语言",
  한국어: "언어 변경",
  日本語: "言語を変更",
};

describe("/press", () => {
  afterEach(() => {
    cleanup();
    document.cookie = "lang=; path=/; max-age=0";
  });

  describe("when a visitor opens the page", () => {
    beforeEach(() => {
      render(<PressRouter />);
    });

    it("should show the Sites top bar with no section selected", () => {
      const banner = within(screen.getByRole("banner", { name: "Decentraland" }));
      expect(banner.getByRole("button", { name: "Open menu" })).toBeTruthy();
      expect(banner.queryByRole("button", { name: "Toggle navigation menu" })).toBeNull();
      expect(document.querySelector('[aria-current="page"]')).toBeNull();
    });

    it("should show the English press kit in the main landmark", () => {
      expect(
        within(screen.getByRole("main")).getByRole("heading", {
          level: 1,
          name: "Decentraland Press Kit",
        }),
      ).toBeTruthy();
    });

    it("should link to the press mailbox", () => {
      expect(
        within(screen.getByRole("main"))
          .getByRole("link", {
            name: "press@decentraland.org",
          })
          .getAttribute("href"),
      ).toBe("mailto:press@decentraland.org");
    });

    it("should open the media kit in a separate tab", () => {
      expect(
        within(screen.getByRole("main")).getByRole("link", {
          name: "Download Press Kit",
        }),
      ).toMatchObject({
        href: "https://dcl.gg/press",
        target: "_blank",
        rel: "noopener noreferrer",
      });
    });
  });

  describe.each([
    [
      "English",
      "Decentraland Press Kit",
      "Download Press Kit",
      "Press Kit | Decentraland",
    ],
    ["Español", "Kit de Prensa de Decentraland", "Descargar Kit de Prensa"],
    ["Français", "Kit Presse Decentraland", "Télécharger le Kit de Presse"],
    ["中文", "Decentraland 新闻资料包", "下载新闻资料包"],
    ["한국어", "Decentraland 프레스 키트", "프레스 키트 다운로드"],
    ["日本語", "Decentralandプレスキット", "プレスキットをダウンロード"],
  ])(
    "when a visitor picks %s from the footer",
    (language, heading, download, pageTitle = heading) => {
      beforeEach(async () => {
        const user = userEvent.setup();
        render(<PressWithLanguagePicker />);
        await user.click(
          screen.getByRole("button", { name: /change language/i }),
        );
        await user.click(screen.getByRole("option", { name: language }));
      });

      it("should translate the heading and media kit link", () => {
        const main = within(screen.getByRole("main"));
        expect(
          main.getByRole("heading", { level: 1, name: heading }),
        ).toBeTruthy();
        expect(main.getByRole("link", { name: download })).toBeTruthy();
      });

      it("should translate the footer", () => {
        expect(
          within(screen.getByRole("contentinfo")).getByRole("button", {
            name: `${FOOTER_CHANGE_LANGUAGE[language]}: ${language}`,
          }),
        ).toBeTruthy();
      });

      it("should update the browser title", () => {
        expect(document.title).toBe(pageTitle);
      });
    },
  );
});
