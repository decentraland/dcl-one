import { fireEvent, render, screen } from "@testing-library/react";
import { expect, test, vi } from "vitest";
import CoverImage from "./CoverImage";

test("failed thumbnails fall back to the original once, then show an accessible placeholder", () => {
  const view = render(<CoverImage src="/thumb" fallbackSrc="/original" alt="Genesis Plaza" />);
  fireEvent.error(screen.getByRole("img"));
  expect(screen.getByRole("img")).toHaveAttribute("src", "/original");
  fireEvent.error(screen.getByRole("img"));
  expect(screen.getByRole("img", { name: "Genesis Plaza" })).not.toHaveAttribute("src");
  view.rerender(<CoverImage src="/different" alt="Another place" />);
  fireEvent.load(screen.getByRole("img"));
  expect(screen.getByRole("img", { name: "Another place" })).toHaveAttribute("src", "/different");
});

test("an image that failed before hydration still loads the original fallback", () => {
  const complete = vi.spyOn(HTMLImageElement.prototype, "complete", "get").mockReturnValue(true);
  const width = vi.spyOn(HTMLImageElement.prototype, "naturalWidth", "get").mockImplementation(function (this: HTMLImageElement) {
    return this.getAttribute("src") === "/original" ? 640 : 0;
  });
  try {
    render(<CoverImage src="/unavailable-thumbnail" fallbackSrc="/original" alt="Scene" />);
    const image = screen.getByRole("img", { name: "Scene" });
    expect(image).toHaveAttribute("src", "/original");
    expect(image.parentElement).toHaveAttribute("data-state", "ready");
  } finally {
    complete.mockRestore();
    width.mockRestore();
  }
});
