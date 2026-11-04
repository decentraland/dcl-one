// Node26. Actual Social handlers/components/adapter and pinned Gatekeeper voice logic.
// Usage: node <script> <social-root> <gatekeeper-root> <output>; all dependencies local.
const fs = require("fs"),
  crypto = require("crypto"),
  root = process.argv[2],
  gatekeeper = process.argv[3];
require(root + "/node_modules/ts-node/register/transpile-only");
process.env.NODE_PATH = root + "/node_modules";
const Module = require("module");
Module._initPaths();
const originalLoad = Module._load;
Module._load = function (name, ...args) {
  if (name === "@livekit/protocol") return { DisconnectReason: {} };
  return originalLoad.call(this, name, ...args);
};
const { createVoiceComponent } = require(
  gatekeeper + "/src/logic/voice/voice.ts",
);
const { getCommunityVoiceChatStatusHandler } = require(
  gatekeeper +
    "/src/controllers/handlers/community-voice-chat/get-community-voice-chat-status-handler.ts",
);
const { getBulkCommunityVoiceChatStatusHandler } = require(
  gatekeeper +
    "/src/controllers/handlers/community-voice-chat/get-bulk-community-voice-chat-status-handler.ts",
);
const { getAllActiveCommunityVoiceChatsHandler } = require(
  gatekeeper +
    "/src/controllers/handlers/get-all-active-community-voice-chats-handler.ts",
);
const { createCommsGatekeeperComponent } = require(
  root + "/src/adapters/comms-gatekeeper.ts",
);
const { createCommunityComponent } = require(
  root + "/src/logic/community/communities.ts",
);
const { createCommunityRequestsComponent } = require(
  root + "/src/logic/community/requests.ts",
);
const { createCommunityVoiceComponent } = require(
  root + "/src/logic/community-voice/community-voice.ts",
);
const handlers = require(root + "/src/controllers/handlers/http");
const id = "00000000-0000-4000-8000-000000000001",
  rid = "00000000-0000-4000-8000-000000000002",
  address = "0x1111111111111111111111111111111111111111";
const logs = {
  getLogger: () => ({ info() {}, debug() {}, warn() {}, error() {} }),
};
const routes = [
  ["/v1/communities/:id", "getCommunityHandler"],
  ["/v2/communities/:id", "getCommunityV2Handler"],
  ["/v1/communities", "getCommunitiesHandler"],
  ["/v2/communities", "getCommunitiesV2Handler"],
  ["/v1/members/:address/requests", "getMemberRequestsHandler"],
  ["/v2/members/:address/requests", "getMemberRequestsV2Handler"],
  ["/v1/community-voice-chats/active", "getActiveCommunityVoiceChatsHandler"],
  ["/v1/communities?onlyWithActiveVoiceChat=true", "getCommunitiesHandler"],
  ["/v2/communities?onlyWithActiveVoiceChat=true", "getCommunitiesV2Handler"],
];
(async () => {
  const cases = [];
  for (const phase of [
    "companion-sql-error",
    "provider-http-error",
    "community-sql-error",
    "companion-single-count-error",
  ]) {
    for (const [path, handler] of phase === "companion-single-count-error"
      ? routes.slice(0, 2)
      : routes) {
      const trace = [];
      const fault = async (name) => {
        trace.push(name);
        throw Error("fixture voice SQL failure");
      };
      const voice = createVoiceComponent({
        logs,
        livekit: {
          getCommunityVoiceChatRoomName: (id) => "voice-chat-community-" + id,
        },
        voiceDB: {
          isCommunityRoomActive: async () =>
            ["companion-single-count-error", "community-sql-error"].includes(
              phase,
            )
              ? true
              : fault("isCommunityRoomActive"),
          getCommunityUsersInRoom: async () =>
            phase === "community-sql-error"
              ? [{ isModerator: true }]
              : fault("getCommunityUsersInRoom"),
          isActiveCommunityUser: () => true,
          getBulkCommunityVoiceChatStatus: async () =>
            phase === "community-sql-error"
              ? [
                  {
                    communityId: id,
                    active: true,
                    participantCount: 1,
                    moderatorCount: 1,
                  },
                ]
              : fault("getBulkCommunityVoiceChatStatus"),
          getAllActiveCommunityVoiceChats: async () =>
            phase === "community-sql-error"
              ? [{ communityId: id, participantCount: 1, moderatorCount: 1 }]
              : fault("getAllActiveCommunityVoiceChats"),
        },
      });
      const adapter = await createCommsGatekeeperComponent({
        logs,
        config: {
          requireString: async (key) =>
            key === "COMMS_GATEKEEPER_URL"
              ? "http://gatekeeper.test"
              : "fixture-token",
        },
        fetcher: {
          fetch: async (url, options) => {
            trace.push(new URL(url).pathname);
            if (phase === "provider-http-error")
              return new Response("{}", { status: 500 });
            let result;
            if (new URL(url).pathname.endsWith("/active"))
              result = await getAllActiveCommunityVoiceChatsHandler({
                components: { logs, voice },
              });
            else if (options.method === "POST")
              result = await getBulkCommunityVoiceChatStatusHandler({
                components: { logs, voice },
                request: new Request(url, {
                  method: "POST",
                  body: options.body,
                }),
              });
            else
              result = await getCommunityVoiceChatStatusHandler({
                components: { logs, voice },
                params: { communityId: id },
              });
            return new Response(JSON.stringify(result.body), {
              status: result.status,
            });
          },
        },
      });
      const community = {
        id,
        name: "Fixture",
        description: "Description",
        ownerAddress: address,
        privacy: "public",
        visibility: "all",
        active: true,
        role: "owner",
        membersCount: 1,
        friends: [],
      };
      const request = {
        id: rid,
        communityId: id,
        memberAddress: address,
        type: "request_to_join",
        status: "pending",
      };
      const core = async (name, value) => {
        trace.push(name);
        if (phase === "community-sql-error")
          throw Error("permission denied for table communities");
        return value;
      };
      const db = {
        getCommunity: () => core("getCommunity", community),
        getCommunityPublicInformation: () =>
          core("getCommunityPublicInformation", community),
        getCommunityMembersCount: () => core("getCommunityMembersCount", 1),
        getCommunities: () => core("getCommunities", [community]),
        getCommunitiesCount: () => core("getCommunitiesCount", 1),
        getMemberRequests: () => core("getMemberRequests", [request]),
        getMemberRequestsCount: () => core("getMemberRequestsCount", 1),
      };
      const common = {
        logs,
        communitiesDb: db,
        commsGatekeeper: adapter,
        registry: { getProfiles: async () => [] },
        communityOwners: {
          getOwnerName: async () => "Owner",
          getOwnersNames: async () => ({ [address]: "Owner" }),
        },
        communityEvents: { isCurrentlyHostingEvents: async () => false },
        communityThumbnail: { getThumbnail: async () => undefined },
        communityPlaces: {
          getPlacesWithPositionsAndWorlds: async () => ({
            positions: [],
            worlds: [],
          }),
        },
      };
      const communities = createCommunityComponent(common),
        communityRequests = createCommunityRequestsComponent({
          ...common,
          communities,
        }),
        communityVoice = await createCommunityVoiceComponent(common);
      const response = await handlers[handler]({
        components: { logs, communities, communityRequests, communityVoice },
        params: { id, address },
        verification: { auth: address },
        url: new URL(
          "http://social.test" +
            path.replace(":id", id).replace(":address", address),
        ),
      });
      const expected =
        phase === "community-sql-error"
          ? 500
          : phase === "provider-http-error" &&
              (path.includes("/:id") || path.includes("voice-chats/active"))
            ? 500
            : 200;
      if (response.status !== expected)
        throw Error(JSON.stringify({ phase, path, response, trace }));
      if (
        phase.startsWith("companion-") &&
        !trace.some(
          (x) =>
            x.startsWith("getCommunityUsersInRoom") ||
            x.startsWith("isCommunityRoomActive") ||
            x.startsWith("getBulkCommunityVoiceChatStatus") ||
            x.startsWith("getAllActiveCommunityVoiceChats"),
        )
      )
        throw Error("No SQL fault exercised");
      cases.push({ phase, path, handler, response, trace });
    }
  }
  const socialPaths = [
    "src/utils/fetch.ts",
    "src/adapters/comms-gatekeeper.ts",
    "src/logic/community/communities.ts",
    "src/logic/community/requests.ts",
    "src/logic/community/utils.ts",
    "src/logic/community-voice/community-voice.ts",
  ];
  for (const handler of new Set(cases.map((c) => c.handler))) {
    const names = fs
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
    if (names.length !== 1) throw Error("Nonunique handler " + handler);
    socialPaths.push("src/controllers/handlers/http/" + names[0]);
  }
  const gatekeeperPaths = [
    "src/logic/voice/voice.ts",
    "src/controllers/handlers/community-voice-chat/get-community-voice-chat-status-handler.ts",
    "src/controllers/handlers/community-voice-chat/get-bulk-community-voice-chat-status-handler.ts",
    "src/controllers/handlers/get-all-active-community-voice-chats-handler.ts",
  ];
  const hash = (base, paths) =>
    paths.map((path) => ({
      path,
      sha256: crypto
        .createHash("sha256")
        .update(fs.readFileSync(base + "/" + path))
        .digest("hex"),
    }));
  fs.writeFileSync(
    process.argv[4],
    JSON.stringify(
      {
        source: {
          repository: "https://github.com/decentraland/social-service-ea",
          revision: "b6b2cefefdf727b2ed47ddb887e77d226a41eea6",
          sources: hash(root, socialPaths),
        },
        companion: {
          repository: "https://github.com/decentraland/comms-gatekeeper",
          revision: "be6a4b97a2189157884503485892ff46d0ac68da",
          sources: hash(gatekeeper, gatekeeperPaths),
        },
        runtime: process.version,
        scope:
          "Actual handlers, Social community components and HTTP adapter, and companion voice logic with local SQL/fetch/domain providers. SQL failures are injected local Error instances, not execution of database adapters. Companion SQL errors become idle/empty; direct provider HTTP500 remains distinct. Does not identify every deployed companion revision.",
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
