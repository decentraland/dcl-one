// @vitest-environment jsdom
import { cleanup, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { AuthIdentity } from "@/data/lib/auth/types";
import { CatalystError, postJSON } from "@/data/lib/catalyst/client";
import type { EventProfile } from "@/data/lib/catalyst/places/event-management";
import { currentPath, fakeEventsApi, makeIdentity, MODERATOR, readsOf, renderAdminPage, type FakeEventsApi } from "./admin.test-utils";
import UsersAdminPage from "./UsersAdminPage";

const auth = vi.hoisted(() => ({ identity: null as AuthIdentity | null, isConnected: false }));
vi.hoisted(() => { Object.assign(window, { __vite_plugin_react_preamble_installed__: true }); });
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => auth }));
vi.mock("@/data/lib/auth/session", () => ({ getIdentity: () => auth.identity }));
vi.mock("../../components/auth/signin-store", () => ({ openSignIn: vi.fn() }));
vi.mock("../../components/chrome/SitesLayout", () => ({ default: ({ children }: { children: React.ReactNode }) => <>{children}</> }));
vi.mock("@/data/lib/catalyst/client", async (original) => ({
  ...await original<typeof import("@/data/lib/catalyst/client")>(),
  postJSON: vi.fn(), signedGetJSON: vi.fn(),
}));

const PATH = "/events/admin/users";
const NEW_WALLET = "0x4444444444444444444444444444444444444444";
const LIST = "/events/api/profiles/settings";
const saving = (wallet: string, permissions: string[]) => [`/events/api/profiles/${wallet.toLowerCase()}/settings`, { permissions }, expect.objectContaining({ method: "PATCH" })];
const address = (index: number) => `0x${String(index).padStart(40, "a")}`;

describe("when opening the Users page", () => {
  let identity: AuthIdentity;
  let profile: EventProfile;
  let api: FakeEventsApi;
  let user: ReturnType<typeof userEvent.setup>;

  beforeEach(() => {
    identity = makeIdentity();
    auth.identity = identity;
    auth.isConnected = true;
    profile = { user: MODERATOR, permissions: ["approve_any_event", "edit_any_profile"] };
    api = {
      profile,
      profiles: [
        profile,
        { user: address(1), permissions: ["approve_own_event", "edit_any_event", "edit_any_schedule"] },
        ...Array.from({ length: 10 }, (_, index) => ({ user: address(index + 2), permissions: [] })),
      ],
      events: [],
      failNext: {},
    };
    fakeEventsApi(api);
    // The accounts' names come from one Catalyst profiles read.
    vi.stubGlobal("fetch", vi.fn(async () => Response.json([{ avatars: [{ ethAddress: address(1), name: "Selene" }] }])));
    user = userEvent.setup();
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    vi.resetAllMocks();
  });

  describe("and the account holds the profile permission but is not a moderator", () => {
    beforeEach(() => {
      profile.permissions = ["edit_any_profile"];
      api.profiles = "forbidden";
    });

    it("should redirect to /events", async () => {
      renderAdminPage(<UsersAdminPage />, PATH);
      await waitFor(() => expect(currentPath()).toBe("/events"));
    });
  });

  describe("and the list of grants fails for a reason other than a refusal", () => {
    beforeEach(() => { api.failNext.profiles = new CatalystError("Grants unavailable", LIST, 503); });

    it("should offer a retry instead of redirecting, and list the accounts once it succeeds", async () => {
      renderAdminPage(<UsersAdminPage />, PATH);
      expect((await screen.findByRole("alert")).textContent).toBe("Grants unavailable");
      expect(currentPath()).toBe(PATH);
      await user.click(screen.getByRole("button", { name: "Retry" }));
      expect(await screen.findByRole("table", { name: "Users" })).toBeTruthy();
      expect(readsOf(LIST)).toBe(2);
    });
  });

  describe("and the account has no profile permission", () => {
    beforeEach(() => { profile.permissions = ["approve_any_event"]; });

    it("should redirect to /events without reading other accounts", async () => {
      renderAdminPage(<UsersAdminPage />, PATH);
      await waitFor(() => expect(currentPath()).toBe("/events"));
      expect(readsOf(LIST)).toBe(0);
    });
  });

  it("should list the accounts with a check per grant and the account's name", async () => {
    renderAdminPage(<UsersAdminPage />, PATH);
    const table = await screen.findByRole("table", { name: "Users" });
    expect(within(table).getAllByRole("columnheader").map((cell) => cell.textContent)).toEqual([
      "User", "Approve Own Events", "Approve Events", "Edit Events", "Edit Users",
    ]);
    const row = (await within(table).findByRole("button", { name: `${address(1)} (Selene)` })).closest("tr")!;
    expect(within(row).getAllByRole("img", { name: "Granted" })).toHaveLength(2);
    expect(screen.getByText("1–10 of 12")).toBeTruthy();
  });

  it("should show the admin bar with Users current", async () => {
    renderAdminPage(<UsersAdminPage />, PATH);
    const bar = await screen.findByRole("navigation", { name: "Event administration" });
    expect((await within(bar).findByRole("link", { name: "Users" })).getAttribute("aria-current")).toBe("page");
    expect(within(bar).getByRole("link", { name: "Pending Events" })).toBeTruthy();
  });

  it("should show the next page of accounts", async () => {
    renderAdminPage(<UsersAdminPage />, PATH);
    await screen.findByText("1–10 of 12");
    await user.click(screen.getByRole("button", { name: "Next page" }));
    expect(screen.getByText("11–12 of 12")).toBeTruthy();
  });

  it("should filter the accounts by name", async () => {
    renderAdminPage(<UsersAdminPage />, PATH);
    await screen.findByText("1–10 of 12");
    await user.type(screen.getByRole("searchbox", { name: "Search" }), "selene");
    expect(screen.getByText("1–1 of 1")).toBeTruthy();
  });

  it("should say when no account matches the search", async () => {
    renderAdminPage(<UsersAdminPage />, PATH);
    await screen.findByText("1–10 of 12");
    await user.type(screen.getByRole("searchbox", { name: "Search" }), "0xnothing");
    expect(screen.getByText("No matching users")).toBeTruthy();
  });

  it("should change the rows per page without reading the accounts again", async () => {
    renderAdminPage(<UsersAdminPage />, PATH);
    await screen.findByText("1–10 of 12");
    await user.selectOptions(screen.getByRole("combobox", { name: "Rows per page:" }), "25");
    expect(screen.getByText("1–12 of 12")).toBeTruthy();
    expect(readsOf(LIST)).toBe(1);
  });

  describe("and no account holds a grant", () => {
    beforeEach(() => { api.profiles = []; });

    it("should say no admins are configured", async () => {
      renderAdminPage(<UsersAdminPage />, PATH);
      expect(await screen.findByText("No admins configured")).toBeTruthy();
    });
  });

  it("should keep Save Changes disabled until the wallet address is valid", async () => {
    renderAdminPage(<UsersAdminPage />, PATH);
    await user.click(await screen.findByRole("button", { name: "+ Add User" }));
    const dialog = screen.getByRole("dialog", { name: "Add User" });
    const save = within(dialog).getByRole<HTMLButtonElement>("button", { name: "Save Changes" });
    expect(save.disabled).toBe(true);
    await user.type(within(dialog).getByRole("textbox", { name: "Wallet Address" }), "0x123");
    expect(within(dialog).getByText("Enter a valid Ethereum address")).toBeTruthy();
    expect(save.disabled).toBe(true);
    await user.click(save);
    expect(postJSON).not.toHaveBeenCalled();
  });

  it("should add a wallet without settings with only the chosen grants and refresh the list", async () => {
    renderAdminPage(<UsersAdminPage />, PATH);
    await user.click(await screen.findByRole("button", { name: "+ Add User" }));
    const dialog = screen.getByRole("dialog", { name: "Add User" });
    await user.type(within(dialog).getByRole("textbox", { name: "Wallet Address" }), NEW_WALLET);
    await user.click(within(dialog).getByRole("switch", { name: "Approve Events" }));
    await user.click(within(dialog).getByRole("button", { name: "Save Changes" }));
    expect(await screen.findByText("Permissions updated")).toBeTruthy();
    expect(postJSON).toHaveBeenCalledWith(...saving(NEW_WALLET, ["approve_any_event"]));
    expect(screen.queryByRole("dialog")).toBeNull();
    await waitFor(() => expect(readsOf(LIST)).toBe(2));
  });

  describe("and the typed wallet is already listed", () => {
    // Typed in another case than the list holds it, as a pasted checksummed address would be.
    const listed = address(1).toUpperCase().replace("0X", "0x");

    it("should turn the dialog into Edit User with the wallet's grants, and keep them all on save", async () => {
      renderAdminPage(<UsersAdminPage />, PATH);
      await user.click(await screen.findByRole("button", { name: "+ Add User" }));
      await user.type(within(screen.getByRole("dialog")).getByRole("textbox", { name: "Wallet Address" }), listed);
      const dialog = screen.getByRole("dialog", { name: "Edit User" });
      expect(within(dialog).getByRole<HTMLInputElement>("switch", { name: "Approve Own Events" }).checked).toBe(true);
      expect(within(dialog).getByRole<HTMLInputElement>("switch", { name: "Edit Events" }).checked).toBe(true);
      await user.click(within(dialog).getByRole("switch", { name: "Approve Events" }));
      await user.click(within(dialog).getByRole("button", { name: "Save Changes" }));
      await screen.findByText("Permissions updated");
      expect(postJSON).toHaveBeenCalledWith(...saving(listed, ["approve_own_event", "edit_any_event", "edit_any_schedule", "approve_any_event"]));
    });

    it("should return to Add User with the earlier choices when the address changes again", async () => {
      renderAdminPage(<UsersAdminPage />, PATH);
      await user.click(await screen.findByRole("button", { name: "+ Add User" }));
      const dialog = screen.getByRole("dialog", { name: "Add User" });
      await user.click(within(dialog).getByRole("switch", { name: "Approve Events" }));
      const wallet = within(dialog).getByRole("textbox", { name: "Wallet Address" });
      await user.type(wallet, listed);
      expect(screen.getByRole("dialog", { name: "Edit User" })).toBe(dialog);
      await user.type(wallet, "{Backspace}");
      expect(screen.getByRole("dialog", { name: "Add User" })).toBe(dialog);
      expect(within(dialog).getByRole<HTMLInputElement>("switch", { name: "Approve Events" }).checked).toBe(true);
      expect(within(dialog).getByRole<HTMLInputElement>("switch", { name: "Approve Own Events" }).checked).toBe(false);
    });
  });

  describe("and the typed wallet is not listed but holds grants", () => {
    beforeEach(() => {
      api.settings = { [NEW_WALLET]: { user: NEW_WALLET, permissions: ["approve_own_event", "edit_any_schedule"] } };
    });

    it("should keep the wallet's grants and add the chosen one", async () => {
      renderAdminPage(<UsersAdminPage />, PATH);
      await user.click(await screen.findByRole("button", { name: "+ Add User" }));
      const dialog = screen.getByRole("dialog", { name: "Add User" });
      await user.type(within(dialog).getByRole("textbox", { name: "Wallet Address" }), NEW_WALLET);
      await user.click(within(dialog).getByRole("switch", { name: "Approve Events" }));
      await user.click(within(dialog).getByRole("button", { name: "Save Changes" }));
      await screen.findByText("Permissions updated");
      expect(postJSON).toHaveBeenCalledWith(...saving(NEW_WALLET, ["approve_own_event", "edit_any_schedule", "approve_any_event"]));
    });

    it("should still keep them when the list starts naming the wallet after it was typed", async () => {
      const { client } = renderAdminPage(<UsersAdminPage />, PATH);
      await user.click(await screen.findByRole("button", { name: "+ Add User" }));
      const dialog = screen.getByRole("dialog", { name: "Add User" });
      await user.type(within(dialog).getByRole("textbox", { name: "Wallet Address" }), NEW_WALLET);
      await user.click(within(dialog).getByRole("switch", { name: "Approve Events" }));
      api.profiles = [...(api.profiles as EventProfile[]), api.settings![NEW_WALLET]!];
      await client.invalidateQueries();
      await waitFor(() => expect(readsOf(LIST)).toBe(2));
      await user.click(within(dialog).getByRole("button", { name: "Save Changes" }));
      await screen.findByText("Permissions updated");
      expect(postJSON).toHaveBeenCalledWith(...saving(NEW_WALLET, ["approve_own_event", "edit_any_schedule", "approve_any_event"]));
    });
  });

  describe("and reading the typed wallet's grants fails", () => {
    beforeEach(() => { api.failNext.settings = new Error("Settings unavailable"); });

    it("should save nothing and report the failure in the dialog", async () => {
      renderAdminPage(<UsersAdminPage />, PATH);
      await user.click(await screen.findByRole("button", { name: "+ Add User" }));
      const dialog = screen.getByRole("dialog", { name: "Add User" });
      await user.type(within(dialog).getByRole("textbox", { name: "Wallet Address" }), NEW_WALLET);
      await user.click(within(dialog).getByRole("switch", { name: "Approve Events" }));
      await user.click(within(dialog).getByRole("button", { name: "Save Changes" }));
      expect((await within(dialog).findByRole("alert")).textContent).toBe("Unable to read this wallet's current permissions. Please try again.");
      expect(postJSON).not.toHaveBeenCalled();
      expect(within(dialog).getByRole<HTMLButtonElement>("button", { name: "Save Changes" }).disabled).toBe(false);
    });
  });

  describe("and the signed-in account types its own wallet while not listed", () => {
    beforeEach(() => {
      api.profiles = [{ user: address(1), permissions: ["approve_own_event"] }];
      api.settings = { [MODERATOR]: profile };
    });

    it("should lock its own user administration and keep it and its other grants on save", async () => {
      renderAdminPage(<UsersAdminPage />, PATH);
      await user.click(await screen.findByRole("button", { name: "+ Add User" }));
      const dialog = screen.getByRole("dialog", { name: "Add User" });
      await user.type(within(dialog).getByRole("textbox", { name: "Wallet Address" }), MODERATOR);
      const own = within(dialog).getByRole<HTMLInputElement>("switch", { name: "Edit Users" });
      expect(own.disabled).toBe(true);
      expect(own.checked).toBe(true);
      expect(within(dialog).getByText("You cannot remove your own admin permission")).toBeTruthy();
      await user.click(within(dialog).getByRole("button", { name: "Save Changes" }));
      await screen.findByText("Permissions updated");
      expect(postJSON).toHaveBeenCalledWith(...saving(MODERATOR, ["approve_any_event", "edit_any_profile"]));
    });
  });

  describe("and saving fails", () => {
    beforeEach(() => { api.failNext.save = new Error("Permissions not saved"); });

    it("should keep the dialog and its choices and report the failure", async () => {
      renderAdminPage(<UsersAdminPage />, PATH);
      await user.click(await screen.findByRole("button", { name: "+ Add User" }));
      const dialog = screen.getByRole("dialog", { name: "Add User" });
      await user.type(within(dialog).getByRole("textbox", { name: "Wallet Address" }), NEW_WALLET);
      await user.click(within(dialog).getByRole("switch", { name: "Edit Events" }));
      await user.click(within(dialog).getByRole("button", { name: "Save Changes" }));
      expect((await within(dialog).findByRole("alert")).textContent).toBe("Permissions not saved");
      expect(within(dialog).getByRole("alert").dataset.tone).toBe("error");
      expect(within(dialog).getByRole<HTMLInputElement>("switch", { name: "Edit Events" }).checked).toBe(true);
      expect(screen.queryByText("Permissions updated")).toBeNull();
      expect(readsOf(LIST)).toBe(1);
    });
  });

  it("should edit a user from its row and keep the grants the dialog does not show", async () => {
    renderAdminPage(<UsersAdminPage />, PATH);
    await user.click(await screen.findByRole("button", { name: `${address(1)} (Selene)` }));
    const dialog = screen.getByRole("dialog", { name: "Edit User" });
    expect(within(dialog).getByText("Selene")).toBeTruthy();
    expect(within(dialog).queryByRole("textbox", { name: "Wallet Address" })).toBeNull();
    expect(within(dialog).getByRole<HTMLInputElement>("switch", { name: "Approve Own Events" }).checked).toBe(true);
    await user.click(within(dialog).getByRole("switch", { name: "Edit Events" }));
    await user.click(within(dialog).getByRole("button", { name: "Save Changes" }));
    await screen.findByText("Permissions updated");
    expect(postJSON).toHaveBeenCalledWith(...saving(address(1), ["approve_own_event", "edit_any_schedule"]));
  });

  describe("and another moderator changed the account's grants after the list loaded", () => {
    beforeEach(() => {
      api.settings = { [address(1)]: { user: address(1), permissions: ["approve_own_event", "edit_any_event", "edit_any_schedule", "approve_any_event"] } };
    });

    it("should keep the grant the other moderator added when a switch is turned off", async () => {
      renderAdminPage(<UsersAdminPage />, PATH);
      await user.click(await screen.findByRole("button", { name: `${address(1)} (Selene)` }));
      const dialog = screen.getByRole("dialog", { name: "Edit User" });
      expect(within(dialog).getByRole<HTMLInputElement>("switch", { name: "Approve Events" }).checked).toBe(false);
      await user.click(within(dialog).getByRole("switch", { name: "Edit Events" }));
      await user.click(within(dialog).getByRole("button", { name: "Save Changes" }));
      await screen.findByText("Permissions updated");
      expect(postJSON).toHaveBeenCalledWith(...saving(address(1), ["approve_own_event", "edit_any_schedule", "approve_any_event"]));
    });

    it("should add a switch turned on to the account's current grants", async () => {
      api.settings = { [address(1)]: { user: address(1), permissions: ["approve_own_event", "edit_any_schedule"] } };
      renderAdminPage(<UsersAdminPage />, PATH);
      await user.click(await screen.findByRole("button", { name: `${address(1)} (Selene)` }));
      const dialog = screen.getByRole("dialog", { name: "Edit User" });
      await user.click(within(dialog).getByRole("switch", { name: "Edit Users" }));
      await user.click(within(dialog).getByRole("button", { name: "Save Changes" }));
      await screen.findByText("Permissions updated");
      // Edit Events, removed by the other moderator and left untouched here, stays removed.
      expect(postJSON).toHaveBeenCalledWith(...saving(address(1), ["approve_own_event", "edit_any_schedule", "edit_any_profile"]));
    });
  });

  describe("and reading the edited account's grants fails", () => {
    beforeEach(() => { api.failNext.settings = new Error("Settings unavailable"); });

    it("should save nothing and report the failure in the dialog", async () => {
      renderAdminPage(<UsersAdminPage />, PATH);
      await user.click(await screen.findByRole("button", { name: `${address(1)} (Selene)` }));
      const dialog = screen.getByRole("dialog", { name: "Edit User" });
      await user.click(within(dialog).getByRole("switch", { name: "Edit Events" }));
      await user.click(within(dialog).getByRole("button", { name: "Save Changes" }));
      expect((await within(dialog).findByRole("alert")).textContent).toBe("Unable to read this wallet's current permissions. Please try again.");
      expect(postJSON).not.toHaveBeenCalled();
      expect(within(dialog).getByRole<HTMLInputElement>("switch", { name: "Edit Events" }).checked).toBe(false);
    });
  });

  it("should not let the signed-in account remove its own user administration", async () => {
    renderAdminPage(<UsersAdminPage />, PATH);
    await user.click(await screen.findByRole("button", { name: MODERATOR }));
    const dialog = screen.getByRole("dialog", { name: "Edit User" });
    expect(within(dialog).getByRole<HTMLInputElement>("switch", { name: "Edit Users" }).disabled).toBe(true);
    expect(within(dialog).getByText("You cannot remove your own admin permission")).toBeTruthy();
  });

  describe("and the server keeps a moderator's effective grants", () => {
    beforeEach(() => {
      vi.mocked(postJSON).mockResolvedValue({ ok: true, data: { user: MODERATOR, permissions: ["approve_any_event", "edit_any_profile"] } });
    });

    it("should say the server kept them instead of claiming the change", async () => {
      renderAdminPage(<UsersAdminPage />, PATH);
      await user.click(await screen.findByRole("button", { name: MODERATOR }));
      const dialog = screen.getByRole("dialog", { name: "Edit User" });
      await user.click(within(dialog).getByRole("switch", { name: "Approve Events" }));
      await user.click(within(dialog).getByRole("button", { name: "Save Changes" }));
      expect(await screen.findByText(/The server kept the account's effective permissions/)).toBeTruthy();
    });
  });

  it("should close the editor without saving on Cancel", async () => {
    renderAdminPage(<UsersAdminPage />, PATH);
    await user.click(await screen.findByRole("button", { name: "+ Add User" }));
    await user.click(within(screen.getByRole("dialog", { name: "Add User" })).getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(postJSON).not.toHaveBeenCalled();
  });

  // The Sites client renders in StrictMode, which runs each effect's cleanup and setup once more after
  // mounting; a save's result must still close the dialog, report and clear the busy state.
  describe("in StrictMode, as the Sites client renders", () => {
    async function addApprover() {
      renderAdminPage(<UsersAdminPage />, PATH, { strict: true });
      await user.click(await screen.findByRole("button", { name: "+ Add User" }));
      const dialog = screen.getByRole("dialog", { name: "Add User" });
      await user.type(within(dialog).getByRole("textbox", { name: "Wallet Address" }), NEW_WALLET);
      await user.click(within(dialog).getByRole("switch", { name: "Approve Events" }));
      await user.click(within(dialog).getByRole("button", { name: "Save Changes" }));
      return dialog;
    }

    it("should close the dialog and report a saved change", async () => {
      await addApprover();
      expect(await screen.findByText("Permissions updated")).toBeTruthy();
      expect(screen.queryByRole("dialog")).toBeNull();
    });

    it("should keep the dialog with the failure and let it be saved again", async () => {
      api.failNext.save = new Error("Permissions not saved");
      const dialog = await addApprover();
      expect((await within(dialog).findByRole("alert")).textContent).toBe("Permissions not saved");
      const save = within(dialog).getByRole<HTMLButtonElement>("button", { name: "Save Changes" });
      expect(save.disabled).toBe(false);
      await user.click(save);
      expect(await screen.findByText("Permissions updated")).toBeTruthy();
    });
  });
});
