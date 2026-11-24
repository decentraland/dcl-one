import { expect, it } from "vitest";
import { render } from "@testing-library/react";

import { Coin } from "../atoms/icons";

it("the Coin icon carries no text", () => {
  expect(render(<Coin size={13} />).container.textContent).toBe("");
  expect(render(<Coin size={13} ring={false} />).container.textContent).toBe("");
});
