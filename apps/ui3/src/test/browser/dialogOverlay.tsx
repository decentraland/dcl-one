// A page for Dialog.browser.test.ts: a dialog with a select and a menu in it, run in a real browser.
import { useState } from "react";
import { createRoot } from "react-dom/client";
import "../../atoms/primitives.css";
import Dialog from "../../atoms/Dialog";
import { Menu, MenuItem } from "../../atoms/Menu";
import { Select, SelectOption } from "../../atoms/Select";

declare global {
  interface Window { dialogCloses: string[] }
}
window.dialogCloses = [];

function Page() {
  const [open, setOpen] = useState(true);
  const [size, setSize] = useState("small");
  const [anchor, setAnchor] = useState<HTMLButtonElement | null>(null);
  const [menuOpen, setMenuOpen] = useState(false);
  return (
    <Dialog open={open} title="Settings" size="tiny" onClose={(reason) => { window.dialogCloses.push(reason); setOpen(false); }}>
      <Select aria-label="Size" value={size} onChange={setSize}>
        <SelectOption value="small">Small</SelectOption>
        <SelectOption value="large">Large</SelectOption>
      </Select>
      <button type="button" data-trigger="menu" ref={setAnchor} onClick={() => setMenuOpen((value) => !value)}>Actions</button>
      <Menu open={menuOpen} anchor={anchor} onClose={() => setMenuOpen(false)} aria-label="Actions">
        <MenuItem>Rename</MenuItem>
      </Menu>
    </Dialog>
  );
}

createRoot(document.getElementById("root")!).render(<Page />);
