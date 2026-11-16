const PermissionItem = {}
for (const [name, value] of [
    ['PI_ALLOW_TO_MOVE_PLAYER_INSIDE_SCENE', 0],
    ['PI_ALLOW_TO_TRIGGER_AVATAR_EMOTE', 1],
    ['PI_USE_WEB3_API', 2],
    ['PI_USE_WEBSOCKET', 3],
    ['PI_USE_FETCH', 4],
    ['PI_ALLOW_MEDIA_HOSTNAMES', 5],
    ['PI_OPEN_EXTERNAL_LINK', 6],
    ['UNRECOGNIZED', -1]
]) {
    PermissionItem[name] = value
    PermissionItem[value] = name
}
module.exports.PermissionItem = PermissionItem

module.exports.hasPermission = async function (body = {}) {
    const [hasPermission] = await Deno.core.ops.op_get_permissions([body.permission ?? 0])
    return { hasPermission }
}

module.exports.hasManyPermissions = async function (body = {}) {
    return { hasManyPermission: await Deno.core.ops.op_get_permissions(body.permissions ?? []) }
}
