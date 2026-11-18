import { describe, expect, test } from "vitest";
import { render, screen, within } from "@testing-library/react";

import Address from "./Address";

const WALLET = "0x1234567890abcdef1234567890abcdef12345678";

describe("Address", () => {
  test("the short form shows both ends, keeps the full address as the title and gives screen readers the full address", () => {
    render(<Address value={WALLET} />);

    const shown = screen.getByText("0x1234\u20265678");
    expect(shown).toHaveAttribute("aria-hidden", "true");
    const el = shown.parentElement as HTMLElement;
    expect(el.tagName).toBe("SPAN");
    expect(el).not.toHaveAttribute("aria-label");
    expect(el).toHaveAttribute("title", WALLET);
    expect(within(el).getByText(WALLET)).not.toHaveAttribute("aria-hidden");
  });

  test("a control named by its content takes the full address", () => {
    render(<button><Address value={WALLET} /></button>);

    expect(screen.getByRole("button", { name: WALLET })).toBeInTheDocument();
  });

  test("the full form shows the address with no extra name", () => {
    render(<Address value={WALLET} shorten={false} />);

    const el = screen.getByText(WALLET);
    expect(el).not.toHaveAttribute("aria-label");
    expect(el).not.toHaveAttribute("title");
  });

  test("strong renders a strong element and keeps the class of the caller", () => {
    render(<Address value={WALLET} strong className="mine" />);

    const el = screen.getByTitle(WALLET);
    expect(el.tagName).toBe("STRONG");
    expect(el).toHaveClass("mine");
  });

  test("a value too short to cut is shown whole", () => {
    render(<Address value="0x1234" />);

    expect(screen.getByText("0x1234")).not.toHaveAttribute("aria-hidden");
  });
});
