// `~system/SceneRewards` (protocol scene_rewards.proto). A scene's authoritative server pays a player a reward out of
// a Credits program that Decentraland set up. Only the engine decides whether it can: an orchestrated headless server
// holding the scene's Credits delegation asks credits-server; anywhere else the answer is GRE_NOT_AVAILABLE.

// The protocol enum as a runtime value, shaped like the generated ts-proto enum.
function makeEnum(entries) {
    const e = {};
    for (const [name, value] of entries) {
        e[name] = value;
        e[value] = name;
    }
    return e;
}

const GrantRewardError = makeEnum([
    ['GRE_UNSPECIFIED', 0],
    ['GRE_NOT_AVAILABLE', 1],
    ['GRE_INVALID_REQUEST', 2],
    ['GRE_RATE_LIMITED', 3],
    ['GRE_UNAVAILABLE', 4],
    ['GRE_NOT_AUTHORIZED', 5],
    ['GRE_RECIPIENT_NOT_CONNECTED', 6],
    ['GRE_PROGRAM_PAUSED', 7],
    ['GRE_OUTSIDE_SCHEDULE', 8],
    ['GRE_BUDGET_EXHAUSTED', 9],
    ['GRE_PROGRAM_CAP_REACHED', 10],
    ['GRE_PLAYER_CAP_REACHED', 11],
    ['GRE_REQUIREMENT_NOT_MET', 12],
    ['GRE_RECIPIENT_NOT_ELIGIBLE', 13],
    ['UNRECOGNIZED', -1],
]);
module.exports.GrantRewardError = GrantRewardError;

// A value that is not a string goes to the engine as "", so the engine alone decides the answer: GRE_NOT_AVAILABLE
// where it cannot pay, GRE_INVALID_REQUEST where it can.
function text(value) {
    return typeof value === 'string' ? value : '';
}

module.exports.grantReward = async function (body) {
    const request = body ?? {};
    const op = Deno.core.ops.op_grant_reward;
    if (typeof op !== 'function') {
        return { ok: false, error: GrantRewardError.GRE_NOT_AVAILABLE };
    }
    const answer = await op(text(request.programId), text(request.recipient), text(request.outcomeId));
    if (answer?.ok === true) {
        // a payment is confirmed only with its whole receipt: a boolean replay flag, a receipt id of 1 to 128
        // visible ASCII characters, and a positive whole number of Credits written canonically. Anything less is
        // an unknown outcome, so the scene retries with the same outcome id.
        const { replayed, receiptId, credits } = answer;
        if (typeof replayed !== 'boolean' || typeof receiptId !== 'string' || !/^[\x21-\x7e]{1,128}$/.test(receiptId) ||
            typeof credits !== 'string' || !/^[1-9][0-9]*$/.test(credits)) {
            return { ok: false, error: GrantRewardError.GRE_UNAVAILABLE };
        }
        return { ok: true, replayed, receiptId, credits };
    }
    // an answer that is not a refusal object (null, undefined, a host speaking another shape) is an unknown outcome
    if (answer?.ok !== false) {
        return { ok: false, error: GrantRewardError.GRE_UNAVAILABLE };
    }
    return { ok: false, error: answer.error ?? GrantRewardError.GRE_UNSPECIFIED };
};
