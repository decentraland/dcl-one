import { act, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import EventDetail, { shareEvent } from "./EventDetail";

afterEach(() => { vi.unstubAllGlobals(); vi.useRealTimers(); });

const ev = { id: "e1", title: "Beyond The NFT", url: "https://example.org/events/e1" } as unknown as Parameters<typeof shareEvent>[0];

test("shareEvent copies the link when there is no share sheet and says so", async () => {
  const writeText = vi.fn(() => Promise.resolve());
  vi.stubGlobal("navigator", { clipboard: { writeText } });
  await expect(shareEvent(ev)).resolves.toBe("copied");
  expect(writeText).toHaveBeenCalledWith("https://example.org/events/e1");
});

test("the Share button reports the copy for two seconds, then goes back to Share", async () => {
  vi.useFakeTimers();
  vi.stubGlobal("navigator", { clipboard: { writeText: vi.fn(() => Promise.resolve()) } });
  render(<EventDetail event={ev as never} />);
  const button = screen.getByRole("button", { name: "Share" });
  await act(async () => { fireEvent.click(button); await Promise.resolve(); await Promise.resolve(); });
  expect(screen.getByRole("status")).toHaveTextContent("Link copied");
  expect(screen.getByRole("button", { name: "Link copied" })).toBeInTheDocument();
  await act(async () => { vi.advanceTimersByTime(2100); });
  expect(screen.queryByRole("status")).toBeNull();
  expect(screen.getByRole("button", { name: "Share" })).toBeInTheDocument();
});
