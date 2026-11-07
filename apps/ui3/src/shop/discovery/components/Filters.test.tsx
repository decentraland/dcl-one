import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { CategoryFilter } from "./Filters";

function filter(category: string) {
  return <CategoryFilter category={category} subCategory={null} onCategory={() => undefined} onSub={() => undefined} />;
}

describe("CategoryFilter", () => {
  it("opens the group of a category with sub-categories, and closes it when the page moves to one without", () => {
    const view = render(filter("wearable"));
    expect(screen.getByRole("button", { name: "Wearables" })).toHaveAttribute("aria-expanded", "true");
    // Items going to All, or Creator switching to its collections: the open group closes.
    for (const category of ["all", "__collections"]) {
      view.rerender(filter("wearable"));
      expect(screen.getByRole("button", { name: "Wearables" })).toHaveAttribute("aria-expanded", "true");
      view.rerender(filter(category));
      expect(screen.getByRole("button", { name: "Wearables" })).toHaveAttribute("aria-expanded", "false");
    }
    view.rerender(filter("emote"));
    expect(screen.getByRole("button", { name: "Emotes" })).toHaveAttribute("aria-expanded", "true");
    expect(screen.getByRole("button", { name: "Wearables" })).toHaveAttribute("aria-expanded", "false");
  });
});
