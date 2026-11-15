import { useState } from "react";
import { describe, expect, test, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { Tab, TabPanel, Tabs, type TabsProps } from "./Tabs";

type HarnessProps = Partial<TabsProps> & { onChange?: (value: string) => void; initial?: string; keepMounted?: boolean };

function Harness({ onChange, initial = "scenes", keepMounted, ...props }: HarnessProps) {
  const [value, setValue] = useState(initial);
  return (
    <>
      <button>Before</button>
      <Tabs value={value} onChange={(next) => { onChange?.(next); setValue(next); }} aria-label="Library" {...props}>
        <Tab value="scenes" label="Scenes" />
        <Tab value="worlds" label="Worlds" disabled />
        <Tab value="templates" label="Templates" icon={<img alt="Template glyph" />} />
        <Tab value="assets">Assets</Tab>
        <TabPanel value="scenes" keepMounted={keepMounted}>Scene list</TabPanel>
        <TabPanel value="worlds" keepMounted={keepMounted}>World list</TabPanel>
        <TabPanel value="templates" keepMounted={keepMounted}>Template list</TabPanel>
        <TabPanel value="assets" keepMounted={keepMounted}>Asset list</TabPanel>
      </Tabs>
      <button>After</button>
    </>
  );
}

const tab = (name: string) => screen.getByRole("tab", { name });

describe("Tabs", () => {
  test("the list, the tabs and the panel have their roles and names", () => {
    render(<Harness />);

    const list = screen.getByRole("tablist", { name: "Library" });
    expect(list).toHaveAttribute("aria-orientation", "horizontal");
    expect(screen.getAllByRole("tab").map((node) => node.textContent)).toEqual(["Scenes", "Worlds", "Templates", "Assets"]);
    for (const node of screen.getAllByRole("tab")) expect(list).toContainElement(node);
    expect(list).not.toContainElement(screen.getByRole("tabpanel"));
    expect(tab("Worlds")).toBeDisabled();
  });

  test("the glyph of a tab is not part of its name", () => {
    render(<Harness />);

    expect(tab("Templates")).toBeInTheDocument();
    expect(screen.queryByRole("img", { name: "Template glyph" })).toBeNull();
  });

  test("only the tab with the value of the list is selected", () => {
    render(<Harness initial="templates" />);

    expect(tab("Templates")).toHaveAttribute("aria-selected", "true");
    expect(tab("Scenes")).toHaveAttribute("aria-selected", "false");
    expect(tab("Assets")).toHaveAttribute("aria-selected", "false");
  });

  test("each tab controls its panel and each panel takes its name from its tab, with no id from the caller", () => {
    render(<Harness initial="templates" />);

    const panels = screen.getAllByRole("tabpanel", { hidden: true });
    expect(panels).toHaveLength(4);
    for (const name of ["Scenes", "Worlds", "Templates", "Assets"]) {
      const controlled = panels.filter((panel) => panel.id !== "" && panel.id === tab(name).getAttribute("aria-controls"));
      expect(controlled).toHaveLength(1);
      expect(controlled[0]).toHaveAttribute("aria-labelledby", tab(name).id);
    }
    expect(screen.getByRole("tabpanel", { name: "Templates" })).toHaveTextContent("Template list");
  });

  test("two lists on one page do not share ids", () => {
    render(<><Harness /><Harness /></>);

    const ids = screen.getAllByRole("tab").map((node) => node.id);
    expect(new Set(ids).size).toBe(ids.length);
    const panels = screen.getAllByRole("tabpanel", { hidden: true }).map((node) => node.id);
    expect(new Set(panels).size).toBe(panels.length);
  });

  test("only the panel of the selected tab shows, and the others are hidden and empty", () => {
    render(<Harness />);

    expect(screen.getAllByRole("tabpanel")).toHaveLength(1);
    expect(screen.getByRole("tabpanel", { name: "Scenes" })).toHaveTextContent("Scene list");
    expect(screen.queryByText("Template list")).toBeNull();
  });

  test("a panel that stays mounted keeps its children while it is hidden", () => {
    render(<Harness keepMounted />);

    expect(screen.getAllByRole("tabpanel")).toHaveLength(1);
    expect(screen.getByText("Template list")).not.toBeVisible();
    expect(screen.getByText("Scene list")).toBeVisible();
  });

  test("a press on a tab selects it and shows its panel", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);

    await user.click(tab("Templates"));

    expect(onChange).toHaveBeenCalledTimes(1);
    expect(onChange).toHaveBeenCalledWith("templates");
    expect(tab("Templates")).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tabpanel", { name: "Templates" })).toHaveTextContent("Template list");
  });

  test("a press on the selected tab or on a disabled tab changes nothing", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);

    await user.click(tab("Scenes"));
    await user.click(tab("Worlds"));

    expect(onChange).not.toHaveBeenCalled();
    expect(tab("Scenes")).toHaveAttribute("aria-selected", "true");
  });

  test("Tab enters the list at the selected tab and leaves it to the panel", async () => {
    const user = userEvent.setup();
    render(<Harness initial="templates" />);

    await user.tab();
    expect(screen.getByRole("button", { name: "Before" })).toHaveFocus();
    await user.tab();
    expect(tab("Templates")).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("tabpanel", { name: "Templates" })).toHaveFocus();
    await user.tab();
    expect(screen.getByRole("button", { name: "After" })).toHaveFocus();
  });

  test("ArrowRight moves focus to the next tab, skips a disabled one and goes from the last to the first", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(tab("Scenes"));

    await user.keyboard("{ArrowRight}");
    expect(tab("Templates")).toHaveFocus();
    await user.keyboard("{ArrowRight}");
    expect(tab("Assets")).toHaveFocus();
    await user.keyboard("{ArrowRight}");
    expect(tab("Scenes")).toHaveFocus();
  });

  test("ArrowLeft moves focus to the previous tab, skips a disabled one and goes from the first to the last", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(tab("Scenes"));

    await user.keyboard("{ArrowLeft}");
    expect(tab("Assets")).toHaveFocus();
    await user.keyboard("{ArrowLeft}");
    expect(tab("Templates")).toHaveFocus();
    await user.keyboard("{ArrowLeft}");
    expect(tab("Scenes")).toHaveFocus();
  });

  test("End moves focus to the last tab and Home to the first", async () => {
    const user = userEvent.setup();
    render(<Harness initial="templates" />);
    await user.click(tab("Templates"));

    await user.keyboard("{End}");
    expect(tab("Assets")).toHaveFocus();
    await user.keyboard("{Home}");
    expect(tab("Scenes")).toHaveFocus();
  });

  test("the arrow keys move focus and do not select; Enter and Space select the focused tab", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Harness onChange={onChange} />);
    await user.click(tab("Scenes"));

    await user.keyboard("{ArrowRight}");
    expect(onChange).not.toHaveBeenCalled();
    expect(tab("Scenes")).toHaveAttribute("aria-selected", "true");

    await user.keyboard("{Enter}");
    expect(onChange).toHaveBeenLastCalledWith("templates");
    expect(tab("Templates")).toHaveAttribute("aria-selected", "true");

    await user.keyboard("{ArrowRight} ");
    expect(onChange).toHaveBeenLastCalledWith("assets");
    expect(tab("Assets")).toHaveAttribute("aria-selected", "true");
  });

  test("with automatic activation the arrow keys also select", async () => {
    const user = userEvent.setup();
    const onChange = vi.fn();
    render(<Harness onChange={onChange} activation="auto" />);
    await user.click(tab("Scenes"));

    await user.keyboard("{ArrowRight}");

    expect(onChange).toHaveBeenCalledWith("templates");
    expect(tab("Templates")).toHaveAttribute("aria-selected", "true");
    expect(tab("Templates")).toHaveFocus();
    expect(screen.getByRole("tabpanel", { name: "Templates" })).toBeInTheDocument();
  });

  test("a vertical list says so and follows ArrowDown and ArrowUp, not ArrowRight and ArrowLeft", async () => {
    const user = userEvent.setup();
    render(<Harness orientation="vertical" />);
    expect(screen.getByRole("tablist")).toHaveAttribute("aria-orientation", "vertical");
    await user.click(tab("Scenes"));

    await user.keyboard("{ArrowRight}{ArrowLeft}");
    expect(tab("Scenes")).toHaveFocus();

    await user.keyboard("{ArrowDown}");
    expect(tab("Templates")).toHaveFocus();
    await user.keyboard("{ArrowUp}{ArrowUp}");
    expect(tab("Assets")).toHaveFocus();
  });

  test("a horizontal list does not follow ArrowDown and ArrowUp", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(tab("Scenes"));

    await user.keyboard("{ArrowDown}{ArrowUp}");

    expect(tab("Scenes")).toHaveFocus();
  });

  test("only the selected tab is in the tab order", async () => {
    const user = userEvent.setup();
    render(<Harness />);
    await user.click(tab("Scenes"));
    await user.keyboard("{ArrowRight}");
    expect(tab("Templates")).toHaveFocus();

    await user.tab();

    expect(screen.getByRole("tabpanel", { name: "Scenes" })).toHaveFocus();
  });

  test("a list with no panel of its own gives its tabs no panel to control", () => {
    render(
      <Tabs value="a" aria-label="Views">
        <Tab value="a" label="Grid" />
        <Tab value="b" label="List" />
      </Tabs>,
    );

    expect(tab("Grid")).not.toHaveAttribute("aria-controls");
    expect(tab("List")).not.toHaveAttribute("aria-controls");
  });

  test("a panel outside the list links to its tab through the shared id base", () => {
    render(
      <>
        <Tabs value="b" idBase="views" aria-label="Views">
          <Tab value="a" label="Grid" />
          <Tab value="b" label="List" />
        </Tabs>
        <section>
          <TabPanel value="a" idBase="views" active={false}>Grid view</TabPanel>
          <TabPanel value="b" idBase="views" active>List view</TabPanel>
        </section>
      </>,
    );

    const panel = screen.getByRole("tabpanel", { name: "List" });
    expect(panel).toHaveTextContent("List view");
    expect(tab("List")).toHaveAttribute("aria-controls", panel.id);
    expect(screen.getAllByRole("tabpanel")).toHaveLength(1);
  });

  test("ids the caller gives to a tab and a panel replace the generated ones", () => {
    render(
      <>
        <Tabs value="a" aria-label="Views">
          <Tab value="a" label="Grid" id="grid-tab" aria-controls="grid-panel" />
        </Tabs>
        <TabPanel value="a" active id="grid-panel" aria-labelledby="grid-tab">Grid view</TabPanel>
      </>,
    );

    expect(tab("Grid")).toHaveAttribute("aria-controls", "grid-panel");
    expect(screen.getByRole("tabpanel", { name: "Grid" })).toHaveTextContent("Grid view");
  });
});
