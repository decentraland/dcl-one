import { afterEach, describe, expect, it, vi } from 'vitest'
import { TransactionNotFoundError, TransactionReceiptNotFoundError, hashTypedData, numberToHex, parseAbi } from 'viem'
import { ContractName, getCollectionContract, getContract, getCouponManager } from '~/lib/chain/contracts'
import { buildChequeTypedData } from '~/lib/publishLinkedItems'
import { WrongAccountError, WrongChainError, isWalletRejection } from '~/lib/walletErrors'
import { type Session } from './auth'
import {
  readContract,
  readTransactionOutcome,
  readWalletChainId,
  sendContractTransaction,
  sendWalletTransaction,
  signTypedData,
  subscribeWalletChain,
  switchWalletChain,
  waitForTransaction,
  type TypedDataTypes
} from '.'
import { waitForReceipt, watchTransaction } from './transactions'
import { OFFCHAIN_MARKETPLACE_TYPES, getTradeDomain } from '~/lib/trades'
import { typedDataDigest } from '~/test/chain'

vi.mock('decentraland-connect', () => ({ getRpcUrls: () => ({ 80002: 'https://rpc.example.com' }) }))

afterEach(() => {
  vi.unstubAllGlobals()
})

const manager = {
  ...getCouponManager(ContractName.OffChainMarketplaceV3, 80002),
  abi: parseAbi(['function increaseSignerSignatureIndex()'])
}
const call = { contract: manager, method: 'increaseSignerSignatureIndex', args: [] }
const CALLDATA = '0xceefad9f'

type Behavior =
  | 'switches'
  | 'needs-add'
  | 'needs-add-wrapped'
  | 'refuses'
  | 'ignores'
  | 'changes-account'
  | 'reverts'
  | 'dismisses'
  | 'denies-signature'
type Request = { method: string; params?: unknown[] }

const ACCOUNT = '0x00000000000000000000000000000000000000aa'
const OTHER = '0x00000000000000000000000000000000000000bb'
const COLLECTION = '0x00000000000000000000000000000000000000ee'
const HASH = `0x${'ab'.repeat(32)}`

/**
 * A wallet's EIP-1193 provider on `chainId`, signed in as `account`, reacting to a switch request the way `behavior`
 * says. It knows the transactions it sent; `knowsSent: false` plays a wallet whose node doesn't return them yet.
 */
function walletOn(chainId: number, behavior: Behavior = 'switches', account = ACCOUNT, knowsSent = true) {
  let current = chainId
  let active = account
  const request = vi.fn(async ({ method, params = [] }: Request): Promise<unknown> => {
    const [first] = params as [Record<string, string>]
    switch (method) {
      case 'eth_requestAccounts':
      case 'eth_accounts':
        return [active]
      case 'eth_getCode':
        return '0x'
      case 'eth_chainId':
        return numberToHex(current)
      case 'wallet_switchEthereumChain':
        if (behavior === 'refuses') throw Object.assign(new Error('User rejected'), { code: 4001 })
        if (behavior === 'needs-add') throw Object.assign(new Error('Unrecognized chain'), { code: 4902 })
        // MetaMask mobile wraps the 4902 in an internal error.
        if (behavior === 'needs-add-wrapped') {
          throw Object.assign(new Error('Internal'), { code: -32603, data: { originalError: { code: 4902 } } })
        }
        // Magic answers the switch without moving.
        if (behavior === 'ignores') return null
        if (behavior === 'changes-account') active = OTHER
        current = Number(first.chainId)
        return null
      case 'wallet_addEthereumChain':
        current = Number(first.chainId)
        return null
      case 'eth_estimateGas':
        if (behavior === 'reverts') throw Object.assign(new Error('execution reverted'), { code: 3 })
        return '0x5208'
      case 'eth_sendTransaction':
        if (behavior === 'dismisses') throw Object.assign(new Error('Request cancelled'), { code: 4001 })
        return HASH
      case 'eth_getTransactionByHash':
        return knowsSent ? { hash: HASH, from: active, nonce: '0x7' } : null
      case 'eth_signTypedData_v4':
        if (behavior === 'dismisses') throw Object.assign(new Error('Request cancelled'), { code: 4001 })
        if (behavior === 'denies-signature') {
          throw Object.assign(new Error('MetaMask Typed Message Signature: User denied message signature.'), {
            code: 4001
          })
        }
        return `0x${'11'.repeat(64)}1b`
    }
    throw new Error(`Unexpected ${method}`)
  })
  const sent = () =>
    request.mock.calls
      .filter(([{ method }]) => method === 'eth_sendTransaction')
      .map(([{ params }]) => params?.[0] as Record<string, string>)
  return { session: { address: ACCOUNT, provider: { request } } as unknown as Session, request, sent }
}

/** The read node behind https://rpc.example.com, answering JSON-RPC by method. */
function readNode(answers: Record<string, unknown>) {
  const fetch = vi.fn(async (_url: string, init: { body: string }) => {
    const body = JSON.parse(init.body) as { id: number; method: string }
    if (!(body.method in answers)) throw new Error(`Unexpected ${body.method}`)
    return new Response(JSON.stringify({ jsonrpc: '2.0', id: body.id, result: answers[body.method] }))
  })
  vi.stubGlobal('fetch', fetch)
  return fetch
}

describe('when sending a transaction from the wallet itself', () => {
  it('should send it straight away when the wallet is already on the contract chain', async () => {
    const { session, request, sent } = walletOn(80002)
    await expect(sendWalletTransaction(session, call)).resolves.toEqual({ hash: HASH, nonce: 7 })
    expect(request).not.toHaveBeenCalledWith(expect.objectContaining({ method: 'wallet_switchEthereumChain' }))
    expect(sent()).toEqual([
      expect.objectContaining({ from: ACCOUNT, to: manager.address, data: CALLDATA, gas: '0x5208' })
    ])
  })

  it('should hand out the hash before reading its nonce', async () => {
    const { session, request } = walletOn(80002)
    const nonceReadsAtHandOut: number[] = []
    const onSent = vi.fn(() => {
      const reads = request.mock.calls.filter(([args]) => args.method === 'eth_getTransactionByHash')
      nonceReadsAtHandOut.push(reads.length)
    })
    await expect(sendWalletTransaction(session, call, onSent)).resolves.toEqual({ hash: HASH, nonce: 7 })
    expect(onSent).toHaveBeenCalledWith(HASH)
    expect(nonceReadsAtHandOut).toEqual([0])
  })

  it('should move the wallet to the contract chain first', async () => {
    const { session, request, sent } = walletOn(1)
    await sendWalletTransaction(session, call)
    expect(request).toHaveBeenCalledWith({ method: 'wallet_switchEthereumChain', params: [{ chainId: '0x13882' }] })
    expect(sent()).toHaveLength(1)
  })

  it.each(['needs-add', 'needs-add-wrapped'] as const)(
    'should add the chain when the wallet does not know it yet (%s)',
    async behavior => {
      const { session, request, sent } = walletOn(1, behavior)
      await sendWalletTransaction(session, call)
      expect(request).toHaveBeenCalledWith({
        method: 'wallet_addEthereumChain',
        params: [expect.objectContaining({ chainId: '0x13882', rpcUrls: ['https://rpc-amoy.polygon.technology'] })]
      })
      expect(sent()).toHaveLength(1)
    }
  )

  it('should send nothing when the creator refuses the switch', async () => {
    const { session, sent } = walletOn(1, 'refuses')
    await expect(sendWalletTransaction(session, call)).rejects.toMatchObject({ code: 4001 })
    expect(sent()).toHaveLength(0)
  })

  it('should send nothing when the wallet answers the switch but stays where it was', async () => {
    const { session, sent } = walletOn(1, 'ignores')
    await expect(sendWalletTransaction(session, call)).rejects.toBeInstanceOf(WrongChainError)
    expect(sent()).toHaveLength(0)
  })

  // A cancellation keyed by its caller mines fine from another account and ends nothing.
  it('should send nothing when the wallet has moved to another account', async () => {
    const { session, sent } = walletOn(80002, 'switches', OTHER)
    await expect(sendWalletTransaction(session, call)).rejects.toBeInstanceOf(WrongAccountError)
    expect(sent()).toHaveLength(0)
  })

  it('should send nothing when the account changes while switching chains', async () => {
    const { session, sent } = walletOn(1, 'changes-account')
    await expect(sendWalletTransaction(session, call)).rejects.toBeInstanceOf(WrongAccountError)
    expect(sent()).toHaveLength(0)
  })

  it('should report a dismissed prompt as a wallet rejection', async () => {
    const { session } = walletOn(80002, 'dismisses')
    const error: unknown = await sendWalletTransaction(session, call).catch((e: unknown) => e)
    expect(isWalletRejection(error)).toBe(true)
  })

  it('should send nothing when the call would revert', async () => {
    const { session, sent } = walletOn(80002, 'reverts')
    await expect(sendWalletTransaction(session, call)).rejects.toThrow(/execution reverted/)
    expect(sent()).toHaveLength(0)
  })

  it("should read the nonce from the read node when the wallet doesn't return the transaction", async () => {
    readNode({ eth_getTransactionByHash: { hash: HASH, from: ACCOUNT, nonce: '0x9' } })
    const { session } = walletOn(80002, 'switches', ACCOUNT, false)
    await expect(sendWalletTransaction(session, call)).resolves.toEqual({ hash: HASH, nonce: 9 })
  })

  // Amoy rejects tips under its floor, which wallets often propose.
  it("should floor the tip where the environment's network asks for it", async () => {
    const { session, sent } = walletOn(80002)
    await sendWalletTransaction(session, call)
    expect(BigInt(sent()[0]?.maxPriorityFeePerGas)).toBe(30_000_000_000n)
  })

  it('should floor the tip on the regular path too, when the wallet is already on the contract chain', async () => {
    const { session, sent } = walletOn(80002)
    await expect(sendContractTransaction(session, call)).resolves.toBe(HASH)
    expect(BigInt(sent()[0]?.maxPriorityFeePerGas)).toBe(30_000_000_000n)
  })
})

describe('when the wallet is on another chain than the contract', () => {
  const contract = getCollectionContract(80002, COLLECTION)
  const transfer = { contract, method: 'transferCreatorship', args: [OTHER] }
  const RELAYER = 'https://transactions-api.decentraland.zone/v1/transactions'

  /** The read node answering the nonce, and the relayer answering `relay`; returns what each was sent. */
  function nodeAndRelayer(relay: object = { ok: true, txHash: HASH }) {
    const fetch = vi.fn(async (url: string, init: { body: string }) => {
      if (url === RELAYER) return new Response(JSON.stringify(relay), { status: 200 })
      const body = JSON.parse(init.body) as { id: number; method: string }
      return new Response(JSON.stringify({ jsonrpc: '2.0', id: body.id, result: numberToHex(3, { size: 32 }) }))
    })
    vi.stubGlobal('fetch', fetch)
    const bodies = (relayed: boolean) =>
      fetch.mock.calls.filter(([url]) => (url === RELAYER) === relayed).map(([, init]) => JSON.parse(init.body))
    return { relayed: () => bodies(true), nodeCalls: () => bodies(false) }
  }

  it('should relay the exact calldata as a Polygon meta-transaction, gas paid by the relayer', async () => {
    const { relayed } = nodeAndRelayer()
    const { session, sent } = walletOn(1)
    await expect(sendContractTransaction(session, transfer)).resolves.toBe(HASH)
    expect(sent()).toHaveLength(0)
    const [{ transactionData }] = relayed()
    expect(transactionData.from).toBe(ACCOUNT)
    expect(transactionData.params[0]).toBe(COLLECTION)
    expect(transactionData.params[1]).toContain(
      '6d2e4b1b00000000000000000000000000000000000000000000000000000000000000bb'
    )
  })

  it("should sign the nonce read from the contract's chain, not the wallet's", async () => {
    const { nodeCalls } = nodeAndRelayer()
    const { session, request } = walletOn(1)
    await sendContractTransaction(session, transfer)
    expect(nodeCalls()).toEqual([
      expect.objectContaining({
        method: 'eth_call',
        params: [{ data: `0x2d0335ab${ACCOUNT.slice(2).padStart(64, '0')}`, to: COLLECTION }, 'latest']
      })
    ])
    const signing = request.mock.calls.find(([{ method }]) => method === 'eth_signTypedData_v4')!
    expect(JSON.parse(signing[0].params![1] as string)).toMatchObject({ message: { nonce: 3, from: ACCOUNT } })
  })

  it('should report a dismissed signature as a wallet rejection and relay nothing', async () => {
    const { relayed } = nodeAndRelayer()
    const error = await sendContractTransaction(walletOn(1, 'denies-signature').session, transfer).catch(e => e)
    expect(error).toMatchObject({ code: 'user_denied' })
    expect(isWalletRejection(error)).toBe(true)
    expect(relayed()).toHaveLength(0)
  })

  it('should pass on what the relayer refused the transaction for', async () => {
    nodeAndRelayer({ ok: false, message: 'Max limit reached', code: 'user_limit_reached' })
    await expect(sendContractTransaction(walletOn(1).session, transfer)).rejects.toMatchObject({
      message: 'Max limit reached',
      code: 'user_limit_reached'
    })
  })
})

describe('when signing typed data', () => {
  it('should ask the wallet for the same EIP-712 digest the order is pinned to', async () => {
    const { session, request } = walletOn(80002)
    const salt = '0x' + '22'.repeat(32)
    const { domain, types, value } = buildChequeTypedData(
      80002,
      'urn:decentraland:amoy:collections-thirdparty:brand',
      3,
      salt
    )

    await signTypedData(session, domain, types, 'ConsumeSlots', value)

    const [{ params }] = request.mock.calls.find(([{ method }]) => method === 'eth_signTypedData_v4')!
    const [signer, payload] = params as [string, string]
    expect(signer).toBe(ACCOUNT)
    expect(hashTypedData(JSON.parse(payload))).toBe(
      '0xa96ab1f5e7bde3bee11f939568e08b49d8fcdc039b990c25f672be61acd8c97b'
    )
  })

  it('should send an eth_signTypedData_v4 request whose order payload hashes to the pinned order digest', async () => {
    const { session, request } = walletOn(80002)
    const order = {
      checks: {
        uses: 90,
        expiration: 4102444800,
        effective: 1757500000,
        salt: '0x' + '00'.repeat(31) + '01',
        contractSignatureIndex: 2,
        signerSignatureIndex: 3,
        allowedRoot: '0x' + '00'.repeat(32),
        externalChecks: []
      },
      sent: [{ assetType: 4, contractAddress: '0x00000000000000000000000000000000000000cc', value: '7', extra: '0x' }],
      received: [
        {
          assetType: 2,
          contractAddress: getContract(ContractName.MANAToken, 80002).address,
          value: '500000000000000000',
          extra: '0x',
          beneficiary: ACCOUNT
        }
      ]
    }

    await expect(
      signTypedData(session, getTradeDomain(80002), OFFCHAIN_MARKETPLACE_TYPES, 'Trade', order)
    ).resolves.toBe(`0x${'11'.repeat(64)}1b`)

    const [{ params }] = request.mock.calls.find(([{ method }]) => method === 'eth_signTypedData_v4')!
    const [signer, json] = params as [string, string]
    expect(signer).toBe(ACCOUNT)
    const payload = JSON.parse(json) as {
      primaryType: string
      domain: Record<string, string>
      types: TypedDataTypes
      message: Record<string, unknown>
    }
    expect(payload.primaryType).toBe('Trade')
    expect(payload.domain).toEqual({
      name: 'DecentralandMarketplacePolygon',
      version: '1.0.0',
      salt: '0x0000000000000000000000000000000000000000000000000000000000013882',
      verifyingContract: getContract(ContractName.OffChainMarketplaceV3, 80002).address
    })
    expect(payload.types.EIP712Domain).toEqual([
      { name: 'name', type: 'string' },
      { name: 'version', type: 'string' },
      { name: 'verifyingContract', type: 'address' },
      { name: 'salt', type: 'bytes32' }
    ])
    const types = Object.fromEntries(Object.entries(payload.types).filter(([name]) => name !== 'EIP712Domain'))
    expect(typedDataDigest(payload.domain, types, payload.message)).toBe(
      '0xfc5244b13838fde268d232e2e06c9720b233590078a5dbd7c2ce17bebe156e9f'
    )
  })
})

describe('when the creator dismisses a typed-data prompt', () => {
  it('should report it as a wallet rejection', async () => {
    const { session } = walletOn(80002, 'dismisses')
    const { domain, types, value } = buildChequeTypedData(80002, 'urn:x', 1, '0x' + '22'.repeat(32))
    const error: unknown = await signTypedData(session, domain, types, 'ConsumeSlots', value).catch((e: unknown) => e)
    expect(isWalletRejection(error)).toBe(true)
  })
})

describe('when the navbar asks where the wallet is', () => {
  const sessionAnswering = (answer: unknown) =>
    ({ provider: { request: vi.fn(async () => answer) } }) as unknown as Session

  it('should read the live chain, however the wallet encodes it', async () => {
    await expect(readWalletChainId(sessionAnswering('0x13882'))).resolves.toBe(80002)
    await expect(readWalletChainId(sessionAnswering(137))).resolves.toBe(137)
    await expect(readWalletChainId(sessionAnswering('137'))).resolves.toBe(137)
  })

  it('should throw rather than guess on an unusable answer', async () => {
    await expect(readWalletChainId(sessionAnswering('not-a-chain'))).rejects.toThrow(/unusable/)
    await expect(readWalletChainId(sessionAnswering(null))).rejects.toThrow(/unusable/)
    await expect(readWalletChainId(sessionAnswering('0x0'))).rejects.toThrow(/unusable/)
    await expect(readWalletChainId(sessionAnswering('0x'))).rejects.toThrow(/unusable/)
  })

  it('should follow chain and account changes until unsubscribed', () => {
    const on = vi.fn()
    const removeListener = vi.fn()
    const session = { provider: { request: vi.fn(), on, removeListener } } as unknown as Session
    const onChange = vi.fn()
    const unsubscribe = subscribeWalletChain(session, onChange)
    expect(on).toHaveBeenCalledWith('chainChanged', onChange)
    expect(on).toHaveBeenCalledWith('accountsChanged', onChange)
    unsubscribe()
    expect(removeListener).toHaveBeenCalledWith('chainChanged', onChange)
    expect(removeListener).toHaveBeenCalledWith('accountsChanged', onChange)
  })

  it('should subscribe to nothing on a provider without events', () => {
    const session = { provider: { request: vi.fn() } } as unknown as Session
    expect(() => subscribeWalletChain(session, vi.fn())()).not.toThrow()
  })

  it('should let a refusal through when switching on request', async () => {
    const { session } = walletOn(1, 'refuses')
    await expect(switchWalletChain(session, 80002)).rejects.toMatchObject({ code: 4001 })
  })
})

describe('when reading the chain', () => {
  const mana = getContract(ContractName.MANAToken, 80002)

  it('should answer a uint256 as a bigint', async () => {
    readNode({ eth_call: numberToHex(10n ** 18n, { size: 32 }) })
    await expect(readContract(mana, 'balanceOf', [ACCOUNT])).resolves.toBe(10n ** 18n)
  })

  it('should answer whether a mined transaction succeeded', async () => {
    readNode({ eth_getTransactionReceipt: { transactionHash: HASH, status: '0x1', blockNumber: '0x10', logs: [] } })
    await expect(waitForTransaction(80002, HASH)).resolves.toBe(true)
    readNode({ eth_getTransactionReceipt: { transactionHash: HASH, status: '0x0', blockNumber: '0x10', logs: [] } })
    await expect(waitForTransaction(80002, HASH)).resolves.toBe(false)
  })

  it('should answer unknown rather than fail when the read node errors', async () => {
    vi.stubGlobal(
      'fetch',
      vi.fn(async (_url: string, init: { body: string }) => {
        const { id } = JSON.parse(init.body) as { id: number }
        return new Response(JSON.stringify({ jsonrpc: '2.0', id, error: { code: -32602, message: 'bad params' } }))
      })
    )
    await expect(readTransactionOutcome(80002, HASH, ACCOUNT, 4)).resolves.toBe('unknown')
  })

  it('should call a transaction superseded once its sender used its nonce, and pending otherwise', async () => {
    readNode({ eth_getTransactionReceipt: null, eth_getTransactionCount: '0x5' })
    await expect(readTransactionOutcome(80002, HASH, ACCOUNT, 4)).resolves.toBe('superseded')
    await expect(readTransactionOutcome(80002, HASH, ACCOUNT, 5)).resolves.toBe('unknown')
  })
})

describe('when waiting for a transaction to be mined', () => {
  function node(answers: (() => Promise<unknown>)[]) {
    let time = 0
    const getTransactionReceipt = vi.fn(() => answers.shift()?.() ?? Promise.reject(new Error('No more answers')))
    return {
      reader: { getTransactionReceipt } as never,
      clock: {
        now: () => time,
        sleep: vi.fn(async (ms: number) => {
          time += ms
        })
      }
    }
  }
  const down = () => Promise.reject(new Error('fetch failed'))
  const pending = () => Promise.reject(new TransactionReceiptNotFoundError({ hash: HASH as `0x${string}` }))
  const mined = (status: 'success' | 'reverted') => () => Promise.resolve({ status })

  it('should keep waiting through failed reads until the receipt arrives', async () => {
    const { reader, clock } = node([down, down, mined('success')])
    await expect(waitForReceipt(reader, HASH, clock)).resolves.toBe(true)
    expect(clock.sleep).toHaveBeenCalledTimes(2)
  })

  it('should answer false for a reverted transaction, however long it took', async () => {
    const { reader, clock } = node([pending, pending, pending, mined('reverted')])
    await expect(waitForReceipt(reader, HASH, clock)).resolves.toBe(false)
  })

  it('should give up only once the timeout passes', async () => {
    const { reader, clock } = node(Array.from({ length: 20 }, () => pending))
    await expect(waitForReceipt(reader, HASH, { ...clock, timeoutMs: 10_000, pollMs: 1_000 })).rejects.toThrow(
      /not mined/
    )
    expect(clock.sleep).toHaveBeenCalledTimes(10)
  })
})

describe('when watching a transaction until it settles', () => {
  const options = { timeoutMs: 10_000, pollMs: 1_000 }

  function chain(state: {
    receipt?: { status: 'success' | 'reverted' } | null
    tx?: { from: string; nonce: number } | null
    count?: number
    fail?: boolean
  }) {
    let time = 0
    const read = <T>(value: T | null | undefined, missing: () => Error) => {
      if (state.fail) return Promise.reject(new Error('rpc down'))
      return value ? Promise.resolve(value) : Promise.reject(missing())
    }
    return {
      reader: {
        getTransactionReceipt: vi.fn(() =>
          read(state.receipt, () => new TransactionReceiptNotFoundError({ hash: HASH as `0x${string}` }))
        ),
        getTransaction: vi.fn(() =>
          read(state.tx, () => new TransactionNotFoundError({ hash: HASH as `0x${string}` }))
        ),
        getTransactionCount: vi.fn(() =>
          state.fail ? Promise.reject(new Error('rpc down')) : Promise.resolve(state.count ?? 0)
        )
      } as never,
      clock: {
        now: () => time,
        sleep: vi.fn(async (ms: number) => {
          time += ms
        })
      }
    }
  }

  it('should answer how a mined transaction ended', async () => {
    const { reader, clock } = chain({ receipt: { status: 'success' } })
    await expect(watchTransaction(reader, HASH, { ...options, ...clock })).resolves.toBe('mined')
    const reverted = chain({ receipt: { status: 'reverted' } })
    await expect(watchTransaction(reverted.reader, HASH, { ...options, ...reverted.clock })).resolves.toBe('reverted')
  })

  it('should answer dropped once another transaction used its nonce, as a wallet speed-up or cancel does', async () => {
    const { reader, clock } = chain({ tx: { from: ACCOUNT, nonce: 4 }, count: 5 })
    await expect(watchTransaction(reader, HASH, { ...options, ...clock })).resolves.toBe('dropped')
  })

  it('should answer dropped from the sender recorded at send time once the node forgot the transaction', async () => {
    const forgotten = chain({ count: 5 })
    await expect(
      watchTransaction(forgotten.reader, HASH, { ...options, ...forgotten.clock, sender: { from: ACCOUNT, nonce: 4 } })
    ).resolves.toBe('dropped')
  })

  it('should hand over the sender the first time the node returns the transaction', async () => {
    const { reader, clock } = chain({ tx: { from: ACCOUNT, nonce: 4 }, count: 4 })
    const onSeen = vi.fn()
    await watchTransaction(reader, HASH, { ...options, ...clock, onSeen })
    expect(onSeen).toHaveBeenCalledTimes(1)
    expect(onSeen).toHaveBeenCalledWith({ from: ACCOUNT, nonce: 4 })
  })

  it('should stop waiting at the timeout, as pending while the node still holds it', async () => {
    const { reader, clock } = chain({ tx: { from: ACCOUNT, nonce: 4 }, count: 4 })
    await expect(watchTransaction(reader, HASH, { ...options, ...clock })).resolves.toBe('pending')
    expect(clock.sleep).toHaveBeenCalledTimes(10)
  })

  it('should never call a transaction the node never saw dropped, nor one it could not read', async () => {
    const unknown = chain({})
    await expect(watchTransaction(unknown.reader, HASH, { ...options, ...unknown.clock })).resolves.toBe('pending')
    const down = chain({ fail: true })
    await expect(watchTransaction(down.reader, HASH, { ...options, ...down.clock })).resolves.toBe('pending')
  })
})
