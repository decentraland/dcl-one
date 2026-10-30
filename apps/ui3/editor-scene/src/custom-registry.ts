
import { engine, Schemas } from '@dcl/sdk/ecs'
import sceneMetadataSchema from './scene-metadata-schema.json'


enum TriggerType {
  ON_CLICK = 'on_click',
  ON_INPUT_ACTION = 'on_input_action',
  ON_STATE_CHANGE = 'on_state_change',
  ON_SPAWN = 'on_spawn',
  ON_TWEEN_END = 'on_tween_end',
  ON_COUNTER_CHANGE = 'on_counter_change',
  ON_PLAYER_ENTERS_AREA = 'on_player_enters_area',
  ON_PLAYER_LEAVES_AREA = 'on_player_leaves_area',
  ON_DELAY = 'on_delay',
  ON_LOOP = 'on_loop',
  ON_CLONE = 'on_clone',
  ON_CLICK_IMAGE = 'on_click_image',
  ON_DAMAGE = 'on_damage',
  ON_GLOBAL_CLICK = 'on_global_click',
  ON_GLOBAL_PRIMARY = 'on_global_primary',
  ON_GLOBAL_SECONDARY = 'on_global_secondary',
  ON_TICK = 'on_tick',
  ON_HEAL_PLAYER = 'on_heal_player',
  ON_PLAYER_SPAWN = 'on_player_spawn'
}

enum TriggerConditionType {
  WHEN_STATE_IS = 'when_state_is',
  WHEN_STATE_IS_NOT = 'when_state_is_not',
  WHEN_COUNTER_EQUALS = 'when_counter_equals',
  WHEN_COUNTER_IS_GREATER_THAN = 'when_counter_is_greater_than',
  WHEN_COUNTER_IS_LESS_THAN = 'when_counter_is_less_than',
  WHEN_DISTANCE_TO_PLAYER_LESS_THAN = 'when_distance_to_player_less_than',
  WHEN_DISTANCE_TO_PLAYER_GREATER_THAN = 'when_distance_to_player_greater_than',
  WHEN_PREVIOUS_STATE_IS = 'when_previous_state_is',
  WHEN_PREVIOUS_STATE_IS_NOT = 'when_previous_state_is_not'
}

enum TriggerConditionOperation {
  AND = 'and',
  OR = 'or'
}

enum AdminPermissions {
  PUBLIC = 'PUBLIC',
  PRIVATE = 'PRIVATE'
}

enum MediaSource {
  VideoURL,
  LiveStream
}


export const CUSTOM_REGISTRY_DEFS = [
  engine.defineComponent('asset-packs::ActionTypes', {
    value: Schemas.Array(Schemas.Map({ type: Schemas.String, jsonSchema: Schemas.String }))
  }),
  engine.defineComponent('asset-packs::Actions', {
    id: Schemas.Int,
    value: Schemas.Array(
      Schemas.Map({
        name: Schemas.String,
        type: Schemas.String,
        jsonPayload: Schemas.String,
        allowedInBasicView: Schemas.Optional(Schemas.Boolean),
        basicViewId: Schemas.Optional(Schemas.String),
        default: Schemas.Optional(Schemas.Boolean)
      })
    )
  }),
  engine.defineComponent('asset-packs::Counter', {
    id: Schemas.Number,
    value: Schemas.Int
  }),
  engine.defineComponent('asset-packs::Triggers', {
    value: Schemas.Array(
      Schemas.Map({
        type: Schemas.EnumString<TriggerType>(TriggerType, TriggerType.ON_INPUT_ACTION),
        conditions: Schemas.Optional(
          Schemas.Array(
            Schemas.Map({
              id: Schemas.Optional(Schemas.Int),
              type: Schemas.EnumString<TriggerConditionType>(
                TriggerConditionType,
                TriggerConditionType.WHEN_STATE_IS
              ),
              value: Schemas.String
            })
          )
        ),
        operation: Schemas.Optional(
          Schemas.EnumString<TriggerConditionOperation>(
            TriggerConditionOperation,
            TriggerConditionOperation.AND
          )
        ),
        actions: Schemas.Array(
          Schemas.Map({
            id: Schemas.Optional(Schemas.Int),
            name: Schemas.Optional(Schemas.String)
          })
        ),
        basicViewId: Schemas.Optional(Schemas.String)
      })
    )
  }),
  engine.defineComponent('asset-packs::States', {
    id: Schemas.Number,
    value: Schemas.Array(Schemas.String),
    defaultValue: Schemas.Optional(Schemas.String),
    currentValue: Schemas.Optional(Schemas.String),
    previousValue: Schemas.Optional(Schemas.String)
  }),
  engine.defineComponent('asset-packs::CounterBar', {
    primaryColor: Schemas.Optional(Schemas.String),
    secondaryColor: Schemas.Optional(Schemas.String),
    maxValue: Schemas.Optional(Schemas.Float)
  }),
  engine.defineComponent('asset-packs::AdminTools', {
    adminPermissions: Schemas.EnumString<AdminPermissions>(AdminPermissions, AdminPermissions.PUBLIC),
    authorizedAdminUsers: Schemas.Map({
      me: Schemas.Boolean,
      sceneOwners: Schemas.Boolean,
      allowList: Schemas.Boolean,
      adminAllowList: Schemas.Array(Schemas.String)
    }),
    moderationControl: Schemas.Map({
      isEnabled: Schemas.Boolean,
      kickCoordinates: Schemas.Map({
        x: Schemas.Number,
        y: Schemas.Number,
        z: Schemas.Number
      }),
      allowNonOwnersManageAdminAllowList: Schemas.Boolean
    }),
    textAnnouncementControl: Schemas.Map({
      isEnabled: Schemas.Boolean,
      playSoundOnEachAnnouncement: Schemas.Boolean,
      showAuthorOnEachAnnouncement: Schemas.Boolean
    }),
    videoControl: Schemas.Map({
      isEnabled: Schemas.Boolean,
      disableVideoPlayersSound: Schemas.Boolean,
      showAuthorOnVideoPlayers: Schemas.Boolean,
      linkAllVideoPlayers: Schemas.Boolean,
      videoPlayers: Schemas.Optional(
        Schemas.Array(
          Schemas.Map({
            entity: Schemas.Int,
            customName: Schemas.String
          })
        )
      )
    }),
    smartItemsControl: Schemas.Map({
      isEnabled: Schemas.Boolean,
      linkAllSmartItems: Schemas.Boolean,
      smartItems: Schemas.Optional(
        Schemas.Array(
          Schemas.Map({
            entity: Schemas.Int,
            customName: Schemas.String,
            defaultAction: Schemas.String
          })
        )
      )
    }),
    rewardsControl: Schemas.Map({
      isEnabled: Schemas.Boolean,
      rewardItems: Schemas.Optional(
        Schemas.Array(
          Schemas.Map({
            entity: Schemas.Int,
            customName: Schemas.String
          })
        )
      )
    })
  }),
  engine.defineComponent('asset-packs::VideoScreen', {
    thumbnail: Schemas.String,
    defaultMediaSource: Schemas.EnumNumber<MediaSource>(MediaSource, MediaSource.VideoURL),
    defaultURL: Schemas.String
  }),
  engine.defineComponent('asset-packs::Rewards', {
    campaignId: Schemas.String,
    dispenserKey: Schemas.String,
    testMode: Schemas.Boolean
  }),
  engine.defineComponent('asset-packs::TextAnnouncements', {
    text: Schemas.String,
    author: Schemas.Optional(Schemas.String),
    id: Schemas.String
  }),
  engine.defineComponent('asset-packs::VideoControlState', {
    endsAt: Schemas.Optional(Schemas.Int64),
    streamKey: Schemas.Optional(Schemas.String)
  }),
  engine.defineComponent('asset-packs::Script', {
    value: Schemas.Array(
      Schemas.Map({
        path: Schemas.String,
        priority: Schemas.Number,
        layout: Schemas.Optional(Schemas.String)
      })
    )
  }),
  engine.defineComponent('asset-packs::Placeholder', {
    src: Schemas.String
  }),

  engine.defineComponentFromSchema('inspector::SceneMetadata-v3', Schemas.fromJson(sceneMetadataSchema as Parameters<typeof Schemas.fromJson>[0])),
  engine.defineComponent('inspector::Selection', { gizmo: Schemas.Int }),
  engine.defineComponent('inspector::Nodes', {
    value: Schemas.Array(
      Schemas.Map({
        entity: Schemas.Entity,
        open: Schemas.Optional(Schemas.Boolean),
        children: Schemas.Array(Schemas.Entity)
      })
    )
  }),
  engine.defineComponent('inspector::TransformConfig', {
    porportionalScaling: Schemas.Optional(Schemas.Boolean)
  }),
  engine.defineComponent('inspector::Hide', { value: Schemas.Boolean }),
  engine.defineComponent('inspector::Lock', { value: Schemas.Boolean }),
  engine.defineComponent('inspector::Ground', {}),
  engine.defineComponent('inspector::Tile', {}),
  engine.defineComponent('inspector::CustomAsset', { assetId: Schemas.String }),
  engine.defineComponent('inspector::UIState', {
    sceneInfoPanelVisible: Schemas.Optional(Schemas.Boolean)
  })
]
