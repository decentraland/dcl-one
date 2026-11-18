// For specs that pin chain outputs (calldata, EIP-712 digests, signatures) as literals. The literals are the
// reference, not these helpers: a port of this file must leave every pinned value passing unchanged.
import { decodeFunctionData, hashTypedData, recoverTypedDataAddress, type Hex, type TypedDataDefinition } from 'viem'
import { privateKeyToAccount } from 'viem/accounts'
import { encodeContractCall, type ContractCall } from '~/lib/chain/contracts'
import { type TypedDataDomain, type TypedDataTypes } from '~/lib/auth'

/** A throwaway key, so pinned signatures come out the same on every run. */
export const TEST_PRIVATE_KEY = '0x1111111111111111111111111111111111111111111111111111111111111111'
export const TEST_SIGNER = '0x19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A'

export const testAccount = privateKeyToAccount(TEST_PRIVATE_KEY)

/** The target and calldata the call puts on chain. */
export function toTransaction(call: ContractCall): { to: string; data: string } {
  return { to: call.contract.address, data: encodeContractCall(call) }
}

/** The one type no other type references, as a wallet infers it. */
export function rootType(types: TypedDataTypes): string {
  const referenced = new Set(
    Object.values(types).flatMap(fields => fields.map(field => field.type.replace(/\[\d*\]$/, '')))
  )
  const roots = Object.keys(types).filter(name => !referenced.has(name))
  if (roots.length !== 1) throw new Error(`Typed data has no single root type: ${roots.join(', ')}`)
  return roots[0]
}

export function typedDataDefinition(
  domain: TypedDataDomain,
  types: TypedDataTypes,
  primaryType: string,
  value: Record<string, unknown>
): TypedDataDefinition {
  return { domain, types, primaryType, message: value } as unknown as TypedDataDefinition
}

export function typedDataDigest(domain: TypedDataDomain, types: TypedDataTypes, value: Record<string, unknown>): Hex {
  return hashTypedData(typedDataDefinition(domain, types, rootType(types), value))
}

/** Signs like a wallet would; refuses a `primaryType` that is not the root of `types`, as a wallet does. */
export function signTypedDataWithTestKey(
  domain: TypedDataDomain,
  types: TypedDataTypes,
  primaryType: string,
  value: Record<string, unknown>
): Promise<string> {
  const root = rootType(types)
  if (root !== primaryType) throw new Error(`primaryType ${primaryType} is not the root type ${root}`)
  return testAccount.signTypedData(typedDataDefinition(domain, types, primaryType, value))
}

/** The address that signed `value`, checksummed, as builder-server and the contracts recover it. */
export function recoverTypedDataSigner(
  domain: TypedDataDomain,
  types: TypedDataTypes,
  value: Record<string, unknown>,
  signature: string
): Promise<string> {
  return recoverTypedDataAddress({
    ...typedDataDefinition(domain, types, rootType(types), value),
    signature: signature as Hex
  })
}

/** The call's arguments as the contract decodes them: uints as `bigint`, named tuples as objects. */
export function decodeCall(call: ContractCall): readonly unknown[] {
  return decodeFunctionData({ abi: call.contract.abi, data: encodeContractCall(call) as Hex }).args ?? []
}
