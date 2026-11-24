// @vitest-environment jsdom
import "@testing-library/jest-dom/vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { StrictMode } from "react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, describe, expect, it, vi, type Mock } from "vitest";

// The library component is transformed with React Refresh outside a route module; its preamble is stubbed.
vi.hoisted(() => {
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshSig$: () => (type: unknown) => type,
    $RefreshReg$: () => {},
  });
});

const signIn = vi.hoisted(() => ({ openSignIn: vi.fn() }));
vi.mock("../../components/auth/signin-store", () => ({ openSignIn: signIn.openSignIn }));

import CreateCommunity from "./CreateCommunity";
import type { CreateFn } from "./submit";

const trackCtx = { sid: "sid-1", story: "landings/create-community" as const, variant: "wizard" };

// `null` renders the page signed out.
function setup(
  create: Mock<CreateFn> | null = vi.fn<CreateFn>().mockResolvedValue({ id: "c 1", name: "Builders", privacy: "public", visibility: "all" }),
) {
  const router = createMemoryRouter(
    [
      { path: "/communities/new", element: <CreateCommunity trackCtx={trackCtx} create={create ?? undefined} signedOut={!create} track={vi.fn()} /> },
      { path: "/social", element: <p>social home</p> },
      { path: "/social/communities/:id", element: <p>community page</p> },
    ],
    { initialEntries: ["/communities/new"] },
  );
  // StrictMode runs effects twice, as the dev server does: the kept draft must survive the second run.
  render(
    <StrictMode>
      <RouterProvider router={router} />
    </StrictMode>,
  );
  return { router, create };
}

afterEach(() => {
  cleanup();
  sessionStorage.clear();
  vi.resetAllMocks();
});

describe("CreateCommunity", () => {
  describe("when a community is created", () => {
    it("should open the new community's page", async () => {
      const { router } = setup();
      await userEvent.type(screen.getByRole("textbox", { name: /Community name/i }), "Builders");
      await userEvent.click(screen.getByRole("button", { name: "Create" }));
      expect(await screen.findByText("community page")).toBeInTheDocument();
      expect(router.state.location.pathname).toBe("/social/communities/c%201");
    });
  });

  describe("when the visitor cancels", () => {
    it("should go to the social page without creating", async () => {
      const { router, create } = setup();
      await userEvent.click(screen.getByRole("button", { name: "Cancel" }));
      expect(await screen.findByText("social home")).toBeInTheDocument();
      expect(router.state.location.pathname).toBe("/social");
      expect(create).not.toHaveBeenCalled();
    });
  });

  describe("when the create fails", () => {
    it("should stay on the form and show the error", async () => {
      const { router } = setup(vi.fn().mockRejectedValue(new Error("Create community failed (500): boom")));
      await userEvent.type(screen.getByRole("textbox", { name: /Community name/i }), "Builders");
      await userEvent.click(screen.getByRole("button", { name: "Create" }));
      expect(await screen.findByRole("alert")).toHaveTextContent("boom");
      expect(router.state.location.pathname).toBe("/communities/new");
    });
  });

  describe("when the visitor comes back from signing in", () => {
    it("should restore the kept form", async () => {
      sessionStorage.setItem("dcl.create-community.draft", JSON.stringify({ name: "Builders", description: "We build" }));
      await act(async () => {
        setup();
      });
      expect(screen.getByRole("textbox", { name: /Community name/i })).toHaveValue("Builders");
      expect(screen.getByRole("textbox", { name: "Description" })).toHaveValue("We build");
      expect(sessionStorage.getItem("dcl.create-community.draft")).toBeNull();
    });
  });

  describe("when a signed-out visitor leaves the description blank and signs in", () => {
    it("should keep the description blank across the round trip", async () => {
      setup(null);
      await userEvent.type(screen.getByRole("textbox", { name: /Community name/i }), "Builders");
      await userEvent.click(screen.getByRole("button", { name: "Create" }));
      expect(signIn.openSignIn).toHaveBeenCalledTimes(1);
      expect(JSON.parse(sessionStorage.getItem("dcl.create-community.draft")!)).toMatchObject({ name: "Builders", description: "" });
      cleanup();
      await act(async () => {
        setup();
      });
      expect(screen.getByRole("textbox", { name: /Community name/i })).toHaveValue("Builders");
      expect(screen.getByRole("textbox", { name: "Description" })).toHaveValue("");
    });
  });
});
