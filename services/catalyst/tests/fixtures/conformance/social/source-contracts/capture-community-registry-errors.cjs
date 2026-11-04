// Node26; actual shared registry adapter, community wrappers and HTTP handlers.
// Usage: node <this> <pinned-source-root> <output>. All providers are local.
const { sourceCapture, SOCIAL_REVISION, sha256 } = require(
  require("node:path").join(
    require("node:child_process").execFileSync("git", ["-C", __dirname, "rev-parse", "--show-toplevel"], { encoding: "utf8" }).trim(),
    "tooling/social-capture-common.cjs",
  ),
);
const capture = sourceCapture(process.argv[2]);
const fs = require("fs"),
  root = capture.upstream;
const { createRegistryComponent } = require(root + "/src/adapters/registry.ts");
const { createCommunityOwnersComponent } = require(
  root + "/src/logic/community/owners.ts",
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
const { createCommunityPostsComponent } = require(
  root + "/src/logic/community/posts.ts",
);
const handlers = require(root + "/src/controllers/handlers/http");
const owner = "0x1111111111111111111111111111111111111111",
  id = "00000000-0000-4000-8000-000000000001",
  rid = "00000000-0000-4000-8000-000000000002",
  logs = {
    getLogger: () => ({ info() {}, warn() {}, error() {}, debug() {} }),
  },
  noop = async () => {};
(async () => {
  const cases = [];
  for (const phase of ["read-error", "write-error"])
    for (const [path, handler] of [
      ["/v1/communities", "getCommunitiesHandler"],
      ["/v1/communities/:id/members", "getCommunityMembersHandler"],
      ["/v1/communities/:id/bans", "getBannedMembersHandler"],
      ["/v1/communities/:id/requests", "getCommunityRequestsHandler"],
      ["/v1/members/:address/requests", "getMemberRequestsHandler"],
      ["/v1/communities/:id/posts", "getCommunityPostsHandler"],
      ...(phase === "read-error"
        ? [
            ["/v2/communities", "getCommunitiesV2Handler"],
            ["/v2/communities/:id/members", "getCommunityMembersV2Handler"],
            ["/v2/communities/:id/bans", "getBannedMembersV2Handler"],
            ["/v2/communities/:id/requests", "getCommunityRequestsV2Handler"],
            ["/v2/members/:address/requests", "getMemberRequestsV2Handler"],
            ["/v2/communities/:id/posts", "getCommunityPostsV2Handler"],
          ]
        : []),
    ]) {
      const trace = [];
      const registry = await createRegistryComponent({
        logs,
        config: { requireString: async () => "http://registry.test" },
        redis: {
          mGet: async (keys) => {
            trace.push({ phase: "cache-read", keys });
            if (phase === "read-error")
              throw Error("permission denied for table registry_profile_cache");
            return keys.map(() => null);
          },
          put: async () => {
            trace.push({ phase: "cache-write" });
            throw Error("fixture cache write failure");
          },
        },
        fetcher: {
          fetch: async () => {
            trace.push({ phase: "registry-read" });
            return new Response(
              JSON.stringify([
                {
                  avatars: [
                    {
                      userId: owner,
                      name: "Owner",
                      hasClaimedName: true,
                      avatar: {
                        snapshots: { face256: "http://cdn.test/face.png" },
                      },
                    },
                  ],
                },
              ]),
              { status: 200 },
            );
          },
        },
      });
      const community = {
        id,
        name: "Fixture",
        description: "Description",
        ownerAddress: owner,
        privacy: "public",
        visibility: "all",
        active: true,
        role: "owner",
        membersCount: 1,
        friends: [],
      };
      const member = {
        memberAddress: owner,
        role: "owner",
        friendshipStatus: "none",
      };
      const request = {
        id: rid,
        communityId: id,
        memberAddress: owner,
        type: "request_to_join",
        status: "pending",
      };
      const values = {
        getCommunity: async () => community,
        communityExists: async () => true,
        getCommunityMemberRole: async () => "owner",
        getCommunityMembers: async () => [member],
        getCommunityMembersCount: async () => 1,
        getBannedMembers: async () => [member],
        getBannedMembersCount: async () => 1,
        getCommunityRequests: async () => [request],
        getCommunityRequestsCount: async () => 1,
        getMemberRequests: async () => [request],
        getMemberRequestsCount: async () => 1,
        getCommunities: async () => [community],
        getCommunitiesCount: async () => 1,
        getPosts: async () => [
          {
            id: rid,
            communityId: id,
            authorAddress: owner,
            content: "Post",
            createdAt: 1,
            likesCount: 0,
            likedByUser: false,
          },
        ],
        getPostsCount: async () => 1,
      };
      const db = new Proxy(values, {
        get: (v, k) => (k === "then" ? undefined : v[k] || noop),
      });
      const common = {
        logs,
        registry,
        communitiesDb: db,
        communityRoles: new Proxy({}, { get: () => noop }),
        communityThumbnail: { getThumbnail: async () => undefined },
        communityEvents: { isCurrentlyHostingEvents: async () => false },
        commsGatekeeper: { getCommunitiesVoiceChatStatus: async () => ({}) },
        communityOwners: createCommunityOwnersComponent({ registry }),
      };
      const communities = createCommunityComponent(common),
        communityMembers = await createCommunityMembersComponent(common),
        communityBans = await createCommunityBansComponent(common),
        communityRequests = createCommunityRequestsComponent({
          ...common,
          communities,
        }),
        communityPosts = createCommunityPostsComponent(common);
      const response = await handlers[handler]({
        components: {
          logs,
          config: { getString: async () => undefined },
          communities,
          communityMembers,
          communityBans,
          communityRequests,
          communityPosts,
        },
        request: new Request("http://social.test/"),
        verification: { auth: owner },
        params: { id, address: owner },
        url: new URL("http://social.test/?type=request_to_join"),
      });
      if (
        response.status !==
        (phase === "read-error" && path.startsWith("/v1/") ? 500 : 200)
      )
        throw Error(JSON.stringify({ phase, path, response, trace }));
      if (path.startsWith("/v2/") && trace.length)
        throw Error("V2 unexpectedly reads profiles");
      if (
        phase === "write-error" &&
        !trace.some((call) => call.phase === "cache-write")
      )
        throw Error("Cache put failure was not exercised");
      cases.push({ phase, path, handler, response, trace });
    }
  const paths = [
    "src/utils/fetch.ts",
    "src/utils/tracing.ts",
    "src/adapters/registry.ts",
    "src/logic/community/owners.ts",
    "src/logic/community/communities.ts",
    "src/logic/community/members.ts",
    "src/logic/community/bans.ts",
    "src/logic/community/requests.ts",
    "src/logic/community/posts.ts",
  ];
  for (const handler of new Set(cases.map((value) => value.handler))) {
    const candidates = fs
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
    if (candidates.length !== 1)
      throw Error("Handler source not unique: " + handler);
    paths.push("src/controllers/handlers/http/" + candidates[0]);
  }
  fs.writeFileSync(
    process.argv[3],
    JSON.stringify(
      {
        source: "https://github.com/decentraland/social-service-ea",
        revision: SOCIAL_REVISION,
        runtime: process.version,
        scope:
          "Actual shared registry adapter, community wrappers and HTTP handlers; local cache/fetch/domain dependencies. Read failures propagate; cache put failures are swallowed. The mGet Error message is chosen to match the target SQL permission fault; the source cache provider is Redis, not SQL.",
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
})().catch((e) => {
  console.error(e);
  process.exitCode = 1;
});
