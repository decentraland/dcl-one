import { cleanup, render } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import DeWorkspace from "./DeWorkspace";
import * as histories from "../history";

afterEach(() => { cleanup(); vi.restoreAllMocks(); });

it("clears stale undo state only when an external scene restore has been acknowledged", () => {
  const clear = vi.fn();
  const original = histories.createHistory;
  vi.spyOn(histories, "createHistory").mockImplementation((...args) => ({ ...original(...args), clear }));
  const view = render(<DeWorkspace title="Shared scene" externalSceneVersion={0} />);
  const initial = clear.mock.calls.length;
  view.rerender(<DeWorkspace title="Shared scene" externalSceneVersion={0} operationsBusy />);
  expect(clear).toHaveBeenCalledTimes(initial);
  view.rerender(<DeWorkspace title="Shared scene" externalSceneVersion={1} />);
  expect(clear).toHaveBeenCalledTimes(initial + 1);
});
