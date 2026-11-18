// Content of the overview page that is not copy: asset URLs, destinations, ids. Every id doubles as the
// i18n key under `overview.*`, so the copy for an entry lives in en.json / es.json.
import { config } from '~/config'
import dclLogo from '~/assets/overview/dcl-logo.svg'

const sitesUrl = config.get('SITES_URL')
const docsUrl = config.get('DOCS_URL')
const discordUrl = config.get('DISCORD_URL')
const shopUrl = config.get('SHOP_URL')

const CONTENTFUL_IMAGES = 'https://images.ctfassets.net/ea2ybdmmn1kv'
const CONTENTFUL_VIDEOS = 'https://videos.ctfassets.net/ea2ybdmmn1kv'

export const CREATOR_DOCS_URL = `${docsUrl}/creator/`
export const CREATOR_HUB_DOWNLOAD_URL = `${sitesUrl}/download/creator-hub`
export const YOUTUBE_CHANNEL_URL = 'https://www.youtube.com/@decentraland_foundation/videos'
export const SUBMIT_TUTORIAL_URL =
  'https://docs.google.com/forms/d/e/1FAIpQLScn1HL8_ZFL_Lw-7sbqL9g4WLctWALWUKthGEtjBGQlmIuHLQ/viewform'
export const FAQS_URL = `${docsUrl}/faqs/`
export const DISCORD_URL = discordUrl

type Media = { url: string; width: number; height: number }

export const heroData = {
  words: ['wearables', 'emotes', 'worlds', 'experiences', 'scenes', 'games'] as const,
  /** First frame of the video, so the fade-in lines up; phones crop it to fill the hero. Served with the app. */
  poster: {
    url: `${import.meta.env.BASE_URL}overview/hero-1920.webp`,
    srcSet: `${import.meta.env.BASE_URL}overview/hero-1280.webp 1280w, ${import.meta.env.BASE_URL}overview/hero-1920.webp 1920w`,
    width: 1920,
    height: 1080
  },
  video: {
    url: `${CONTENTFUL_VIDEOS}/5ELJfyKfvgJMlWi3QXzyt7/28d4d5202e965c08eabc3f9efcf329ea/hero-desktop.mp4`,
    width: 1920,
    height: 1080
  } satisfies Media
}

export type WhyCard = {
  id: 'join' | 'create' | 'benefit'
  image: Media
  url: string
  gradient: 'orchid' | 'apricot' | 'raspberry'
}

export const whyCards: WhyCard[] = [
  {
    id: 'join',
    image: {
      url: `${CONTENTFUL_IMAGES}/2l0VUCaHXFG7NwltyZ1nWA/a6e18252e09a9916e8d735f098ef452a/Image_1.png`,
      width: 880,
      height: 466
    },
    url: discordUrl,
    gradient: 'orchid'
  },
  {
    id: 'create',
    image: {
      url: `${CONTENTFUL_IMAGES}/77yzgLkg2oQ7GAZLPtEAwY/64a8d2741a6e3f1f90b15636c4d37638/Image_2.png`,
      width: 880,
      height: 466
    },
    url: CREATOR_DOCS_URL,
    gradient: 'apricot'
  },
  {
    id: 'benefit',
    image: {
      url: `${CONTENTFUL_IMAGES}/3iCBvRrzEtgD7LT8PYxMZn/2f5fba37f044d426d24e25ce33dc1f9c/Image_3.png`,
      width: 880,
      height: 476
    },
    url: `${docsUrl}/creator/wearables-and-emotes/wearables/creating-wearables`,
    gradient: 'raspberry'
  }
]

export type Skill =
  'modeling' | 'fashion' | 'animation' | 'typescript' | 'creator_hub' | 'no_code' | 'click' | 'imagination'

export type CreateLink = {
  id: string
  url: string
  /** The Creator Hub only ships desktop installers, so its download links dead-end on phones and are hidden there. */
  desktopOnly?: boolean
}

export type CreateTab = {
  id: string
  /** The Click `tab` value: sites' English tab title, fixed so the warehouse dimension never moves with the copy. */
  analyticsTab: string
  skills: Skill[]
  links: CreateLink[]
}

export type CreateCard = {
  id: 'wearables' | 'emotes' | 'experiences'
  /** The Click `card` value: sites' card id. */
  analyticsCard: string
  image: string
  background: string
  tabs: CreateTab[]
}

const shopItems = `${shopUrl}/items`
const buildingTutorialsUrl =
  'https://www.youtube.com/watch?v=wm8ZD2kSyKA&list=PLAcRraQmr_GPrMmQekqbMWhyBxo3lXs8p&pp=iAQB'
const downloadHubLink: CreateLink = { id: 'download_hub', url: CREATOR_HUB_DOWNLOAD_URL, desktopOnly: true }

export const createCards: CreateCard[] = [
  {
    id: 'wearables',
    analyticsCard: 'design-unique-wearables',
    image: `${CONTENTFUL_IMAGES}/3Cu7b7wDmHMxdlnV1kPIHK/af575947c309275575ee1f456042596a/PNG_1.png`,
    background: `${CONTENTFUL_IMAGES}/5tQ56AeW3FWfsnDIgIr1ix/1ff51b5358a8ddf13a4bb4bddaf36601/BG_1.png`,
    tabs: [
      {
        id: 'regular',
        analyticsTab: 'Regular',
        skills: ['modeling', 'fashion'],
        links: [
          { id: 'creating', url: `${docsUrl}/creator/wearables-and-emotes/wearables/creating-wearables` },
          { id: 'shop', url: `${shopItems}?category=wearable` },
          {
            id: 'publishing',
            url: `${docsUrl}/creator/wearables-and-emotes/publishing-collections/publishing-collections`
          },
          {
            id: 'tutorials',
            url: 'https://www.youtube.com/watch?v=zl43Fw7zROQ&list=PLEl6fe1igtKBFDcxaC64Uxamo7kQUi5mf&pp=iAQB'
          }
        ]
      },
      {
        id: 'smart',
        analyticsTab: 'Smart Wearables',
        skills: ['animation', 'fashion', 'modeling', 'typescript'],
        links: [
          { id: 'portable_experiences', url: `${docsUrl}/creator/scenes-sdk7/kinds-of-projects/portable-experiences` },
          { id: 'sdk', url: `${docsUrl}/creator/scenes-sdk7/getting-started/sdk-101` },
          { id: 'smart_wearables', url: `${docsUrl}/creator/scenes-sdk7/kinds-of-projects/smart-wearables` },
          { id: 'shop', url: `${shopItems}?category=wearable&smart=true` }
        ]
      }
    ]
  },
  {
    id: 'emotes',
    analyticsCard: 'animate-expressive-emotes',
    image: `${CONTENTFUL_IMAGES}/5TILWmR3rrA6K2DMTrPwXx/aeafb6cfc8fd3ef829833c41fb4cda16/PNG_2.png`,
    background: `${CONTENTFUL_IMAGES}/26Oa8X59NGyCSnL2j2Tvrw/091c890f8a5bfcdf8878a3c09c1268ae/BG_2.png`,
    tabs: [
      {
        id: 'motion',
        analyticsTab: 'More than Motion',
        skills: ['typescript', 'modeling', 'animation'],
        links: [
          { id: 'creating', url: `${docsUrl}/creator/wearables-and-emotes/emotes/creating-emotes` },
          {
            id: 'tutorials',
            url: 'https://www.youtube.com/watch?v=-iWslh4uQIk&list=PLAcRraQmr_GN8LcnnQk2BByo9L2Orvp9c&pp=iAQB'
          },
          {
            id: 'publishing',
            url: `${docsUrl}/creator/wearables-and-emotes/publishing-collections/publishing-collections`
          },
          { id: 'shop', url: `${shopItems}?category=emote` }
        ]
      }
    ]
  },
  {
    id: 'experiences',
    analyticsCard: 'craft-immersive-experiences',
    image: `${CONTENTFUL_IMAGES}/3JXtpqW33ILyBYzrpzykKl/f246d9529cbbc4d033c271cd02d4376b/PNG_3.png`,
    background: `${CONTENTFUL_IMAGES}/5E9WJJcBi3qeuqqetuleeT/6308247b5ca650adf34aaec41d7e7182/BG_3.png`,
    tabs: [
      {
        id: 'scenes',
        analyticsTab: 'Create Scenes',
        skills: ['creator_hub', 'no_code'],
        links: [
          { id: 'about_editor', url: `${docsUrl}/creator/scene-editor/get-started/about-editor` },
          { id: 'publish_scene', url: `${docsUrl}/creator/scene-editor/publish/publish-scene` },
          { id: 'dev_workflow', url: `${docsUrl}/creator/scenes-sdk7/getting-started/dev-workflow` },
          downloadHubLink
        ]
      },
      {
        id: 'basic',
        analyticsTab: 'Basic',
        skills: ['click', 'imagination'],
        links: [
          { id: 'places', url: `${sitesUrl}/places` },
          downloadHubLink,
          { id: 'tutorials', url: buildingTutorialsUrl },
          { id: 'worlds_guide', url: `${sitesUrl}/blog/about-decentraland/decentraland-worlds-your-own-virtual-space` }
        ]
      },
      {
        id: 'advanced',
        analyticsTab: 'Advanced',
        skills: ['typescript', 'modeling', 'animation'],
        links: [
          downloadHubLink,
          { id: 'docs', url: `${docsUrl}/creator/scenes-sdk7/getting-started/sdk-101` },
          { id: 'tutorials', url: buildingTutorialsUrl }
        ]
      }
    ]
  }
]

export type Testimonial = { id: string; name: string; image: string; url: string }

const canessaImage = `${CONTENTFUL_IMAGES}/3ugHhjVpg2TJRKpGrE5HJO/2c267463b5cab7923fedcd2a8912c17e/anessa.png`

export const testimonials: Testimonial[] = [
  {
    id: 'mrdhingia',
    name: 'MrDhingia',
    image: `${CONTENTFUL_IMAGES}/7w2syTuMGjA3VCJZ78YK8q/1a37abd7658ba66e16f1c8d7d4a28333/dhingia.png`,
    url: 'https://x.com/MrDhingia'
  },
  { id: 'canessa', name: 'Canessa', image: canessaImage, url: 'https://x.com/CanessaDCL' },
  {
    id: 'polygonal_mind',
    name: 'Polygonal Mind',
    image: `${CONTENTFUL_IMAGES}/2A10uagc69WkV4Put4Je8K/b5f7d12193e3e7bc6c126c8b1cf574b9/polygonal.png`,
    url: 'https://x.com/polygonalmind'
  },
  {
    id: 'tangpoko',
    name: 'TangPoko',
    image: `${CONTENTFUL_IMAGES}/3VUTEOy199FdbePR0n4v9h/2011425bade058c0b6ceeda046bbc71c/tang.png`,
    url: 'https://x.com/tangpoko'
  },
  {
    id: 'nikki_fuego',
    name: 'Nikki Fuego',
    image: `${CONTENTFUL_IMAGES}/1qNFtMuqQ6NUgVFZxqIMWO/53a6bd692059f897f8df5a99c9570ecf/Nikki.png`,
    url: 'https://x.com/NikkiFuego92'
  }
]

export type LearnCard = { id: string; author: string; authorImage: string; videoId: string; date: string }

const youtube = (videoId: string) => ({
  videoId,
  thumbnail: `https://img.youtube.com/vi/${videoId}/hqdefault.jpg`,
  url: `https://www.youtube.com/watch?v=${videoId}`
})

export const learnVideo = youtube

export const learnCards: LearnCard[] = [
  { id: 'blender_first_hat', author: 'Decentraland', authorImage: dclLogo, videoId: '6Q8FNyjFTxc', date: '2026-02-11' },
  {
    id: 'first_virtual_gallery',
    author: 'Decentraland',
    authorImage: dclLogo,
    videoId: 'HJ_UYh7IUf8',
    date: '2025-10-01'
  },
  {
    id: 'canessa_intro_building',
    author: 'CanessaBuilds',
    authorImage: canessaImage,
    videoId: 'BqvYMWxlcj8',
    date: '2025-09-14'
  },
  {
    id: 'office_hours_custom_components',
    author: 'Decentraland',
    authorImage: dclLogo,
    videoId: 'uVGSrsnmfTw',
    date: '2025-08-11'
  },
  {
    id: 'first_wearable_academy',
    author: 'Decentraland',
    authorImage: dclLogo,
    videoId: 'UjHkVZKSWCc',
    date: '2025-04-15'
  }
]

export const faqIds = ['what_is', 'what_create', 'become_creator', 'monetize', 'need_land', 'need_crypto'] as const
