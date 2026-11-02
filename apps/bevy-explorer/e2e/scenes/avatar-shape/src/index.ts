import { AvatarShape, Entity, Transform, engine } from '@dcl/sdk/ecs'
import { Color3, Quaternion, Vector3 } from '@dcl/sdk/math'
import { setupVisualTest } from './visual-test-setup'

/**
 * avatar-shape — AvatarShape component test matrix.
 *
 * Visual contract: five NPCs arranged left-to-right along X at z=12, all
 * facing the camera (which sits at z=2 looking back toward z=12). The row
 * exercises every axis the AvatarShape component cares about: body shape,
 * wearable resolution (base + NFT), position, rotation, scale, and parent
 * transform inheritance.
 *
 * Avatar matrix — one row per case under test:
 *
 *  Col  X    Body shape  Wearables          Rotation   Scale   Emote (local GLB)
 *  ────────────────────────────────────────────────────────────────────────────
 *   0   2    BaseMale    base only          180°       1.0     Friendliest_Player
 *   1   5    BaseFemale  base + NFT (ETH)   225°       1.0     Player_Of_Month
 *   2   8    BaseMale    base + NFT (POL)   180°       1.2     Top_Seller
 *   3  11    BaseFemale  base + NFT (POL)   180°       0.8     Wearable_Creator   ← PARENT
 *   4  13    BaseMale    base + NFT (ETH)   inherited  inh.    Friendliest_Player ← CHILD of col 3
 *
 * Axes covered per column (capitals = primary axis for that column):
 *   col 0: baseline — BASE-COLLECTION WEARABLES only, default scale, BaseMale
 *   col 1: NFT WEARABLE (ethereum) on a BaseFemale, off-axis rotation
 *   col 2: NFT WEARABLE (polygon) on a BaseMale, LARGER SCALE (1.2)
 *   col 3: SMALLER SCALE (0.8), acts as the PARENT for col 4
 *   col 4: PARENT-CHILD transform inheritance — col 4's world position,
 *          rotation and scale are derived from col 3's transform applied to
 *          col 4's local transform (see math below).
 *
 * Parent-child math (col 3 = parent, col 4 = child):
 *   parent pos = (11, 0.05, 12), rot = 180°Y, scale = 0.8
 *   child local pos = (-2.5, 0, 0), local rot = 0°Y, local scale = 1.5
 *     → parent's 180°Y rotation maps local +X to world -X, so child's local
 *       (-2.5, 0, 0) becomes world (+2.5, 0, 0) before scaling.
 *     → scaled by parent's 0.8: world offset = (+2, 0, 0).
 *     → world pos  = (13, 0.05, 12)
 *     → world rot  = 180° + 0° = 180° (faces camera)
 *     → world scale = 0.8 × 1.5 = 1.2
 *
 * Emotes (expressionTriggerId): each avatar plays a local "winner-pose" emote
 * GLB from `assets/Models/` — these are single-shot animations that finish in
 * a held end-frame (designed for podium displays), so the rig stops moving
 * once the emote completes and the snapshot is deterministic. The local path
 * goes directly in `expressionTriggerId`; `emotes` stays empty. (See the
 * Genesis Plaza Hall of Fame scene for the same pattern in production.)
 * `expressionTriggerTimestamp` is set to 1 so the trigger fires at startup
 * and the emote plays through to its hold. Using a looping idle would shift
 * the rig between frames and break pixel-diff.
 *
 * There are 4 emote files for 5 avatars, so col 4 (the child) reuses col 0's
 * emote — the column is exercising the parent-child transform chain, not the
 * emote axis.
 */
export function main() {
  // ── col 0 — BaseMale, base-collection wearables only ─────────────────────
  makeAvatar({
    x: 2,
    rotationDeg: 180,
    scale: 1.0,
    id: 'npc-1',
    name: 'NPC One',
    bodyShape: 'urn:decentraland:off-chain:base-avatars:BaseMale',
    wearables: [
      'urn:decentraland:off-chain:base-avatars:blue_tshirt',
      'urn:decentraland:off-chain:base-avatars:brown_pants',
      'urn:decentraland:off-chain:base-avatars:sneakers',
      'urn:decentraland:off-chain:base-avatars:casual_hair_01',
      'urn:decentraland:off-chain:base-avatars:eyes_00',
      'urn:decentraland:off-chain:base-avatars:eyebrows_00',
      'urn:decentraland:off-chain:base-avatars:mouth_00',
    ],
    skinColor: Color3.create(0.88, 0.69, 0.53),
    hairColor: Color3.create(0.23, 0.12, 0.05),
    eyeColor: Color3.create(0.22, 0.48, 0.72),
    emoteId: EMOTE_FRIENDLIEST_PLAYER,
  })

  // ── col 1 — BaseFemale, base + NFT wearable (layer 1 ethereum) ───────────────────
  makeAvatar({
    x: 5,
    rotationDeg: 225,
    scale: 1.0,
    id: 'npc-2',
    name: 'NPC Two',
    bodyShape: 'urn:decentraland:off-chain:base-avatars:BaseFemale',
    wearables: [
      'urn:decentraland:off-chain:base-avatars:f_red_simple_tshirt',
      'urn:decentraland:off-chain:base-avatars:f_jeans',
      'urn:decentraland:off-chain:base-avatars:bun_shoes',
      'urn:decentraland:off-chain:base-avatars:standard_hair',
      'urn:decentraland:off-chain:base-avatars:f_eyes_00',
      'urn:decentraland:off-chain:base-avatars:f_eyebrows_00',
      'urn:decentraland:off-chain:base-avatars:f_mouth_00',
      // NFT wearable — Halloween 2019 collection (Ethereum mainnet)
      'urn:decentraland:ethereum:collections-v1:halloween_2019:funny_skull_mask',
    ],
    skinColor: Color3.create(0.95, 0.82, 0.72),
    hairColor: Color3.create(0.75, 0.45, 0.1),
    eyeColor: Color3.create(0.08, 0.52, 0.18),
    emoteId: EMOTE_FRIENDLIEST_PLAYER,
  })

  // ── col 2 — BaseMale, base + NFT wearables (ethereum + polygon), scale 1.2 ──
  makeAvatar({
    x: 8,
    rotationDeg: 180,
    scale: 1.2,
    id: 'npc-3',
    name: 'NPC Three',
    bodyShape: 'urn:decentraland:off-chain:base-avatars:BaseMale',
    wearables: [
      'urn:decentraland:off-chain:base-avatars:sport_jacket',
      'urn:decentraland:off-chain:base-avatars:basketball_shorts',
      'urn:decentraland:off-chain:base-avatars:sport_black_shoes',
      'urn:decentraland:off-chain:base-avatars:cool_hair',
      'urn:decentraland:off-chain:base-avatars:eyes_00',
      'urn:decentraland:off-chain:base-avatars:eyebrows_01',
      'urn:decentraland:off-chain:base-avatars:mouth_00',
      // NFT wearable — DCL Launch hat (Ethereum mainnet collections-v1)
      'urn:decentraland:ethereum:collections-v1:dcl_launch:dcl_hat_hat',
      // NFT wearable — CoRoNoscope eyewear (Polygon collections-v2)
      'urn:decentraland:matic:collections-v2:0x19f3a2e14ef3f35c7402304ff02b26bef7f31235:0',
    ],
    skinColor: Color3.create(0.35, 0.22, 0.14),
    hairColor: Color3.create(0.05, 0.05, 0.05),
    eyeColor: Color3.create(0.6, 0.35, 0.1),
    emoteId: EMOTE_TOP_SELLER,
  })

  // ── col 3 — PARENT — BaseFemale + NFT, scale 0.8 ─────────────────────────
  const parent = makeAvatar({
    x: 11,
    rotationDeg: 180,
    scale: 0.8,
    id: 'npc-4',
    name: 'NPC Four (Parent)',
    bodyShape: 'urn:decentraland:off-chain:base-avatars:BaseFemale',
    wearables: [
      'urn:decentraland:off-chain:base-avatars:f_sport_purple_tshirt',
      'urn:decentraland:off-chain:base-avatars:f_sport_shorts',
      'urn:decentraland:off-chain:base-avatars:sneakers',
      'urn:decentraland:off-chain:base-avatars:cornrows',
      'urn:decentraland:off-chain:base-avatars:f_eyes_02',
      'urn:decentraland:off-chain:base-avatars:f_eyebrows_02',
      'urn:decentraland:off-chain:base-avatars:f_mouth_02',
      // NFT wearable — DCL Launch colorful hat (Ethereum mainnet collections-v1)
      'urn:decentraland:ethereum:collections-v1:dcl_launch:colorful_hat_hat',
    ],
    skinColor: Color3.create(0.18, 0.11, 0.07),
    hairColor: Color3.create(0.03, 0.03, 0.04),
    eyeColor: Color3.create(0.52, 0.26, 0.08),
    emoteId: EMOTE_WEARABLE_CREATOR,
  })

  // ── col 4 — CHILD of col 3 — verifies parent transform inheritance ───────
  // Local transform: -2.5 m on local X, identity rotation, scale 1.5.
  // The parent's 180°Y rotation flips local +X → world -X, scaled by 0.8,
  // so the child ends up +2 m to the parent's right in world space. See
  // the file-header math block for the full derivation.
  const child = engine.addEntity()
  AvatarShape.create(child, {
    id: 'npc-5',
    name: 'NPC Five (Child of Four)',
    bodyShape: 'urn:decentraland:off-chain:base-avatars:BaseMale',
    wearables: [
      'urn:decentraland:off-chain:base-avatars:croupier_shirt',
      'urn:decentraland:off-chain:base-avatars:distressed_black_Jeans',
      'urn:decentraland:off-chain:base-avatars:classic_shoes',
      'urn:decentraland:off-chain:base-avatars:short_hair',
      'urn:decentraland:off-chain:base-avatars:eyes_01',
      'urn:decentraland:off-chain:base-avatars:eyebrows_02',
      'urn:decentraland:off-chain:base-avatars:mouth_01',
      // NFT wearable — Atari Launch green hat (Ethereum mainnet collections-v1)
      'urn:decentraland:ethereum:collections-v1:atari_launch:atari_green_hat',
    ],
    emotes: [],
    skinColor: Color3.create(0.72, 0.56, 0.42),
    hairColor: Color3.create(0.58, 0.3, 0.05),
    eyeColor: Color3.create(0.15, 0.4, 0.65),

    expressionTriggerId: EMOTE_FRIENDLIEST_PLAYER,
    expressionTriggerTimestamp: 1,
  })
  Transform.create(child, {
    parent,
    position: Vector3.create(-2.5, 0, 0),
    rotation: Quaternion.Identity(),
    scale: Vector3.create(1.5, 1.5, 1.5),
  })

  // ── camera ───────────────────────────────────────────────────────────────
  // Camera at z=2, slightly raised, looking back toward the avatar row at
  // z=12. lookAt Y=1.2 frames the avatars chest-up while still keeping their
  // feet inside the bottom of the shot.
  setupVisualTest({
    lookAtPos: Vector3.create(7.5, 1.2, 12),
    cameraPos: Vector3.create(7.5, 2, 5.5),
  })
}

// ── helpers ───────────────────────────────────────────────────────────────

const AVATAR_ROW_Z = 12
const AVATAR_GROUND_Y = 0.05

// Local emote GLBs in assets/Models/. Passed directly as expressionTriggerId;
// the renderer accepts a relative file path here (same pattern Genesis Plaza's
// Hall of Fame uses for its podium pose emotes).
const EMOTE_FRIENDLIEST_PLAYER = 'assets/Models/Friendliest_Player_emote.glb'
const EMOTE_PLAYER_OF_MONTH = 'assets/Models/Player_Of_Month_emote.glb'
const EMOTE_TOP_SELLER = 'assets/Models/Top_Seller_emote.glb'
const EMOTE_WEARABLE_CREATOR = 'assets/Models/Wearable_Creator_emote.glb'

interface AvatarSpec {
  /** World X position; Z is fixed at AVATAR_ROW_Z. */
  x: number
  /** Rotation around Y in degrees. 180° = facing the camera at z=2. */
  rotationDeg: number
  /** Uniform scale. */
  scale: number
  id: string
  name: string
  bodyShape: string
  wearables: string[]
  skinColor: Color3
  hairColor: Color3
  eyeColor: Color3
  /** Path to a local emote GLB (e.g. assets/Models/Top_Seller_emote.glb).
   *  Must be a single-shot emote that holds its final frame — no looping
   *  idle, or pixel-diff will be non-deterministic. */
  emoteId: string
}

/** Spawn one AvatarShape entity at (x, AVATAR_GROUND_Y, AVATAR_ROW_Z). */
function makeAvatar(spec: AvatarSpec): Entity {
  const e = engine.addEntity()
  AvatarShape.create(e, {
    id: spec.id,
    name: spec.name,
    bodyShape: spec.bodyShape,
    wearables: spec.wearables,
    emotes: [],
    skinColor: spec.skinColor,
    hairColor: spec.hairColor,
    eyeColor: spec.eyeColor,
    expressionTriggerId: spec.emoteId,
    // Timestamp=1 freezes the trigger at startup so the pose is locked in
    // before the visual-regression snapshot is taken.
    expressionTriggerTimestamp: 1,
  })
  Transform.create(e, {
    position: Vector3.create(spec.x, AVATAR_GROUND_Y, AVATAR_ROW_Z),
    rotation: Quaternion.fromEulerDegrees(0, spec.rotationDeg, 0),
    scale: Vector3.create(spec.scale, spec.scale, spec.scale),
  })
  return e
}
