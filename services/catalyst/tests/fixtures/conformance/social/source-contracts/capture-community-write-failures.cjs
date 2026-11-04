// Node 26; actual source components, HTTP handlers and SQL adapter against a
// disposable schema. Usage: node <this> <source-root> <output> <postgres-url>.
const fs = require("fs"),
  crypto = require("crypto"),
  root = process.argv[2];
require(root + "/node_modules/ts-node/register/transpile-only");
const { Client } = require(root + "/node_modules/pg");
const { createCommunitiesDBComponent } = require(
  root + "/src/adapters/communities-db.ts",
);
const { createCommunityBansComponent } = require(
  root + "/src/logic/community/bans.ts",
);
const { createCommunityComponent } = require(
  root + "/src/logic/community/communities.ts",
);
const { createCommunityPlacesComponent } = require(
  root + "/src/logic/community/places.ts",
);
const { banMemberHandler, updateCommunityHandler } = require(
  root + "/src/controllers/handlers/http",
);
const id = "aabbccdd-0000-4000-8000-000000000001",
  owner = "0x1111111111111111111111111111111111111111",
  member = "0x2222222222222222222222222222222222222222",
  post = "00000000-0000-4000-8000-000000000002",
  request = "00000000-0000-4000-8000-000000000003",
  oldPlace = "00000000-0000-4000-8000-000000000004",
  newPlace = "00000000-0000-4000-8000-000000000005";
const logs = {
    getLogger: () => ({ info() {}, warn() {}, error() {}, debug() {} }),
  },
  noop = async () => {};
(async () => {
  const client = new Client({ connectionString: process.argv[4] });
  await client.connect();
  const schema = "community_write_" + crypto.randomBytes(12).toString("hex");
  const cases = [];
  try {
    await client.query(`CREATE SCHEMA ${schema}`);
    await client.query(`SET search_path TO ${schema}`);
    await client.query(
      `CREATE TABLE communities(id uuid PRIMARY KEY,name text,description text,owner_address text,private boolean,unlisted boolean,active boolean,created_at timestamptz DEFAULT now(),updated_at timestamptz DEFAULT now()); CREATE TABLE community_members(community_id uuid,member_address text,role text,PRIMARY KEY(community_id,member_address)); CREATE TABLE community_posts(id uuid PRIMARY KEY,community_id uuid); CREATE TABLE community_post_likes(post_id uuid,user_address text); CREATE TABLE community_bans(community_id uuid,banned_address text,banned_by text,active boolean,PRIMARY KEY(community_id,banned_address)); CREATE TABLE community_requests(id uuid PRIMARY KEY,community_id uuid,member_address text,type text,status text); CREATE TABLE community_places(id text,community_id uuid,added_by text,added_at timestamptz DEFAULT now(),PRIMARY KEY(id,community_id));`,
    );
    const query = (q, values) =>
      client.query(typeof q === "string" ? q : q.text, values || q.values);
    const pg = {
      query,
      exists: async (q, key) => (await query(q)).rows[0][key],
      withTransaction: async (work) => {
        await client.query("BEGIN");
        try {
          const result = await work({ query });
          await client.query("COMMIT");
          return result;
        } catch (e) {
          await client.query("ROLLBACK");
          throw e;
        }
      },
    };
    const db = createCommunitiesDBComponent({ pg, logs });
    for (const name of [
      "ban-kick-failure",
      "ban-unlike-failure",
      "ban-insert-failure",
      "ban-requests-failure",
      "ban-success",
      "update-place-insert-failure",
      "update-accept-insert-failure",
      "update-accept-delete-failure",
      "update-success",
    ]) {
      await client.query(
        "TRUNCATE communities,community_members,community_posts,community_post_likes,community_bans,community_requests,community_places",
      );
      await client.query(
        "INSERT INTO communities(id,name,description,owner_address,private,unlisted,active) VALUES($1,$2,$3,$4,true,false,true)",
        [id, "Fixture community", "Description", owner],
      );
      await client.query(
        "INSERT INTO community_members VALUES($1,$2,'owner')",
        [id, owner],
      );
      if (name.startsWith("ban-"))
        await client.query(
          "INSERT INTO community_members VALUES($1,$2,'member')",
          [id, member],
        );
      await client.query("INSERT INTO community_posts VALUES($1,$2)", [
        post,
        id,
      ]);
      await client.query("INSERT INTO community_post_likes VALUES($1,$2)", [
        post,
        member,
      ]);
      await client.query(
        "INSERT INTO community_requests VALUES($1,$2,$3,'request_to_join','pending')",
        [request, id, member],
      );
      await client.query(
        "INSERT INTO community_places(id,community_id,added_by) VALUES($1,$2,$3)",
        [oldPlace, id, owner],
      );
      if (name === "update-place-insert-failure")
        await client.query(
          "INSERT INTO community_places(id,community_id,added_by) VALUES($1,$2,$3)",
          [newPlace, id, owner],
        );
      const faults = {
        "ban-kick-failure": ["community_members", "DELETE"],
        "ban-unlike-failure": ["community_post_likes", "DELETE"],
        "ban-insert-failure": ["community_bans", "INSERT"],
        "ban-requests-failure": ["community_requests", "DELETE"],
        "update-place-insert-failure": ["community_places", "INSERT"],
        "update-accept-insert-failure": ["community_members", "INSERT"],
        "update-accept-delete-failure": ["community_requests", "DELETE"],
      };
      const fault = faults[name];
      if (fault) {
        await client.query(
          `CREATE FUNCTION reject_write() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'fixture ${name}'; END $$`,
        );
        await client.query(
          `CREATE TRIGGER reject_write BEFORE ${fault[1]} ON ${fault[0]} FOR EACH ROW EXECUTE FUNCTION reject_write()`,
        );
      }
      const events = [],
        trace = [];
      const common = {
        logs,
        communitiesDb: db,
        communityRoles: new Proxy({}, { get: () => noop }),
        analytics: {
          fireEvent: (event, properties) => {
            trace.push("analytics");
            events.push({ event, properties });
          },
        },
        communityThumbnail: {
          buildThumbnailUrl: () => "/raw.png",
          getThumbnail: async () => undefined,
        },
        communityBroadcaster: {
          broadcast: async () => {
            trace.push("sns");
          },
        },
        commsGatekeeper: {
          kickUserFromCommunityVoiceChat: async () => {
            trace.push("voice");
          },
        },
        pubsub: {
          publishInChannel: async () => {
            trace.push("pubsub");
          },
        },
        communityComplianceValidator: { validateCommunityContent: noop },
      };
      const places = await createCommunityPlacesComponent({
        ...common,
        placesApi: { getDestinations: async () => [{ id: newPlace, owner }] },
      });
      const bans = await createCommunityBansComponent(common);
      const communities = createCommunityComponent({
        ...common,
        communityPlaces: places,
      });
      const components = {
        logs,
        communityBans: bans,
        communities,
        communityFieldsValidator: {
          validate: async () => ({
            privacy: "public",
            ...(name === "update-place-insert-failure"
              ? { placeIds: [newPlace] }
              : {}),
          }),
        },
      };
      const context = {
        components,
        verification: { auth: owner },
        params: { id, memberAddress: member },
        formData: { fields: { privacy: "public" }, files: {} },
      };
      const response = await (name.startsWith("ban-")
        ? banMemberHandler(context)
        : updateCommunityHandler(context));
      await new Promise((resolve) => setImmediate(resolve));
      const state = (
        await client.query(
          `SELECT (SELECT private FROM communities WHERE id=$1) AS private,(SELECT count(*)::int FROM community_members WHERE community_id=$1 AND member_address=$2) AS members,(SELECT count(*)::int FROM community_post_likes WHERE user_address=$2) AS likes,(SELECT count(*)::int FROM community_bans WHERE community_id=$1 AND active) AS bans,(SELECT count(*)::int FROM community_requests WHERE community_id=$1) AS requests,(SELECT array_agg(id ORDER BY id) FROM community_places WHERE community_id=$1) AS places`,
          [id, member],
        )
      ).rows[0];
      cases.push({ name, response, state, events, trace });
      if (fault) {
        await client.query(`DROP TRIGGER reject_write ON ${fault[0]}`);
        await client.query("DROP FUNCTION reject_write()");
      }
    }
  } finally {
    await client.query("SET search_path TO public");
    await client.query(`DROP SCHEMA IF EXISTS ${schema} CASCADE`);
    await client.end();
  }
  const paths = [
    "src/adapters/communities-db.ts",
    "src/logic/community/bans.ts",
    "src/logic/community/communities.ts",
    "src/logic/community/places.ts",
    "src/controllers/handlers/http/ban-member-handler.ts",
    "src/controllers/handlers/http/update-community-handler.ts",
  ];
  const fixture = {
    provenance: {
      repository: "https://github.com/decentraland/social-service-ea",
      revision: "b6b2cefefdf727b2ed47ddb887e77d226a41eea6",
      runtime: process.version,
      sources: paths.map((path) => ({
        path,
        sha256: crypto
          .createHash("sha256")
          .update(fs.readFileSync(root + "/" + path))
          .digest("hex"),
      })),
      scope:
        "Actual source HTTP handlers, community components and SQL adapter with real PostgreSQL triggers. Authority, thumbnail, analytics and external notification providers are local test dependencies; no source authentication middleware or production crash/concurrency test.",
    },
    id,
    owner,
    member,
    oldPlace,
    newPlace,
    cases,
  };
  fs.writeFileSync(process.argv[3], JSON.stringify(fixture, null, 2) + "\n");
})().catch((error) => {
  process.stderr.write(String(error.stack) + "\n");
  process.exitCode = 1;
});
