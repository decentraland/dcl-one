export const TRIGGER_ID = {
  click: "on_click",
  input: "on_input_action",
} as const;

export const ACTION_ID = {
  tween: "start_tween",
  visibility: "set_visibility",
  sound: "play_sound",
  animate: "play_animation",
} as const;
