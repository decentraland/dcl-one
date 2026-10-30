// from: marketplace@f3956826:webapp/src/components/Modals/SaveToListModal, webapp/src/modules/favorites/sagas.ts (BULK_PICK_START, BULK_PICK_REQUEST)
// Copyright 2020 Metaverse Holdings Ltd. (marketplace), Apache-2.0.
// Save an item into any of the signed-in account's lists at once: the lists come back marked with
// whether the item is already in each, and saving sends one bulk pick/unpick for the difference.

import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useEffect, useState } from "react";

import Button from "../../../atoms/Button";
import { bulkPick, fetchLists } from "../lib/favorites";
import { ListEditor } from "./ListEditor";
import { LoadFailure, Loading, useWrite, WriteStatus } from "./kit";

export default function SaveToLists({ itemKey, itemName, onClose }: { itemKey: string; itemName: string; onClose: () => void }) {
  const qc = useQueryClient();
  const lists = useQuery({ queryKey: ["favorite-lists", "for-item", itemKey], queryFn: () => fetchLists({ first: 100, itemId: itemKey }) });
  const [chosen, setChosen] = useState<Record<string, boolean>>({});
  const [creating, setCreating] = useState(false);
  const write = useWrite();
  useEffect(() => {
    if (lists.data) setChosen(Object.fromEntries(lists.data.results.map((l) => [l.id, !!l.isItemInList])));
  }, [lists.data]);

  if (lists.isPending) return <Loading label="Loading your lists…" />;
  if (lists.isError) return <LoadFailure error={lists.error} what="Your lists" onRetry={() => void lists.refetch()} />;
  const before = Object.fromEntries(lists.data.results.map((l) => [l.id, !!l.isItemInList]));
  const pickedFor = Object.keys(chosen).filter((id) => chosen[id] && !before[id]);
  const unpickedFrom = Object.keys(chosen).filter((id) => !chosen[id] && before[id]);

  return (
    <div className="mk-panel" role="group" aria-label={`Save ${itemName} to lists`} data-testid="shop-save-to-lists">
      <h2 className="mk-panel__title">Save to lists</h2>
      <ul className="mk-list">
        {lists.data.results.map((l) => (
          <li key={l.id}>
            <label className="mk-inline-check">
              <input type="checkbox" checked={!!chosen[l.id]} onChange={(e) => setChosen((c) => ({ ...c, [l.id]: e.target.checked }))} disabled={write.busy} />
              {l.isDefaultList ? "Favorites" : l.name} <span className="mk-badge">{l.isPrivate ? "Private" : "Shared"}</span>
            </label>
          </li>
        ))}
      </ul>
      {creating ? (
        <ListEditor
          onCancel={() => setCreating(false)}
          onSaved={(list) => {
            setCreating(false);
            setChosen((c) => ({ ...c, [list.id]: true }));
            void lists.refetch();
          }}
        />
      ) : (
        <Button size="sm" variant="ghost" onClick={() => setCreating(true)}>
          Create a list
        </Button>
      )}
      <div className="mk-actions">
        <Button
          disabled={write.busy || (pickedFor.length === 0 && unpickedFrom.length === 0)}
          onClick={() =>
            void write.run("Saving…", "Saved.", async () => {
              await bulkPick(itemKey, pickedFor, unpickedFrom);
              await qc.invalidateQueries({ queryKey: ["favorite-lists"] });
              await qc.invalidateQueries({ queryKey: ["favorite-list-items"] });
              onClose();
              return null;
            })
          }
        >
          Save
        </Button>
        <Button variant="ghost" onClick={onClose} disabled={write.busy}>
          Cancel
        </Button>
      </div>
      <WriteStatus state={write.state} />
    </div>
  );
}
