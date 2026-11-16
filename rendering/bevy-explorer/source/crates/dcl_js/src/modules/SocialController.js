// Subscribe when the scene loads the module so events are retained between polls.
Deno.core.ops.op_subscribe_legacy_avatar_events()

module.exports.pullAvatarEvents = async function () {
    return { events: await Deno.core.ops.op_pull_legacy_avatar_events() }
}
