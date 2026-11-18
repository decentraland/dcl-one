import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
// @dcl/schemas' root barrel doesn't re-export `getNetwork` / `getNetworkMapping`.
import { ChainId, getNetwork, getNetworkMapping } from '@dcl/schemas/dist/dapps/chain-id'
import { Network } from '@dcl/schemas/dist/dapps/network'
import { readWalletChainId, subscribeWalletChain, switchWalletChain, type Session } from '~/lib/auth'
import { captureError } from '~/lib/monitoring'
import { isWalletRejection } from '~/lib/walletErrors'
import { config } from '~/config'

const KNOWN_CHAINS = Object.values(ChainId).filter((id): id is ChainId => typeof id === 'number')

/**
 * The networks this deployment can transact on: its Polygon chain first, then the Ethereum chain
 * @dcl/schemas pairs it with. Derived rather than listed so a testnet deployment never offers mainnet.
 */
export function supportedChains(polygonChainId: number = Number(config.get('MATIC_CHAIN_ID'))): number[] {
  const counterpart = KNOWN_CHAINS.find(id => {
    try {
      return getNetwork(id) === Network.ETHEREUM && Number(getNetworkMapping(id)[Network.MATIC]) === polygonChainId
    } catch {
      return false
    }
  })
  return counterpart !== undefined && Number(counterpart) !== polygonChainId
    ? [polygonChainId, Number(counterpart)]
    : [polygonChainId]
}

export type WalletChain = {
  /** The wallet's current chain, or undefined until the first read resolves or without a wallet. */
  chainId?: number
  /** Networks offered for this deployment. */
  chains: number[]
  /** The chain a switch is awaiting the wallet's answer for, if any. */
  pendingChainId?: number
  /** Asks the wallet to switch. Call ONLY from a user gesture; resolves once the wallet answers either way. */
  switchTo: (chainId: number) => Promise<void>
}

/**
 * Which network the wallet is on, and moving it only when the user asked. Nothing here switches on its own:
 * wallets reject `wallet_*` requests they can't attribute to a click. Pass null for a wallet that has no
 * network to choose (social logins), so nothing is asked and the navbar hides its chain pill.
 */
export function useWalletChain(session: Session | null): WalletChain {
  const [chainId, setChainId] = useState<number | undefined>(session?.chainId)
  const [pendingChainId, setPendingChainId] = useState<number | undefined>()
  const chains = useMemo(() => supportedChains(), [])
  // The session a switch was asked of, so one answered after a sign-out writes nothing.
  const current = useRef(session)
  current.current = session

  // First read, then the wallet's own events. A failed read keeps the last known value: a wallet that
  // won't answer is not evidence of a different chain.
  useEffect(() => {
    if (!session) {
      setChainId(undefined)
      return
    }
    let live = true
    const refresh = () => {
      readWalletChainId(session)
        .then(next => {
          if (live) setChainId(next)
        })
        .catch((error: unknown) => captureError(error, { flow: 'network-selector', step: 'read-chain' }))
    }
    refresh()
    // Only a refresh: MetaMask emits `chainChanged` before the switch request resolves, so clearing the
    // in-flight guard here would let a quick second pick send a request the wallet answers with -32002.
    const unsubscribe = subscribeWalletChain(session, refresh)
    return () => {
      live = false
      unsubscribe()
    }
  }, [session])

  const switchTo = useCallback(
    async (target: number) => {
      // One request in flight: a second click would make the first answer look like the second's.
      if (!session || target === chainId || pendingChainId !== undefined) return
      setPendingChainId(target)
      try {
        await switchWalletChain(session, target)
      } catch (error) {
        if (!isWalletRejection(error)) captureError(error, { flow: 'network-selector', step: 'switch-chain', target })
      } finally {
        setPendingChainId(undefined)
      }
      if (current.current !== session) return
      // Some wallets emit no `chainChanged` for a switch they consider a no-op, so confirm once. A read
      // that fails here leaves the label to the wallet's own events.
      try {
        const settled = await readWalletChainId(session)
        if (current.current === session) setChainId(settled)
      } catch (error) {
        captureError(error, { flow: 'network-selector', step: 'read-chain' })
      }
    },
    [session, chainId, pendingChainId]
  )

  return { chainId, chains, pendingChainId, switchTo }
}
