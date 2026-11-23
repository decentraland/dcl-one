import { useEffect, useState } from "react";

import { Button, Modal } from "@dcl/one-ui";
import { GeneralDelete } from "@dcl/one-ui/foundations/icons";
import { useMutation } from "@tanstack/react-query";

import { magicApiKey } from "@/data/lib/auth/magic-config";
import { hasMagicSession, magicClient } from "@/data/lib/auth/magic-session";
import { getIdentity } from "@/data/lib/auth/session";
import { signedFetch } from "@/data/lib/auth/signer";

import styles from "./AccountSidebar.module.css";

export function DeleteMagicAccount({ address, onSignOut }: { address: string; onSignOut: () => void }) {
  const [available, setAvailable] = useState(false);
  useEffect(() => {
    setAvailable(Boolean(magicApiKey()) && hasMagicSession());
  }, []);

  return available ? <DeleteMagicAccountAction address={address} onSignOut={onSignOut} /> : null;
}

function DeleteMagicAccountAction({ address, onSignOut }: { address: string; onSignOut: () => void }) {
  const [open, setOpen] = useState(false);
  const deletion = useMutation({
    mutationFn: async () => {
      const identity = getIdentity();
      if (!identity || identity.signer.toLowerCase() !== address.toLowerCase()) {
        throw new Error("Sign in again to delete this account.");
      }
      const magic = await magicClient(magicApiKey());
      if (!(await magic.user.isLoggedIn())) {
        throw new Error("Sign in with your Magic account again to continue.");
      }
      const didToken = await magic.user.getIdToken({ lifespan: 120 });
      const response = await signedFetch(identity, "/auth-api/accounts", {
        method: "DELETE",
        metadata: { didToken },
      });
      if (!response.ok) throw new Error("Magic could not complete the deletion. Please try again.");
    },
    onSuccess: () => {
      setOpen(false);
      onSignOut();
    },
  });

  const close = () => {
    deletion.reset();
    setOpen(false);
  };

  return (
    <>
      <button type="button" className={styles.item} onClick={() => setOpen(true)}>
        <GeneralDelete size={20} aria-hidden="true" />
        <span className={styles.itemLabel}>Delete Magic account</span>
      </button>
      {open && (
        <Modal
          title="Delete your Magic account?"
          description="This permanently removes Magic access to your wallet. Export your wallet key before continuing to keep access to your assets."
          onClose={close}
          dismissible={!deletion.isPending}
          actions={
            <>
              <Button variant="secondary" disabled={deletion.isPending} onClick={close}>Cancel</Button>
              <Button loading={deletion.isPending} onClick={() => deletion.mutate()}>Delete Magic account</Button>
            </>
          }
        >
          {deletion.error && <p role="alert">{deletion.error.message}</p>}
        </Modal>
      )}
    </>
  );
}
