import { config } from '/@/config';
import { fetch } from '/shared/fetch';
import { chunk, retry } from '/shared/utils';
import type { WorldDeployment } from './worlds';

/**
 * The LAND list answers every parcel and estate of a wallet in one response; a
 * large holder's runs to megabytes on a cold cache.
 */
const LANDS_ACCESS_TIMEOUT_MS = 30_000;

/** A large estate can hold hundreds of parcels; the content server takes them in batches. */
const MAX_POINTERS_PER_REQUEST = 100;

/**
 * Resolving a batch of pointers answers with every scene's full entity record, so
 * it runs to hundreds of kilobytes and outlasts the default timeout on a cold cache.
 */
const ENTITIES_TIMEOUT_MS = 30_000;
const MAX_ENTITY_IDS_PER_REQUEST = 50;
const ADDRESS = /^0x[0-9a-fA-F]{40}$/;

const SEPARATOR = ',';

export type Coords = [number, number];

export const coordsToId = (x: string | number, y: string | number) => `${x}${SEPARATOR}${y}`;

/**
 * Strict counterpart to `idToCoords`, for input that may not be a coordinate at
 * all — a scene's `base` from a deployment, or an id from a URL.
 *
 * `idToCoords` maps anything unparseable to an empty string, which reads back as
 * `0,0` once coerced to a number. That silently points at a real parcel someone
 * else owns, so anything not exactly `<int>,<int>` is rejected here instead.
 */
export const parseCoords = (id: string): { x: number; y: number } | null => {
  const match = /^(-?\d+),(-?\d+)$/.exec(id.trim());
  return match ? { x: Number(match[1]), y: Number(match[2]) } : null;
};
export const idToCoords = (id: string): [string | number, string | number] => {
  const [x = '', y = ''] = id
    ?.split(SEPARATOR)
    .map(num => parseInt(num, 10))
    .map(num => (isNaN(num) ? '' : num)) || ['', ''];
  return [x, y];
};

/**
 * Who the Catalyst lets deploy, read off the LAND list. It mirrors the access check
 * every deployment goes through (`checkLAND` in decentraland/checker-contract,
 * `validations/access/subgraph/scenes.ts` in decentraland/content-validator): the
 * token's owner, its approved address, its update operator, or someone the owner
 * approved on that same registry. A rented-out token's owner is the Rentals
 * contract and its update operator is whoever the tenant named, so the lessor
 * holds no right to it until it is claimed back.
 */
type DeployAccess = {
  address: string;
  /** Owners whose latest LAND registry authorization to `address` is approved. */
  landGrantors: Set<string>;
  /** Owners whose latest Estate registry authorization to `address` is approved. */
  estateGrantors: Set<string>;
};

const isAddress = (candidate: string | null | undefined, address: string) =>
  !!candidate && candidate.toLowerCase() === address;

const canDeployTo = (
  token: Pick<ParcelFields, 'owner' | 'operator' | 'updateOperator'>,
  access: DeployAccess,
  grantors: Set<string>,
) =>
  isAddress(token.owner.address, access.address) ||
  isAddress(token.operator, access.address) ||
  isAddress(token.updateOperator, access.address) ||
  grantors.has(token.owner.address.toLowerCase());

/** Every parcel the lands cover, estates expanded into their parcels. */
export function ownedCoords(lands: Land[]): Coords[] {
  return lands.flatMap(land =>
    land.parcels?.length
      ? land.parcels.map(({ x, y }): Coords => [x, y])
      : land.x !== undefined && land.y !== undefined
        ? [[land.x, land.y] as Coords]
        : [],
  );
}

const fromParcel = (parcel: ParcelFields, role: RoleType, access: DeployAccess) => {
  const id = coordsToId(parcel.x, parcel.y);

  const result: Land = {
    id,
    tokenId: parcel.tokenId,
    name: parcel.data?.name || '',
    type: LandType.PARCEL,
    roles: [role],
    role,
    description: (parcel.data && parcel.data.description) || null,
    x: parseInt(parcel.x, 10),
    y: parseInt(parcel.y, 10),
    owner: parcel.owner.address,
    operators: [],
    updateOperator:
      parcel.updateOperator && !isZero(parcel.updateOperator) ? parcel.updateOperator : null,
    deployable: canDeployTo(parcel, access, access.landGrantors),
  };

  if (parcel.updateOperator) {
    result.operators.push(parcel.updateOperator);
  }

  return result;
};

const fromEstate = (estate: EstateFields, role: RoleType, access: DeployAccess) => {
  const id = estate.id;

  const result: Land = {
    id,
    tokenId: id,
    name: estate.data?.name || '',
    type: LandType.ESTATE,
    roles: [role],
    role,
    description: (estate.data && estate.data.description) || null,
    size: estate.size,
    parcels: estate.parcels.map(parcel => ({
      x: parseInt(parcel.x, 10),
      y: parseInt(parcel.y, 10),
      id: coordsToId(parcel.x, parcel.y),
      updateOperator:
        parcel.updateOperator && !isZero(parcel.updateOperator) ? parcel.updateOperator : null,
    })),
    owner: estate.owner.address,
    operators: [],
    updateOperator:
      estate.updateOperator && !isZero(estate.updateOperator) ? estate.updateOperator : null,
    deployable: canDeployTo(estate, access, access.estateGrantors),
  };

  if (estate.updateOperator) {
    result.operators.push(estate.updateOperator);
  }

  return result;
};

function isZero(addr: string) {
  return /^0x(0)+$/.test(addr);
}

export enum LandType {
  PARCEL = 'parcel',
  ESTATE = 'estate',
}

export enum RoleType {
  OWNER = 1,
  LESSOR = 2,
  TENANT = 3,
  OPERATOR = 4,
  MANAGER = 5,
}

export type Land = {
  id: string;
  tokenId: string;
  type: LandType;
  roles: RoleType[];
  role: RoleType;
  x?: number;
  y?: number;
  parcels?: { x: number; y: number; id: string; updateOperator: string | null }[];
  size?: number;
  name: string;
  description: string | null;
  owner: string;
  operators: string[];
  /** The token's own update operator (a collaborator), apart from managers; null when unset. */
  updateOperator: string | null;
  /** Whether the Catalyst accepts a deployment here from the wallet it was fetched for. */
  deployable: boolean;
};

export type ParcelFields = {
  x: string;
  y: string;
  tokenId: string;
  owner: {
    address: string;
  };
  /** The address approved for this one token (`approve`), if any. */
  operator: string | null;
  updateOperator: string | null;
  data: {
    name: string | null;
    description: string | null;
  } | null;
};

export type EstateFields = {
  id: string;
  owner: {
    address: string;
  };
  /** The address approved for this one token (`approve`), if any. */
  operator: string | null;
  updateOperator: string | null;
  size: number;
  parcels: Pick<ParcelFields, 'x' | 'y' | 'tokenId' | 'updateOperator'>[];
  data: {
    name: string | null;
    description: string | null;
  } | null;
};

export type Authorization = {
  address: string;
  type: LandType;
};

export type RentalFields = {
  id: string;
  contractAddress: string;
  tokenId: string;
  lessor: string;
  tenant: string;
  operator: string;
  startedAt: string;
  endsAt: string;
};

export type Rental = {
  id: string;
  type: LandType;
  tokenId: string;
  lessor: string;
  tenant: string;
  operator: string;
  /** Epoch milliseconds. */
  startedAt: number;
  /** Epoch milliseconds. */
  endsAt: number;
};

export type LandQueryResult = {
  tenantParcels: ParcelFields[];
  tenantEstates: EstateFields[];
  lessorParcels: ParcelFields[];
  lessorEstates: EstateFields[];
  ownerParcels: ParcelFields[];
  ownerEstates: EstateFields[];
  updateOperatorParcels: ParcelFields[];
  updateOperatorEstates: EstateFields[];
  approvedParcels: ParcelFields[];
  approvedEstates: EstateFields[];
  ownerAuthorizations: AuthorizationEvent<{ operator: string }>[];
  operatorAuthorizations: AuthorizationEvent<{ owner: { address: string } }>[];
  /** The land of every owner whose UpdateManager or ApprovalForAll grant to the wallet is in force, per registry. */
  grantorParcels: ParcelFields[];
  grantorEstates: EstateFields[];
};

type AuthorizationEvent<T> = T & {
  type: string;
  isApproved: boolean;
  tokenAddress: string;
  /** Block time in microseconds plus the log index, so later events always sort higher. */
  timestamp: string;
};

/**
 * Only the latest authorization per counterparty, registry and kind is in force. The
 * index answers that state already (one row each); the reduction stays so a list
 * that carries a grant beside its later revoke still reads as revoked.
 */
function latestAuthorizations<T extends AuthorizationEvent<object>>(
  events: T[],
  counterparty: (event: T) => string,
): T[] {
  const latest = new Map<string, T>();
  for (const event of events) {
    const key = `${counterparty(event).toLowerCase()}-${event.tokenAddress.toLowerCase()}-${event.type}`;
    const seen = latest.get(key);
    if (!seen || BigInt(event.timestamp) > BigInt(seen.timestamp)) latest.set(key, event);
  }
  return [...latest.values()];
}

export type LandDeployment = Omit<WorldDeployment, 'metadata'> & {
  metadata: Omit<WorldDeployment['metadata'], 'worldConfiguration' | 'source'> & {
    source?: WorldDeployment['metadata']['source'] & { isEmpty?: boolean };
  };
};

export enum Color {
  NEON_BLUE = '#00D3FF',
  SUMMER_RED = '#FF2D55',
  VOID = '#16141A',
  LUISXVI_VIOLET = '#A524B3',
  IDONTFEELTHATWELL = '#F3F2F5',
  SHADOWS = '#242129',
  NIGHT_TIME = '#691FA9',
  BLUISH_STEEL = '#676370',
  INNOCENCE = '#FFFFFF',
  CANDY_PURPLE = '#C640CD',
  SUNISH = '#FFBC5B',
  MILLENIAL_ORANGE = '#FC9965',
  OJ_NOT_SIMPSON = '#FF7439',
  DARK = '#18141A',
}

export const colorByRole: Record<RoleType, string> = {
  [RoleType.OWNER]: Color.SUMMER_RED,
  [RoleType.LESSOR]: Color.MILLENIAL_ORANGE,
  [RoleType.OPERATOR]: Color.LUISXVI_VIOLET,
  [RoleType.TENANT]: Color.SUMMER_RED,
  [RoleType.MANAGER]: Color.LUISXVI_VIOLET,
};

export class Lands {
  private LAND_REGISTRY_ADDRESS = config.get('LAND_REGISTRY_ADDRESS');
  private ESTATE_REGISTRY_ADDRESS = config.get('ESTATE_REGISTRY_ADDRESS');
  private PEER_URL = config.get('PEER_URL');

  public getContentSrcUrl(hash: string) {
    return `${this.PEER_URL}/content/contents/${hash}`;
  }

  /**
   * Every list the LAND page needs, from the content server's land-authz index
   * (`POST /lambdas/users/{address}/lands-access`). A failed or unready read
   * throws rather than answering an empty list of LAND.
   */
  private async fetchLandsAccess(
    address: string,
    tenantTokenIds: string[],
    lessorTokenIds: string[],
  ): Promise<LandQueryResult> {
    const response: Response = await fetch(
      `${this.PEER_URL}/lambdas/users/${address}/lands-access`,
      {
        method: 'POST',
        body: JSON.stringify({ tenantTokenIds, lessorTokenIds }),
        headers: { Accept: 'application/json', 'Content-Type': 'application/json' },
      },
      LANDS_ACCESS_TIMEOUT_MS,
    );
    if (!response.ok) {
      await response.body?.cancel().catch(() => undefined);
      throw new Error(`Failed to fetch the LAND list: ${response.status}`);
    }
    return response.json();
  }

  fetchLand = async (
    _address: string,
    tenantTokenIds: string[] = [],
    lessorTokenIds: string[] = [],
  ): Promise<[Land[], Authorization[]]> => {
    const address = _address.toLowerCase();

    const data = await this.fetchLandsAccess(address, tenantTokenIds, lessorTokenIds);

    const lands: Land[] = [];
    const landUpdateManagers = new Set<string>();
    const estateUpdateManagers = new Set<string>();

    const grants = latestAuthorizations(
      data.operatorAuthorizations,
      event => event.owner.address,
    ).filter(grant => grant.isApproved);
    const grantorsOn = (registry: string) =>
      new Set(
        grants
          .filter(grant => grant.tokenAddress.toLowerCase() === registry)
          .map(grant => grant.owner.address.toLowerCase()),
      );
    const access: DeployAccess = {
      address,
      landGrantors: grantorsOn(this.LAND_REGISTRY_ADDRESS),
      estateGrantors: grantorsOn(this.ESTATE_REGISTRY_ADDRESS),
    };
    const grantorLand = { parcels: data.grantorParcels, estates: data.grantorEstates };

    // parcels and estates that I own
    for (const parcel of data.ownerParcels) {
      lands.push(fromParcel(parcel, RoleType.OWNER, access));
    }
    for (const estate of data.ownerEstates) {
      lands.push(fromEstate(estate, RoleType.OWNER, access));
    }

    // parcels and estates that I operate, or that were approved to me one by one
    for (const parcel of [...data.updateOperatorParcels, ...data.approvedParcels]) {
      lands.push(fromParcel(parcel, RoleType.OPERATOR, access));
    }
    for (const estate of [...data.updateOperatorEstates, ...data.approvedEstates]) {
      lands.push(fromEstate(estate, RoleType.OPERATOR, access));
    }

    // parcels and estates that I've rented
    for (const parcel of data.tenantParcels) {
      lands.push(fromParcel(parcel, RoleType.TENANT, access));
    }
    for (const estate of data.tenantEstates) {
      lands.push(fromEstate(estate, RoleType.TENANT, access));
    }

    // parcels and estates that I've put for rent
    for (const parcel of data.lessorParcels) {
      lands.push(fromParcel(parcel, RoleType.LESSOR, access));
    }
    for (const estate of data.lessorEstates) {
      lands.push(fromEstate(estate, RoleType.LESSOR, access));
    }

    for (const { operator, isApproved, tokenAddress } of latestAuthorizations(
      data.ownerAuthorizations,
      event => event.operator,
    )) {
      if (!isApproved) continue;
      if (tokenAddress === this.LAND_REGISTRY_ADDRESS) landUpdateManagers.add(operator);
      if (tokenAddress === this.ESTATE_REGISTRY_ADDRESS) estateUpdateManagers.add(operator);
    }

    for (const parcel of grantorLand.parcels) {
      const land = fromParcel(parcel, RoleType.MANAGER, access);
      land.operators.push(address);
      if (!lands.some(_land => _land.id === land.id)) {
        lands.push(land);
      }
    }
    for (const estate of grantorLand.estates) {
      if (estate.parcels.length > 0) {
        const land = fromEstate(estate, RoleType.MANAGER, access);
        land.operators.push(address);
        if (!lands.some(_land => _land.id === land.id)) {
          lands.push(land);
        }
      }
    }

    // add operators for all my lands
    const authorizations: Authorization[] = [];
    for (const operator of landUpdateManagers.values()) {
      authorizations.push({ address: operator, type: LandType.PARCEL });
      const parcels = lands.filter(
        land => land.type === LandType.PARCEL && land.role === RoleType.OWNER,
      );
      for (const parcel of parcels) {
        parcel.operators.push(operator);
      }
    }
    for (const operator of estateUpdateManagers.values()) {
      authorizations.push({ address: operator, type: LandType.ESTATE });
      const estates = lands.filter(
        land => land.type === LandType.ESTATE && land.role === RoleType.OWNER,
      );
      for (const estate of estates) {
        estate.operators.push(operator);
      }
    }

    const landsMap: Record<string, Land> = {};

    for (const land of lands) {
      // Remove empty estates
      if (land.type === LandType.ESTATE && land.parcels!.length <= 0) {
        continue;
      }

      // Remove duplicated and zero address operators
      land.operators = Array.from(new Set(land.operators)).filter(address => !isZero(address));

      const savedLand = landsMap[land.id];

      if (savedLand) {
        // Update existing land roles
        savedLand.roles = [...savedLand.roles, land.role].sort();
        savedLand.role = savedLand.roles[0];
        savedLand.deployable ||= land.deployable;
      } else {
        // Add to the total map
        landsMap[land.id] = land;
      }
    }

    return [Object.values(landsMap), authorizations];
  };

  /**
   * The scenes deployed on any of the given parcels, one entry per scene.
   *
   * `/content/entities/active` resolves many pointers at once and answers each
   * scene once, however many parcels it covers — so a 100-parcel estate holding
   * one scene costs one request and yields one entity, not a hundred. The empty
   * scene an unpublish leaves behind (`metadata.source.isEmpty`) is not a scene.
   */
  public async fetchLandPublishedScenes(coords: Coords[]): Promise<LandDeployment[]> {
    const groups = await Promise.all(
      chunk(coords, MAX_POINTERS_PER_REQUEST).map(group =>
        retry(async () => {
          const result = await fetch(
            `${this.PEER_URL}/content/entities/active`,
            {
              method: 'POST',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify({ pointers: group.map(([x, y]) => coordsToId(x, y)) }),
            },
            ENTITIES_TIMEOUT_MS,
          );

          if (!result.ok) {
            throw new Error(`Failed to resolve ${group.length} parcels: ${result.status}`);
          }
          return (await result.json()) as LandDeployment[];
        }),
      ),
    );

    const byEntityId = new Map(
      groups
        .flat()
        .filter(scene => !scene.metadata?.source?.isEmpty)
        .map(scene => [scene.id, scene]),
    );
    return [...byEntityId.values()];
  }

  /**
   * Who deployed each scene entity, as a lowercased address keyed by entity id.
   * Best effort: a chunk that fails leaves its ids out instead of failing the rest.
   */
  public async fetchDeployers(entityIds: string[]): Promise<Record<string, string>> {
    const asked = new Set(entityIds);
    const chunks = await Promise.allSettled(
      chunk(entityIds, MAX_ENTITY_IDS_PER_REQUEST).map(async group => {
        const query = group.map(id => `entityId=${encodeURIComponent(id)}`).join('&');
        const result = await fetch(
          `${this.PEER_URL}/content/deployments?${query}`,
          undefined,
          ENTITIES_TIMEOUT_MS,
        );
        if (!result.ok) {
          await result.body?.cancel();
          throw new Error(`Failed to read ${group.length} deployers: ${result.status}`);
        }
        const body = (await result.json()) as {
          deployments?: Array<{ entityId?: unknown; deployedBy?: unknown }>;
        };
        return body.deployments ?? [];
      }),
    );

    const deployers: Record<string, string> = {};
    for (const outcome of chunks) {
      if (outcome.status !== 'fulfilled') continue;
      for (const { entityId, deployedBy } of outcome.value) {
        if (typeof entityId !== 'string' || !asked.has(entityId)) continue;
        if (typeof deployedBy !== 'string' || !ADDRESS.test(deployedBy)) continue;
        deployers[entityId] = deployedBy.toLowerCase();
      }
    }
    return deployers;
  }
}

export const rentalFields = () => `
  fragment rentalFields on Rental {
    id
    contractAddress
    tokenId
    lessor
    tenant
    operator
    startedAt
    endsAt
  }
`;

const getRentalsQuery = () => `query Rentals($address: Bytes) {
    tenantRentals: rentals(where: { tenant: $address, isActive: true }) {
      ...rentalFields
    }
    lessorRentals: rentals(where: { lessor: $address, isActive: true }) {
      ...rentalFields
    }
  }
  ${rentalFields()}
`;

export class Rentals {
  private subgraph = config.get('RENTALS_SUBGRAPH');
  private LAND_REGISTRY_ADDRESS = config.get('LAND_REGISTRY_ADDRESS');
  private ESTATE_REGISTRY_ADDRESS = config.get('ESTATE_REGISTRY_ADDRESS');

  fetchRentalTokenIds = async (
    address: string,
  ): Promise<{ lessorRentals: Rental[]; tenantRentals: Rental[] }> => {
    const response: Response = await fetch(this.subgraph, {
      method: 'POST',
      body: JSON.stringify({
        query: getRentalsQuery(),
        variables: {
          address: address.toLowerCase(),
        },
      }),
      headers: {
        Accept: '*/*',
        'Content-Type': 'application/json',
      },
    });

    const {
      data,
    }: {
      data: {
        lessorRentals: RentalFields[];
        tenantRentals: RentalFields[];
      };
    } = await response.json();

    const now = Date.now();
    return {
      lessorRentals: data.lessorRentals.map(this.fromRentalFields),
      tenantRentals: data.tenantRentals
        .map(this.fromRentalFields)
        .filter(rental => rental.endsAt > now),
    };
  };

  getLandType = (contractAddress: string): LandType => {
    switch (contractAddress.toLowerCase()) {
      case this.LAND_REGISTRY_ADDRESS:
        return LandType.PARCEL;
      case this.ESTATE_REGISTRY_ADDRESS:
        return LandType.ESTATE;
      default:
        throw new Error(`Could not derive land type from contract address "${contractAddress}"`);
    }
  };

  fromRentalFields = (fields: RentalFields): Rental => {
    return {
      id: fields.id,
      type: this.getLandType(fields.contractAddress),
      tokenId: fields.tokenId,
      lessor: fields.lessor,
      tenant: fields.tenant,
      operator: fields.operator,
      startedAt: +fields.startedAt * 1000,
      endsAt: +fields.endsAt * 1000,
    };
  };
}
