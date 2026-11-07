export const BaseComponentNames = {
  ACTION_TYPES: 'smart-items::ActionTypes',
  ACTIONS: 'smart-items::Actions',
  COUNTER: 'smart-items::Counter',
  TRIGGERS: 'smart-items::Triggers',
  STATES: 'smart-items::States',
  COUNTER_BAR: 'smart-items::CounterBar',
  ADMIN_TOOLS: 'smart-items::AdminTools',
  VIDEO_SCREEN: 'smart-items::VideoScreen',
  REWARDS: 'smart-items::Rewards',
  TEXT_ANNOUNCEMENTS: 'smart-items::TextAnnouncements',
  VIDEO_CONTROL_STATE: 'smart-items::VideoControlState',
  SCRIPT: 'smart-items::Script',
  PLACEHOLDER: 'smart-items::Placeholder'
} as const

export enum AdminPermissions {
  PUBLIC = 'PUBLIC',
  PRIVATE = 'PRIVATE'
}

export enum MediaSource {
  VideoURL,
  LiveStream
}
