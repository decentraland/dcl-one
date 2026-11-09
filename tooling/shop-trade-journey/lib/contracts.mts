// dcl-one's own CreditsManagerPolygon on Amoy: the manager tooling/dev.py's testnet profile names in
// CREDITS_MANAGER_CONTRACT, which the stack signs credits for. The Shop's settlement gate refuses the
// shared upstream one (0x8052…2fb3 in contracts.decentraland.org's `amoy` list). contracts.test.mts keeps
// this in step with dev.py.
export const AMOY_CREDITS_MANAGER = "0xb65fd692d15c30c3adcacdd5ccf80e1facecfc7c" as const;
