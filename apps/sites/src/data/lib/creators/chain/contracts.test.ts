import { describe, expect, it } from 'vitest'
import { type Abi, type AbiParameter } from 'viem'
import {
  ContractName,
  getCollectionContract,
  getContract,
  getCouponManager,
  getMarketplaceName,
  MARKETPLACES,
  type ContractData
} from './contracts'

// Expected values were checked against decentraland-transactions@3.3.0's getContract before it was removed.
const CHAINS = [1, 11155111, 137, 80002]

const DEPLOYMENTS: [ContractName, number, string, string, string, ContractData['metaTransaction']][] = [
  [
    'ChainlinkOracle',
    137,
    '0xe18B1361d41afC44658216F3Dc27e48c2336e3c2',
    'Decentraland ChainLink Oracle',
    '1',
    undefined
  ],
  [
    'ChainlinkOracle',
    80002,
    '0x7474ae33311ad160ed35c8d9cea95f0856ca782a',
    'Decentraland ChainLink Oracle',
    '1',
    undefined
  ],
  [
    'CollectionDiscountCoupon',
    137,
    '0xc914507fe297b2dddd1232ac3a8903f1c125e794',
    'CollectionDiscountCoupon',
    '1.0.0',
    undefined
  ],
  [
    'CollectionDiscountCoupon',
    80002,
    '0x4ee8f6b87f4917a3bbc7c8bb3a06db8555f83db9',
    'CollectionDiscountCoupon',
    '1.0.0',
    undefined
  ],
  ['CollectionFactoryV3', 137, '0x3195e88aE10704b359764CB38e429D24f1c2f781', 'CollectionFactory', '3', undefined],
  ['CollectionFactoryV3', 80002, '0x802de0c509add2ee29de24de7225daaff4741c43', 'CollectionFactory', '3', undefined],
  [
    'CollectionManager',
    137,
    '0x9D32AaC179153A991e832550d9F96441Ea27763A',
    'Decentraland Collection Manager',
    '1',
    'functionSignature'
  ],
  [
    'CollectionManager',
    80002,
    '0x5309ae874fc4eb21adcd63f8b6c3f766cc3b1849',
    'Decentraland Collection Manager',
    '1',
    'functionSignature'
  ],
  [
    'CollectionStore',
    137,
    '0x214ffC0f0103735728dc66b61A22e4F163e275ae',
    'Decentraland Collection Store',
    '1',
    'functionSignature'
  ],
  [
    'CollectionStore',
    80002,
    '0xe36abc9ec616c83caaa386541380829106149d68',
    'Decentraland Collection Store',
    '1',
    'functionSignature'
  ],
  [
    'Committee',
    137,
    '0xaeec95a8aa671a6d3fec56594827d7804964fa70',
    'Decentraland Collection Committee',
    '1',
    'functionSignature'
  ],
  [
    'Committee',
    80002,
    '0x53c9c388000b6fc91b91fb466742d0e869785c3b',
    'Decentraland Collection Committee',
    '1',
    'functionSignature'
  ],
  ['CouponManagerV2', 137, '0x3fd3056ee72a2a85e9392fab3a450e7736536081', 'CouponManager', '1.0.0', undefined],
  ['CouponManagerV2', 80002, '0xa40b1d129b8906888720686f3a01921ddf37716f', 'CouponManager', '1.0.0', undefined],
  ['CouponManagerV3', 1, '0xf9180eed9fcd5f8b3921c1b8caeb771c10faeb26', 'CouponManager', '1.0.0', undefined],
  ['CouponManagerV3', 11155111, '0xed558211ae5ae57a6704423918cb9b8501051af0', 'CouponManager', '1.0.0', undefined],
  ['CouponManagerV3', 137, '0x655fdfa91d69ea49f4ce1a8f7f7e2622c8630813', 'CouponManager', '1.0.0', undefined],
  ['CouponManagerV3', 80002, '0x6c956587d9fe70032781edcdc626310648575382', 'CouponManager', '1.0.0', undefined],
  [
    'CreditsManager',
    137,
    '0x8b3a40ca1b6f5cafc99d112a4d02e897d1fd8cc5',
    'Decentraland Credits',
    '1.0.0',
    'functionData'
  ],
  [
    'CreditsManager',
    80002,
    '0x8052a560e6e6ac86eeb7e711a4497f639b322fb3',
    'Decentraland Credits',
    '1.0.0',
    'functionData'
  ],
  ['Forwarder', 137, '0xBF6755A83C0dCDBB2933A96EA778E00b717d7004', 'Forwarder', '2', undefined],
  ['Forwarder', 80002, '0x7b1fe9de545b22cb553766817b84d655ce8121c9', 'Forwarder', '2', undefined],
  ['MANAToken', 1, '0x0f5d2fb29fb7d3cfee444a200298f468908cc942', 'MANAToken', '1', 'functionSignature'],
  ['MANAToken', 11155111, '0xfa04d2e2ba9aec166c93dfeeba7427b2303befa9', 'MANAToken', '1', 'functionSignature'],
  ['MANAToken', 137, '0xA1c57f48F0Deb89f569dFbE6E2B7f46D33606fD4', '(PoS) Decentraland MANA', '1', 'functionSignature'],
  [
    'MANAToken',
    80002,
    '0x7ad72b9f944ea9793cf4055d88f81138cc2c63a0',
    'Decentraland MANA(PoS)',
    '1',
    'functionSignature'
  ],
  [
    'OffChainMarketplace',
    1,
    '0x2d6b3508f9aca32d2550f92b2addba932e73c1ff',
    'DecentralandMarketplaceEthereum',
    '1.0.0',
    undefined
  ],
  [
    'OffChainMarketplace',
    11155111,
    '0x54bdd3ffc1448bdab0251f05dd554eec803adb2f',
    'DecentralandMarketplaceEthereum',
    '1.0.0',
    undefined
  ],
  [
    'OffChainMarketplace',
    137,
    '0x540fb08eDb56AaE562864B390542C97F562825BA',
    'DecentralandMarketplacePolygon',
    '1.0.0',
    'functionData'
  ],
  [
    'OffChainMarketplace',
    80002,
    '0x6ab20ae56673ed65f520b7be332aeb61b3ed727d',
    'DecentralandMarketplacePolygon',
    '1.0.0',
    'functionData'
  ],
  [
    'OffChainMarketplaceV2',
    1,
    '0x1b67d0e31eeb6b52d8eeed71d3616c2f5b33b8e7',
    'DecentralandMarketplaceEthereum',
    '1.0.0',
    undefined
  ],
  [
    'OffChainMarketplaceV2',
    11155111,
    '0x1b67d0e31eeb6b52d8eeed71d3616c2f5b33b8e7',
    'DecentralandMarketplaceEthereum',
    '1.0.0',
    undefined
  ],
  [
    'OffChainMarketplaceV2',
    137,
    '0xa40b1d129b8906888720686f3a01921ddf37716f',
    'DecentralandMarketplacePolygon',
    '1.0.0',
    'functionData'
  ],
  [
    'OffChainMarketplaceV2',
    80002,
    '0x1b67d0e31eeb6b52d8eeed71d3616c2f5b33b8e7',
    'DecentralandMarketplacePolygon',
    '1.0.0',
    'functionData'
  ],
  [
    'OffChainMarketplaceV3',
    1,
    '0x0f11d0d1671519683bd48abf3dbe779e300941cd',
    'DecentralandMarketplaceEthereum',
    '1.0.0',
    undefined
  ],
  [
    'OffChainMarketplaceV3',
    11155111,
    '0x257db44ac97789c16ab277eae87dcde0c246cc9f',
    'DecentralandMarketplaceEthereum',
    '1.0.0',
    undefined
  ],
  [
    'OffChainMarketplaceV3',
    137,
    '0xe38ef22abe871513555cba89adfe45ab4f548ada',
    'DecentralandMarketplacePolygon',
    '1.0.0',
    'functionData'
  ],
  [
    'OffChainMarketplaceV3',
    80002,
    '0x36fd1434a6c4b8ade80c9847c1d15033ce34488c',
    'DecentralandMarketplacePolygon',
    '1.0.0',
    'functionData'
  ],
  [
    'RaritiesWithOracle',
    137,
    '0xA9158E22F89Bb3F69c5600338895Cb5FB81e5090',
    'Decentraland Rarities with Oracle',
    '1',
    'functionSignature'
  ],
  [
    'RaritiesWithOracle',
    80002,
    '0x25b6b4bac4adb582a0abd475439da6730777fbf7',
    'Decentraland Rarities with Oracle',
    '1',
    'functionSignature'
  ],
  [
    'ThirdPartyRegistry',
    137,
    '0x1C436C1EFb4608dFfDC8bace99d2B03c314f3348',
    'Decentraland Third Party Registry',
    '1',
    'functionSignature'
  ],
  [
    'ThirdPartyRegistry',
    80002,
    '0x7d7c0b9d97385bada5fec6861e97d0df414af3c3',
    'Decentraland Third Party Registry',
    '1',
    'functionSignature'
  ]
]

const param = (p: AbiParameter): string =>
  (p.type.startsWith('tuple')
    ? `(${(p as { components: readonly AbiParameter[] }).components.map(param).join(', ')})${p.type.slice(5)}`
    : p.type) + (p.name ? ` ${p.name}` : '')

const functions = (abi: Abi) =>
  abi
    .flatMap(item =>
      item.type === 'function'
        ? [
            `${item.name}(${item.inputs.map(param).join(', ')}) ${item.stateMutability} returns (${item.outputs.map(param).join(', ')})`
          ]
        : []
    )
    .sort()

// Names and outputs included: readContract returns named tuple outputs as objects, and struct inputs are encoded by name.
const FUNCTIONS: Partial<Record<ContractName, string[]>> = {
  ChainlinkOracle: ['getRate() view returns (uint256)'],
  CollectionManager: [
    'createCollection(address _forwarder, address _factory, bytes32 _salt, string _name, string _symbol, string _baseURI, address _creator, (string rarity, uint256 price, address beneficiary, string metadata)[] _items) nonpayable returns ()'
  ],
  Committee: [
    'manageCollection(address _collectionManager, address _forwarder, address _collection, bytes[] _data) nonpayable returns ()'
  ],
  CouponManagerV2: [
    'cancelSignature((bytes signature, (uint256 uses, uint256 expiration, uint256 effective, bytes32 salt, uint256 contractSignatureIndex, uint256 signerSignatureIndex, bytes32 allowedRoot, bytes32[] allowedProof, (address contractAddress, bytes4 selector, bytes value, bool required)[] externalChecks) checks, address couponAddress, bytes data, bytes callerData)[] _coupons) nonpayable returns ()',
    'contractSignatureIndex() view returns (uint256)',
    'signerSignatureIndex(address) view returns (uint256)'
  ],
  CouponManagerV3: [
    'cancelSignature((bytes signature, (uint256 uses, uint256 expiration, uint256 effective, bytes32 salt, uint256 contractSignatureIndex, uint256 signerSignatureIndex, bytes32 allowedRoot, bytes32[] allowedProof, (address contractAddress, bytes4 selector, bytes value, bool required)[] externalChecks) checks, address couponAddress, bytes data, bytes callerData)[] _coupons) nonpayable returns ()',
    'contractSignatureIndex() view returns (uint256)',
    'signerSignatureIndex(address) view returns (uint256)'
  ],
  CreditsManager: [
    'useCredits(((uint256 value, uint256 expiresAt, bytes32 salt)[] credits, bytes[] creditsSignatures, (address target, bytes4 selector, bytes data, uint256 expiresAt, bytes32 salt) externalCall, bytes customExternalCallSignature, uint256 maxUncreditedValue, uint256 maxCreditedValue) _args) nonpayable returns ()'
  ],
  MANAToken: [
    'allowance(address owner, address spender) view returns (uint256)',
    'approve(address spender, uint256 amount) nonpayable returns (bool)',
    'balanceOf(address account) view returns (uint256)'
  ],
  OffChainMarketplace: [
    'cancelSignature((address signer, bytes signature, (uint256 uses, uint256 expiration, uint256 effective, bytes32 salt, uint256 contractSignatureIndex, uint256 signerSignatureIndex, bytes32 allowedRoot, bytes32[] allowedProof, (address contractAddress, bytes4 selector, bytes value, bool required)[] externalChecks) checks, (uint256 assetType, address contractAddress, uint256 value, address beneficiary, bytes extra)[] sent, (uint256 assetType, address contractAddress, uint256 value, address beneficiary, bytes extra)[] received)[] _trades) nonpayable returns ()',
    'contractSignatureIndex() view returns (uint256)',
    'manaUsdAggregator() view returns (address)',
    'signerSignatureIndex(address) view returns (uint256)'
  ],
  OffChainMarketplaceV2: [
    'cancelSignature((address signer, bytes signature, (uint256 uses, uint256 expiration, uint256 effective, bytes32 salt, uint256 contractSignatureIndex, uint256 signerSignatureIndex, bytes32 allowedRoot, bytes32[] allowedProof, (address contractAddress, bytes4 selector, bytes value, bool required)[] externalChecks) checks, (uint256 assetType, address contractAddress, uint256 value, address beneficiary, bytes extra)[] sent, (uint256 assetType, address contractAddress, uint256 value, address beneficiary, bytes extra)[] received)[] _trades) nonpayable returns ()',
    'contractSignatureIndex() view returns (uint256)',
    'manaUsdAggregator() view returns (address)',
    'signerSignatureIndex(address) view returns (uint256)'
  ],
  OffChainMarketplaceV3: [
    'cancelSignature((address signer, bytes signature, (uint256 uses, uint256 expiration, uint256 effective, bytes32 salt, uint256 contractSignatureIndex, uint256 signerSignatureIndex, bytes32 allowedRoot, bytes32[] allowedProof, (address contractAddress, bytes4 selector, bytes value, bool required)[] externalChecks) checks, (uint256 assetType, address contractAddress, uint256 value, address beneficiary, bytes extra)[] sent, (uint256 assetType, address contractAddress, uint256 value, address beneficiary, bytes extra)[] received)[] _trades) nonpayable returns ()',
    'contractSignatureIndex() view returns (uint256)',
    'manaUsdAggregator() view returns (address)',
    'signerSignatureIndex(address) view returns (uint256)'
  ],
  RaritiesWithOracle: [
    'getRarityByName(string _rarity) view returns ((string name, uint256 maxSupply, uint256 price))'
  ],
  ThirdPartyRegistry: [
    'addThirdParties((string id, string metadata, string resolver, address[] managers, bool[] managerValues, uint256 slots)[] _thirdParties, bool[] _areProgrammatic, uint256[] _maxPrices) nonpayable returns ()',
    'buyItemSlots(string _thirdPartyId, uint256 _qty, uint256 _maxPrice) nonpayable returns ()',
    'itemSlotPrice() view returns (uint256)',
    'programmaticBasePurchasedSlots() view returns (uint256)',
    'reviewThirdParties((string id, bool value, (string id, string metadata, string contentHash, bool value)[] items)[] _thirdParties) nonpayable returns ()',
    'reviewThirdPartyWithRoot(string _thirdPartyId, bytes32 _root, (uint256 qty, bytes32 salt, bytes32 sigR, bytes32 sigS, uint8 sigV)[] _consumeSlotsParams) nonpayable returns ()',
    'thirdParties(string) view returns (bool isApproved, bytes32 root, uint256 maxItems, uint256 consumedSlots, uint256 registered, string metadata, string resolver)',
    'updateThirdParties((string id, string metadata, string resolver, address[] managers, bool[] managerValues, uint256 slots)[] _thirdParties) nonpayable returns ()'
  ]
}

const COLLECTION_FUNCTIONS = [
  'editItemsData(uint256[] _itemIds, uint256[] _prices, address[] _beneficiaries, string[] _metadatas) nonpayable returns ()',
  'issueTokens(address[] _beneficiaries, uint256[] _itemIds) nonpayable returns ()',
  'items(uint256) view returns (string rarity, uint256 maxSupply, uint256 totalSupply, uint256 price, address beneficiary, string metadata, string contentHash)',
  'rescueItems(uint256[] _itemIds, string[] _contentHashes, string[] _metadatas) nonpayable returns ()',
  'setApproved(bool _value) nonpayable returns ()',
  'setManagers(address[] _managers, bool[] _values) nonpayable returns ()',
  'setMinters(address[] _minters, bool[] _values) nonpayable returns ()',
  'transferCreatorship(address _newCreator) nonpayable returns ()'
]

function deployment(name: ContractName, chainId: number) {
  try {
    return getContract(name, chainId)
  } catch {
    return undefined
  }
}

describe('the contract registry', () => {
  it.each(DEPLOYMENTS)('should hold %s on chain %i', (name, chainId, address, eip712Name, version, metaTransaction) => {
    expect({ ...getContract(name, chainId), abi: undefined }).toEqual({
      address,
      name: eip712Name,
      version,
      chainId,
      abi: undefined,
      metaTransaction
    })
  })

  it('should hold no deployment beyond those', () => {
    const held = Object.values(ContractName).flatMap(name =>
      CHAINS.filter(chainId => deployment(name, chainId)).map(chainId => `${name}@${chainId}`)
    )
    expect(held.sort()).toEqual(DEPLOYMENTS.map(([name, chainId]) => `${name}@${chainId}`).sort())
  })

  it.each(Object.values(ContractName))('should declare exactly the functions the app calls on %s', name => {
    const chainId = CHAINS.find(chain => deployment(name, chain))!
    expect(functions(getContract(name, chainId).abi)).toEqual(FUNCTIONS[name] ?? [])
  })

  it.each([137, 80002])("should describe a collection's own contract on chain %i", chainId => {
    const collection = getCollectionContract(chainId, '0x00000000000000000000000000000000000000ee')
    expect({ ...collection, abi: undefined }).toEqual({
      address: '0x00000000000000000000000000000000000000ee',
      name: 'Decentraland Collection',
      version: '2',
      chainId,
      abi: undefined,
      metaTransaction: 'functionSignature'
    })
    expect(functions(collection.abi)).toEqual(COLLECTION_FUNCTIONS)
  })

  it.each([1, 11155111])('should have no collection contract on chain %i', chainId => {
    expect(() => getCollectionContract(chainId, '0x00000000000000000000000000000000000000ee')).toThrow()
  })
})

describe('when resolving a marketplace from its address', () => {
  it.each(MARKETPLACES.flatMap(name => CHAINS.map(chainId => [name, chainId] as const)))(
    'should name %s from its address on chain %i',
    (name, chainId) => {
      expect(getMarketplaceName(getContract(name, chainId).address.toUpperCase().replace('0X', '0x'))).toBe(name)
    }
  )

  it('should name no marketplace for any other address', () => {
    expect(getMarketplaceName(getContract('MANAToken', 80002).address)).toBeUndefined()
    expect(getMarketplaceName(getContract('ChainlinkOracle', 137).address)).toBeUndefined()
  })
})

describe('when pairing a marketplace with its coupon manager', () => {
  it.each([
    ['OffChainMarketplaceV2', 137, '0x3fd3056ee72a2a85e9392fab3a450e7736536081'],
    ['OffChainMarketplaceV2', 80002, '0xa40b1d129b8906888720686f3a01921ddf37716f'],
    ['OffChainMarketplaceV3', 1, '0xf9180eed9fcd5f8b3921c1b8caeb771c10faeb26'],
    ['OffChainMarketplaceV3', 11155111, '0xed558211ae5ae57a6704423918cb9b8501051af0'],
    ['OffChainMarketplaceV3', 137, '0x655fdfa91d69ea49f4ce1a8f7f7e2622c8630813'],
    ['OffChainMarketplaceV3', 80002, '0x6c956587d9fe70032781edcdc626310648575382']
  ] as const)('should pair %s on chain %i with the manager it redeems through', (marketplace, chainId, manager) => {
    expect(getCouponManager(marketplace, chainId).address).toBe(manager)
  })

  it.each([
    ['OffChainMarketplace', 137],
    ['OffChainMarketplaceV2', 1]
  ] as const)('should pair %s on chain %i with none', (marketplace, chainId) => {
    expect(() => getCouponManager(marketplace, chainId)).toThrow()
  })
})
