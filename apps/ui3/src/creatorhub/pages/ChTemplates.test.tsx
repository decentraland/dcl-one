import { existsSync } from "node:fs";
import { join } from "node:path";
import { afterEach, describe, expect, test, vi } from "vitest";
import { cleanup, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import ChTemplates, { STARTER_TEMPLATES } from "./ChTemplates";

const PUBLIC_DIR = join(__dirname, "../../../public");
const DIFFICULTIES = ["Easy", "Intermediate", "Hard"];

afterEach(() => cleanup());

describe("starter templates catalog", () => {
  test("every template has a unique id, a name, a description, a difficulty and a shipped thumbnail", () => {
    expect(STARTER_TEMPLATES.length).toBeGreaterThan(0);
    expect(new Set(STARTER_TEMPLATES.map((t) => t.id)).size).toBe(STARTER_TEMPLATES.length);
    for (const t of STARTER_TEMPLATES) {
      expect(t.title, t.id).toMatch(/\S/);
      expect(t.description, t.id).toMatch(/\S/);
      expect(DIFFICULTIES, t.id).toContain(t.difficulty_level);
      expect(t.github_link, t.id).toMatch(/^https:\/\/github\.com\//);
      const thumb = t.thumb?.replace(/^\//, "");
      expect(thumb, t.id).toMatch(/^assets\/templates\/[a-z0-9-]+\.(jpg|webp|png)$/);
      expect(existsSync(join(PUBLIC_DIR, thumb!)), `${t.id}: ${thumb}`).toBe(true);
    }
  });

  test("each template card shows its name, difficulty and image", () => {
    render(<ChTemplates chrome={false} />);
    for (const t of STARTER_TEMPLATES) {
      const card = screen.getByRole("button", { name: `Open ${t.title}` });
      expect(within(card).getByText(t.difficulty_level!)).toBeInTheDocument();
      expect(card.querySelector(".chtpl__thumb")).toHaveStyle({ backgroundImage: `url(${t.thumb})` });
    }
  });
});

describe("create scene dialog", () => {
  test("Create hands the template over once and holds the dialog in an opening state", async () => {
    const onCreate = vi.fn();
    const onClose = vi.fn();
    render(<ChTemplates chrome={false} onCreate={onCreate} />);
    await userEvent.click(screen.getByRole("button", { name: "Open Escape Room" }));
    const dialog = screen.getByRole("dialog", { name: "Create Scene" });
    expect(dialog).toHaveTextContent("Escape Room");
    const create = within(dialog).getByRole("button", { name: "Create" });
    await userEvent.click(create);
    await userEvent.click(create);
    expect(onCreate).toHaveBeenCalledTimes(1);
    expect(onCreate).toHaveBeenCalledWith({ template: expect.objectContaining({ id: "escape-room" }) });
    expect(onClose).not.toHaveBeenCalled();
    expect(within(dialog).getByText(/Opening the editor/)).toBeInTheDocument();
    expect(create).toBeDisabled();
    expect(within(dialog).getByRole("button", { name: "Cancel" })).toBeDisabled();
  });
});
