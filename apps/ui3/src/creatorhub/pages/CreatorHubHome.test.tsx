import { afterEach, describe, expect, test } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";

import CreatorHubHome from "./CreatorHubHome";

const TEMPLATES = [
  { id: "escape-room", title: "Escape Room", difficulty: "Hard", href: "/create/scene-editor?new=1&template=escape-room" },
];

afterEach(() => cleanup());

describe("creator hub home", () => {
  test("keeps the templates row and shows neither What's happening nor Live from the network", () => {
    render(<CreatorHubHome chrome={false} templates={TEMPLATES} />);
    expect(screen.getByRole("region", { name: "Start from a template" })).toBeInTheDocument();
    expect(screen.getByRole("link", { name: /Escape Room/ })).toHaveAttribute("href", TEMPLATES[0]!.href);
    expect(screen.queryByText("What's happening")).toBeNull();
    expect(screen.queryByText("Live from the network")).toBeNull();
    expect(screen.queryByText("See what's on")).toBeNull();
    expect(screen.queryByText("See all places")).toBeNull();
  });

  test("starts compact and exposes the regular and airy top spacings for comparison", () => {
    const { container, rerender } = render(<CreatorHubHome chrome={false} />);
    expect(container.querySelector(".chh")).toHaveClass("chh--top-compact");
    rerender(<CreatorHubHome chrome={false} topSpacing="airy" />);
    expect(container.querySelector(".chh")).toHaveClass("chh--top-airy");
    expect(container.querySelector(".chh")).not.toHaveClass("chh--top-compact");
  });
});
