import { DeepReadonlyObject, PBAvatarLocomotionSettings, PBInputModifier } from "@dcl/sdk/ecs";
import { settings } from "./settings";
import { AIR_JUMP_COUNT, DOUBLE_JUMP_HEIGHT, GLIDE_FALL_SPEED, GLIDE_HORIZONTAL_SPEED, LANDING_STUN_TIME } from "./constants";

export var sprintSpeed = 0;
export var jogSpeed = 0;
export var walkSpeed = 0;
export var jumpHeight = 0;
export var sprintJumpHeight = 0;
export var doubleJumpHeight = 0;
export var glidingSpeed = 0;
export var glidingFallingSpeed = 0;
export var landingStunTime = LANDING_STUN_TIME;
var doubleJumpEnabled = false;
export var glideEnabled = false;
// Slots available between two grounded frames. The vertical model counts the jumps
// since the ground and falls through to a glide once the air slot is spent.
export var maxGroundJumps = 0; // 1 unless disableJump
export var maxAirJumps = 0;    // 1 unless disableDoubleJump
export var disableOrientation = false;

export function initParameters(
    locomotion: DeepReadonlyObject<PBAvatarLocomotionSettings> | undefined,
    modifiers: DeepReadonlyObject<PBInputModifier> | undefined
) {
    // Engine-provided locomotion values win over the settings.ts defaults.
    const baseSprintSpeed = locomotion?.runSpeed ?? settings.sprintSpeed;
    const baseJogSpeed = locomotion?.jogSpeed ?? settings.jogSpeed;
    const baseWalkSpeed = locomotion?.walkSpeed ?? settings.walkSpeed;

    const baseJumpHeight = locomotion?.jumpHeight ?? settings.jumpHeight;
    const baseSprintJumpHeight = locomotion?.runJumpHeight ?? settings.sprintJumpHeight;

    // Double-jump height and glide speeds have no entry in settings.ts, so the constants are their defaults.
    doubleJumpHeight = locomotion?.doubleJumpHeight ?? DOUBLE_JUMP_HEIGHT;
    glidingSpeed = locomotion?.glidingSpeed ?? GLIDE_HORIZONTAL_SPEED;
    glidingFallingSpeed = locomotion?.glidingFallingSpeed ?? GLIDE_FALL_SPEED;
    // taken as set: 0 or less means no stun
    landingStunTime = locomotion?.hardLandingCooldown ?? LANDING_STUN_TIME;

    const jogEnabled = !modifiers?.mode?.standard.disableJog && !modifiers?.mode?.standard.disableAll;
    const walkEnabled = !modifiers?.mode?.standard.disableWalk && !modifiers?.mode?.standard.disableAll;
    const runEnabled = !modifiers?.mode?.standard.disableRun && !modifiers?.mode?.standard.disableAll;
    const anyEnabled = jogEnabled || walkEnabled || runEnabled;

    const jumpEnabled = !modifiers?.mode?.standard.disableJump && !modifiers?.mode?.standard.disableAll;
    // Air-jump and glide are independent of the ground-jump gate so that, with
    // disableJump set, the player can still air-jump or glide after walking
    // off a cliff (the air-jump slot is replenished on every grounded frame).
    doubleJumpEnabled = !modifiers?.mode?.standard.disableDoubleJump && !modifiers?.mode?.standard.disableAll;
    glideEnabled = !modifiers?.mode?.standard.disableGliding && !modifiers?.mode?.standard.disableAll;

    jogSpeed = jogEnabled ? baseJogSpeed
        : walkEnabled ? baseWalkSpeed
        : runEnabled ? baseSprintSpeed
        : 0;

    walkSpeed = walkEnabled ? baseWalkSpeed : jogSpeed;
    sprintSpeed = runEnabled ? baseSprintSpeed : jogSpeed;

    jumpHeight = jumpEnabled ? baseJumpHeight : 0;
    sprintJumpHeight = jumpEnabled ? baseSprintJumpHeight : 0;

    maxGroundJumps = jumpEnabled ? 1 : 0;
    maxAirJumps = doubleJumpEnabled && doubleJumpHeight > 0 ? AIR_JUMP_COUNT : 0;

    disableOrientation = !anyEnabled;
}