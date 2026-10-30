import { useRef, useState } from "react";
import Button from "../../atoms/Button";
import ContextMenu from "../../components/ContextMenu";
import Modal from "../../components/Modal";

export default function WearableDraftActions({ name, onDelete }: { name: string; onDelete: () => Promise<void> }) {
  const [menu, setMenu] = useState(false);
  const [confirm, setConfirm] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const trigger = useRef<HTMLButtonElement>(null);
  const deleting = useRef(false);
  function closeMenu() { setMenu(false); trigger.current?.focus(); }
  async function remove() {
    if (deleting.current) return;
    deleting.current = true;
    setBusy(true);
    setError("");
    try { await onDelete(); setConfirm(false); }
    catch (error) { setError(error instanceof Error ? error.message : "Could not delete this draft. Try again."); }
    finally { deleting.current = false; setBusy(false); }
  }
  return <div className="cwh__draft-actions" onBlur={event => {
    if (!event.currentTarget.contains(event.relatedTarget)) setMenu(false);
  }}>
    <button ref={trigger} type="button" aria-label={`Draft actions for ${name}`} aria-haspopup="menu"
      aria-expanded={menu} onClick={() => setMenu(value => !value)}>⋯</button>
    {menu && <ContextMenu autoFocus onClose={closeMenu} items={[
      { kind: "button", label: "Delete draft", danger: true, onClick: () => { closeMenu(); setError(""); setConfirm(true); } },
    ]} />}
    {confirm && <Modal ariaLabel="Delete wearable draft?" role="alertdialog"
      onClose={busy ? undefined : () => setConfirm(false)} closeOnBackdrop={!busy} closeOnEsc={!busy}>
      <h2>Delete wearable draft?</h2>
      <p>“{name}” will be removed from your drafts. This cannot be undone.</p>
      {error && <p role="alert">{error}</p>}
      <div className="cwh__delete-buttons">
        <Button variant="secondary" disabled={busy} onClick={() => setConfirm(false)}>Cancel</Button>
        <Button disabled={busy} onClick={remove}>{busy ? "Deleting…" : "Delete draft"}</Button>
      </div>
    </Modal>}
  </div>;
}
