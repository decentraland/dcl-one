import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { MemoryRouter, useLocation, useNavigate } from "react-router";
import { expect, it } from "vitest";

import { useUrlFilters } from "./hooks";

function Filters() {
  const [, patch] = useUrlFilters({ rarities: [] as string[], sort: "newest" });
  const { search } = useLocation();
  const navigate = useNavigate();
  return <>
    <output data-testid="query">{search}</output>
    <button onClick={() => { patch({ rarities: ["rare"] }); patch({ sort: "price-asc" }); }}>Combine</button>
    <button onClick={() => navigate("?rarities=epic&from=shared")}>Open shared URL</button>
    <button onClick={() => patch({ sort: "price-asc" })}>Sort</button>
    <button onClick={() => patch({ rarities: [], sort: "newest" })}>Reset</button>
  </>;
}

it("composes rapid filter edits while preserving external navigation and unrelated query state", async () => {
  render(<MemoryRouter initialEntries={["/shop/items?from=initial"]}><Filters /></MemoryRouter>);
  const query = () => Object.fromEntries(new URLSearchParams(screen.getByTestId("query").textContent!));
  fireEvent.click(screen.getByText("Combine"));
  await waitFor(() => expect(query()).toEqual({ from: "initial", rarities: "rare", sort: "price-asc" }));
  fireEvent.click(screen.getByText("Open shared URL"));
  await waitFor(() => expect(query()).toEqual({ rarities: "epic", from: "shared" }));
  fireEvent.click(screen.getByText("Sort"));
  await waitFor(() => expect(query()).toEqual({ rarities: "epic", from: "shared", sort: "price-asc" }));
  fireEvent.click(screen.getByText("Reset"));
  await waitFor(() => expect(query()).toEqual({ from: "shared" }));
});
