// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { CREATOR_FUND_CONTACT_EMAIL, CREATOR_FUND_CONTACT_PROMPT, CREATOR_FUND_FAQ } from "../creator-fund/faq";
import { HELP_FAQ, HELP_SUPPORT_UPDATES } from "./content";
import { HelpView, type HelpViewProps } from "./HelpView";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

const UP = "https://up.example/about";
const DOWN = "https://down.example/about";
const SERVICES = [
  { name: "Up Service", url: UP },
  { name: "Down Service", url: DOWN },
];

let fetchMock: ReturnType<typeof vi.fn>;

beforeEach(() => {
  fetchMock = vi.fn(async (url: string) => new Response(null, { status: url === UP ? 200 : 503 }));
  vi.stubGlobal("fetch", fetchMock);
});

afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
  delete (window as Window & { Intercom?: unknown }).Intercom;
});

function renderView(props: Partial<HelpViewProps> = {}) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  return render(
    <QueryClientProvider client={client}>
      <HelpView services={SERVICES} statusPageUrl="https://status.example/" intercomAppId="" {...props} />
    </QueryClientProvider>,
  );
}

const panel = () => screen.getByRole("tabpanel");

describe("HelpView tabs", () => {
  it("opens on the FAQ tab", () => {
    renderView();
    expect(screen.getByRole("tab", { name: "FAQ" }).getAttribute("aria-selected")).toBe("true");
    expect(within(panel()).getByRole("heading", { name: "Frequently Asked Questions" })).toBeTruthy();
    expect(panel().getAttribute("aria-labelledby")).toBe(screen.getByRole("tab", { name: "FAQ" }).id);
  });

  it("switches section on click", async () => {
    renderView();
    await userEvent.setup().click(screen.getByRole("tab", { name: "SUPPORT UPDATES" }));
    expect(screen.getByRole("tab", { name: "SUPPORT UPDATES" }).getAttribute("aria-selected")).toBe("true");
    expect(screen.getByRole("tab", { name: "FAQ" }).getAttribute("aria-selected")).toBe("false");
    expect(within(panel()).getByRole("heading", { name: "Support Updates" })).toBeTruthy();
  });

  it("moves and selects with the arrow keys, Home and End, wrapping at the ends", async () => {
    const user = userEvent.setup();
    renderView();
    await user.click(screen.getByRole("tab", { name: "FAQ" }));

    await user.keyboard("{ArrowRight}");
    const creatorFund = screen.getByRole("tab", { name: "CREATOR FUND" });
    expect(document.activeElement).toBe(creatorFund);
    expect(creatorFund.getAttribute("aria-selected")).toBe("true");
    expect(within(panel()).getByRole("heading", { name: "Creator Fund" })).toBeTruthy();

    await user.keyboard("{End}");
    expect(document.activeElement).toBe(screen.getByRole("tab", { name: "SUPPORT UPDATES" }));

    await user.keyboard("{ArrowDown}");
    expect(document.activeElement).toBe(screen.getByRole("tab", { name: "FAQ" }));

    await user.keyboard("{ArrowLeft}");
    expect(document.activeElement).toBe(screen.getByRole("tab", { name: "SUPPORT UPDATES" }));

    await user.keyboard("{Home}");
    expect(screen.getByRole("tab", { name: "FAQ" }).getAttribute("aria-selected")).toBe("true");
  });

  it("keeps only the selected tab in the Tab order", () => {
    renderView();
    const tabIndexes = screen.getAllByRole("tab").map((tab) => tab.tabIndex);
    expect(tabIndexes).toEqual([0, -1, -1]);
  });
});

describe("HelpView accordions", () => {
  it("lists every FAQ question, closed", () => {
    renderView();
    const questions = within(panel()).getAllByRole("button");
    expect(questions.map((q) => q.textContent)).toEqual(HELP_FAQ.map((item) => item.question));
    expect(questions.every((q) => q.getAttribute("aria-expanded") === "false")).toBe(true);
  });

  it("opens a question to show its answer and closes it on a second click", async () => {
    const user = userEvent.setup();
    renderView();
    const question = screen.getByRole("button", { name: HELP_FAQ[0].question });
    await user.click(question);
    expect(question.getAttribute("aria-expanded")).toBe("true");
    const answer = document.getElementById(question.getAttribute("aria-controls")!)!;
    expect(answer.hidden).toBe(false);
    expect(answer.textContent).toBe(HELP_FAQ[0].answer);

    await user.click(question);
    expect(question.getAttribute("aria-expanded")).toBe("false");
    expect(answer.hidden).toBe(true);
  });

  it("closes the open question when another opens", async () => {
    const user = userEvent.setup();
    renderView();
    const first = screen.getByRole("button", { name: HELP_FAQ[0].question });
    const second = screen.getByRole("button", { name: HELP_FAQ[1].question });
    await user.click(first);
    await user.click(second);
    expect(first.getAttribute("aria-expanded")).toBe("false");
    expect(second.getAttribute("aria-expanded")).toBe("true");
  });

  it("lists every support update", async () => {
    renderView();
    await userEvent.setup().click(screen.getByRole("tab", { name: "SUPPORT UPDATES" }));
    expect(within(panel()).getAllByRole("button").map((q) => q.textContent)).toEqual(
      HELP_SUPPORT_UPDATES.map((item) => item.question),
    );
  });
});

describe("HelpView Creator Fund tab", () => {
  it("lists the Creator Fund questions with their answers and ends with the contact address", async () => {
    const user = userEvent.setup();
    renderView();
    await user.click(screen.getByRole("tab", { name: "CREATOR FUND" }));
    const questions = within(panel()).getAllByRole("button");
    expect(questions.map((q) => q.textContent)).toEqual(CREATOR_FUND_FAQ.map((entry) => entry.question));
    for (const [index, entry] of CREATOR_FUND_FAQ.entries()) {
      expect(document.getElementById(questions[index].getAttribute("aria-controls")!)!.textContent).toBe(entry.answer);
    }
    const contact = within(panel()).getByRole("link", { name: CREATOR_FUND_CONTACT_EMAIL });
    expect(contact.getAttribute("href")).toBe(`mailto:${CREATOR_FUND_CONTACT_EMAIL}`);
    expect(contact.parentElement!.textContent).toBe(`${CREATOR_FUND_CONTACT_PROMPT} ${CREATOR_FUND_CONTACT_EMAIL}`);
  });

  it("reports opening a question by its tracking name, and not closing it", async () => {
    const user = userEvent.setup();
    const onCreatorFundClick = vi.fn();
    renderView({ onCreatorFundClick });
    await user.click(screen.getByRole("tab", { name: "CREATOR FUND" }));
    const question = screen.getByRole("button", { name: CREATOR_FUND_FAQ[0].question });
    await user.click(question);
    await user.click(question);
    expect(onCreatorFundClick.mock.calls).toEqual([[CREATOR_FUND_FAQ[0].trackingName]]);
  });

  it("reports a click on the contact address", async () => {
    const user = userEvent.setup();
    const onCreatorFundClick = vi.fn();
    renderView({ onCreatorFundClick });
    await user.click(screen.getByRole("tab", { name: "CREATOR FUND" }));
    const contact = screen.getByRole("link", { name: CREATOR_FUND_CONTACT_EMAIL });
    contact.addEventListener("click", (event) => event.preventDefault());
    await user.click(contact);
    expect(onCreatorFundClick.mock.calls).toEqual([["Email Creator Fund"]]);
  });
});

describe("HelpView system status", () => {
  it("pings every service once and announces the overall state when they settle", async () => {
    renderView();
    await waitFor(() => expect(screen.getByRole("status").textContent).toBe("System Status: Experiencing Problems"));
    expect(fetchMock.mock.calls.map(([url]) => url).sort()).toEqual([DOWN, UP]);
  });

  it("keeps the announcement empty and offers no toggle while the services are checked", () => {
    fetchMock.mockImplementation(() => new Promise<Response>(() => {}));
    renderView();
    expect(screen.getByRole("status").textContent).toBe("");
    expect(screen.queryByRole("button", { name: "System Status" })).toBeNull();
  });

  it("lists each service with its state when opened, and closes on Escape", async () => {
    const user = userEvent.setup();
    renderView();
    const toggle = await screen.findByRole("button", { name: "System Status" });
    await user.click(toggle);
    expect(toggle.getAttribute("aria-expanded")).toBe("true");
    const list = document.getElementById(toggle.getAttribute("aria-controls")!)!;
    expect(list.hidden).toBe(false);
    expect(within(list).getAllByRole("listitem").map((item) => item.textContent)).toEqual([
      "Up Service (up)",
      "Down Service (down)",
    ]);
    expect(within(list).getByRole("link", { name: "View Details" }).getAttribute("href")).toBe("https://status.example/");

    await user.keyboard("{Escape}");
    expect(list.hidden).toBe(true);
    expect(document.activeElement).toBe(toggle);
  });

  it("says all is operational when every service answers OK", async () => {
    fetchMock.mockImplementation(async () => new Response(null, { status: 200 }));
    renderView();
    await waitFor(() => expect(screen.getByRole("status").textContent).toBe("System Status: All Operational"));
  });

  it("says services are shut down when no service answers", async () => {
    fetchMock.mockImplementation(async () => {
      throw new TypeError("Failed to fetch");
    });
    renderView();
    await waitFor(() => expect(screen.getByRole("status").textContent).toBe("System Status: Services Shutdown"));
  });
});

describe("HelpView chat banner", () => {
  it("opens the chat widget once it has loaded", async () => {
    const intercom = vi.fn();
    (window as Window & { Intercom?: unknown }).Intercom = intercom;
    renderView();
    await userEvent.setup().click(screen.getByRole("button", { name: "CHAT NOW" }));
    expect(intercom).toHaveBeenCalledWith("show");
  });
});
