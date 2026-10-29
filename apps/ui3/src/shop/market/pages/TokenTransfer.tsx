// from: marketplace@3bf3662f:webapp/src/components/TransferPage (TransferPage.tsx, TransferPage.css),
//       webapp/src/components/Routes/Routes.tsx (ProtectedRoute: signed out goes to sign-in);
//       shop@2d3bf62:app/src/components/TransferModal/TransferModal.tsx; catalyrst-v2 ui3/src/marketplace/pages/MkTransferPage.tsx
// Copyright 2020 Metaverse Holdings Ltd. (marketplace, shop), Apache-2.0.

import { useQueryClient } from "@tanstack/react-query";
import { useId, useState } from "react";
import { Link, useNavigate } from "react-router";

import { isPolygon } from "../../model/chain";
import { shopPath } from "../../routes";
import { patchManageCaches } from "../../seller/lib/manage-cache";
import { useShopWallet } from "../../wallet/context";
import { EnsureQueryClient, NetworkNotice, shortAddr, useShopProvider, useWrite, WalletGate, WriteStatus } from "../components/kit";
import { TokenFrame, useIsOwner, WithRouteToken, type TokenRecord } from "../components/TokenAction";
import { transferItem, transferProblem, type TransferProblem } from "../purchase/transfer";
import { categoryLabel, ConfirmValueModal, RequireSignIn, TaButton, TaField, TokenActionRoot } from "./tokenActionUi";

const PROBLEM_TEXT: Record<TransferProblem, string> = {
  invalidAddress: "That's not a valid address",
  zeroAddress: "That address cannot receive items.",
  sameAddress: "That is your own address. Enter a different recipient.",
};

type Blocked = "notOwner" | "onSale" | null;

export function TransferForm({ token, blocked = null }: { token: TokenRecord; blocked?: Blocked }) {
  const qc = useQueryClient();
  const navigate = useNavigate();
  const { account } = useShopWallet();
  const provider = useShopProvider();
  const id = useId();
  const [to, setTo] = useState("");
  const [confirming, setConfirming] = useState(false);
  const write = useWrite();
  const problem = to.trim() ? transferProblem(to, account) : null;
  const collection = isPolygon(token.chainId) && (token.category === "wearable" || token.category === "emote");
  const canTransfer = blocked === null;
  const tokenHref = shopPath("tokenDetail", { contractAddress: token.contractAddress, tokenId: token.tokenId });

  if (write.state.kind === "done") {
    return (
      <div className="shop-ta-done" data-testid="shop-transfer-done">
        <p role="status">
          <b>{token.name}</b> is on its way to {shortAddr(to.trim())}.
        </p>
        <Link to={shopPath("myItems")}>Back to My items</Link>
      </div>
    );
  }

  const submit = () => {
    setConfirming(false);
    void write.run(collection ? "Confirm in your wallet (no gas needed)…" : "Confirm the transaction in your wallet…", "Transferred.", async () => {
      const hash = await transferItem({ provider: provider!, contractAddress: token.contractAddress, chainId: token.chainId, tokenId: token.tokenId, to, collection });
      patchManageCaches(qc, { address: account ?? undefined, contractAddress: token.contractAddress, tokenId: token.tokenId }, { kind: "gone" });
      await qc.invalidateQueries({ queryKey: ["token", token.contractAddress, token.tokenId] });
      return hash;
    });
  };

  return (
    <form
      className="shop-ta-form"
      onSubmit={(e) => {
        e.preventDefault();
        if (!canTransfer || problem || !to.trim() || write.busy) return;
        setConfirming(true);
      }}
    >
      {canTransfer && !collection ? <NetworkNotice chainId={token.chainId} /> : null}
      <div className="shop-ta-fields">
        <TaField label="Recipient address" htmlFor={`${id}-to`} invalid={!!problem} message={problem ? PROBLEM_TEXT[problem] : undefined}>
          <input
            id={`${id}-to`}
            value={to}
            onChange={(e) => setTo(e.target.value)}
            placeholder="0x..."
            autoComplete="off"
            spellCheck={false}
            disabled={!canTransfer || write.busy}
            aria-invalid={problem ? true : undefined}
          />
        </TaField>
      </div>
      {canTransfer ? (
        <p className="shop-ta-warning">
          Remember that transferring is an irreversible operation.
          <br />
          Please check the address carefully.
        </p>
      ) : null}
      <div className="shop-ta-buttons">
        <TaButton disabled={write.busy} onClick={() => navigate(tokenHref)}>
          Cancel
        </TaButton>
        <TaButton type="submit" primary disabled={!canTransfer || !to.trim() || !!problem || write.busy}>
          {write.busy ? "Transferring…" : "Transfer"}
        </TaButton>
      </div>
      <WriteStatus state={write.state} />
      {confirming ? (
        <ConfirmValueModal
          testId="shop-transfer-confirm"
          value={null}
          content={
            <p>
              You are about to transfer <b>{token.name}</b> to <b>{to.trim()}</b>. This cannot be undone.
            </p>
          }
          onCancel={() => setConfirming(false)}
          onConfirm={submit}
        />
      ) : null}
    </form>
  );
}

export function TokenTransferView({ token }: { token: TokenRecord }) {
  const owner = useIsOwner(token);
  const blocked: Blocked = !owner ? "notOwner" : token.isOnSale ? "onSale" : null;
  return (
    <TokenActionRoot rarity={token.rarity}>
      <TokenFrame token={token} title={`Transfer ${categoryLabel(token.category)}`}>
        {blocked === "notOwner" ? (
          <div className="shop-ta-subtitle shop-ta-subtitle--error" role="alert" data-testid="shop-not-owner">
            You are not the owner of <b>{token.name}</b>.
          </div>
        ) : blocked === "onSale" ? (
          <div className="shop-ta-subtitle shop-ta-subtitle--error" role="alert" data-testid="shop-transfer-on-sale">
            <b>{token.name}</b> can't be transferred because it's on sale.
          </div>
        ) : (
          <div className="shop-ta-subtitle">
            You are about to transfer <b>{token.name}</b>.
          </div>
        )}
        {blocked ? (
          <TransferForm token={token} blocked={blocked} />
        ) : (
          <WalletGate what="Transferring an item">
            <TransferForm token={token} />
          </WalletGate>
        )}
      </TokenFrame>
    </TokenActionRoot>
  );
}

export default function TokenTransfer() {
  return (
    <EnsureQueryClient>
      <RequireSignIn>
        <TokenActionRoot>
          <WithRouteToken title="Transfer">{(token) => <TokenTransferView token={token} />}</WithRouteToken>
        </TokenActionRoot>
      </RequireSignIn>
    </EnsureQueryClient>
  );
}
