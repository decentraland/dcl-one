import { useEffect, useState } from "react";
import "./toggle.css";

type ToggleProps = {
  checked?: boolean;
  defaultChecked?: boolean;
  onChange?: (checked: boolean) => void;
  ariaLabel?: string;
  ariaLabelledBy?: string;
  disabled?: boolean;
  id?: string;
  className?: string;
};

// A host that is not built by Vite has no `import.meta.env`.
function isDev(): boolean {
  return typeof import.meta.env !== "undefined" && import.meta.env.DEV;
}

export default function Toggle({ checked, defaultChecked = false, onChange, ariaLabel, ariaLabelledBy, disabled = false, id, className }: ToggleProps) {
  const [internal, setInternal] = useState(defaultChecked);
  const isControlled = checked !== undefined;
  const on = isControlled ? checked : internal;

  useEffect(() => {
    if (isDev() && !ariaLabel && !ariaLabelledBy) {
      console.warn(
        "Toggle: rendered without ariaLabel/ariaLabelledBy \u{2014} the switch has no accessible name. Pass ariaLabel (or ariaLabelledBy) so screen readers can announce what it controls."
      );
    }
  }, [ariaLabel, ariaLabelledBy]);

  function toggle() {
    if (!isControlled) setInternal(!on);
    onChange?.(!on);
  }

  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={ariaLabel}
      aria-labelledby={ariaLabelledBy}
      id={id}
      className={"toggle" + (on ? " is-on" : "") + (className ? " " + className : "")}
      disabled={disabled}
      onClick={toggle}
    >
      <span className="toggle__knob" />
    </button>
  );
}
