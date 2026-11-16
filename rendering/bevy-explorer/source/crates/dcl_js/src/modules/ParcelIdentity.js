module.exports.getParcel = async function () {
    const scene = await Deno.core.ops.op_scene_information()
    return {
        land: {
            sceneId: scene.urn,
            sceneJsonData: scene.metadataJson || '{}',
            baseUrl: scene.baseUrl,
            // The engine loads glTF content; it exposes no legacy asset-bundle endpoint.
            baseUrlBundles: '',
            mappingsResponse: {
                parcelId: scene.urn,
                rootCid: scene.urn,
                contents: scene.content
            }
        },
        cid: scene.urn
    }
}

module.exports.getSceneId = async function () {
    return { sceneId: Deno.core.ops.op_scene_id() }
}

module.exports.getIsEmpty = async function () {
    // An executing scene has an assigned entity; resolve that identity before replying.
    Deno.core.ops.op_scene_id()
    return { isEmpty: false }
}
