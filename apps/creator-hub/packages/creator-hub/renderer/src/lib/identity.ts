import type { Hex } from 'viem';
import { generatePrivateKey, privateKeyToAccount, signMessage } from 'viem/accounts';
import type { AuthChain, AuthLinkType } from '@dcl/schemas';

export type { AuthChain, AuthLink } from '@dcl/schemas';

/**
 * A key pair and its address. `privateKey` and `publicKey` are hex text. The keys made here have no
 * `0x` prefix and the public key has no leading `04`; a stored identity can have the prefix.
 */
export type IdentityKeys = {
  privateKey: string;
  publicKey: string;
  address: string;
};

/**
 * What signs for a wallet until `expiration`: the ephemeral keys and the chain that proves the
 * wallet accepted them. The auth site makes it and `@dcl/single-sign-on-client` stores it.
 */
export type AuthIdentity = {
  ephemeralIdentity: IdentityKeys;
  expiration: Date;
  authChain: AuthChain;
};

// The link types are written as text, so this file needs no code from `@dcl/schemas`.
const SIGNER = 'SIGNER' as AuthLinkType.SIGNER;
const PERSONAL_EPHEMERAL = 'ECDSA_EPHEMERAL' as AuthLinkType.ECDSA_PERSONAL_EPHEMERAL;
const CONTRACT_WALLET_EPHEMERAL =
  'ECDSA_EIP_1654_EPHEMERAL' as AuthLinkType.ECDSA_EIP_1654_EPHEMERAL;
const SIGNED_ENTITY = 'ECDSA_SIGNED_ENTITY' as AuthLinkType.ECDSA_PERSONAL_SIGNED_ENTITY;

/** `0x`, 64 bytes of signature and the recovery byte. A contract wallet signature is longer. */
const PERSONAL_SIGNATURE_LENGTH = 132;

const toHex = (key: string): Hex => (key.startsWith('0x') ? (key as Hex) : `0x${key}`);

/** New random keys, or the keys of `privateKey`. */
export function createIdentityKeys(privateKey: Hex = generatePrivateKey()): IdentityKeys {
  const { publicKey, address } = privateKeyToAccount(privateKey);
  return {
    privateKey: privateKey.slice(2),
    publicKey: publicKey.slice(4),
    address,
  };
}

/** Signs `message` as a wallet does for `personal_sign`. */
export function signWithKeys(keys: Pick<IdentityKeys, 'privateKey'>, message: string) {
  return signMessage({ message, privateKey: toHex(keys.privateKey) });
}

/** The text a wallet signs to accept the ephemeral keys. Verifiers read both values from it. */
export function ephemeralMessage(ephemeralAddress: string, expiration: Date): string {
  return `Decentraland Login\nEphemeral address: ${ephemeralAddress}\nExpiration: ${expiration.toISOString()}`;
}

/**
 * Makes an identity that expires in `minutes`. `signer` is the wallet: it gets the login message
 * and gives its signature.
 */
export async function createIdentity(
  ownerAddress: string,
  ephemeralIdentity: IdentityKeys,
  minutes: number,
  signer: (message: string) => Promise<string>,
): Promise<AuthIdentity> {
  const expiration = new Date();
  expiration.setMinutes(expiration.getMinutes() + minutes);
  const message = ephemeralMessage(ephemeralIdentity.address, expiration);
  const signature = await signer(message);
  return {
    ephemeralIdentity,
    expiration,
    authChain: [
      { type: SIGNER, payload: ownerAddress, signature: '' },
      {
        type:
          signature.length === PERSONAL_SIGNATURE_LENGTH
            ? PERSONAL_EPHEMERAL
            : CONTRACT_WALLET_EPHEMERAL,
        payload: message,
        signature,
      },
    ],
  };
}

/**
 * The chain of the identity plus a link that signs `payload` with the ephemeral key. It reads the
 * identity before the first `await`, so the caller's check of the account stays valid.
 */
export async function signPayload(
  identity: Pick<AuthIdentity, 'ephemeralIdentity' | 'authChain'>,
  payload: string,
): Promise<AuthChain> {
  const { authChain, ephemeralIdentity } = identity;
  const signature = await signWithKeys(ephemeralIdentity, payload);
  return [...authChain, { type: SIGNED_ENTITY, payload, signature }];
}
