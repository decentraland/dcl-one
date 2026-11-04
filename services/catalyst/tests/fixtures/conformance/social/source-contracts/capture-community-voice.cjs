// Run with Node26 and materialized pinned sources; all dependency responses are local mocks.
// Usage: TS_NODE_COMPILER_OPTIONS='{"module":"CommonJS","moduleResolution":"node"}' node <script> <social-root> <gatekeeper-root> <output-json>
const fs = require("fs");
const path = require("path");
const root = process.argv[2];
const gatekeeper = process.argv[3];
require(root + "/node_modules/ts-node/register/transpile-only");
process.env.NODE_PATH = root + "/node_modules";
const Module = require("module");
Module._initPaths();
// No media operation runs in the inactive branch; this unused enum import is stubbed.
const load = Module._load;
Module._load = function (name, ...rest) {
  if (name === "@livekit/protocol") return { DisconnectReason: {} };
  return load.call(this, name, ...rest);
};
const logs = {
  getLogger: () => ({ info() {}, debug() {}, error() {}, warn() {} }),
};
const { createVoiceComponent } = require(
  gatekeeper + "/src/logic/voice/voice.ts",
);
const { getCommunityVoiceChatStatusHandler } = require(
  gatekeeper +
    "/src/controllers/handlers/community-voice-chat/get-community-voice-chat-status-handler.ts",
);
const { createCommsGatekeeperComponent } = require(
  root + "/src/adapters/comms-gatekeeper.ts",
);
const { createCommunityComponent } = require(
  root + "/src/logic/community/communities.ts",
);
const { getCommunityV2Handler } = require(
  root + "/src/controllers/handlers/http/get-community-v2-handler.ts",
);
const { getCommunityHandler } = require(
  root + "/src/controllers/handlers/http/get-community-handler.ts",
);
const { createCommunityVoiceComponent } = require(
  root + "/src/logic/community-voice/community-voice.ts",
);
const { getActiveCommunityVoiceChatsHandler } = require(
  root +
    "/src/controllers/handlers/http/get-active-community-voice-chats-handler.ts",
);
const id = "00000000-0000-4000-8000-000000000001";
const other = "00000000-0000-4000-8000-000000000002";
const address = "0x0000000000000000000000000000000000000001";
(async () => {
  const gatekeeperVoice = createVoiceComponent({
    logs,
    voiceDB: { isCommunityRoomActive: async () => false },
    livekit: {
      getCommunityVoiceChatRoomName: (id) => "voice-chat-community-" + id,
    },
  });
  const inactive = await getCommunityVoiceChatStatusHandler({
    components: { logs, voice: gatekeeperVoice },
    params: { communityId: id },
  });
  const adapter = await createCommsGatekeeperComponent({
    logs,
    config: {
      requireString: async (key) =>
        key === "COMMS_GATEKEEPER_URL"
          ? "http://fixture.invalid"
          : "fixture-token",
    },
    fetcher: {
      fetch: async () =>
        new Response(JSON.stringify(inactive.body), {
          status: inactive.status,
        }),
    },
  });
  const status = await adapter.getCommunityVoiceChatStatus(id);
  const base = {
    id,
    name: "Fixture community",
    description: "Description",
    ownerAddress: address,
    privacy: "public",
    active: true,
    membersCount: 1,
  };
  const communities = createCommunityComponent({
    logs,
    commsGatekeeper: adapter,
    communitiesDb: {
      getCommunityPublicInformation: async () => base,
      getCommunity: async () => ({ ...base, role: "none", visibility: "all" }),
      getCommunityMembersCount: async () => 1,
    },
    communityEvents: { isCurrentlyHostingEvents: async () => false },
  });
  const detail = await getCommunityV2Handler({
    components: { logs, communities },
    params: { id },
  });
  const cases = [
    { name: "inactive200", gatekeeper: inactive, adapter: status, detail },
  ];
  for (const [name, handler] of [
    ["detail-v1-error", getCommunityHandler],
    ["detail-v2-error", getCommunityV2Handler],
  ]) {
    const result = await handler({
      components: {
        logs,
        communities: {
          getCommunityPublicInformation: async () => {
            throw new Error("fixture community lookup failed");
          },
          getCommunityPublicInformationWithoutProfile: async () => {
            throw new Error("fixture community lookup failed");
          },
        },
      },
      params: { id },
    });
    cases.push({ name, result });
  }
  for (const name of [
    "empty",
    "gatekeeper-error",
    "community-error",
    "member-thumbnail-error",
    "nonmember-places-error",
  ]) {
    const stats =
      name === "empty"
        ? []
        : [
            { communityId: id, participantCount: 2, moderatorCount: 1 },
            { communityId: other, participantCount: 3, moderatorCount: 1 },
          ];
    const component = await createCommunityVoiceComponent({
      logs,
      commsGatekeeper: {
        getAllActiveCommunityVoiceChats: async () => {
          if (name === "gatekeeper-error")
            throw new Error("Server responded with status 500");
          return stats;
        },
      },
      communitiesDb: {
        getCommunities: async () => {
          if (name === "community-error")
            throw new Error("fixture community lookup failed");
          return stats.map((row) => ({
            id: row.communityId,
            name:
              row.communityId === id ? "Member community" : "Public community",
            role: row.communityId === id ? "member" : "none",
            privacy: "public",
            visibility: "all",
          }));
        },
      },
      communityPlaces: {
        getPlacesWithPositionsAndWorlds: async (community) => {
          if (name === "nonmember-places-error" && community === other)
            throw new Error("fixture places failure");
          return { positions: ["0,0"], worlds: [] };
        },
      },
      communityThumbnail: {
        getThumbnail: async (community) => {
          if (name === "member-thumbnail-error" && community === id)
            throw new Error("fixture thumbnail failure");
          return undefined;
        },
      },
    });
    const result = await getActiveCommunityVoiceChatsHandler({
      components: { logs, communityVoice: component },
      verification: { auth: address },
    });
    cases.push({ name, result });
  }
  fs.writeFileSync(
    process.argv[4],
    JSON.stringify(
      {
        source: "decentraland/social-service-ea",
        revision: "b6b2cefefdf727b2ed47ddb887e77d226a41eea6",
        companion: {
          source: "decentraland/comms-gatekeeper",
          revision: "be6a4b97a2189157884503485892ff46d0ac68da",
        },
        node: process.version,
        sources: {
          social: [
            "src/adapters/comms-gatekeeper.ts",
            "src/logic/community/communities.ts",
            "src/logic/community-voice/community-voice.ts",
            "src/controllers/handlers/http/get-community-v2-handler.ts",
            "src/controllers/handlers/http/get-community-handler.ts",
            "src/controllers/handlers/http/get-active-community-voice-chats-handler.ts",
          ],
          companion: [
            "src/logic/voice/voice.ts",
            "src/controllers/handlers/community-voice-chat/get-community-voice-chat-status-handler.ts",
          ],
        },
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
