import { useEffect, useState } from "react";

import { Button, Modal } from "@dcl/one-ui";
import { GeneralDelete } from "@dcl/one-ui/foundations/icons";
import { useMutation } from "@tanstack/react-query";

import { magicApiKey } from "@/data/lib/auth/magic-config";
import { hasMagicSession, magicClient } from "@/data/lib/auth/magic-session";
import { getIdentity } from "@/data/lib/auth/session";
import { signRequest } from "@/data/lib/auth/signer";

import styles from "./AccountSidebar.module.css";
import { ACCOUNT_COPY } from "./copy";

const copy = ACCOUNT_COPY.magicDeletion;

class DeletionError extends Error {
  constructor(message: string, readonly support = false) {
    super(message);
  }
}

/** The relay's 403s share a status, so its `error` field tells them apart. */
const FORBIDDEN = new Map<unknown, string>([
  ["Origin not allowed", copy.originRefused],
  ["Magic DID token is too old", copy.forbidden],
  ["Magic DID token was already used", copy.reused],
  ["Magic wallet does not match the request signer", copy.walletMismatch],
]);

function deletionError(status: number, error: unknown): DeletionError {
  switch (status) {
    case 401: return new DeletionError(copy.signInAgain);
    case 403: return new DeletionError(FORBIDDEN.get(error) ?? copy.signInAgain);
    case 429: return new DeletionError(copy.rateLimited, true);
    case 502:
    case 504: return new DeletionError(copy.unknown);
    case 503: return new DeletionError(copy.unavailable, true);
    default: return new DeletionError(copy.failed);
  }
}

async function errorField(response: Response): Promise<unknown> {
  const body: unknown = await response.json().catch(() => null);
  return body && typeof body === "object" ? (body as { error?: unknown }).error : undefined;
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
      const url = "/auth-api/accounts";
      const { headers } = await signRequest(identity, "DELETE", url, { didToken });
      const response = await fetch(url, { method: "DELETE", headers }).catch(() => {
        throw new DeletionError(copy.unknown);
      });
      if (!response.ok) throw deletionError(response.status, await errorField(response));
      await response.body?.cancel().catch(() => {});
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
          {deletion.error && (
            <p role="alert">
              {deletion.error.message}
              {deletion.error instanceof DeletionError && deletion.error.support && (
                <>
                  {" "}
                  <a href={copy.supportHref} target="_blank" rel="noopener noreferrer">{copy.support}</a>
                </>
              )}
            </p>
          )}
        </Modal>
      )}
    </>
  );
}
