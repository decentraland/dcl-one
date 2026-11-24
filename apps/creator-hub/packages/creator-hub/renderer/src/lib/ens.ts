import { config } from '/@/config';
import { fetch } from '/shared/fetch';
import { createLimit } from '/shared/limit';

export type Domain = { name: string };
export type DomainsQueryResult = { data: { domains: Domain[] } } | { errors: any };

export type OwnerByENSTuple = {
  name: string;
  wrappedOwner: {
    id: string;
  };
};
export type OwnerByENSQueryResult =
  | {
      data: {
        domains: OwnerByENSTuple[];
      };
    }
  | { errors: any };

export class ENS {
  private subgraph = config.get('ENS_SUBGRAPH');
  private subgraphTimeout = 10000;

  public async fetchNames(address: string) {
    const response: Response = await fetch(
      this.subgraph,
      {
        method: 'POST',
        body: JSON.stringify({
          query: `{
          domains(
            where: {or: [
              { wrappedOwner: "${address.toLowerCase()}" },
              { registrant: "${address.toLowerCase()}" }
            ]}
          ) {
            name
          }
        }`,
        }),
      },
      this.subgraphTimeout,
    );

    if (!response.ok) {
      throw new Error(response.status.toString());
    }

    const queryResult: DomainsQueryResult = await response.json();

    if ('errors' in queryResult) {
      throw new Error(JSON.stringify(queryResult.errors));
    }

    return queryResult.data.domains.map(domain => domain.name);
  }

  public async fetchNamesOwners(domains: string[]): Promise<Record<string, string>> {
    if (!domains) {
      return {};
    }

    const response: Response = await fetch(
      this.subgraph,
      {
        method: 'POST',
        body: JSON.stringify({
          query: `query getOwners($domains: [String]) {
          domains(where: { name_in: $domains }) {
            name
            wrappedOwner {
              id
            }
          }
        }`,
          variables: { domains },
        }),
      },
      this.subgraphTimeout,
    );

    if (!response.ok) {
      throw new Error(response.status.toString());
    }

    const queryResult: OwnerByENSQueryResult = await response.json();

    if ('errors' in queryResult) {
      throw new Error(JSON.stringify(queryResult.errors));
    }

    const results: Record<string, string> = {};
    queryResult.data.domains.forEach(({ wrappedOwner, name }) => {
      if (wrappedOwner && wrappedOwner.id) {
        results[name] = wrappedOwner.id;
      }
    });
    return results;
  }
}

/** A page of the content server's `/lambdas/users/{address}/names` (lamb2's shape). */
type DCLNamesPage = {
  elements: { name: string }[];
  totalAmount: number;
};

/** lamb2's largest `pageSize` for `/lambdas/users/{address}/names`. */
const NAMES_PAGE_SIZE = 1000;

/** How many `/lambdas/names/{name}/owner` lookups run at once. */
const OWNER_LOOKUPS = createLimit(8);

/**
 * DCL NAMEs from the content server's lambdas (`PEER_URL`), the node's own NAME source: on a
 * dcl-one node its nft index, on another catalyst lamb2's reader.
 */
export class DCLNames {
  private lambdas = `${config.get('PEER_URL')}/lambdas`;
  private timeout = 10000;

  /** Every DCL NAME `address` holds, as `<name>.dcl.eth`, walked page by page. */
  public async fetchNames(address: string) {
    const owner = encodeURIComponent(address.toLowerCase());
    const results: string[] = [];
    for (let pageNum = 1; ; pageNum++) {
      const response: Response = await fetch(
        `${this.lambdas}/users/${owner}/names?pageSize=${NAMES_PAGE_SIZE}&pageNum=${pageNum}`,
        undefined,
        this.timeout,
      );
      if (!response.ok) {
        await discard(response);
        throw new Error(response.status.toString());
      }
      const page: DCLNamesPage = await response.json();
      results.push(...page.elements.map(({ name }) => `${name}.dcl.eth`));
      if (page.elements.length < NAMES_PAGE_SIZE || results.length >= page.totalAmount) {
        return results;
      }
    }
  }

  /**
   * The holder of each NAME in `names` (bare labels, no `.dcl.eth`), keyed by the lowercased
   * label; a NAME nobody holds is left out. One request per distinct NAME (the lambdas have no
   * batch owner route; a contributable list is a handful), at most 8 at once. Any failed lookup
   * fails the whole read, and the lookups still queued behind it are not sent.
   */
  public async fetchNamesOwners(names: string[]): Promise<Record<string, string>> {
    const results: Record<string, string> = {};
    const labels = [...new Set(names.map(name => name.toLowerCase()))];
    let failed = false;
    await Promise.all(
      labels.map(label =>
        OWNER_LOOKUPS(async () => {
          if (failed) {
            return;
          }
          try {
            const response: Response = await fetch(
              `${this.lambdas}/names/${encodeURIComponent(label)}/owner`,
              undefined,
              this.timeout,
            );
            if (response.status === 404) {
              await discard(response);
              return;
            }
            if (!response.ok) {
              await discard(response);
              throw new Error(response.status.toString());
            }
            const { owner }: { owner?: string } = await response.json();
            if (owner) {
              results[label] = owner;
            }
          } catch (error) {
            failed = true;
            throw error;
          }
        }),
      ),
    );
    return results;
  }
}

/** Releases a response whose body is not read, so its connection is not held until collected. */
async function discard(response: Response) {
  await response.body?.cancel().catch(() => undefined);
}
