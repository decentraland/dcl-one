// Node26; actual community handlers/components and friendship-bearing SQL builders.
// Usage: node <script> <pinned-social-root> <output>. All providers are local.
const { sourceCapture, SOCIAL_REVISION, sha256 } = require(
  require("node:path").join(
    require("node:child_process").execFileSync("git", ["-C", __dirname, "rev-parse", "--show-toplevel"], { encoding: "utf8" }).trim(),
    "tooling/social-capture-common.cjs",
  ),
);
const capture = sourceCapture(process.argv[2]);
const fs = require("fs"),
  root = capture.upstream;
const { createCommunitiesDBComponent } = require(
  root + "/src/adapters/communities-db.ts",
);
const { createCommunityComponent } = require(
  root + "/src/logic/community/communities.ts",
);
const { createCommunityMembersComponent } = require(
  root + "/src/logic/community/members.ts",
);
const { createCommunityBansComponent } = require(
  root + "/src/logic/community/bans.ts",
);
const { createCommunityRequestsComponent } = require(
  root + "/src/logic/community/requests.ts",
);
const handlers = require(root + "/src/controllers/handlers/http");
const id = "00000000-0000-4000-8000-000000000001",
  rid = "00000000-0000-4000-8000-000000000002",
  viewer = "0x1111111111111111111111111111111111111111",
  peer = "0x2222222222222222222222222222222222222222";
const logs = {
    getLogger: () => ({ info() {}, warn() {}, debug() {}, error() {} }),
  },
  noop = async () => {};
const routes = [
  ["/v1/communities", "getCommunitiesHandler"],
  ["/v2/communities", "getCommunitiesV2Handler"],
  ["/v1/communities/:id/members", "getCommunityMembersHandler"],
  ["/v2/communities/:id/members", "getCommunityMembersV2Handler"],
  ["/v1/communities/:id/bans", "getBannedMembersHandler"],
  ["/v2/communities/:id/bans", "getBannedMembersV2Handler"],
  ["/v1/communities/:id/requests", "getCommunityRequestsHandler"],
  ["/v2/communities/:id/requests", "getCommunityRequestsV2Handler"],
  ["/v1/members/:address/requests", "getMemberRequestsHandler"],
  ["/v2/members/:address/requests", "getMemberRequestsV2Handler"],
];
(async () => {
  const cases = [];
  for (const phase of [
    "friendship-read-error",
    "accepted-friend",
    "empty-page-error",
    "anonymous-error",
    "missing-profile",
    "friend-and-profile-error",
  ]) {
    for (const [path, handler] of phase === "anonymous-error"
      ? routes.slice(0, 4)
      : phase === "missing-profile"
        ? routes.slice(2, 8)
        : phase === "friend-and-profile-error"
          ? [routes[0], routes[2], routes[4]]
          : routes) {
      const trace = [],
        empty = phase === "empty-page-error",
        anonymous = phase === "anonymous-error";
      const member = {
        communityId: id,
        memberAddress: peer,
        role: "member",
        joinedAt: "2024-01-01T00:00:00.000Z",
      };
      const community = {
        id,
        name: "Fixture",
        description: "Description",
        ownerAddress: viewer,
        privacy: "public",
        active: true,
        visibility: "all",
        role: "owner",
        membersCount: 1,
        friends: [peer],
      };
      const pg = {
        query: async (query) => {
          const text = query.text;
          trace.push({ text, values: query.values });
          const hasFriends = /\bFROM friendships\b/.test(text);
          if (
            hasFriends &&
            [
              "friendship-read-error",
              "empty-page-error",
              "anonymous-error",
              "friend-and-profile-error",
            ].includes(phase)
          )
            throw Error("permission denied for table friendships");
          if (empty) return { rows: [] };
          if (/cb\.banned_address AS "memberAddress"/.test(text))
            return {
              rows: [
                {
                  communityId: id,
                  memberAddress: peer,
                  bannedAt: "2024-01-01T00:00:00.000Z",
                  ...(hasFriends
                    ? { lastFriendshipAction: "accept", actingUser: viewer }
                    : {}),
                },
              ],
            };
          if (/cm\.community_id AS "communityId"/.test(text))
            return {
              rows: [
                {
                  ...member,
                  ...(hasFriends
                    ? { lastFriendshipAction: "accept", actingUser: viewer }
                    : {}),
                },
              ],
            };
          return { rows: [community] };
        },
      };
      const query = pg.query;
      pg.query = async (...args) => {
        const result = await query(...args);
        if (
          phase === "missing-profile" &&
          result.rows[0]?.memberAddress === peer
        ) {
          result.rows.push({
            ...result.rows[0],
            memberAddress: viewer,
            role: "owner",
          });
        }
        return result;
      };
      const built = createCommunitiesDBComponent({ logs, pg });
      const request = {
        id: rid,
        communityId: id,
        memberAddress: peer,
        type: "request_to_join",
        status: "pending",
      };
      const values = {
        getCommunity: async () => community,
        communityExists: async () => true,
        getCommunityMemberRole: async () => "owner",
        getCommunityMembersCount: async () =>
          empty ? 0 : phase === "missing-profile" ? 2 : 1,
        getBannedMembersCount: async () =>
          empty ? 0 : phase === "missing-profile" ? 2 : 1,
        getCommunityRequests: async () =>
          empty
            ? []
            : phase === "missing-profile"
              ? [
                  request,
                  {
                    ...request,
                    id: "00000000-0000-4000-8000-000000000003",
                    memberAddress: viewer,
                  },
                ]
              : [request],
        getCommunityRequestsCount: async () =>
          empty ? 0 : phase === "missing-profile" ? 2 : 1,
        getMemberRequests: async () =>
          empty ? [] : [{ ...request, memberAddress: viewer }],
        getMemberRequestsCount: async () => (empty ? 0 : 1),
        getCommunitiesCount: async () => (empty ? 0 : 1),
        getPublicCommunitiesCount: async () => (empty ? 0 : 1),
      };
      for (const key of [
        "getCommunityMembers",
        "getBannedMembers",
        "getCommunities",
        "getCommunitiesPublicInformation",
      ])
        values[key] = (...args) => built[key](...args);
      const db = new Proxy(values, {
        get: (v, k) => (k === "then" ? undefined : v[k] || noop),
      });
      const registry = {
        getProfiles: async (addresses) => {
          if (phase === "friend-and-profile-error")
            throw Error("permission denied for table registry_profile_cache");
          return addresses
            .filter(
              (address) => phase !== "missing-profile" || address !== peer,
            )
            .map((address) => ({
              avatars: [
                {
                  userId: address,
                  name: "Profile",
                  hasClaimedName: true,
                  avatar: {
                    snapshots: { face256: "http://cdn.test/face.png" },
                  },
                },
              ],
            }));
        },
      };
      const common = {
        logs,
        communitiesDb: db,
        registry,
        communityOwners: {
          getOwnersNames: async () => {
            if (phase === "friend-and-profile-error")
              throw Error("permission denied for table registry_profile_cache");
            return { [viewer]: "Profile" };
          },
        },
        commsGatekeeper: { getCommunitiesVoiceChatStatus: async () => ({}) },
        communityRoles: new Proxy({}, { get: () => noop }),
        communityThumbnail: { getThumbnail: async () => undefined },
      };
      const communities = createCommunityComponent(common),
        communityMembers = await createCommunityMembersComponent(common),
        communityBans = await createCommunityBansComponent(common),
        communityRequests = createCommunityRequestsComponent({
          ...common,
          communities,
        });
      const response = await handlers[handler]({
        components: {
          config: { getString: async () => undefined },
          logs,
          communities,
          communityMembers,
          communityBans,
          communityRequests,
        },
        request: new Request("http://social.test/"),
        params: { id, address: viewer },
        verification: anonymous ? undefined : { auth: viewer },
        url: new URL("http://social.test/?type=request_to_join"),
      });
      const shouldFail =
        !anonymous &&
        [
          "friendship-read-error",
          "empty-page-error",
          "friend-and-profile-error",
        ].includes(phase) &&
        !path.endsWith("/:id/requests") &&
        !(empty && path.includes("/members/:address/requests"));
      if (response.status !== (shouldFail ? 500 : 200))
        throw Error(JSON.stringify({ phase, path, response, trace }));
      if (
        anonymous &&
        trace.some((call) => /\bFROM friendships\b/.test(call.text))
      )
        throw Error("Anonymous path unexpectedly read friendships");
      if (path.endsWith("/:id/requests") && trace.length)
        throw Error(
          "Community request aggregation unexpectedly queried friendships",
        );
      cases.push({ phase, path, handler, response, trace });
    }
  }
  const paths = [
    "src/adapters/communities-db.ts",
    "src/logic/queries.ts",
    "src/logic/community/communities.ts",
    "src/logic/community/members.ts",
    "src/logic/community/bans.ts",
    "src/logic/community/requests.ts",
    "src/logic/community/utils.ts",
  ];
  for (const handler of new Set(cases.map((c) => c.handler))) {
    const matches = fs
      .readdirSync(root + "/src/controllers/handlers/http")
      .filter(
        (file) =>
          file.endsWith(".ts") &&
          fs
            .readFileSync(
              root + "/src/controllers/handlers/http/" + file,
              "utf8",
            )
            .includes("function " + handler + "("),
      );
    if (matches.length !== 1) throw Error("Nonunique handler " + handler);
    paths.push("src/controllers/handlers/http/" + matches[0]);
  }
  fs.writeFileSync(
    process.argv[3],
    JSON.stringify(
      {
        source: "https://github.com/decentraland/social-service-ea",
        revision: SOCIAL_REVISION,
        runtime: process.version,
        scope:
          "Actual community handlers/components and communities-db friendship-bearing SQL builders. A local pg provider rejects generated queries referencing friendships with a permission Error; it does not execute those SQL statements. Other DB/profile/domain dependencies are local fixtures. Anonymous paths omit the relationship CTE; community request aggregation performs no relationship read.",
        sources: paths.map((path) => ({
          path,
          sha256: sha256(root, path),
        })),
        cases,
      },
      null,
      2,
    ) + "\n",
  );
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
