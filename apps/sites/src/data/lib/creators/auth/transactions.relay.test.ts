import { afterEach, describe, expect, it, vi } from 'vitest'
import { parseAbi } from 'viem'
import { getCollectionContract, getContract, type ContractData } from '~/lib/chain/contracts'
import { WrongAccountError, isWalletRejection } from '~/lib/walletErrors'
import { TEST_SIGNER, testAccount } from '~/test/chain'
import { type Session } from './auth'
import { sendContractTransaction } from '.'

vi.mock('decentraland-connect', () => ({ getRpcUrls: () => ({ 80002: 'https://rpc.example.com' }) }))

afterEach(() => {
  vi.unstubAllGlobals()
})

// Expected values were produced by decentraland-transactions@3.3.0's sendMetaTransaction on this same harness.
const RELAYER = 'https://transactions-api.decentraland.zone/v1/transactions'
const COLLECTION = '0x00000000000000000000000000000000000000ee'
const HASH = `0x${'ab'.repeat(32)}`
const NONCE = `0x${'0'.repeat(62)}2a`

type Request = { method: string; params?: unknown[] }
type Scenario = {
  code?: string
  ledger?: boolean
  denies?: boolean
  signError?: unknown
  noRequestAccounts?: boolean
  activeAccount?: string
  /** The account the wallet answers from its second accounts request on. */
  switchesTo?: string
  wrapsAnswers?: boolean
  relay?: { status: number; body: object }
}

/** A wallet on Ethereum signing with the test key, a Polygon read node answering the nonce, and the relayer. */
function world(scenario: Scenario = {}) {
  const walletLog: Request[] = []
  let answered = 0
  const accounts = () => {
    const active = answered++ > 0 && scenario.switchesTo ? scenario.switchesTo : (scenario.activeAccount ?? TEST_SIGNER)
    const list = [active]
    return scenario.wrapsAnswers ? { result: list } : list
  }
  const request = async ({ method, params = [] }: Request): Promise<unknown> => {
    if (method !== 'eth_chainId') walletLog.push({ method, params })
    switch (method) {
      case 'eth_chainId':
        return '0x1'
      case 'eth_requestAccounts':
        if (scenario.noRequestAccounts) throw new Error('Unsupported method')
        return accounts()
      case 'eth_accounts':
        return accounts()
      case 'eth_getCode':
        return scenario.code ?? '0x'
      case 'eth_signTypedData_v4': {
        if (scenario.denies) throw new Error('MetaMask Typed Message Signature: User denied message signature.')
        // eslint-disable-next-line @typescript-eslint/only-throw-error -- EIP-1193 wallets reject with plain objects
        if (scenario.signError) throw scenario.signError
        const { types, domain, primaryType, message } = JSON.parse(params[1] as string)
        const withoutDomain = Object.fromEntries(Object.entries(types).filter(([name]) => name !== 'EIP712Domain'))
        const signature = await testAccount.signTypedData({ types: withoutDomain, domain, primaryType, message })
        if (!scenario.ledger) return signature
        // Ledger answers the recovery id as 0 or 1.
        return signature.slice(0, -2) + (parseInt(signature.slice(-2), 16) - 27).toString(16).padStart(2, '0')
      }
    }
    throw new Error(`Unexpected ${method}`)
  }
  const relayed: unknown[] = []
  const nodeCalls: Request[] = []
  vi.stubGlobal(
    'fetch',
    vi.fn(async (url: string, init: RequestInit) => {
      if (url === RELAYER) {
        relayed.push(JSON.parse(init.body as string))
        const { status, body } = scenario.relay ?? { status: 200, body: { ok: true, txHash: HASH } }
        return new Response(JSON.stringify(body), { status })
      }
      const body = JSON.parse(init.body as string) as { id: number; method: string; params: unknown[] }
      nodeCalls.push({ method: body.method, params: body.params })
      return new Response(JSON.stringify({ jsonrpc: '2.0', id: body.id, result: NONCE }))
    })
  )
  const session = { address: TEST_SIGNER, provider: { request } } as unknown as Session
  return { session, walletLog, relayed, nodeCalls }
}

async function send(contract: ContractData, method: string, args: unknown[], scenario?: Scenario) {
  const w = world(scenario)
  let outcome: { hash: string } | { error: { message: string; code: string } }
  try {
    outcome = { hash: await sendContractTransaction(w.session, { contract, method, args }) }
  } catch (error) {
    const { message, code } = error as { message: string; code: string }
    outcome = { error: { message, code } }
  }
  return { ...w, outcome }
}

const transfer = [
  getCollectionContract(80002, COLLECTION),
  'transferCreatorship',
  ['0x00000000000000000000000000000000000000bb']
] as const
// increaseSignerSignatureIndex() keeps the calldata short; the meta-transaction style comes from the deployment.
const increaseIndex = [
  { ...getContract('OffChainMarketplaceV3', 80002), abi: parseAbi(['function increaseSignerSignatureIndex()']) },
  'increaseSignerSignatureIndex',
  []
] as const

const FUNCTION_SIGNATURE = {
  typedData:
    '{"types":{"EIP712Domain":[{"name":"name","type":"string"},{"name":"version","type":"string"},{"name":"verifyingContract","type":"address"},{"name":"salt","type":"bytes32"}],"MetaTransaction":[{"name":"nonce","type":"uint256"},{"name":"from","type":"address"},{"name":"functionSignature","type":"bytes"}]},"domain":{"name":"Decentraland Collection","version":"2","verifyingContract":"0x00000000000000000000000000000000000000ee","salt":"0x0000000000000000000000000000000000000000000000000000000000013882"},"primaryType":"MetaTransaction","message":{"nonce":42,"from":"0x19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A","functionSignature":"0x6d2e4b1b00000000000000000000000000000000000000000000000000000000000000bb"}}',
  relayed: [
    '0x0c53c51c',
    '00000000000000000000000019E7E376E7C213B7E7e7e46cc70A5dD086DAff2A',
    '00000000000000000000000000000000000000000000000000000000000000a0',
    'ae080084cc6936e98d428d4b64894e6819d688acf1d0bd779c75c9a7dbcf7385',
    '2449f77902b066ed6b8e42ddf9d4211dd160fb9ed6e8ffdb2902005608040af7',
    '000000000000000000000000000000000000000000000000000000000000001b',
    '0000000000000000000000000000000000000000000000000000000000000024',
    '6d2e4b1b00000000000000000000000000000000000000000000000000000000',
    '000000bb00000000000000000000000000000000000000000000000000000000'
  ].join('')
}

const FUNCTION_DATA = {
  typedData:
    '{"types":{"EIP712Domain":[{"name":"name","type":"string"},{"name":"version","type":"string"},{"name":"verifyingContract","type":"address"},{"name":"salt","type":"bytes32"}],"MetaTransaction":[{"name":"nonce","type":"uint256"},{"name":"from","type":"address"},{"name":"functionData","type":"bytes"}]},"domain":{"name":"DecentralandMarketplacePolygon","version":"1.0.0","verifyingContract":"0x36fd1434a6c4b8ade80c9847c1d15033ce34488c","salt":"0x0000000000000000000000000000000000000000000000000000000000013882"},"primaryType":"MetaTransaction","message":{"nonce":42,"from":"0x19E7E376E7C213B7E7e7e46cc70A5dD086DAff2A","functionData":"0xceefad9f"}}',
  relayed: [
    '0xd8ed1acc',
    '00000000000000000000000019E7E376E7C213B7E7e7e46cc70A5dD086DAff2A',
    '0000000000000000000000000000000000000000000000000000000000000060',
    '0000000000000000000000000000000000000000000000000000000000000084',
    '0000000000000000000000000000000000000000000000000000000000000004',
    'ceefad9f00000000000000000000000000000000000000000000000000000000',
    '00000041a5aec020c18b2c6e51fe654d911839c51d5c3b19991dc57eb56b6563',
    '9c82b15150fec555c6ce5e668d93f8fb1e89a28da25d3ad61d66b5bc1d7ba65b',
    'd088ec0a1b000000000000000000000000000000000000000000000000000000',
    '00000000'
  ].join('')
}

const sendTransfer = (scenario?: Scenario) => send(transfer[0], transfer[1], [...transfer[2]], scenario)
const sendIncreaseIndex = (scenario?: Scenario) => send(increaseIndex[0], increaseIndex[1], [], scenario)

describe('when a call goes gasless through the relayer', () => {
  it.each([
    ['functionSignature', sendTransfer, transfer[0].address, FUNCTION_SIGNATURE],
    ['functionData', sendIncreaseIndex, increaseIndex[0].address, FUNCTION_DATA]
  ] as const)('should sign and relay the %s style exactly as the relayer expects', async (_, run, to, expected) => {
    const { outcome, walletLog, nodeCalls, relayed } = await run()
    expect(outcome).toEqual({ hash: HASH })
    expect(walletLog).toEqual([
      { method: 'eth_requestAccounts', params: [] },
      { method: 'eth_getCode', params: [TEST_SIGNER.toLowerCase(), 'latest'] },
      { method: 'eth_signTypedData_v4', params: [TEST_SIGNER, expected.typedData] }
    ])
    expect(nodeCalls).toEqual([
      { method: 'eth_call', params: [{ data: `0x2d0335ab${TEST_SIGNER.slice(2).padStart(64, '0')}`, to }, 'latest'] }
    ])
    expect(relayed).toEqual([{ transactionData: { from: TEST_SIGNER, params: [to, expected.relayed] } }])
  })

  it.each([
    ['functionSignature', sendTransfer, FUNCTION_SIGNATURE],
    ['functionData', sendIncreaseIndex, FUNCTION_DATA]
  ] as const)("should relay a Ledger's 0/1 recovery id as 27/28 in the %s style", async (_, run, expected) => {
    const { relayed } = await run({ ledger: true })
    expect(relayed).toEqual([
      { transactionData: expect.objectContaining({ params: [expect.any(String), expected.relayed] }) }
    ])
  })

  it('should fall back to eth_accounts for a wallet that refuses eth_requestAccounts', async () => {
    const { outcome, walletLog } = await sendTransfer({ noRequestAccounts: true })
    expect(outcome).toEqual({ hash: HASH })
    expect(walletLog.slice(0, 2).map(({ method }) => method)).toEqual(['eth_requestAccounts', 'eth_accounts'])
  })

  it('should accept an EIP-7702 delegated account, which still signs with its own key', async () => {
    const { outcome } = await sendTransfer({ code: `0xef0100${'cd'.repeat(20)}` })
    expect(outcome).toEqual({ hash: HASH })
  })

  it.each<[string, Scenario, { message: string; code: string }, number]>([
    [
      'the signer is a contract',
      { code: '0x6080604052' },
      { message: 'Contract accounts are not supported', code: 'contract_account' },
      0
    ],
    [
      'the creator denies the signature',
      { denies: true },
      { message: 'MetaMask Typed Message Signature: User denied message signature.', code: 'user_denied' },
      0
    ],
    [
      'the relayer refuses with a reason',
      { relay: { status: 400, body: { ok: false, message: 'Max limit', code: 'user_limit_reached' } } },
      { message: 'Max limit', code: 'user_limit_reached' },
      1
    ],
    [
      'the relayer refuses without one',
      { relay: { status: 500, body: { ok: false } } },
      { message: 'HTTP Error. Status: 500.', code: 'unknown' },
      1
    ],
    [
      'the relayer fails with a body of another shape',
      { relay: { status: 502, body: { error: 'Bad gateway' } } },
      { message: 'HTTP Error. Status: 502.', code: 'unknown' },
      1
    ],
    [
      'the relayer answers without a transaction hash',
      { relay: { status: 200, body: { ok: true } } },
      { message: 'The relayer answered without a transaction hash', code: 'unknown' },
      1
    ]
  ])('should fail with a coded error when %s', async (_, scenario, error, relays) => {
    const { outcome, relayed } = await sendTransfer(scenario)
    expect(outcome).toEqual({ error })
    expect(relayed).toHaveLength(relays)
  })

  it('should treat a dismissed signature as a wallet rejection whatever the wallet says', async () => {
    const { session } = world({ signError: { code: 4001, message: 'whatever' } })
    const error = await sendContractTransaction(session, {
      contract: transfer[0],
      method: transfer[1],
      args: [...transfer[2]]
    }).catch((e: unknown) => e)
    expect(isWalletRejection(error)).toBe(true)
    expect((error as Error).message).toBe('whatever')
  })

  it('should accept a legacy wallet that wraps its accounts in `result`', async () => {
    const { outcome } = await sendTransfer({ wrapsAnswers: true })
    expect(outcome).toEqual({ hash: HASH })
  })

  it('should sign with the account it checked even if the wallet would answer another one next', async () => {
    const { outcome, walletLog, relayed } = await sendTransfer({
      switchesTo: '0x00000000000000000000000000000000000000bb'
    })
    expect(outcome).toEqual({ hash: HASH })
    expect(walletLog.find(({ method }) => method === 'eth_signTypedData_v4')?.params?.[0]).toBe(TEST_SIGNER)
    expect(relayed).toEqual([{ transactionData: expect.objectContaining({ from: TEST_SIGNER }) }])
  })

  it("should refuse to sign when the wallet's active account is not the session's", async () => {
    const w = world({ activeAccount: '0x00000000000000000000000000000000000000bb' })
    const error = await sendContractTransaction(w.session, {
      contract: transfer[0],
      method: transfer[1],
      args: [...transfer[2]]
    }).catch((e: unknown) => e)
    expect(error).toBeInstanceOf(WrongAccountError)
    expect(w.walletLog.map(({ method }) => method)).not.toContain('eth_signTypedData_v4')
    expect(w.relayed).toHaveLength(0)
  })
})
