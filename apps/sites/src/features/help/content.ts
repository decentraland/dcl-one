/**
 * The /help copy, in English exactly as decentraland/sites `component.landing.help.*` (src/intl/en.json) has it.
 * Sites in this repository has no translation layer yet.
 */

export type HelpQuestion = { readonly question: string; readonly answer: string };

export const HELP_COPY = {
  pageTitle: "Decentraland - Help Center",
  title: "Help Center",
  description: "Browse through FAQs and Support Updates or speak with the Support Team directly using the chat box on the bottom right.",
  tabs: {
    faq: "FAQ",
    creatorFund: "CREATOR FUND",
    supportUpdates: "SUPPORT UPDATES",
  },
  faq: {
    title: "Frequently Asked Questions",
    paragraph: "Don't see your question below? Check out our full FAQ resources",
    ctaLink: "here",
  },
  creatorFund: {
    title: "Creator Fund",
    paragraph: "Answers about the Creator Fund Beta Season.",
  },
  supportUpdates: {
    title: "Support Updates",
  },
  status: {
    title: "System Status",
    details: "View Details",
    ok: "All Operational",
    down: "Services Shutdown",
    slow: "Experiencing Problems",
  },
  chat: {
    title: "Need Help?",
    subtitle: "Our support team is here to answer your questions!",
    button: "CHAT NOW",
  },
} as const;

/** Where the FAQ section's "here" link points. */
export const FULL_FAQ_URL = "https://docs.decentraland.org/faqs/decentraland-101";

export const HELP_FAQ: readonly HelpQuestion[] = [
  {
    question: "How do I enter Decentraland?",
    answer:
      "The first step is downloading the Decentraland app onto your computer at decentraland.org/download. Once installed, log into your Decentraland account by connecting your Google, Discord, or another social profile, or a digital wallet such as MetaMask or Coinbase. Once online verification is complete, open the Decentraland app and click 'Jump Into Decentraland'. From here you can customize your avatar, explore the community-built world, or attend an event and make some friends!",
  },
  {
    question: "Do I need crypto or a digital wallet to use Decentraland?",
    answer:
      "No, you do not need to own crypto or already have a digital wallet to use Decentraland. Decentraland is free to use, and if you'd like to purchase something from the Marketplace, you can use a credit/debit card in addition to cryptocurrency. If you sign into Decentraland with a social account such as Google or Discord, a digital wallet will be made for your account behind the scenes, so you don't have to worry about anything.",
  },
  {
    question: "What hardware do I need to run Decentraland?",
    answer:
      "Currently, Decentraland is available for PC on Windows and Mac. For Windows, you'll need at least Windows 10 64-bit, an Intel i5 7th gen or AMD Ryzen 5 CPU, an Nvidia RTX 20 Series or AMD Radeon RX 5000 Series GPU, 6 GB VRAM, 16 GB RAM, and 8 GB storage. For Mac, you'll need at least macOS 11 Big Sur with an Apple M1 chip, 16 GB RAM, and 8 GB storage. Decentraland is also available on mobile — download it on Android and iOS from the App Store or Google Play Store. For full hardware specs, visit docs.decentraland.org/faqs/decentraland-101.",
  },
  {
    question: "How do I explore and navigate Decentraland?",
    answer:
      "Decentraland consists of Genesis City, an open traversable world made up of community parcels referenced by coordinates, as well as individual Worlds, more intimate 3D spaces that can be teleported into. To explore Genesis City, use your arrow or WASD keys to move around, jump to locations from the map, or type '/goto x,y' in the chat to teleport to specific coordinates. To visit a World, type '/goto WorldName' in the chat.",
  },
  {
    question: "How do I get Wearables, Emotes, LAND, and NAMEs in the Marketplace?",
    answer:
      "Browse the Marketplace at decentraland.org/marketplace for hundreds of community-made Wearables and Emotes to customize your digital identity, buy or rent Genesis City LAND parcels, or claim your unique NAME which comes with its own World. You can pay by card or with a variety of cryptocurrencies. Anything you purchase is truly owned by you, with ownership registered on the blockchain. You can resell items whenever you wish.",
  },
  {
    question: "How can I meet people in Decentraland?",
    answer:
      "The best way to meet new people in Decentraland is to attend events and start chatting! Browse the event page at decentraland.org/events for current and upcoming events and don't be shy — Decentraland's community is known for being welcoming. Keep an eye out for weekly tours and meetups, such as ABC DCL - Adventures of Decentraland, which are specifically targeted at new community members!",
  },
  {
    question: "How can I learn about in-world events?",
    answer:
      "Decentraland's Event Page at decentraland.org/events is the official place where anyone in the community can post their in-world events. To stay on top of events, check the event page regularly, follow Decentraland on social channels such as X, or subscribe to the weekly newsletter at decentraland.beehiiv.com to learn about upcoming activities!",
  },
  {
    question: "How do I take pictures in-world?",
    answer:
      "Press C on your keyboard to open the Camera. A helpful guide with camera controls will appear on the bottom right of your screen. Your Gallery has space for up to 500 photos, from which you can easily share your images with a link or download them onto your computer. You can feature your favorite photos on your Decentraland Profile by toggling the 'Set as Public' option.",
  },
  {
    question: "How can I get involved in Decentraland governance?",
    answer:
      "Decentraland's DAO is the heart of the community-driven world's governance. To get started, read through the DAO forums at decentraland.org/governance to learn about current issues, see what the community consensus is, and add your own comments. Learn more on the DAO's official page at decentraland.org/dao.",
  },
];

export const HELP_SUPPORT_UPDATES: readonly HelpQuestion[] = [
  {
    question: "Attention Creators: Backface Culling Updates",
    answer:
      "Backface Culling is now set to On on the engine by default on all model materials.\n\nWhy: Significant Performance Optimization.\nHow: Because it is an engine optimization, it also affects materials that are already published.\n\nAction Points:\nWe encourage you to check your scenes following the Troubleshooting section in the documentation.\n\nSome scenarios worth checking first:\n- Flags, curtains, walls made of Planes: If a wall made of a plane with a single-sided material, it won't be seen from the other side.\n- Grass / Leaves of a tree made of Planes with single-sided materials: Same case as above.\n- Interior of Cubes and other shapes that act as rooms: If the cube material is facing out, it won't be seen from the inside.",
  },
  {
    question: "Visual Studio Extension - Support Ending",
    answer:
      "With the focus on the Creator Hub, the Visual Studio Extension is no longer supported.\n\nThe Extension has been removed from the VS Marketplace.",
  },
  {
    question: "Player.logs - How to get them",
    answer:
      "Below is a guide on how to find and generate the Player.log file. These files can be helpful when it comes to troubleshooting issues reported.\n\nOpen the chat in the Decentraland Desktop app and type: /logs\n\nOr:\n\nFor Windows:\nNavigate to: C:\\Users\\{YourUserName}\\AppData\\LocalLow\\Decentraland\\Explorer\nFind the file named Player.log\n\nFor Mac:\nOpen Finder and press Cmd + Shift + G to open the \"Go to Folder\" dialog.\nEnter the path: /Users/UserName/Library/Logs/Decentraland/Explorer/\nFind the file named Player.log\n\nPlayer.log file would show issues with the current/active session.\nPlayer-prev.log file would show issues with the previous session.\n\nLauncher Logs (output.log):\nWindows: C:\\Users\\username\\AppData\\Roaming\\DecentralandLauncherLight\nMac: /Users/UserName/Library/Logs/DecentralandLauncherLight",
  },
];
