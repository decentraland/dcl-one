import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, test, vi } from "vitest";

import type { BridgeChatLine } from "../../overlay/bridge";
import { ChatView, type ChatIo } from "./Chat";
import type { ConsoleLine, ConsoleSource } from "./chatCommands";

const ME = { address: "0xabc0000000000000000000000000000000000abc", name: "Me" };
const T0 = Date.UTC(2026, 8, 18, 10, 0);

function engineLine(message: string, i: number): BridgeChatLine {
  return { senderName: "", senderAddress: "", channel: "System", message, timestamp: T0 + i };
}

function echoLine(message: string, i: number): BridgeChatLine {
  return { senderName: ME.name, senderAddress: ME.address, channel: "System", message, timestamp: T0 + i };
}

function harness(withConsole = true) {
  const send = vi.fn();
  const listeners = new Set<(line: ConsoleLine) => void>();
  const source: ConsoleSource = (l) => {
    listeners.add(l);
    return () => listeners.delete(l);
  };
  const chat: BridgeChatLine[] = [];
  const io = (): ChatIo => ({
    chat: [...chat],
    players: [],
    me: ME,
    blocked: [],
    live: true,
    send,
    ...(withConsole ? { console: source } : {}),
  });
  const view = render(<ChatView open onToggle={() => {}} io={io()} />);
  const input = () => screen.getByLabelText("Send a message to Nearby chat");
  const type = async (text: string): Promise<void> => {
    fireEvent.change(input(), { target: { value: text } });
    await act(async () => { fireEvent.keyDown(input(), { key: "Enter" }); });
  };
  const arrive = (lines: BridgeChatLine[]): void => {
    for (const l of lines) {
      chat.push(l);
      act(() => {
        for (const cb of [...listeners]) cb(l);
      });
    }
    view.rerender(<ChatView open onToggle={() => {}} io={io()} />);
  };
  return { send, type, arrive, listeners };
}

const help = () => screen.getByText(/Chat commands:/);

afterEach(() => {
  vi.useRealTimers();
});

describe("/help against the engine console", () => {
  test("sends /help, hides the raw reply and renders the merged list after the echo, curated entries first and bare engine names after", async () => {
    const h = harness();
    await h.type("/help");
    expect(h.send).toHaveBeenCalledWith("/help");
    expect(screen.queryByText(/Chat commands:/)).toBeNull();

    h.arrive([
      echoLine("/help", 0),
      engineLine("Available commands:", 1),
      engineLine("  /asset_catalog   - ", 2),
      engineLine("  /clear           - ", 3),
      engineLine("  /fps             - Set the target frame rate", 4),
      engineLine("  /help            - ", 5),
      engineLine("[ok]", 6),
    ]);
    await act(async () => {});

    const text = help();
    expect(text).toHaveTextContent("/help [command]");
    expect(text).toHaveTextContent("/fps <fps>");
    expect(text).toHaveTextContent("/asset_catalog");
    expect(text).not.toHaveTextContent("/teleport");
    expect(text.closest("[data-console]")).toHaveAttribute("data-console", "output");

    expect(screen.queryByText("Available commands:")).toBeNull();
    expect(screen.queryByText("[ok]")).toBeNull();
    expect(screen.queryByText(/\/asset_catalog\s+-/)).toBeNull();

    const echo = screen.getByText("/help");
    expect(echo.closest("[data-console]")).toHaveAttribute("data-console", "echo");
    expect(echo.compareDocumentPosition(text) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    expect(h.listeners.size).toBe(0);
  });

  test("falls back to the curated table when the engine does not answer within the timeout", async () => {
    vi.useFakeTimers();
    const h = harness();
    await h.type("/help");
    expect(h.send).toHaveBeenCalledWith("/help");

    expect(screen.queryByText(/Chat commands:/)).toBeNull();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(10_000);
    });
    expect(help()).toHaveTextContent("/teleport <x> <y> [realm]");
    expect(help()).toHaveTextContent("/clear");
    expect(h.listeners.size).toBe(0);
  });

  test("prints the curated table at once and sends nothing when there is no console to ask", async () => {
    const h = harness(false);
    await h.type("/help");
    expect(h.send).not.toHaveBeenCalled();
    expect(help()).toHaveTextContent("/teleport <x> <y> [realm]");
    expect(help().closest("[data-console]")).toHaveAttribute("data-console", "output");
  });

  test("/help <command> answers from the table when it has the entry and asks the engine otherwise", async () => {
    const h = harness();
    await h.type("/help fps");
    expect(h.send).not.toHaveBeenCalled();
    expect(screen.getByText(/^\/fps <fps>/)).toBeInTheDocument();

    await h.type("/help idnoclip");
    expect(h.send).toHaveBeenCalledWith("/help /idnoclip");
    expect(h.send).toHaveBeenCalledTimes(1);
  });
});

test("travel commands are hidden from chat while ordinary messages and help remain", () => {
  const h = harness();
  h.arrive([echoLine("/goto 10,20", 0), echoLine("/teleport 10 20", 1), echoLine("/changerealm world", 2), echoLine("/help", 3), { ...echoLine("/goto is useful", 4), channel: "Nearby" }]);
  expect(screen.queryByText("/goto 10,20")).toBeNull();
  expect(screen.queryByText("/teleport 10 20")).toBeNull();
  expect(screen.queryByText("/changerealm world")).toBeNull();
  expect(screen.getByText("/help")).toBeInTheDocument();
  expect(screen.getByText("/goto is useful")).toBeInTheDocument();
});

test("a repeated command folds into one console entry with its count, output and done mark", async () => {
  const h = harness();
  await h.type("/reload");
  const at = Date.now();
  h.arrive([
    ...[0, 1, 2, 3].map((i) => ({ ...echoLine("/reload", 0), timestamp: at + i })),
    { ...engineLine("all scenes reloaded", 0), timestamp: at + 4 },
    { ...engineLine("[ok]", 0), timestamp: at + 5 },
  ]);
  const block = screen.getByRole("group", { name: "Console" });
  expect(screen.getAllByText("/reload")).toHaveLength(1);
  expect(screen.getByTitle("Sent 4 times")).toHaveTextContent("\u00d74");
  expect(block).toContainElement(screen.getByText("all scenes reloaded"));
  expect(screen.getByText("all scenes reloaded").closest("[data-console]")).toHaveAttribute("data-console", "output");
  expect(screen.getByRole("img", { name: "Done" })).toBeInTheDocument();
  expect(screen.queryByText("[ok]")).toBeNull();
});
