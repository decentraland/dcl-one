// @vitest-environment jsdom
import type { ReactNode } from "react";
import { cleanup, render, screen } from "@testing-library/react";
import { MemoryRouter, Route, Routes } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";

import ReportSuccess from "./ReportSuccess";

vi.hoisted(() =>
  Object.assign(window, {
    __vite_plugin_react_preamble_installed__: true,
    $RefreshReg$: () => {},
    $RefreshSig$: () => (type: unknown) => type,
  }),
);

vi.mock("../../components/chrome/SitesLayout", () => ({ default: ({ children }: { children?: ReactNode }) => <>{children}</> }));
vi.mock("@dcl/one-ui", async (importOriginal) => ({
  ...(await importOriginal<typeof import("@dcl/one-ui")>()),
  AnimatedBackground: () => <div data-testid="animated-background" />,
}));

afterEach(cleanup);

function renderAt(state: unknown) {
  return render(
    <MemoryRouter initialEntries={[{ pathname: "/report/success", state }]}>
      <Routes>
        <Route path="/report/success" element={<ReportSuccess />} />
        <Route path="/report" element={<p>Report form</p>} />
      </Routes>
    </MemoryRouter>,
  );
}

describe("when the report form's navigation lands on the success page", () => {
  it("should show the card over the animated background", () => {
    renderAt({ submitted: true });
    expect(screen.getByRole("heading", { name: "Report Submitted" })).toBeTruthy();
    const backgrounds = screen.getAllByTestId("animated-background");
    expect(backgrounds).toHaveLength(1);
    expect(backgrounds[0]!.contains(screen.getByRole("heading", { name: "Report Submitted" }))).toBe(false);
  });
});

describe("when the success page is visited without a submission", () => {
  it.each([null, {}, { submitted: "true" }])("should send the visitor to the form (state %j)", (state) => {
    renderAt(state);
    expect(screen.getByText("Report form")).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Report Submitted" })).toBeNull();
  });
});
