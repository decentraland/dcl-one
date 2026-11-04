// Source-only preparation. No network requests or paid providers.
const fs = require("fs");
const root = process.argv[2];
require(root + "/node_modules/ts-node/register/transpile-only");
const { SNSClient } = require(root + "/node_modules/@aws-sdk/client-sns");
const { createSnsComponent } = require(
  root + "/node_modules/@dcl/sns-component",
);
const { createCommunityBroadcasterComponent } = require(
  root + "/src/logic/community/broadcaster.ts",
);
const { Events } = require(root + "/node_modules/@dcl/schemas");
const communityId = "00000000-0000-4000-8000-000000000001";
const timestamp = 1793721600000;
const address = (n) => "0x" + n.toString(16).padStart(40, "0");
let requests = [];
let fail = false;
let partial = false;
SNSClient.prototype.send = async function (command) {
  requests.push({ command: command.constructor.name, input: command.input });
  if (fail) throw new Error("fixture SNS failure");
  if (partial)
    return {
      Successful: [
        { Id: "msg_0", MessageId: "accepted" },
        { Id: "msg_2" },
        { Id: "msg_3", MessageId: "" },
      ],
      Failed: [
        { Id: "msg_1" },
        { Id: "msg_01" },
        { Id: "msg_2junk" },
        { Id: "msg_-1" },
        { Id: "msg_99" },
        { Id: "msg_" },
      ],
    };
  return {
    Successful: (command.input.PublishBatchRequestEntries || []).map(
      (entry) => ({ Id: entry.Id, MessageId: "fixture-" + entry.Id }),
    ),
  };
};
const errors = [];
const logs = {
  getLogger: () => ({
    info() {},
    debug() {},
    warn() {},
    error(...args) {
      errors.push(args);
    },
  }),
};
const source = "b6b2cefefdf727b2ed47ddb887e77d226a41eea6";
(async () => {
  const sns = await createSnsComponent({
    config: {
      requireString: async () => "arn:aws:sns:us-east-1:000000000000:fixture",
      getString: async (key) =>
        key === "AWS_REGION" ? "us-east-1" : undefined,
    },
  });
  const cases = [];
  for (const name of [
    ...Object.keys(Events.SubType.Community),
    "RENAMED_1101",
    "VOICE_OFFLINE",
    "EMPTY_MANAGERS",
    "DATABASE_FAILURE",
    "SNS_FAILURE",
    "BATCH_FAILURE",
  ]) {
    const kind =
      name === "RENAMED_1101"
        ? "RENAMED"
        : name === "VOICE_OFFLINE"
          ? "VOICE_CHAT_STARTED"
          : name === "EMPTY_MANAGERS"
            ? "REQUEST_TO_JOIN_RECEIVED"
            : name.endsWith("FAILURE")
              ? "MEMBER_REMOVED"
              : name;
    const members = Array.from(
      {
        length:
          name === "RENAMED_1101" ? 1101 : name === "EMPTY_MANAGERS" ? 0 : 205,
      },
      (_, i) => ({
        memberAddress: address(i + 1),
        role: i === 0 ? "owner" : i < 3 ? "moderator" : "member",
      }),
    );
    const dbCalls = [];
    const broadcaster = createCommunityBroadcasterComponent({
      sns,
      logs,
      peersStats: {
        getConnectedPeers: async () =>
          name === "VOICE_OFFLINE"
            ? []
            : members
                .filter((_, i) => i % 2 === 0)
                .map((row) => row.memberAddress),
      },
      communitiesDb: {
        getCommunityMembers: async (id, options) => {
          dbCalls.push({ id, options });
          if (name === "DATABASE_FAILURE")
            throw new Error("fixture DB failure");
          const rows = members.filter(
            (row) =>
              (!options.roles || options.roles.includes(row.role)) &&
              (!options.excludedAddresses ||
                options.excludedAddresses.some(
                  (excluded) => row.memberAddress !== excluded.toLowerCase(),
                )) &&
              (!options.filterByMembers ||
                options.filterByMembers.includes(row.memberAddress)),
          );
          return rows.slice(
            options.pagination.offset,
            options.pagination.offset + options.pagination.limit,
          );
        },
      },
    });
    const common = {
      communityId,
      communityName: "Fixture community",
      thumbnailUrl: "N/A",
    };
    const metadata = {
      DELETED: {
        id: communityId,
        name: "Fixture community",
        thumbnailUrl: "N/A",
      },
      DELETED_CONTENT_VIOLATION: {
        id: communityId,
        name: "Fixture community",
        ownerAddress: address(1),
        thumbnailUrl: "N/A",
      },
      RENAMED: {
        id: communityId,
        oldName: "Before",
        newName: "After",
        thumbnailUrl: "N/A",
      },
      MEMBER_BANNED: {
        id: communityId,
        name: "Fixture community",
        memberAddress: address(4),
        thumbnailUrl: "N/A",
      },
      MEMBER_LEFT: { id: communityId, memberAddress: address(4) },
      MEMBER_REMOVED: {
        id: communityId,
        name: "Fixture community",
        memberAddress: address(4),
        thumbnailUrl: "N/A",
      },
      REQUEST_TO_JOIN_RECEIVED: {
        ...common,
        memberAddress: address(4),
        memberName: "Member",
      },
      REQUEST_TO_JOIN_ACCEPTED: { ...common, memberAddress: address(4) },
      INVITE_RECEIVED: { ...common, memberAddress: address(4) },
      OWNERSHIP_TRANSFERRED: {
        ...common,
        oldOwnerAddress: address(1),
        newOwnerAddress: address(4),
      },
      POST_ADDED: {
        ...common,
        postId: "00000000-0000-4000-8000-000000000002",
        authorAddress: address(1),
        addressesToNotify: [],
      },
      VOICE_CHAT_STARTED: common,
    }[kind];
    const event = {
      type: "community",
      subType: Events.SubType.Community[kind],
      key: "fixture-" + kind.toLowerCase(),
      timestamp,
      metadata,
    };
    requests = [];
    errors.length = 0;
    fail = name === "SNS_FAILURE" || name === "BATCH_FAILURE";
    // The DB failure case must use a strategy that actually reads membership.
    if (name === "DATABASE_FAILURE")
      ((event.subType = Events.SubType.Community.RENAMED),
        (event.metadata = { id: communityId, name: "Fixture community" }));
    if (name === "BATCH_FAILURE")
      ((event.subType = Events.SubType.Community.RENAMED),
        (event.metadata = { id: communityId, name: "Fixture community" }));
    await broadcaster.broadcast(
      event,
      kind === "VOICE_CHAT_STARTED"
        ? { excludeAddresses: [address(1)] }
        : undefined,
    );
    cases.push({
      name,
      event,
      dbCalls,
      requests: JSON.parse(JSON.stringify(requests)),
      errors: JSON.parse(JSON.stringify(errors)),
    });
  }
  const adapterCases = [];
  for (const name of ["partial", "network-failure"]) {
    requests = [];
    fail = name === "network-failure";
    partial = name === "partial";
    const events = Array.from({ length: 12 }, (_, index) => ({
      type: "community",
      subType: "community-renamed",
      key: "adapter-" + index,
      timestamp,
      metadata: { id: communityId },
    }));
    const result = await sns.publishMessages(events);
    adapterCases.push({
      name,
      events,
      requests: JSON.parse(JSON.stringify(requests)),
      result,
    });
  }
  fs.writeFileSync(
    process.argv[3],
    JSON.stringify(
      {
        source: "decentraland/social-service-ea",
        revision: source,
        node: process.version,
        snsVersion: require(
          root + "/node_modules/@dcl/sns-component/package.json",
        ).version,
        scope:
          "Actual broadcaster and installed SNS adapter; event input fixtures are source-shaped, all twelve creator outputs are captured separately in community-producers.json.",
        sources: [
          "src/logic/community/broadcaster.ts",
          "node_modules/@dcl/sns-component/dist/src/component.js",
        ],
        cases,
        adapterCases,
      },
      null,
      2,
    ) + "\n",
  );
})().catch((error) => {
  console.error(error);
  process.exitCode = 1;
});
