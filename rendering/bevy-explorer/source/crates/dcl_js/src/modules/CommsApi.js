module.exports.getActiveVideoStreams = async function (body) {
    return await Deno.core.ops.op_get_active_video_streams()
}

module.exports.subscribeToTopic = async function (body) {
    await Deno.core.ops.op_set_topic_subscription(body.topic, true)
    return {}
}

module.exports.unsubscribeFromTopic = async function (body) {
    await Deno.core.ops.op_set_topic_subscription(body.topic, false)
    return {}
}

module.exports.publishData = async function (body) {
    await Deno.core.ops.op_publish_topic_data(body.topic, body.data ?? "")
    return {}
}

module.exports.consumeMessages = async function (body) {
    return await Deno.core.ops.op_consume_topic_messages(body.topic)
}
