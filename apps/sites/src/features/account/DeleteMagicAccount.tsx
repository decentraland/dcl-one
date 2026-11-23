import { useEffect, useState } from "react";

import { Button, Modal } from "@dcl/one-ui";
import { GeneralDelete } from "@dcl/one-ui/foundations/icons";
import { useMutation } from "@tanstack/react-query";

import { magicApiKey } from "@/data/lib/auth/magic-config";
import { hasMagicSession, magicClient } from "@/data/lib/auth/magic-session";
import { getIdentity } from "@/data/lib/auth/session";
import { signedFetch } from "@/data/lib/auth/signer";

import styles from "./AccountSidebar.module.css";
import { ACCOUNT_COPY } from "./copy";

const copy = ACCOUNT_COPY.magicDeletion;

function deletionError(status: number): string {
  switch (status) {
    case 403: return copy.forbidden;
    case 429: return copy.rateLimited;
    case 502: return copy.unknown;
    default: return copy.failed;
  }
}

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
        throw new Error(copy.signInAgain);
      }
      const magic = await magicClient(magicApiKey());
      if (!(await magic.user.isLoggedIn())) {
        throw new Error(copy.magicSignIn);
      }
      const didToken = await magic.user.getIdToken({ lifespan: 120 });
      const response = await signedFetch(identity, "/auth-api/accounts", {
        method: "DELETE",
        metadata: { didToken },
      });
      await response.body?.cancel().catch(() => {});
      if (!response.ok) throw new Error(deletionError(response.status));
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
        <span className={styles.itemLabel}>{copy.action}</span>
      </button>
      {open && (
        <Modal
          title={copy.title}
          description={copy.description}
          onClose={close}
          dismissible={!deletion.isPending}
          actions={
            <>
              <Button variant="secondary" disabled={deletion.isPending} onClick={close}>{copy.cancel}</Button>
              <Button loading={deletion.isPending} onClick={() => deletion.mutate()}>{copy.action}</Button>
            </>
          }
        >
          {deletion.error && <p role="alert">{deletion.error.message}</p>}
        </Modal>
      )}
    </>
  );
}
