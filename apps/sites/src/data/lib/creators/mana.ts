// MANA (Polygon) balance and allowance reads plus the approve call a MANA payment may need first.
import { ContractName, getContract } from '~/lib/chain/contracts'
import { readContract, type ContractCall } from '~/lib/auth'

export function getManaContract(chainId: number) {
  return getContract(ContractName.MANAToken, chainId)
}

export async function fetchManaBalance(address: string, chainId: number): Promise<bigint> {
  return readContract<bigint>(getManaContract(chainId), 'balanceOf', [address])
}

export async function fetchManaAllowance(owner: string, spender: string, chainId: number): Promise<bigint> {
  return readContract<bigint>(getManaContract(chainId), 'allowance', [owner, spender])
}

/**
 * ERC20 approve so `spender` can pull up to `amountWei` (a fee plus its drift headroom). Never unlimited: an
 * open-ended allowance would outlive the publish and expose the wallet's whole balance to the spender.
 */
export function buildManaApproveCall(chainId: number, spender: string, amountWei: bigint): ContractCall {
  if (amountWei <= 0n) throw new Error('MANA approval amount must be positive')
  return { contract: getManaContract(chainId), method: 'approve', args: [spender, amountWei.toString()] }
}
