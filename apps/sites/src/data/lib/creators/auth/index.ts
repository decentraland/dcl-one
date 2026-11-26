import type * as Transactions from './transactions'

export {
  currentSession,
  identityOf,
  isSocialLogin,
  signedFetch,
  signWithIdentity,
  subscribeSession,
  type AuthChain,
  type Session
} from './auth'
export { readWalletChainId, requireAccount, subscribeWalletChain, switchWalletChain } from './provider'
export type {
  ContractCall,
  ContractData,
  TransactionSender,
  TransactionSettlement,
  TypedDataDomain,
  TypedDataTypes
} from './transactions'

const transactions = () => import('./transactions')

// Fetched ahead for a signed-in creator, so a first write doesn't wait on the download and a popup wallet
// still sees the click's user gesture when the request reaches it.
export async function readContract<T>(...args: Parameters<typeof Transactions.readContract>): Promise<T> {
  return (await transactions()).readContract<T>(...args)
}

export async function sendContractTransaction(...args: Parameters<typeof Transactions.sendContractTransaction>) {
  return (await transactions()).sendContractTransaction(...args)
}

export async function sendWalletTransaction(...args: Parameters<typeof Transactions.sendWalletTransaction>) {
  return (await transactions()).sendWalletTransaction(...args)
}

export async function signTypedData(...args: Parameters<typeof Transactions.signTypedData>) {
  return (await transactions()).signTypedData(...args)
}

export async function isContractDeployed(...args: Parameters<typeof Transactions.isContractDeployed>) {
  return (await transactions()).isContractDeployed(...args)
}

export async function readTransactionOutcome(...args: Parameters<typeof Transactions.readTransactionOutcome>) {
  return (await transactions()).readTransactionOutcome(...args)
}

export async function readTransactionSender(...args: Parameters<typeof Transactions.readTransactionSender>) {
  return (await transactions()).readTransactionSender(...args)
}

export async function settleTransaction(...args: Parameters<typeof Transactions.settleTransaction>) {
  return (await transactions()).settleTransaction(...args)
}

export async function waitForTransaction(...args: Parameters<typeof Transactions.waitForTransaction>) {
  return (await transactions()).waitForTransaction(...args)
}
