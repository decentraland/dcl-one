module.exports.getActiveVideoStreams = async function (body) {
    return await Deno.core.ops.op_get_active_video_streams()
}

module.exports.publishData = async function (message) {
    await Deno.core.ops.op_comms_publish_data(message.topic, message.data)
}

module.exports.subscribeToTopic = async function (message) {
    await Deno.core.ops.op_comms_subscribe_topic(message.topic)
}

module.exports.unsubscribeFromTopic = async function (message) {
    await Deno.core.ops.op_comms_unsubscribe_topic(message.topic)
}

// Resolves to an array of { sender, data }, not the protocol's { messages } wrapper: scenes
// already read it as an array.
module.exports.consumeMessages = async function (message) {
    return await Deno.core.ops.op_comms_consume_messages(message.topic)
}
