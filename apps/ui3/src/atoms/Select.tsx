import type { KeyboardEvent, ReactElement, ReactNode } from "react";
import { Children, isValidElement, useEffect, useId, useRef, useState } from "react";
import Popover from "./Popover";
import type { OverlayPlacement } from "./useOverlayPosition";
import styles from "./Select.module.css";

export type SelectOptionProps = {
  value: string;
  disabled?: boolean;
  /** Text that typing matches when the children are not plain text. */
  label?: string;
  className?: string;
  children?: ReactNode;
};

/** One choice of a `Select`. The `Select` reads its props and draws the option. */
export function SelectOption(_props: SelectOptionProps): ReactElement | null {
  return null;
}

type SelectCommonProps = {
  /** Shown when no option is selected. */
  placeholder?: ReactNode;
  disabled?: boolean;
  error?: boolean;
  size?: "small" | "medium";
  /** `plain` has no border, background or padding: the caller styles it through `className`. */
  variant?: "outlined" | "plain";
  fullWidth?: boolean;
  /** Replaces the caret. */
  icon?: ReactNode;
  /** Content between the value and the caret, outside the trigger. */
  endAdornment?: ReactNode;
  /** Controlled popup state. */
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  placement?: OverlayPlacement;
  /** A backdrop with no paint takes the press outside the open list: it closes the list and reaches no other control. */
  modal?: boolean;
  /** Stop the page behind from scrolling while the list is open. The default is the value of `modal`. */
  lockScroll?: boolean;
  id?: string;
  /** Adds a hidden input, so the value goes with a native form. */
  name?: string;
  className?: string;
  /** Class of the popup: carries the token scope into the portal. */
  menuClassName?: string;
  "aria-label"?: string;
  "aria-labelledby"?: string;
  "aria-describedby"?: string;
  /** `SelectOption` elements. Any other child is drawn in the popup as it is. */
  children?: ReactNode;
};

type SelectSingleProps<V extends string> = {
  multiple?: false;
  value: V | "" | null | undefined;
  onChange?: (value: V) => void;
  renderValue?: (value: V) => ReactNode;
  maxSelected?: never;
};

type SelectMultipleProps<V extends string> = {
  multiple: true;
  value: readonly V[];
  onChange?: (value: V[]) => void;
  renderValue?: (value: V[]) => ReactNode;
  /** A choice over this count is ignored. */
  maxSelected?: number;
};

export type SelectProps<V extends string = string> = SelectCommonProps & (SelectSingleProps<V> | SelectMultipleProps<V>);

const TYPEAHEAD_RESET = 500;
const OPEN_KEYS = ["Enter", " ", "ArrowDown", "ArrowUp", "Home", "End"];

const isOption = (node: unknown): node is ReactElement<SelectOptionProps> => isValidElement(node) && node.type === SelectOption;
const textOf = (option: SelectOptionProps) =>
  option.label ?? (typeof option.children === "string" || typeof option.children === "number" ? String(option.children) : option.value);

/** A single or multiple choice from a popup list. Focus stays on the trigger; the list follows the keys. */
export function Select<V extends string = string>(props: SelectProps<V>) {
  const {
    placeholder, disabled = false, error = false, size = "medium", variant = "outlined", fullWidth = false, icon, endAdornment,
    open: openProp, onOpenChange, placement = "bottom-start", modal = false, lockScroll, id, name, className, menuClassName, children,
    "aria-label": ariaLabel, "aria-labelledby": ariaLabelledBy, "aria-describedby": ariaDescribedBy,
  } = props;
  const [anchor, setAnchor] = useState<HTMLDivElement | null>(null);
  const baseId = useId();
  const listId = baseId + "list";
  const optionId = (index: number) => baseId + "option-" + index;
  const [openState, setOpenState] = useState(false);
  const [active, setActive] = useState(-1);
  const typed = useRef({ text: "", at: 0 });

  const nodes = Children.toArray(children);
  const options = nodes.filter(isOption).map((node) => node.props);
  const selected: readonly string[] = props.multiple ? props.value : props.value ? [props.value] : [];
  const open = !disabled && (openProp ?? openState);

  const setOpen = (next: boolean) => {
    if (openProp === undefined) setOpenState(next);
    if (next !== open) onOpenChange?.(next);
  };
  const enabled = options.flatMap((option, index) => (option.disabled ? [] : [index]));
  const firstSelected = () => {
    const index = options.findIndex((option) => !option.disabled && selected.includes(option.value));
    return index >= 0 ? index : enabled[0] ?? -1;
  };
  const openAt = (index: number) => { setActive(index); setOpen(true); };

  // A popup opened by the parent starts at the selected option, like one opened from the trigger.
  useEffect(() => { if (open) setActive((current) => (current < 0 ? firstSelected() : current)); else setActive(-1); }, [open]);
  useEffect(() => {
    if (open && active >= 0) document.getElementById(optionId(active))?.scrollIntoView?.({ block: "nearest" });
  }, [open, active]);

  function choose(index: number) {
    const option = options[index];
    if (!option || option.disabled) return;
    if (props.multiple) {
      const has = props.value.includes(option.value as V);
      if (!has && props.maxSelected !== undefined && props.value.length >= props.maxSelected) return;
      props.onChange?.(has ? props.value.filter((value) => value !== option.value) : [...props.value, option.value as V]);
      return;
    }
    if (option.value !== props.value) props.onChange?.(option.value as V);
    setOpen(false);
  }

  function typeahead(key: string): number {
    const now = Date.now();
    const text = (now - typed.current.at > TYPEAHEAD_RESET ? "" : typed.current.text) + key.toLowerCase();
    typed.current = { text, at: now };
    const at = enabled.indexOf(active);
    const from = text.length === 1 ? at + 1 : Math.max(at, 0);
    const order = [...enabled.slice(from), ...enabled.slice(0, from)];
    return order.find((index) => textOf(options[index]!).trim().toLowerCase().startsWith(text)) ?? -1;
  }

  function onKeyDown(event: KeyboardEvent<HTMLButtonElement>) {
    const { key } = event;
    const printable = key.length === 1 && key !== " " && !event.ctrlKey && !event.metaKey && !event.altKey;
    if (!open) {
      if (OPEN_KEYS.includes(key)) {
        event.preventDefault();
        openAt(key === "Home" ? enabled[0] ?? -1 : key === "End" ? enabled[enabled.length - 1] ?? -1 : firstSelected());
      } else if (printable) {
        const hit = typeahead(key);
        if (hit >= 0) openAt(hit);
      }
      return;
    }
    const at = enabled.indexOf(active);
    if (key === "ArrowDown") setActive(enabled[Math.min(at + 1, enabled.length - 1)] ?? -1);
    else if (key === "ArrowUp") setActive(enabled[Math.max(at - 1, 0)] ?? -1);
    else if (key === "Home") setActive(enabled[0] ?? -1);
    else if (key === "End") setActive(enabled[enabled.length - 1] ?? -1);
    else if (key === "Enter" || key === " ") choose(active);
    else if (key === "Tab") return setOpen(false);
    else if (printable) {
      const hit = typeahead(key);
      if (hit >= 0) setActive(hit);
      return;
    } else return;
    event.preventDefault();
  }

  let display: ReactNode = null;
  if (selected.length > 0) {
    if (props.multiple) {
      display = props.renderValue
        ? props.renderValue([...props.value])
        : options.filter((option) => selected.includes(option.value)).map(textOf).join(", ");
    } else {
      const current = options.find((option) => option.value === props.value);
      display = props.renderValue ? props.renderValue(props.value as V) : current?.children ?? current?.label ?? null;
    }
  }
  const empty = display === null || display === undefined || display === "";

  let optionIndex = -1;
  return (
    <div
      ref={setAnchor}
      className={styles.root + (className ? " " + className : "")}
      data-variant={variant} data-size={size} data-open={open || undefined} data-disabled={disabled || undefined}
      data-error={error || undefined} data-full-width={fullWidth || undefined}
    >
      <button
        type="button" role="combobox" id={id} className={styles.trigger} disabled={disabled}
        aria-haspopup="listbox" aria-expanded={open} aria-controls={open ? listId : undefined}
        aria-activedescendant={open && active >= 0 ? optionId(active) : undefined}
        aria-label={ariaLabel} aria-labelledby={ariaLabelledBy} aria-describedby={ariaDescribedBy} aria-invalid={error || undefined}
        onClick={() => (open ? setOpen(false) : openAt(firstSelected()))}
        onKeyDown={onKeyDown}
        // Space activates a button on key up; the key down already used it.
        onKeyUp={(event) => { if (event.key === " ") event.preventDefault(); }}
      >
        <span className={styles.value} data-placeholder={empty || undefined}>{empty ? placeholder : display}</span>
      </button>
      {endAdornment}
      <span className={styles.caret} aria-hidden="true">{icon ?? <span className={styles.chevron} />}</span>
      {name !== undefined ? <input type="hidden" name={name} value={selected.join(",")} /> : null}
      <Popover
        open={open} anchor={anchor} placement={placement} offset={4} matchAnchorWidth returnFocus={false}
        modal={modal} lockScroll={lockScroll}
        onClose={() => setOpen(false)}
        role="listbox" id={listId} tabIndex={-1} className={styles.list + (menuClassName ? " " + menuClassName : "")}
        aria-multiselectable={props.multiple || undefined} aria-label={ariaLabel} aria-labelledby={ariaLabelledBy}
        // Focus stays on the trigger, which owns the keys.
        onMouseDown={(event) => event.preventDefault()}
      >
        {nodes.map((node) => {
          if (!isOption(node)) return node;
          const index = ++optionIndex;
          const option = node.props;
          return (
            <div
              key={node.key ?? option.value}
              role="option" id={optionId(index)}
              className={styles.option + (option.className ? " " + option.className : "")}
              aria-selected={selected.includes(option.value)} aria-disabled={option.disabled || undefined}
              data-active={index === active || undefined}
              onClick={() => choose(index)}
              onPointerMove={option.disabled || index === active ? undefined : () => setActive(index)}
            >
              {option.children ?? option.label ?? option.value}
            </div>
          );
        })}
      </Popover>
    </div>
  );
}

export default Select;
