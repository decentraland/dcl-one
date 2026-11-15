import type { ButtonHTMLAttributes, HTMLAttributes, KeyboardEvent, ReactElement, ReactNode } from "react";
import { Children, createContext, isValidElement, useContext, useId, useRef } from "react";
import styles from "./Tabs.module.css";

type TabsContextValue = {
  value: string;
  /** Start of the ids of the tabs and the panels: the `idBase` of the caller, or a generated one. */
  idBase: string;
  /** True when a panel with this value has the id the tab would name. */
  hasPanel: (value: string) => boolean;
  select: (value: string) => void;
};

const TabsContext = createContext<TabsContextValue | null>(null);

const tabId = (idBase: string, value: string) => `${idBase}-tab-${value}`;
const panelId = (idBase: string, value: string) => `${idBase}-panel-${value}`;

export type TabsProps = {
  /** The `value` of the selected `Tab`. */
  value: string;
  onChange?: (value: string) => void;
  orientation?: "horizontal" | "vertical";
  /** One row that scrolls when the tabs are wider than the list. */
  scrollable?: boolean;
  /** `auto` selects a tab when the arrow keys move focus to it; `manual` waits for Enter, Space or a click. */
  activation?: "manual" | "auto";
  /**
   * Only for panels rendered outside the list: links each tab to the `TabPanel` with the same `idBase` and
   * `value`. Render a panel for every tab.
   */
  idBase?: string;
  className?: string;
  "aria-label"?: string;
  "aria-labelledby"?: string;
  /**
   * `Tab` elements, and the `TabPanel` elements of the list as direct children: those are drawn after the
   * list, linked to their tabs with generated ids, and the panel of the selected tab is the active one.
   */
  children?: ReactNode;
};

const isPanel = (node: unknown): node is ReactElement<TabPanelProps> => isValidElement(node) && node.type === TabPanel;

/** The list of tabs. One tab is in the tab order; the arrow keys move between the others. */
export function Tabs({
  value, onChange, orientation = "horizontal", scrollable = false, activation = "manual", idBase, className, children, ...aria
}: TabsProps) {
  const list = useRef<HTMLDivElement | null>(null);
  const generatedId = useId();
  const nodes = Children.toArray(children);
  const panels = nodes.filter(isPanel);
  const context: TabsContextValue = {
    value,
    idBase: idBase ?? generatedId,
    // With an `idBase` the caller renders the panels; with none, only the panels drawn here have the id.
    hasPanel: (panel) => idBase !== undefined || panels.some((node) => node.props.value === panel),
    select: (next) => { if (next !== value) onChange?.(next); },
  };

  function onKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    const tabs = list.current ? Array.from(list.current.querySelectorAll<HTMLButtonElement>('[role="tab"]:not(:disabled)')) : [];
    if (tabs.length === 0) return;
    const current = document.activeElement instanceof HTMLButtonElement ? tabs.indexOf(document.activeElement) : -1;
    const [back, forward] = orientation === "vertical" ? ["ArrowUp", "ArrowDown"] : ["ArrowLeft", "ArrowRight"];
    let next: HTMLButtonElement | undefined;
    if (event.key === forward) next = tabs[(current + 1) % tabs.length];
    else if (event.key === back) next = tabs[(current - 1 + tabs.length) % tabs.length];
    else if (event.key === "Home") next = tabs[0];
    else if (event.key === "End") next = tabs[tabs.length - 1];
    else return;
    event.preventDefault();
    next?.focus();
    if (activation === "auto" && next?.dataset.value !== undefined) onChange?.(next.dataset.value);
  }

  return (
    <TabsContext.Provider value={context}>
      <div
        {...aria} ref={list} role="tablist" aria-orientation={orientation}
        className={styles.list + (className ? " " + className : "")}
        data-orientation={orientation} data-scrollable={scrollable || undefined}
        onKeyDown={onKeyDown}
      >
        {nodes.filter((node) => !isPanel(node))}
      </div>
      {panels}
    </TabsContext.Provider>
  );
}

export type TabProps = Omit<ButtonHTMLAttributes<HTMLButtonElement>, "type" | "role" | "value"> & {
  value: string;
  /** The text of the tab; `children` replace it. */
  label?: ReactNode;
  /** Decorative glyph before the label. */
  icon?: ReactNode;
};

export function Tab({ value, label, icon, className, onClick, children, id, "aria-controls": controls, ...rest }: TabProps) {
  const tabs = useContext(TabsContext);
  const selected = tabs?.value === value;
  return (
    <button
      {...rest}
      type="button" role="tab"
      id={id ?? (tabs ? tabId(tabs.idBase, value) : undefined)}
      aria-selected={selected} aria-controls={controls ?? (tabs?.hasPanel(value) ? panelId(tabs.idBase, value) : undefined)}
      tabIndex={selected ? 0 : -1}
      className={styles.tab + (className ? " " + className : "")}
      data-value={value}
      onClick={(event) => { onClick?.(event); if (!event.defaultPrevented) tabs?.select(value); }}
    >
      {icon ? <span className={styles.icon} aria-hidden="true">{icon}</span> : null}
      {children ?? label}
    </button>
  );
}

export type TabPanelProps = Omit<HTMLAttributes<HTMLDivElement>, "role" | "hidden"> & {
  value: string;
  /** An inactive panel is hidden. A direct child of `Tabs` is active when its tab is selected; `active` replaces that. */
  active?: boolean;
  /** For a panel outside its `Tabs`: the same `idBase` as the `Tabs`, so the panel takes its name from its tab. */
  idBase?: string;
  /** Keep the children of an inactive panel mounted. */
  keepMounted?: boolean;
};

export function TabPanel({
  value, active: activeProp, idBase: idBaseProp, keepMounted = false, className, children, id, "aria-labelledby": labelledBy, ...rest
}: TabPanelProps) {
  const tabs = useContext(TabsContext);
  const idBase = idBaseProp ?? tabs?.idBase;
  const active = activeProp ?? tabs?.value === value;
  return (
    <div
      tabIndex={0}
      {...rest}
      role="tabpanel" hidden={!active}
      id={id ?? (idBase ? panelId(idBase, value) : undefined)}
      aria-labelledby={labelledBy ?? (idBase ? tabId(idBase, value) : undefined)}
      className={styles.panel + (className ? " " + className : "")}
    >
      {active || keepMounted ? children : null}
    </div>
  );
}

export default Tabs;
