// The starting looks a new account picks from: four per body type, each an outfit of base wearables
// and its colours, with the picture its slot shows.
import type { AvatarLook } from '@explorer-core/engine/avatarEquip'
import male1 from '../../assets/startup/male-slot-1.webp'
import male2 from '../../assets/startup/male-slot-2.webp'
import male3 from '../../assets/startup/male-slot-3.webp'
import male4 from '../../assets/startup/male-slot-4.webp'
import female1 from '../../assets/startup/female-slot-1.webp'
import female2 from '../../assets/startup/female-slot-2.webp'
import female3 from '../../assets/startup/female-slot-3.webp'
import female4 from '../../assets/startup/female-slot-4.webp'

export type BodyType = 'a' | 'b'

export interface Preset {
  look: AvatarLook
  picture: string
}

const MALE = 'urn:decentraland:off-chain:base-avatars:BaseMale'
const FEMALE = 'urn:decentraland:off-chain:base-avatars:BaseFemale'

/** Body type A (male) and B (female), four slots each. */
export const PRESETS: Record<BodyType, Preset[]> = {
  a: [
  {
    picture: male1,
    look: {
      bodyShape: MALE,
      wearables: [
        'urn:decentraland:off-chain:base-avatars:crocs',
        'urn:decentraland:off-chain:base-avatars:f_short_colored_leggins',
        'urn:decentraland:off-chain:base-avatars:mouth_07',
        'urn:decentraland:off-chain:base-avatars:eyes_21',
        'urn:decentraland:off-chain:base-avatars:modern_hair',
        'urn:decentraland:off-chain:base-avatars:yellow_tshirt',
        'urn:decentraland:off-chain:base-avatars:square_earring',
        'urn:decentraland:off-chain:base-avatars:dcl_watch',
        'urn:decentraland:off-chain:base-avatars:retro_sunglasses'
      ],
      eyes: { r: 0.231373, g: 0.141176, b: 0.05098 },
      hair: { r: 0.85098, g: 0.313725, b: 0.109804 },
      skin: { r: 0.764706, g: 0.54902, b: 0.443137 },
      emotes: [],
      forceRender: []
    }
  },
  {
    picture: male2,
    look: {
      bodyShape: MALE,
      wearables: [
        'urn:decentraland:off-chain:base-avatars:f_eyes_02',
        'urn:decentraland:off-chain:base-avatars:f_mouth_02',
        'urn:decentraland:off-chain:base-avatars:curtained_hair',
        'urn:decentraland:off-chain:base-avatars:classic_shoes',
        'urn:decentraland:off-chain:base-avatars:cord_bracelet',
        'urn:decentraland:off-chain:base-avatars:toruspiercing',
        'urn:decentraland:off-chain:base-avatars:green_square_shirt',
        'urn:decentraland:off-chain:base-avatars:f_short_blue_jeans'
      ],
      eyes: { r: 0.231373, g: 0.141176, b: 0.05098 },
      hair: { r: 0.035294, g: 0.262745, b: 0.007843 },
      skin: { r: 0.556863, g: 0.439216, b: 0.313725 },
      emotes: [],
      forceRender: []
    }
  },
  {
    picture: male3,
    look: {
      bodyShape: MALE,
      wearables: [
        'urn:decentraland:off-chain:base-avatars:semi_afro',
        'urn:decentraland:off-chain:base-avatars:eyes_01',
        'urn:decentraland:off-chain:base-avatars:lincoln_beard',
        'urn:decentraland:off-chain:base-avatars:mouth_05',
        'urn:decentraland:off-chain:base-avatars:sport_jacket',
        'urn:decentraland:off-chain:base-avatars:grey_joggers',
        'urn:decentraland:off-chain:base-avatars:punk_piercing',
        'urn:decentraland:off-chain:base-avatars:sport_colored_shoes'
      ],
      eyes: { r: 0.231373, g: 0.141176, b: 0.05098 },
      hair: { r: 0.231373, g: 0.129412, b: 0.090196 },
      skin: { r: 0.415686, g: 0.286275, b: 0.176471 },
      emotes: [],
      forceRender: []
    }
  },
  {
    picture: male4,
    look: {
      bodyShape: MALE,
      wearables: [
        'urn:decentraland:off-chain:base-avatars:f_eyes_08',
        'urn:decentraland:off-chain:base-avatars:mouth_05',
        'urn:decentraland:off-chain:base-avatars:full_beard',
        'urn:decentraland:off-chain:base-avatars:punk',
        'urn:decentraland:off-chain:base-avatars:eyebrows_12',
        'urn:decentraland:off-chain:base-avatars:aviatorstyle',
        'urn:decentraland:off-chain:base-avatars:black_glove',
        'urn:decentraland:off-chain:base-avatars:sleeveless_punk_shirt',
        'urn:decentraland:off-chain:base-avatars:distressed_black_jeans',
        'urn:decentraland:off-chain:base-avatars:citycomfortableshoes',
        'urn:decentraland:off-chain:base-avatars:punk_piercing'
      ],
      eyes: { r: 0.168627, g: 0.035294, b: 0.019608 },
      hair: { r: 0.141176, g: 0.141176, b: 0.137255 },
      skin: { r: 0.886275, g: 0.713725, b: 0.564706 },
      emotes: [],
      forceRender: []
    }
  }
  ],
  b: [
  {
    picture: female1,
    look: {
      bodyShape: FEMALE,
      wearables: [
        'urn:decentraland:off-chain:base-avatars:eyebrows_01',
        'urn:decentraland:off-chain:base-avatars:mouth_02',
        'urn:decentraland:off-chain:base-avatars:eyes_08',
        'urn:decentraland:off-chain:base-avatars:f_m_sandals',
        'urn:decentraland:off-chain:base-avatars:pony_tail',
        'urn:decentraland:off-chain:base-avatars:brown_sleveless_dress',
        'urn:decentraland:off-chain:base-avatars:kilt',
        'urn:decentraland:off-chain:base-avatars:pearls_earring',
        'urn:decentraland:off-chain:base-avatars:cord_bracelet'
      ],
      eyes: { r: 0.086275, g: 0.117647, b: 0.376471 },
      hair: { r: 0.945098, g: 0.745098, b: 0.317647 },
      skin: { r: 0.886275, g: 0.713725, b: 0.564706 },
      emotes: [],
      forceRender: []
    }
  },
  {
    picture: female2,
    look: {
      bodyShape: FEMALE,
      wearables: [
        'urn:decentraland:off-chain:base-avatars:cord_bracelet',
        'urn:decentraland:off-chain:base-avatars:hair_anime_01',
        'urn:decentraland:off-chain:base-avatars:f_eyes_08',
        'urn:decentraland:off-chain:base-avatars:blue_star_earring',
        'urn:decentraland:off-chain:base-avatars:school_shirt',
        'urn:decentraland:off-chain:base-avatars:f_school_skirt',
        'urn:decentraland:off-chain:base-avatars:ruby_blue_loafer',
        'urn:decentraland:off-chain:base-avatars:eyebrows_09',
        'urn:decentraland:off-chain:base-avatars:f_mouth_05'
      ],
      eyes: { r: 0.168627, g: 0.035294, b: 0.019608 },
      hair: { r: 0.141176, g: 0.141176, b: 0.137255 },
      skin: { r: 0.886275, g: 0.713725, b: 0.564706 },
      emotes: [],
      forceRender: []
    }
  },
  {
    picture: female3,
    look: {
      bodyShape: FEMALE,
      wearables: [
        'urn:decentraland:off-chain:base-avatars:cord_bracelet',
        'urn:decentraland:off-chain:base-avatars:hair_anime_01',
        'urn:decentraland:off-chain:base-avatars:eyebrows_09',
        'urn:decentraland:off-chain:base-avatars:mouth_02',
        'urn:decentraland:off-chain:base-avatars:pink_gem_earring',
        'urn:decentraland:off-chain:base-avatars:f_red_elegant_jacket',
        'urn:decentraland:off-chain:base-avatars:ruby_red_loafer',
        'urn:decentraland:off-chain:base-avatars:f_capris',
        'urn:decentraland:off-chain:base-avatars:eyes_15'
      ],
      eyes: { r: 0.168627, g: 0.035294, b: 0.019608 },
      hair: { r: 0.435294, g: 0.003922, b: 0.039216 },
      skin: { r: 0.501961, g: 0.333333, b: 0.188235 },
      emotes: [],
      forceRender: []
    }
  },
  {
    picture: female4,
    look: {
      bodyShape: FEMALE,
      wearables: [
        'urn:decentraland:off-chain:base-avatars:f_eyebrows_07',
        'urn:decentraland:off-chain:base-avatars:f_mouth_08',
        'urn:decentraland:off-chain:base-avatars:eyes_01',
        'urn:decentraland:off-chain:base-avatars:slicked_hair',
        'urn:decentraland:off-chain:base-avatars:f_skull_earring',
        'urn:decentraland:off-chain:base-avatars:black_sun_glasses',
        'urn:decentraland:off-chain:base-avatars:cord_bracelet',
        'urn:decentraland:off-chain:base-avatars:citycomfortableshoes',
        'urn:decentraland:off-chain:base-avatars:trash_jean',
        'urn:decentraland:off-chain:base-avatars:black_top'
      ],
      eyes: { r: 0.262745, g: 0.262745, b: 0.262745 },
      hair: { r: 0.180392, g: 0.172549, b: 0.172549 },
      skin: { r: 0.772549, g: 0.509804, b: 0.282353 },
      emotes: [],
      forceRender: []
    }
  }
  ]
}
