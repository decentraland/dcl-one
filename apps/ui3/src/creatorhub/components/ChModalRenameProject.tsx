import { useState } from "react";
import Modal from "../../components/Modal";
import Button from "../../atoms/Button";
import "./chmodaldeleteproject.css";

export default function ChModalRenameProject({ title, onClose, onSubmit }: {
  title: string; onClose: () => void; onSubmit: (title: string) => Promise<void>;
}) {
  const [name, setName] = useState(title);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  return <Modal width={540} className="chmodaldeleteproject" ariaLabel="Rename scene" onClose={() => { if (!busy) onClose(); }}>
    <form onSubmit={async event => {
      event.preventDefault();
      if (busy || !name.trim()) return;
      setBusy(true); setError("");
      try { await onSubmit(name.trim()); onClose(); }
      catch (error) { setError(error instanceof Error ? error.message : "The scene could not be renamed."); }
      finally { setBusy(false); }
    }}>
      <h2 className="chmodaldeleteproject__title">Rename scene</h2>
      <label className="chmodaldeleteproject__name">Scene name<input autoFocus value={name} maxLength={512} required disabled={busy} onChange={event => setName(event.target.value)} /></label>
      {error && <p role="alert">{error}</p>}
      <div className="chmodaldeleteproject__actions">
        <Button variant="secondary" type="button" disabled={busy} onClick={onClose}>Cancel</Button>
        <Button variant="primary" type="submit" disabled={busy || !name.trim()}>{busy ? "Saving…" : "Save"}</Button>
      </div>
    </form>
  </Modal>;
}
