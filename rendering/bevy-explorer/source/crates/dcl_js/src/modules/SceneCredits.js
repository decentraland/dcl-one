// `~system/SceneCredits` (library/protocol's kernel/apis/scene_credits.proto). A scene's authoritative server pays a
// player Credits out of a Credits program that Decentraland set up. Only the engine decides whether it can: an
// orchestrated headless server holding the scene's Credits delegation asks credits-server; anywhere else the answer
// is GCE_NOT_AVAILABLE.

// The protocol enum as a runtime value, shaped like the generated ts-proto enum.
function makeEnum(entries) {
    const e = {};
    for (const [name, value] of entries) {
        e[name] = value;
        e[value] = name;
    }
    return e;
}

const GrantCreditsError = makeEnum([
    ['GCE_UNSPECIFIED', 0],
    ['GCE_NOT_AVAILABLE', 1],
    ['GCE_INVALID_REQUEST', 2],
    ['GCE_RATE_LIMITED', 3],
    ['GCE_UNAVAILABLE', 4],
    ['GCE_NOT_AUTHORIZED', 5],
    ['GCE_RECIPIENT_NOT_CONNECTED', 6],
    ['GCE_PROGRAM_PAUSED', 7],
    ['GCE_OUTSIDE_SCHEDULE', 8],
    ['GCE_BUDGET_EXHAUSTED', 9],
    ['GCE_PROGRAM_CAP_REACHED', 10],
    ['GCE_PLAYER_CAP_REACHED', 11],
    ['GCE_REQUIREMENT_NOT_MET', 12],
    ['GCE_RECIPIENT_NOT_ELIGIBLE', 13],
    ['UNRECOGNIZED', -1],
]);
module.exports.GrantCreditsError = GrantCreditsError;

// A value that is not a string goes to the engine as "", so the engine alone decides the answer: GCE_NOT_AVAILABLE
// where it cannot pay, GCE_INVALID_REQUEST where it can.
function text(value) {
    return typeof value === 'string' ? value : '';
}

module.exports.grantCredits = async function (body) {
    const request = body ?? {};
    const op = Deno.core.ops.op_grant_credits;
    if (typeof op !== 'function') {
        return { ok: false, error: GrantCreditsError.GCE_NOT_AVAILABLE };
    }
    const answer = await op(text(request.programId), text(request.recipient), text(request.outcomeId));
    if (answer?.ok === true) {
        // a payment is confirmed only with its whole receipt: a boolean replay flag, a receipt id of 1 to 128
        // visible ASCII characters, and a positive whole number of Credits written canonically. Anything less is
        // an unknown outcome, so the scene retries with the same outcome id.
        const { replayed, receiptId, credits } = answer;
        if (typeof replayed !== 'boolean' || typeof receiptId !== 'string' || !/^[\x21-\x7e]{1,128}$/.test(receiptId) ||
            typeof credits !== 'string' || !/^[1-9][0-9]*$/.test(credits)) {
            return { ok: false, error: GrantCreditsError.GCE_UNAVAILABLE };
        }
        return { ok: true, replayed, receiptId, credits };
    }
    // an answer that is not a refusal object (null, undefined, a host speaking another shape) is an unknown outcome
    if (answer?.ok !== false) {
        return { ok: false, error: GrantCreditsError.GCE_UNAVAILABLE };
    }
    return { ok: false, error: answer.error ?? GrantCreditsError.GCE_UNSPECIFIED };
};
