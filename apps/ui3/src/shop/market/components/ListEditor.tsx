// from: marketplace@f3956826:webapp/src/components/Modals/CreateOrEditListModal, webapp/src/modules/favorites/sagas.ts
//       (CREATE_LIST, UPDATE_LIST, DELETE_LIST_START/REQUEST); catalyrst-v2 ui3/src/marketplace/pages/MkListsPage.tsx
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.
// Create or edit a list: name, description, and whether it is private (only you) or shared (anyone with
// the link). Deleting asks first. Limits are the Marketplace's.

import { useQueryClient } from "@tanstack/react-query";
import { useId, useState } from "react";

import Button from "../../../atoms/Button";
import { createList, deleteList, LIST_DESCRIPTION_MAX, LIST_NAME_MAX, updateList, validateListDraft, type FavoriteList, type ListDraft } from "../lib/favorites";
import { useWrite, WriteStatus } from "./kit";

const PROBLEM_TEXT = {
  nameRequired: "Give the list a name.",
  nameTooLong: `Names are at most ${LIST_NAME_MAX} characters.`,
  descriptionTooLong: `Descriptions are at most ${LIST_DESCRIPTION_MAX} characters.`,
} as const;

export function ListEditor({ list, onSaved, onCancel }: { list?: FavoriteList; onSaved: (list: FavoriteList) => void; onCancel: () => void }) {
  const qc = useQueryClient();
  const id = useId();
  const [draft, setDraft] = useState<ListDraft>({ name: list?.name ?? "", description: list?.description ?? "", isPrivate: list?.isPrivate ?? true });
  const [touched, setTouched] = useState(false);
  const write = useWrite();
  const problem = validateListDraft(draft);
  return (
    <form
      className="mk-panel mk-form"
      aria-label={list ? "Edit list" : "Create a list"}
      onSubmit={(e) => {
        e.preventDefault();
        setTouched(true);
        if (problem) return;
        void write.run(list ? "Saving…" : "Creating…", list ? "Saved." : "Created.", async () => {
          const saved = list ? await updateList(list.id, draft) : await createList(draft);
          await qc.invalidateQueries({ queryKey: ["favorite-lists"] });
          await qc.invalidateQueries({ queryKey: ["favorite-list", saved.id] });
          onSaved(saved);
          return null;
        });
      }}
    >
      <h2 className="mk-panel__title">{list ? "Edit list" : "Create a list"}</h2>
      <label className="mk-field" htmlFor={`${id}-name`}>
        <span className="mk-field__label">Name</span>
        <input id={`${id}-name`} value={draft.name} maxLength={LIST_NAME_MAX + 8} onChange={(e) => setDraft({ ...draft, name: e.target.value })} disabled={write.busy} />
        {touched && problem && problem !== "descriptionTooLong" ? <p className="mk-field__error">{PROBLEM_TEXT[problem]}</p> : null}
      </label>
      <label className="mk-field" htmlFor={`${id}-description`}>
        <span className="mk-field__label">Description (optional)</span>
        <textarea id={`${id}-description`} rows={3} value={draft.description} onChange={(e) => setDraft({ ...draft, description: e.target.value })} disabled={write.busy} />
        {touched && problem === "descriptionTooLong" ? <p className="mk-field__error">{PROBLEM_TEXT.descriptionTooLong}</p> : null}
      </label>
      <fieldset className="mk-field">
        <legend className="mk-field__label">Who can see it</legend>
        <label className="mk-inline-check">
          <input type="radio" name={`${id}-privacy`} checked={draft.isPrivate} onChange={() => setDraft({ ...draft, isPrivate: true })} /> Private: only you
        </label>
        <label className="mk-inline-check">
          <input type="radio" name={`${id}-privacy`} checked={!draft.isPrivate} onChange={() => setDraft({ ...draft, isPrivate: false })} /> Shared: anyone with the link
        </label>
      </fieldset>
      <div className="mk-actions">
        <Button type="submit" disabled={write.busy}>
          {list ? "Save" : "Create list"}
        </Button>
        <Button variant="ghost" onClick={onCancel} disabled={write.busy}>
          Cancel
        </Button>
      </div>
      <WriteStatus state={write.state} />
    </form>
  );
}

export function DeleteList({ list, onDeleted }: { list: FavoriteList; onDeleted: () => void }) {
  const qc = useQueryClient();
  const [asking, setAsking] = useState(false);
  const write = useWrite();
  if (!asking)
    return (
      <Button variant="ghost" tone="danger" size="sm" onClick={() => setAsking(true)}>
        Delete list
      </Button>
    );
  return (
    <div className="mk-note mk-note--warning" role="alertdialog" aria-label="Delete list">
      <p>Delete “{list.name}”? The items stay in the Shop; only the list goes.</p>
      <div className="mk-actions">
        <Button
          size="sm"
          tone="danger"
          disabled={write.busy}
          onClick={() =>
            void write.run("Deleting…", "Deleted.", async () => {
              await deleteList(list.id);
              await qc.invalidateQueries({ queryKey: ["favorite-lists"] });
              onDeleted();
              return null;
            })
          }
        >
          Delete
        </Button>
        <Button size="sm" variant="ghost" onClick={() => setAsking(false)} disabled={write.busy}>
          Keep it
        </Button>
      </div>
      <WriteStatus state={write.state} />
    </div>
  );
}
