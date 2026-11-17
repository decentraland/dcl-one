import { createRef } from "react";
import { describe, expect, test, vi } from "vitest";
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import Alert from "./Alert";

describe("Alert", () => {
  test.each([
    ["error", "alert"],
    ["warning", "alert"],
    ["info", "status"],
    ["success", "status"],
  ] as const)("a message of severity %s has the role %s", (severity, role) => {
    render(<Alert severity={severity}>The scene was saved</Alert>);

    expect(screen.getByRole(role)).toHaveTextContent("The scene was saved");
    expect(screen.queryByRole(role === "alert" ? "status" : "alert")).toBeNull();
  });

  test("a message with no severity is information", () => {
    render(<Alert>Three scenes use this asset</Alert>);

    expect(screen.getByRole("status")).toHaveTextContent("Three scenes use this asset");
  });

  test("the caller can replace the role", () => {
    render(<Alert severity="error" role="note">The file is too large</Alert>);

    expect(screen.getByRole("note")).toHaveTextContent("The file is too large");
    expect(screen.queryByRole("alert")).toBeNull();
  });

  test("the title and the message are both in the alert", () => {
    render(<Alert severity="warning" title="Unsaved changes">Save the scene before you close it.</Alert>);

    const alert = within(screen.getByRole("alert"));
    expect(alert.getByText("Unsaved changes")).toBeInTheDocument();
    expect(alert.getByText("Save the scene before you close it.")).toBeInTheDocument();
  });

  test("the glyph the caller gives is hidden from a screen reader", () => {
    render(<Alert severity="info" icon={<img alt="Bell glyph" />}>New version</Alert>);

    expect(screen.queryByRole("img", { name: "Bell glyph" })).toBeNull();
    expect(screen.getByRole("img", { name: "Bell glyph", hidden: true })).toBeInTheDocument();
  });

  test("the glyph of the severity adds nothing to the text of the alert", () => {
    render(<Alert severity="success">Published</Alert>);

    expect(screen.getByRole("status").textContent).toBe("Published");
    expect(screen.queryByRole("img")).toBeNull();
  });

  test("an alert with no close handler has no button", () => {
    render(<Alert severity="info">New version</Alert>);

    expect(screen.queryByRole("button")).toBeNull();
  });

  test("the close button has a name and calls the caller", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<Alert severity="info" onClose={onClose}>New version</Alert>);

    await user.click(screen.getByRole("button", { name: "Close" }));

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  test("the caller can name the close button", () => {
    render(<Alert severity="info" onClose={() => {}} closeLabel="Dismiss the notice">New version</Alert>);

    expect(screen.getByRole("button", { name: "Dismiss the notice" })).toBeInTheDocument();
  });

  test("the close button is reached and pressed with the keys", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<Alert severity="info" onClose={onClose}>New version</Alert>);

    await user.tab();
    expect(screen.getByRole("button", { name: "Close" })).toHaveFocus();
    await user.keyboard("{Enter}");

    expect(onClose).toHaveBeenCalledTimes(1);
  });

  test("the controls of the caller replace the close button", async () => {
    const user = userEvent.setup();
    const onUpdate = vi.fn();
    render(<Alert severity="info" onClose={() => {}} action={<button onClick={onUpdate}>Update</button>}>New version</Alert>);

    await user.click(screen.getByRole("button", { name: "Update" }));

    expect(onUpdate).toHaveBeenCalledTimes(1);
    expect(screen.getAllByRole("button")).toHaveLength(1);
    expect(screen.getByRole("status")).toContainElement(screen.getByRole("button", { name: "Update" }));
  });

  test("attributes of the caller and the ref reach the element of the alert", () => {
    const ref = createRef<HTMLDivElement>();
    render(<Alert ref={ref} severity="error" aria-label="Deploy error" id="deploy-error">The deploy failed</Alert>);

    const alert = screen.getByRole("alert", { name: "Deploy error" });
    expect(ref.current).toBe(alert);
    expect(alert).toHaveAttribute("id", "deploy-error");
  });
});
