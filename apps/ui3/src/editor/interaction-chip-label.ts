const triggers: Record<string, string> = {
  on_click: "when clicked",
  on_input_action: "when E is pressed",
  on_player_enters_area: "when a player enters",
  on_player_leaves_area: "when a player leaves",
  on_spawn: "on spawn",
};

const actions: Record<string, string> = {
  start_tween: "move it",
  set_visibility: "show / hide",
  play_sound: "play a sound",
  play_animation: "play an animation",
  teleport_player: "teleport the player",
  open_link: "open a link",
};

function readable(id: string): string {
  return id.replace(/([a-z\d])([A-Z])/g, "$1 $2").replaceAll("_", " ").toLowerCase();
}

export function triggerChip(id: string | null | undefined): string {
  return id ? triggers[id] ?? readable(id) : "no trigger";
}

export function actionChip(id: string | null | undefined): string {
  return id ? actions[id] ?? readable(id) : "no action";
}
