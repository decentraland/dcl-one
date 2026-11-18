// The external NFT contract a linked collection is tied to: address input helpers and the on-chain ERC-165 check
// the legacy create modal ran, fixed to probe the ERC-721 interface id (legacy probed ERC-165's own id).
import { type ContractNetwork } from '@dcl/schemas'
import { parseAbi } from 'viem'
import { readContract } from '~/lib/auth'
import { BuilderServerError } from '~/lib/builder'
import { LINKED_CONTRACT_CHAIN_IDS, LINKED_CONTRACT_IN_USE_ERROR } from './thirdParty'

const ADDRESS = /^0x[0-9a-fA-F]{40}$/
const ADDRESS_PREFIX = /^(0|0x|0x[0-9a-fA-F]{0,40})$/

export function isContractAddress(value: string): boolean {
  return ADDRESS.test(value)
}

/** Whether the text could still become an address: the field refuses anything else as it is typed. */
export function isAddressBeingTyped(value: string): boolean {
  return value === '' || ADDRESS_PREFIX.test(value)
}

export const ERC721_INTERFACE_ID = '0x80ac58cd'
export const ERC1155_INTERFACE_ID = '0xd9b67a26'

const ERC165_ABI = parseAbi(['function supportsInterface(bytes4 interfaceId) view returns (bool)'])

export type LinkedContractCheck = 'ok' | 'not_nft' | 'error'

export type SupportsInterface = (chainId: number, address: string, interfaceId: string) => Promise<boolean>

const defaultSupportsInterface: SupportsInterface = (chainId, address, interfaceId) =>
  readContract<boolean>({ abi: ERC165_ABI, address, chainId, name: '', version: '' }, 'supportsInterface', [
    interfaceId
  ])

/**
 * Whether the contract answers as an ERC-721 or ERC-1155 token on its network. `not_nft` when it answers but
 * supports neither; `error` when the call itself fails (wrong network, not a contract, RPC down).
 */
export async function checkLinkedContract(
  network: ContractNetwork,
  address: string,
  supportsInterface: SupportsInterface = defaultSupportsInterface
): Promise<LinkedContractCheck> {
  const chainId = LINKED_CONTRACT_CHAIN_IDS[network]
  if (!chainId || !isContractAddress(address)) return 'error'
  try {
    const [erc721, erc1155] = await Promise.all([
      supportsInterface(chainId, address, ERC721_INTERFACE_ID),
      supportsInterface(chainId, address, ERC1155_INTERFACE_ID)
    ])
    return erc721 || erc1155 ? 'ok' : 'not_nft'
  } catch {
    return 'error'
  }
}

/** Whether a failed save was builder-server refusing a contract another collection of the third party already links. */
export function isContractInUseError(error: unknown): boolean {
  if (!(error instanceof BuilderServerError)) return false
  const text = `${error.message} ${typeof error.data === 'string' ? error.data : JSON.stringify(error.data ?? '')}`
  return text.includes(LINKED_CONTRACT_IN_USE_ERROR)
}
