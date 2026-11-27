// @vitest-environment jsdom
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { act, cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from "vitest";

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

const auth = vi.hoisted(() => ({ address: null as string | null }));
vi.mock("@/data/lib/auth/context", () => ({ useAuth: () => ({ address: auth.address }) }));

const UP = "https://up.example/about";
const DOWN = "https://down.example/about";

let services: HelpViewProps["services"];
let fetchMock: Mock<(url: string) => Promise<Response>>;
let user: ReturnType<typeof userEvent.setup>;

beforeEach(() => {
  services = [
    { name: "Up Service", url: UP },
    { name: "Down Service", url: DOWN },
  ];
  fetchMock = vi.fn(async (url: string) => new Response(null, { status: url === UP ? 200 : 503 }));
  vi.stubGlobal("fetch", fetchMock);
  user = userEvent.setup();
});

afterEach(() => {
  cleanup();
  auth.address = null;
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  delete (window as Window & { Intercom?: unknown }).Intercom;
});

function renderView(props: Partial<HelpViewProps> = {}) {
  const client = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  const tree = () => (
    <QueryClientProvider client={client}>
      <HelpView services={services} statusPageUrl="https://status.example/" intercomAppId="" {...props} />
    </QueryClientProvider>
  );
  const view = render(tree());
  return { ...view, rerenderView: () => view.rerender(tree()) };
}

const panel = () => screen.getByRole("tabpanel");

describe("HelpView tabs", () => {
  describe("when the view opens", () => {
    beforeEach(() => {
      renderView();
    });

    it("should select the FAQ tab", () => {
      expect(screen.getByRole("tab", { name: "FAQ" }).getAttribute("aria-selected")).toBe("true");
    });

    it("should show the FAQ panel labelled by its tab", () => {
      expect(within(panel()).getByRole("heading", { name: "Frequently Asked Questions" })).toBeTruthy();
      expect(panel().getAttribute("aria-labelledby")).toBe(screen.getByRole("tab", { name: "FAQ" }).id);
    });

    it("should keep only the selected tab in the Tab order", () => {
      expect(screen.getAllByRole("tab").map((tab) => tab.tabIndex)).toEqual([0, -1, -1]);
    });
  });

  describe("when another tab is clicked", () => {
    beforeEach(async () => {
      renderView();
      await user.click(screen.getByRole("tab", { name: "SUPPORT UPDATES" }));
    });

    it("should select the clicked tab and deselect FAQ", () => {
      expect(screen.getByRole("tab", { name: "SUPPORT UPDATES" }).getAttribute("aria-selected")).toBe("true");
      expect(screen.getByRole("tab", { name: "FAQ" }).getAttribute("aria-selected")).toBe("false");
    });

    it("should switch the section", () => {
      expect(within(panel()).getByRole("heading", { name: "Support Updates" })).toBeTruthy();
    });
  });

  describe("when navigating with the keyboard", () => {
    it("should move and select with the arrows of either axis, Home and End, wrapping at the ends", async () => {
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

      await user.keyboard("{ArrowUp}");
      expect(document.activeElement).toBe(screen.getByRole("tab", { name: "SUPPORT UPDATES" }));

      await user.keyboard("{ArrowRight}");
      expect(document.activeElement).toBe(screen.getByRole("tab", { name: "FAQ" }));

      await user.keyboard("{ArrowLeft}");
      expect(document.activeElement).toBe(screen.getByRole("tab", { name: "SUPPORT UPDATES" }));

      await user.keyboard("{Home}");
      expect(screen.getByRole("tab", { name: "FAQ" }).getAttribute("aria-selected")).toBe("true");
    });
  });
});

describe("HelpView accordions", () => {
  describe("when the FAQ tab is shown", () => {
    let questions: HTMLElement[];

    beforeEach(() => {
      renderView();
      questions = within(panel()).getAllByRole("button");
    });

    it("should list every FAQ question", () => {
      expect(questions.map((q) => q.textContent)).toEqual(HELP_FAQ.map((item) => item.question));
    });

    it("should keep every question closed", () => {
      expect(questions.every((q) => q.getAttribute("aria-expanded") === "false")).toBe(true);
    });
  });

  describe("when a question is clicked", () => {
    let question: HTMLElement;
    let answer: HTMLElement;

    beforeEach(async () => {
      renderView();
      question = screen.getByRole("button", { name: HELP_FAQ[0].question });
      await user.click(question);
      answer = document.getElementById(question.getAttribute("aria-controls")!)!;
    });

    it("should open the question to show its answer", () => {
      expect(question.getAttribute("aria-expanded")).toBe("true");
      expect(answer.hidden).toBe(false);
      expect(answer.textContent).toBe(HELP_FAQ[0].answer);
    });

    describe("and it is clicked again", () => {
      beforeEach(async () => {
        await user.click(question);
      });

      it("should close it", () => {
        expect(question.getAttribute("aria-expanded")).toBe("false");
        expect(answer.hidden).toBe(true);
      });
    });

    describe("and another question opens", () => {
      let second: HTMLElement;

      beforeEach(async () => {
        second = screen.getByRole("button", { name: HELP_FAQ[1].question });
        await user.click(second);
      });

      it("should close the first question", () => {
        expect(question.getAttribute("aria-expanded")).toBe("false");
      });

      it("should open the second question", () => {
        expect(second.getAttribute("aria-expanded")).toBe("true");
      });
    });
  });

  describe("when the Support Updates tab is shown", () => {
    it("should list every support update", async () => {
      renderView();
      await user.click(screen.getByRole("tab", { name: "SUPPORT UPDATES" }));
      expect(within(panel()).getAllByRole("button").map((q) => q.textContent)).toEqual(
        HELP_SUPPORT_UPDATES.map((item) => item.question),
      );
    });
  });
});

describe("HelpView Creator Fund tab", () => {
  let onCreatorFundClick: Mock<(name: string) => void>;

  beforeEach(async () => {
    onCreatorFundClick = vi.fn();
    renderView({ onCreatorFundClick });
    await user.click(screen.getByRole("tab", { name: "CREATOR FUND" }));
  });

  describe("when the tab is shown", () => {
    let questions: HTMLElement[];

    beforeEach(() => {
      questions = within(panel()).getAllByRole("button");
    });

    it("should list the Creator Fund questions with their answers", () => {
      expect(questions.map((q) => q.textContent)).toEqual(CREATOR_FUND_FAQ.map((entry) => entry.question));
      for (const [index, entry] of CREATOR_FUND_FAQ.entries()) {
        expect(document.getElementById(questions[index].getAttribute("aria-controls")!)!.textContent).toBe(entry.answer);
      }
    });

    it("should end with the contact address", () => {
      const contact = within(panel()).getByRole("link", { name: CREATOR_FUND_CONTACT_EMAIL });
      expect(contact.getAttribute("href")).toBe(`mailto:${CREATOR_FUND_CONTACT_EMAIL}`);
      expect(contact.parentElement!.textContent).toBe(`${CREATOR_FUND_CONTACT_PROMPT} ${CREATOR_FUND_CONTACT_EMAIL}`);
    });
  });

  describe("when a question is opened and closed", () => {
    beforeEach(async () => {
      const question = screen.getByRole("button", { name: CREATOR_FUND_FAQ[0].question });
      await user.click(question);
      await user.click(question);
    });

    it("should report the opening by its tracking name, and not the closing", () => {
      expect(onCreatorFundClick.mock.calls).toEqual([[CREATOR_FUND_FAQ[0].trackingName]]);
    });
  });

  describe("when the contact address is clicked", () => {
    beforeEach(async () => {
      const contact = screen.getByRole("link", { name: CREATOR_FUND_CONTACT_EMAIL });
      contact.addEventListener("click", (event) => event.preventDefault());
      await user.click(contact);
    });

    it("should report the click", () => {
      expect(onCreatorFundClick.mock.calls).toEqual([["Email Creator Fund"]]);
    });
  });
});

describe("HelpView system status", () => {
  describe("when some services are up and others down", () => {
    it("should ping every service once and announce the overall state when they settle", async () => {
      renderView();
      await waitFor(() => expect(screen.getByRole("status").textContent).toBe("System Status: Experiencing Problems"));
      expect(fetchMock.mock.calls.map(([url]) => url).sort()).toEqual([DOWN, UP]);
    });

    describe("and the status toggle is opened", () => {
      let toggle: HTMLElement;
      let list: HTMLElement;

      beforeEach(async () => {
        renderView();
        toggle = await screen.findByRole("button", { name: "System Status" });
        await user.click(toggle);
        list = document.getElementById(toggle.getAttribute("aria-controls")!)!;
      });

      it("should list each service with its state", () => {
        expect(toggle.getAttribute("aria-expanded")).toBe("true");
        expect(list.hidden).toBe(false);
        expect(within(list).getAllByRole("listitem").map((item) => item.textContent)).toEqual([
          "Up Service (up)",
          "Down Service (down)",
        ]);
      });

      it("should link to the status page", () => {
        expect(within(list).getByRole("link", { name: "View Details" }).getAttribute("href")).toBe("https://status.example/");
      });

      describe("and Escape is pressed", () => {
        beforeEach(async () => {
          await user.keyboard("{Escape}");
        });

        it("should close the list and return focus to the toggle", () => {
          expect(list.hidden).toBe(true);
          expect(document.activeElement).toBe(toggle);
        });
      });
    });
  });

  describe("when the services are still being checked", () => {
    beforeEach(() => {
      fetchMock.mockImplementation(() => new Promise<Response>(() => {}));
      renderView();
    });

    it("should keep the announcement empty", () => {
      expect(screen.getByRole("status").textContent).toBe("");
    });

    it("should offer no toggle", () => {
      expect(screen.queryByRole("button", { name: "System Status" })).toBeNull();
    });
  });

  describe("when every service answers OK", () => {
    beforeEach(() => {
      fetchMock.mockImplementation(async () => new Response(null, { status: 200 }));
    });

    it("should say all is operational", async () => {
      renderView();
      await waitFor(() => expect(screen.getByRole("status").textContent).toBe("System Status: All Operational"));
    });
  });

  describe("when no service answers", () => {
    beforeEach(() => {
      fetchMock.mockImplementation(async () => {
        throw new TypeError("Failed to fetch");
      });
    });

    it("should say services are shut down", async () => {
      renderView();
      await waitFor(() => expect(screen.getByRole("status").textContent).toBe("System Status: Services Shutdown"));
    });
  });
});

describe("HelpView chat banner", () => {
  let intercom: Mock<(...args: unknown[]) => void>;

  beforeEach(() => {
    intercom = vi.fn();
  });

  afterEach(() => {
    document.querySelectorAll("script[data-intercom]").forEach((script) => script.remove());
    delete (window as Window & { Intercom?: unknown }).Intercom;
  });

  const chatButton = () => screen.getByRole("button", { name: "CHAT NOW" });
  const contactLink = () => screen.queryByRole("link", { name: "support@decentraland.org" });
  async function checkInlineContact() {
    expect(contactLink()).toBeNull();
    await userEvent.setup().click(chatButton());
    expect(contactLink()?.getAttribute("href")).toBe("mailto:support@decentraland.org");
    expect(chatButton()).toBeTruthy();
  }

  describe("when chat is disabled", () => {
    beforeEach(() => {
      renderView({ intercomAppId: "" });
    });

    it("should keep the original banner", () => {
      expect(screen.getByText("Our support team is here to answer your questions!")).toBeTruthy();
    });

    it("should reveal a contact on click without loading the widget", async () => {
      await checkInlineContact();
      expect(document.querySelector("script[data-intercom]")).toBeNull();
    });
  });

  describe("when the session becomes ready after a first click", () => {
    it("should open the messenger on a later click", async () => {
      renderView({ intercomAppId: "app-test" });
      expect(contactLink()).toBeNull();
      Object.assign(window, { Intercom: intercom });
      document.querySelector("script[data-intercom]")!.dispatchEvent(new Event("load"));
      await waitFor(() => expect(intercom).toHaveBeenCalledWith("boot", expect.not.objectContaining({ hide_default_launcher: expect.anything() })));
      await checkInlineContact();
      act(() => (intercom.mock.calls.find(([command]) => command === "ready")![1] as () => void)());
      await user.click(screen.getByRole("button", { name: "CHAT NOW" }));
      expect(intercom).toHaveBeenCalledWith("show");
      expect(contactLink()).toBeNull();
    });
  });

  describe("when the widget script fails", () => {
    beforeEach(() => {
      vi.spyOn(console, "error").mockImplementation(() => {});
    });

    it("should offer inline contact on click", async () => {
      renderView({ intercomAppId: "app-test" });
      document.querySelector("script[data-intercom]")!.dispatchEvent(new Event("error"));
      await waitFor(() => expect(document.querySelector("script[data-intercom]")).toBeNull());
      await checkInlineContact();
    });
  });

  describe("when the wallet session changes before the first one is ready", () => {
    beforeEach(() => {
      auth.address = "0x" + "ab".repeat(20);
    });

    it("should ignore readiness from the previous wallet session", async () => {
      const view = renderView({ intercomAppId: "app-test" });
      Object.assign(window, { Intercom: intercom });
      document.querySelector("script[data-intercom]")!.dispatchEvent(new Event("load"));
      await waitFor(() => expect(intercom).toHaveBeenCalledWith("boot", expect.objectContaining({ Wallet: auth.address })));
      const firstReady = intercom.mock.calls.find(([command]) => command === "ready")![1] as () => void;
      auth.address = null;
      view.rerenderView();
      await waitFor(() => expect(intercom.mock.calls.filter(([command]) => command === "boot")).toHaveLength(2));
      expect(intercom).toHaveBeenCalledWith("shutdown");
      expect(intercom.mock.lastCall![1]).not.toHaveProperty("Wallet");
      act(() => firstReady());
      await checkInlineContact();
    });
  });
});
