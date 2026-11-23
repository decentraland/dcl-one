// The contracts this app calls, per chain, with their EIP-712 domain and only the ABI functions used here.
// Values from decentraland-transactions@3.3.0 (Apache-2.0).
import { encodeFunctionData, parseAbi, type Abi } from 'viem'

/** How a contract's `executeMetaTransaction` takes the call: r/s/v split, or one signature blob. */
export type MetaTransactionStyle = 'functionSignature' | 'functionData'

export type ContractData<TAbi extends Abi = Abi> = {
  address: string
  /** EIP-712 domain name. */
  name: string
  version: string
  chainId: number
  abi: TAbi
  /** How a meta-transaction to it encodes the call; absent is sent as `functionSignature`, as the package did. */
  metaTransaction?: MetaTransactionStyle
}

export type ContractCall = {
  contract: ContractData
  method: string
  args: unknown[]
}

/** ABI-encoded calldata for a contract call. */
export function encodeContractCall({ contract, method, args }: ContractCall): string {
  return encodeFunctionData({ abi: contract.abi, functionName: method, args })
}

const MANA_ABI = parseAbi([
  'function balanceOf(address account) view returns (uint256)',
  'function allowance(address owner, address spender) view returns (uint256)',
  'function approve(address spender, uint256 amount) returns (bool)'
])

const COLLECTION_ABI = parseAbi([
  'function issueTokens(address[] _beneficiaries, uint256[] _itemIds)',
  'function setMinters(address[] _minters, bool[] _values)',
  'function setManagers(address[] _managers, bool[] _values)',
  'function transferCreatorship(address _newCreator)',
  'function items(uint256) view returns (string rarity, uint256 maxSupply, uint256 totalSupply, uint256 price, address beneficiary, string metadata, string contentHash)',
  'function editItemsData(uint256[] _itemIds, uint256[] _prices, address[] _beneficiaries, string[] _metadatas)',
  'function setApproved(bool _value)',
  'function rescueItems(uint256[] _itemIds, string[] _contentHashes, string[] _metadatas)'
])

const COMMITTEE_ABI = parseAbi([
  'function manageCollection(address _collectionManager, address _forwarder, address _collection, bytes[] _data)'
])

const COLLECTION_MANAGER_ABI = parseAbi([
  'function createCollection(address _forwarder, address _factory, bytes32 _salt, string _name, string _symbol, string _baseURI, address _creator, (string rarity, uint256 price, address beneficiary, string metadata)[] _items)'
])

const CREDITS_MANAGER_ABI = parseAbi([
  'function useCredits(((uint256 value, uint256 expiresAt, bytes32 salt)[] credits, bytes[] creditsSignatures, (address target, bytes4 selector, bytes data, uint256 expiresAt, bytes32 salt) externalCall, bytes customExternalCallSignature, uint256 maxUncreditedValue, uint256 maxCreditedValue) _args)'
])

const RARITIES_ABI = parseAbi([
  'function getRarityByName(string _rarity) view returns ((string name, uint256 maxSupply, uint256 price))'
])

const THIRD_PARTY_REGISTRY_ABI = parseAbi([
  'function addThirdParties((string id, string metadata, string resolver, address[] managers, bool[] managerValues, uint256 slots)[] _thirdParties, bool[] _areProgrammatic, uint256[] _maxPrices)',
  'function updateThirdParties((string id, string metadata, string resolver, address[] managers, bool[] managerValues, uint256 slots)[] _thirdParties)',
  'function buyItemSlots(string _thirdPartyId, uint256 _qty, uint256 _maxPrice)',
  'function reviewThirdPartyWithRoot(string _thirdPartyId, bytes32 _root, (uint256 qty, bytes32 salt, bytes32 sigR, bytes32 sigS, uint8 sigV)[] _consumeSlotsParams)',
  'struct ItemReview { string id; string metadata; string contentHash; bool value; }',
  'struct ThirdPartyReview { string id; bool value; ItemReview[] items; }',
  'function reviewThirdParties(ThirdPartyReview[] _thirdParties)',
  'function thirdParties(string) view returns (bool isApproved, bytes32 root, uint256 maxItems, uint256 consumedSlots, uint256 registered, string metadata, string resolver)',
  'function itemSlotPrice() view returns (uint256)',
  'function programmaticBasePurchasedSlots() view returns (uint256)'
])

const CHAINLINK_ORACLE_ABI = parseAbi(['function getRate() view returns (uint256)'])

const SIGNATURE_INDEXES = [
  'function contractSignatureIndex() view returns (uint256)',
  'function signerSignatureIndex(address) view returns (uint256)'
] as const

const CHECKS = [
  'struct ExternalCheck { address contractAddress; bytes4 selector; bytes value; bool required; }',
  'struct Checks { uint256 uses; uint256 expiration; uint256 effective; bytes32 salt; uint256 contractSignatureIndex; uint256 signerSignatureIndex; bytes32 allowedRoot; bytes32[] allowedProof; ExternalCheck[] externalChecks; }'
] as const

const MARKETPLACE_ABI = parseAbi([
  ...SIGNATURE_INDEXES,
  ...CHECKS,
  'struct Asset { uint256 assetType; address contractAddress; uint256 value; address beneficiary; bytes extra; }',
  'struct Trade { address signer; bytes signature; Checks checks; Asset[] sent; Asset[] received; }',
  'function cancelSignature(Trade[] _trades)',
  // Polygon deployments only.
  'function manaUsdAggregator() view returns (address)'
])

const COUPON_MANAGER_ABI = parseAbi([
  ...SIGNATURE_INDEXES,
  ...CHECKS,
  'struct Coupon { bytes signature; Checks checks; address couponAddress; bytes data; bytes callerData; }',
  'function cancelSignature(Coupon[] _coupons)'
])

// Contracts this app only needs the address of.
const NO_ABI = [] as const

type Deployment = { address: string; name: string; version: string; metaTransaction?: MetaTransactionStyle }

const ETHEREUM = 1
const SEPOLIA = 11155111
const POLYGON = 137
const AMOY = 80002

const MARKETPLACE_ETHEREUM = { name: 'DecentralandMarketplaceEthereum', version: '1.0.0' }
const MARKETPLACE_POLYGON = {
  name: 'DecentralandMarketplacePolygon',
  version: '1.0.0',
  metaTransaction: 'functionData'
} as const
const COUPON_MANAGER = { name: 'CouponManager', version: '1.0.0' }

const REGISTRY = {
  ChainlinkOracle: {
    abi: CHAINLINK_ORACLE_ABI,
    on: {
      [POLYGON]: {
        address: '0xe18B1361d41afC44658216F3Dc27e48c2336e3c2',
        name: 'Decentraland ChainLink Oracle',
        version: '1'
      },
      [AMOY]: {
        address: '0x7474ae33311ad160ed35c8d9cea95f0856ca782a',
        name: 'Decentraland ChainLink Oracle',
        version: '1'
      }
    }
  },
  CollectionDiscountCoupon: {
    abi: NO_ABI,
    on: {
      [POLYGON]: {
        address: '0xc914507fe297b2dddd1232ac3a8903f1c125e794',
        name: 'CollectionDiscountCoupon',
        version: '1.0.0'
      },
      [AMOY]: {
        address: '0x4ee8f6b87f4917a3bbc7c8bb3a06db8555f83db9',
        name: 'CollectionDiscountCoupon',
        version: '1.0.0'
      }
    }
  },
  CollectionFactoryV3: {
    abi: NO_ABI,
    on: {
      [POLYGON]: { address: '0x3195e88aE10704b359764CB38e429D24f1c2f781', name: 'CollectionFactory', version: '3' },
      [AMOY]: { address: '0x802de0c509add2ee29de24de7225daaff4741c43', name: 'CollectionFactory', version: '3' }
    }
  },
  CollectionManager: {
    abi: COLLECTION_MANAGER_ABI,
    on: {
      [POLYGON]: {
        address: '0x9D32AaC179153A991e832550d9F96441Ea27763A',
        name: 'Decentraland Collection Manager',
        version: '1',
        metaTransaction: 'functionSignature'
      },
      [AMOY]: {
        address: '0x5309ae874fc4eb21adcd63f8b6c3f766cc3b1849',
        name: 'Decentraland Collection Manager',
        version: '1',
        metaTransaction: 'functionSignature'
      }
    }
  },
  CollectionStore: {
    abi: NO_ABI,
    on: {
      [POLYGON]: {
        address: '0x214ffC0f0103735728dc66b61A22e4F163e275ae',
        name: 'Decentraland Collection Store',
        version: '1',
        metaTransaction: 'functionSignature'
      },
      [AMOY]: {
        address: '0xe36abc9ec616c83caaa386541380829106149d68',
        name: 'Decentraland Collection Store',
        version: '1',
        metaTransaction: 'functionSignature'
      }
    }
  },
  Committee: {
    abi: COMMITTEE_ABI,
    on: {
      [POLYGON]: {
        address: '0xaeec95a8aa671a6d3fec56594827d7804964fa70',
        name: 'Decentraland Collection Committee',
        version: '1',
        metaTransaction: 'functionSignature'
      },
      [AMOY]: {
        address: '0x53c9c388000b6fc91b91fb466742d0e869785c3b',
        name: 'Decentraland Collection Committee',
        version: '1',
        metaTransaction: 'functionSignature'
      }
    }
  },
  CouponManagerV2: {
    abi: COUPON_MANAGER_ABI,
    on: {
      [POLYGON]: { address: '0x3fd3056ee72a2a85e9392fab3a450e7736536081', ...COUPON_MANAGER },
      [AMOY]: { address: '0xa40b1d129b8906888720686f3a01921ddf37716f', ...COUPON_MANAGER }
    }
  },
  CouponManagerV3: {
    abi: COUPON_MANAGER_ABI,
    on: {
      [ETHEREUM]: { address: '0xf9180eed9fcd5f8b3921c1b8caeb771c10faeb26', ...COUPON_MANAGER },
      [SEPOLIA]: { address: '0xed558211ae5ae57a6704423918cb9b8501051af0', ...COUPON_MANAGER },
      [POLYGON]: { address: '0x655fdfa91d69ea49f4ce1a8f7f7e2622c8630813', ...COUPON_MANAGER },
      [AMOY]: { address: '0x6c956587d9fe70032781edcdc626310648575382', ...COUPON_MANAGER }
    }
  },
  CreditsManager: {
    abi: CREDITS_MANAGER_ABI,
    on: {
      [POLYGON]: {
        address: '0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5',
        name: 'Decentraland Credits',
        version: '1.0.0',
        metaTransaction: 'functionData'
      },
      [AMOY]: {
        address: '0x8052a560e6e6ac86eeb7e711a4497f639b322fb3',
        name: 'Decentraland Credits',
        version: '1.0.0',
        metaTransaction: 'functionData'
      }
    }
  },
  Forwarder: {
    abi: NO_ABI,
    on: {
      [POLYGON]: { address: '0xBF6755A83C0dCDBB2933A96EA778E00b717d7004', name: 'Forwarder', version: '2' },
      [AMOY]: { address: '0x7b1fe9de545b22cb553766817b84d655ce8121c9', name: 'Forwarder', version: '2' }
    }
  },
  MANAToken: {
    abi: MANA_ABI,
    on: {
      [ETHEREUM]: {
        address: '0x0f5d2fb29fb7d3cfee444a200298f468908cc942',
        name: 'MANAToken',
        version: '1',
        metaTransaction: 'functionSignature'
      },
      [SEPOLIA]: {
        address: '0xfa04d2e2ba9aec166c93dfeeba7427b2303befa9',
        name: 'MANAToken',
        version: '1',
        metaTransaction: 'functionSignature'
      },
      [POLYGON]: {
        address: '0xA1c57f48F0Deb89f569dFbE6E2B7f46D33606fD4',
        name: '(PoS) Decentraland MANA',
        version: '1',
        metaTransaction: 'functionSignature'
      },
      [AMOY]: {
        address: '0x7ad72b9f944ea9793cf4055d88f81138cc2c63a0',
        name: 'Decentraland MANA(PoS)',
        version: '1',
        metaTransaction: 'functionSignature'
      }
    }
  },
  OffChainMarketplace: {
    abi: MARKETPLACE_ABI,
    on: {
      [ETHEREUM]: { address: '0x2d6b3508f9aca32d2550f92b2addba932e73c1ff', ...MARKETPLACE_ETHEREUM },
      [SEPOLIA]: { address: '0x54bdd3ffc1448bdab0251f05dd554eec803adb2f', ...MARKETPLACE_ETHEREUM },
      [POLYGON]: { address: '0x540fb08eDb56AaE562864B390542C97F562825BA', ...MARKETPLACE_POLYGON },
      [AMOY]: { address: '0x6ab20ae56673ed65f520b7be332aeb61b3ed727d', ...MARKETPLACE_POLYGON }
    }
  },
  OffChainMarketplaceV2: {
    abi: MARKETPLACE_ABI,
    on: {
      [ETHEREUM]: { address: '0x1b67d0e31eeb6b52d8eeed71d3616c2f5b33b8e7', ...MARKETPLACE_ETHEREUM },
      [SEPOLIA]: { address: '0x1b67d0e31eeb6b52d8eeed71d3616c2f5b33b8e7', ...MARKETPLACE_ETHEREUM },
      [POLYGON]: { address: '0xa40b1d129b8906888720686f3a01921ddf37716f', ...MARKETPLACE_POLYGON },
      [AMOY]: { address: '0x1b67d0e31eeb6b52d8eeed71d3616c2f5b33b8e7', ...MARKETPLACE_POLYGON }
    }
  },
  OffChainMarketplaceV3: {
    abi: MARKETPLACE_ABI,
    on: {
      [ETHEREUM]: { address: '0x0f11d0d1671519683bd48abf3dbe779e300941cd', ...MARKETPLACE_ETHEREUM },
      [SEPOLIA]: { address: '0x257db44ac97789c16ab277eae87dcde0c246cc9f', ...MARKETPLACE_ETHEREUM },
      [POLYGON]: { address: '0xe38ef22abe871513555cba89adfe45ab4f548ada', ...MARKETPLACE_POLYGON },
      [AMOY]: { address: '0x36fd1434a6c4b8ade80c9847c1d15033ce34488c', ...MARKETPLACE_POLYGON }
    }
  },
  RaritiesWithOracle: {
    abi: RARITIES_ABI,
    on: {
      [POLYGON]: {
        address: '0xA9158E22F89Bb3F69c5600338895Cb5FB81e5090',
        name: 'Decentraland Rarities with Oracle',
        version: '1',
        metaTransaction: 'functionSignature'
      },
      [AMOY]: {
        address: '0x25b6b4bac4adb582a0abd475439da6730777fbf7',
        name: 'Decentraland Rarities with Oracle',
        version: '1',
        metaTransaction: 'functionSignature'
      }
    }
  },
  ThirdPartyRegistry: {
    abi: THIRD_PARTY_REGISTRY_ABI,
    on: {
      [POLYGON]: {
        address: '0x1C436C1EFb4608dFfDC8bace99d2B03c314f3348',
        name: 'Decentraland Third Party Registry',
        version: '1',
        metaTransaction: 'functionSignature'
      },
      [AMOY]: {
        address: '0x7d7c0b9d97385bada5fec6861e97d0df414af3c3',
        name: 'Decentraland Third Party Registry',
        version: '1',
        metaTransaction: 'functionSignature'
      }
    }
  }
} satisfies Record<string, { abi: Abi; on: Partial<Record<number, Deployment>> }>

export type ContractName = keyof typeof REGISTRY

export const ContractName = Object.fromEntries(Object.keys(REGISTRY).map(name => [name, name])) as {
  [N in ContractName]: N
}

export type ContractAbi<N extends ContractName> = (typeof REGISTRY)[N]['abi']

/** Throws when the contract is not deployed on `chainId`. */
export function getContract<N extends ContractName>(name: N, chainId: number): ContractData<ContractAbi<N>> {
  const deployment = (REGISTRY[name].on as Partial<Record<number, Deployment>>)[chainId]
  if (!deployment) throw new Error(`Could not get a valid contract for ${name} using chain ${chainId}`)
  return { ...deployment, chainId, abi: REGISTRY[name].abi }
}

const COLLECTION = { name: 'Decentraland Collection', version: '2', metaTransaction: 'functionSignature' } as const
const COLLECTION_CHAINS = new Set([POLYGON, AMOY])

/** A collection's own ERC721CollectionV2 contract: every collection shares its domain name and version. */
export function getCollectionContract(chainId: number, address: string): ContractData<typeof COLLECTION_ABI> {
  if (!COLLECTION_CHAINS.has(chainId)) throw new Error(`Collections do not exist on chain ${chainId}`)
  return { ...COLLECTION, address, chainId, abi: COLLECTION_ABI }
}

export const MARKETPLACES = ['OffChainMarketplace', 'OffChainMarketplaceV2', 'OffChainMarketplaceV3'] as const
export type MarketplaceName = (typeof MARKETPLACES)[number]

/** The off-chain marketplace generation deployed at `address` on any chain, if any. */
export function getMarketplaceName(address: string): MarketplaceName | undefined {
  const wanted = address.toLowerCase()
  return MARKETPLACES.find(name =>
    Object.values(REGISTRY[name].on).some(deployment => deployment.address.toLowerCase() === wanted)
  )
}

const COUPON_MANAGER_OF: Partial<Record<MarketplaceName, 'CouponManagerV2' | 'CouponManagerV3'>> = {
  OffChainMarketplaceV2: 'CouponManagerV2',
  OffChainMarketplaceV3: 'CouponManagerV3'
}

/** The coupon manager a marketplace generation redeems coupons through; throws where it pairs with none. */
export function getCouponManager(marketplace: MarketplaceName, chainId: number) {
  const manager = COUPON_MANAGER_OF[marketplace]
  if (!manager) throw new Error(`No coupon manager is paired with ${marketplace}`)
  return getContract(manager, chainId)
}
