import { afterEach, describe, expect, it, vi } from "vitest";
import { copyText } from "./clipboard";

afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks(); });

describe("copyText", () => {
  it("uses the async clipboard when it works", async () => {
    const writeText = vi.fn(() => Promise.resolve());
    vi.stubGlobal("navigator", { clipboard: { writeText } });
    expect(await copyText("hello")).toBe(true);
    expect(writeText).toHaveBeenCalledWith("hello");
  });

  it("falls back to a selection copy when the clipboard refuses, and reports a total failure", async () => {
    vi.stubGlobal("navigator", { clipboard: { writeText: vi.fn(() => Promise.reject(new Error("Document is not focused"))) } });
    const exec = vi.fn(() => true);
    document.execCommand = exec as unknown as typeof document.execCommand;
    expect(await copyText("log line")).toBe(true);
    expect(exec).toHaveBeenCalledWith("copy");
    expect(document.querySelector("textarea")).toBeNull();
    document.execCommand = vi.fn(() => false) as unknown as typeof document.execCommand;
    expect(await copyText("log line")).toBe(false);
  });
});
