import { actionChip, triggerChip } from "../interaction-chip-label";

export interface RibbonWiringRow {
  trigger: string | null;
  actions: string[];
}

export interface RibbonWiringState {
  smart: boolean;
  wired: boolean;
  rows: RibbonWiringRow[];
}

interface RibbonWiringProps {
  state: RibbonWiringState;
  onOpen?: () => void;
}

const SHOWN_ROWS = 2;

function WiringRow({ row, onOpen }: { row: RibbonWiringRow; onOpen?: () => void }) {
  const [first, ...rest] = row.actions;
  return (
    <button
      type="button"
      className="rb-wiring"
      onClick={onOpen}
      disabled={onOpen === undefined}
      title={first === undefined ? "Nothing reacts until an action is added" : "Open the interactions panel"}
    >
      <span className="rb-wire-chip trigger">{triggerChip(row.trigger)}</span>
      <span className="rb-wire-arrow" aria-hidden="true">
        {"\u{2192}"}
      </span>
      <span className="rb-wire-chip action">{actionChip(first)}</span>
      {rest.length > 0 && <span className="rb-wire-more">+{rest.length}</span>}
    </button>
  );
}

export default function RibbonWiring({ state, onOpen }: RibbonWiringProps) {
  if (!state.smart || state.rows.length === 0) {
    return (
      <div className="rb-wiring-rows">
        <span className="rb-hint">
          {state.smart ? "Nothing wired yet \u{2014} it does not react to anything." : "Not interactive yet."}
        </span>
        <button type="button" className="rb-cmd" onClick={onOpen} disabled={onOpen === undefined}>
          {state.smart ? "Add a trigger" : "Add interaction"}
        </button>
      </div>
    );
  }
  const shown = state.rows.slice(0, state.rows.length > SHOWN_ROWS ? SHOWN_ROWS - 1 : SHOWN_ROWS);
  const hidden = state.rows.length - shown.length;
  return (
    <div className="rb-wiring-rows">
      {shown.map((row, i) => (
        <WiringRow key={i} row={row} onOpen={onOpen} />
      ))}
      {hidden > 0 && (
        <button type="button" className="rb-wiring rb-wiring-more" onClick={onOpen} disabled={onOpen === undefined}>
          +{hidden} more interaction{hidden === 1 ? "" : "s"}
        </button>
      )}
    </div>
  );
}
