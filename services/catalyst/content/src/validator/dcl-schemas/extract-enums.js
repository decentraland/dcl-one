// Prints the enums of @dcl/schemas the metadata schema port copies.
// Usage: node extract-enums.js <path to the unpacked @dcl/schemas package>
const schemas = require(require('path').resolve(process.argv[2], 'dist/index.js'))
const { Wearable, Emote, Scene, HideableWearableCategory } = schemas
const data = Wearable.schema.properties.data.properties
const mappings = Emote.schema.properties.mappings
const toggles = Scene.schema.properties.featureToggles.properties
const contracts = Object.values(mappings.properties)[0].patternProperties
const enums = {
  wearableCategories: data.category.enum,
  hideableBodyParts: HideableWearableCategory.schema.oneOf[1].enum,
  bodyShapes: data.representations.items.properties.bodyShapes.items.enum,
  rarities: Wearable.schema.properties.rarity.enum,
  locales: Wearable.schema.properties.i18n.items.properties.code.enum,
  emoteCategories: Emote.schema.properties.emoteDataADR74.properties.category.enum,
  scenePermissions: Scene.schema.properties.requiredPermissions.items.enum,
  voiceChatToggles: toggles.voiceChat.enum,
  portableExperiencesToggles: toggles.portableExperiences.enum,
  nearbyVoiceChatToggles: toggles.nearbyVoiceChat.enum,
  sceneSourceRotations: Scene.schema.properties.source.properties.rotation.enum,
  mappingNetworks: Object.keys(mappings.properties),
  mappingTypes: Object.values(contracts)[0].items.properties.type.enum
}
for (const [name, values] of Object.entries(enums)) {
  if (!Array.isArray(values) || values.length === 0) throw new Error(`${name} not found`)
}
process.stdout.write(JSON.stringify(enums, null, 2) + '\n')
