import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

import { describe, it, expect } from "vitest";

import {
  MIRRORED_ROWS_WAIT_FOR_A_PASS,
  buildWhere,
  buildListQuery,
  buildCountQuery,
  buildPlaceQuery,
} from "./db-query";
import { placeFromDbRow } from "./db-row";
import { mapPlace } from "./places/index";

// The node's place projection (place_columns), read from its source so the two cannot drift apart.
const NODE_ROWS = readFileSync(
  fileURLToPath(new URL("../../../../../../services/catalyst/explore/src/places/rows.rs", import.meta.url)),
  "utf8",
);

describe("db places counts match the node's places API", () => {
  it("favorites, likes and dislikes add the node's own interaction counts exactly as rows.rs place_columns does", () => {
    for (const column of ["favorites", "likes", "dislikes"]) {
      const node = NODE_ROWS.match(new RegExp(`^\\s*(${column} \\+ COALESCE\\(.*\\) AS ${column}),$`, "m"))?.[1];
      expect(node, `rows.rs projects ${column}`).toContain("explore.place_interaction_counts");
      // The node reads the explore.place_indexed view; sites reads its explore.place leg, unaliased.
      const sites = node!.replace("place_indexed.id", "place.id");
      for (const query of [buildPlaceQuery("p").text, buildListQuery({}).text]) {
        expect(query).toContain(sites);
        expect(query).toMatch(/ FROM explore\.place WHERE /);
      }
    }
  });

  it("a list waits for the places mirror's first complete pass as the node's does", () => {
    const copies = readFileSync(
      fileURLToPath(new URL("../../../../../../services/catalyst/explore/src/copies.rs", import.meta.url)),
      "utf8",
    );
    const local = copies.match(/macro_rules! local_place \{\s*\(\) => \{\s*"([^"]+)"/)?.[1];
    expect(local, "copies.rs names a place of this node's own").toBeDefined();
    expect(MIRRORED_ROWS_WAIT_FOR_A_PASS).toContain(local!.slice(1, -1));
    for (const query of [buildListQuery({}).text, buildCountQuery({ search: "music" }).text]) {
      expect(query).toContain(MIRRORED_ROWS_WAIT_FOR_A_PASS);
    }
    // The detail read is an object: a mirrored place is served whatever the pass.
    expect(buildPlaceQuery("p").text).not.toContain("mirror_state");
  });

  it("the detail query reads one place by its bound id", () => {
    expect(buildPlaceQuery("3ca25728")).toMatchObject({ values: ["3ca25728"] });
    expect(buildPlaceQuery("3ca25728").text).toMatch(/ FROM explore\.place WHERE id = \$1$/);
  });
});

describe("db places query builders (faithful to catalyrst-places SQL)", () => {
  it("no filters: disabled-only WHERE, like_score order, default paging; limit clamps to 0..100 and offset floors", () => {
    const q = buildListQuery({});
    expect(q.text).toContain("WHERE disabled IS FALSE");
    expect(q.text).toMatch(/ORDER BY.*like_score.*DESC/);
    expect(q.text).toContain("LIMIT 100 OFFSET 0");
    expect(q.values).toEqual([]);

    expect(buildListQuery({ limit: 999 }).text).toContain("LIMIT 100");
    expect(buildListQuery({ limit: -5 }).text).toContain("LIMIT 0");
    expect(buildListQuery({ offset: 40 }).text).toContain("OFFSET 40");
  });

  it("categories bind as an array overlap ($1) and the count query mirrors the WHERE without paging/order", () => {
    expect(buildWhere({ categories: "music" }).clause).toContain(
      "categories && $1",
    );
    expect(buildWhere({ categories: "music" }).values).toEqual([["music"]]);
    expect(buildWhere({ categories: ["music", "art"] }).values).toEqual([
      ["music", "art"],
    ]);

    const q = buildCountQuery({ categories: "art" });
    expect(q.text).toMatch(/^SELECT count\(\*\)/);
    expect(q.text).toContain("categories && $1");
    expect(q.text).not.toMatch(/LIMIT|ORDER BY/);
    expect(q.values).toEqual([["art"]]);
  });

  it("search >=3 chars -> FTS+ILIKE clause + rank prefix; term bound 3x", () => {
    const q = buildListQuery({ search: "plaza" });
    expect(q.text).toContain("plainto_tsquery('english', $1)");
    expect(q.text).toContain("title ILIKE $2 OR description ILIKE $2");
    expect(q.text).toContain("ts_rank_cd(");
    expect(q.values).toEqual(["plaza", "%plaza%", "plaza"]);
  });
});

describe("placeFromDbRow + mapPlace on a Postgres-style row", () => {
  it("normalizes a row (nulls + extra cols) into a card; null user_count -> players 0 / not live, null title -> empty", () => {
    const row = {
      id: "abc",
      title: "Genesis Plaza",
      description: null,
      image: "https://x/i.png",
      owner: null,
      creator_address: "0x1",
      positions: ["0,0"],
      base_position: "0,0",
      contact_name: "DAO",
      content_rating: "T",
      disabled: false,
      deployed_at: new Date(),
      favorites: 10,
      likes: 5,
      dislikes: 1,
      categories: ["social"],
      tags: [],
      highlighted: true,
      highlighted_image: null,
      world: false,
      world_name: null,
      user_count: 3,
      user_visits: 100,
      like_rate: 0.834,
      like_score: 0.7,
    };
    const place = placeFromDbRow(row);
    expect(place.user_count).toBe(3);
    expect("content_rating" in place).toBe(false);
    expect("like_score" in place).toBe(false);
    expect(mapPlace(place)).toMatchObject({
      title: "Genesis Plaza",
      players: 3,
      live: true,
      rating: 83,
      coords: "0,0",
      featured: true,
      creator: "DAO",
    });

    const card = mapPlace(
      placeFromDbRow({
        id: "x",
        base_position: "1,1",
        user_count: null,
        like_rate: null,
      }),
    );
    expect(card.players).toBe(0);
    expect(card.live).toBe(false);
    expect(card.title).toBe("");
    expect(card.rating).toBe(0);
  });
});
