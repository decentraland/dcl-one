import { describe, expect, it } from "vitest";

import { loader as assignEns } from "./builder.land-assign-ens";
import { loader as operator } from "./builder.land-operator";
import { loader as transfer } from "./builder.land-transfer";
import { loader as marketplace } from "./marketplace.$";

const LAND = "0xf87e31492faf9a91b02ee0deaad50d51d56d5d4d";
const ESTATE = "0x959e104e1a4db6317fa58f8295f586e1a978c297";

async function target(load: (args: never) => Response | Promise<Response>, url: string): Promise<[number, string | null]> {
  const res = await load({ request: new Request(`https://sites.test${url}`), params: {}, context: {} } as never);
  return [res.status, res.headers.get("Location")];
}

describe("the old LAND URLs land on the Shop pages that do the job", () => {
  it("/marketplace/land -> /shop/lands with the query", async () => {
    expect(await target(marketplace as never, "/marketplace/land?category=estate")).toEqual([308, "/shop/lands?section=estates"]);
  });

  it("/marketplace/rent-land -> the rent page of the LAND it names, else the LAND for rent", async () => {
    expect(await target(marketplace as never, `/marketplace/rent-land?id=${LAND}-42`)).toEqual([308, `/shop/token/${LAND}/42/rent`]);
    expect(await target(marketplace as never, "/marketplace/rent-land")).toEqual([308, "/shop/lands?onlyOnRent=true"]);
  });

  it("/builder/land-{transfer,operator,assign-ens} -> the LAND's transfer or manage page", async () => {
    expect(await target(transfer as never, `/builder/land-transfer?id=${ESTATE}-7`)).toEqual([308, `/shop/token/${ESTATE}/7/transfer`]);
    expect(await target(operator as never, "/builder/land-operator?id=4213")).toEqual([308, `/shop/token/${ESTATE}/4213/manage`]);
    expect(await target(assignEns as never, "/builder/land-assign-ens")).toEqual([308, "/create/land"]);
  });

  it("/marketplace/names/claim (shipped mobile builds) -> /shop/names, the IAP view marker kept", async () => {
    expect(await target(marketplace as never, "/marketplace/names/claim?view=mobile-iap")).toEqual([308, "/shop/names?view=mobile-iap"]);
  });
});
