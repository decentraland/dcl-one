// builder-server responses for the src/lib/builder specs, which mock `~/lib/auth` signedFetch to return them.

export const ADDRESS = '0xabc0000000000000000000000000000000000001'

export const remoteCollection = {
  id: 'a1b2',
  name: 'Pirate Hats',
  eth_address: ADDRESS,
  salt: null,
  contract_address: null,
  urn: 'urn:decentraland:matic:collections-v2:0xcontract',
  is_published: false,
  is_approved: false,
  minters: [],
  managers: [],
  forum_link: null,
  lock: null,
  reviewed_at: null,
  created_at: '2026-03-01T10:00:00Z',
  updated_at: '2026-04-02T10:00:00Z',
  item_count: '3',
  linked_contract_address: null,
  linked_contract_network: null,
  is_mapping_complete: false
}

export function jsonResponse(body: unknown, ok = true, status = 200) {
  return { ok, status, json: () => Promise.resolve(body) }
}

export function okResponse(data: unknown) {
  return jsonResponse({ ok: true, data })
}
