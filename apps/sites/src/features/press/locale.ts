import type { SitesLocale } from "@ui/web/frames/sites-locale";

export const pressCopy = {
  en: {
    title: "Decentraland Press Kit",
    description:
      "We've put together all sorts of goodies that you're welcome to use when writing and talking about our project. Please direct any questions to ",
    download: "Download Press Kit",
  },
  es: {
    title: "Kit de Prensa de Decentraland",
    description:
      "Hemos preparado todo tipo de recursos que podés usar libremente al escribir y hablar sobre nuestro proyecto. Por favor, dirigí cualquier consulta a ",
    download: "Descargar Kit de Prensa",
  },
  fr: {
    title: "Kit Presse Decentraland",
    description:
      "Nous avons rassemblé toutes sortes de ressources que vous pouvez utiliser librement lorsque vous écrivez ou parlez de notre projet. Veuillez adresser toute question à ",
    download: "Télécharger le Kit de Presse",
  },
  zh: {
    title: "Decentraland 新闻资料包",
    description:
      "我们准备了各种素材，欢迎您在撰写和谈论我们的项目时自由使用。如有任何问题，请联系：",
    download: "下载新闻资料包",
  },
  ko: {
    title: "Decentraland 프레스 키트",
    description:
      "프로젝트에 대해 글을 쓰거나 이야기할 때 자유롭게 사용할 수 있는 다양한 자료를 준비했습니다. 문의 사항은 다음으로 연락해 주세요: ",
    download: "프레스 키트 다운로드",
  },
  ja: {
    title: "Decentralandプレスキット",
    description:
      "プロジェクトについて執筆・発信する際にご自由にお使いいただける素材をご用意しました。ご質問は以下までお問い合わせください：",
    download: "プレスキットをダウンロード",
  },
} satisfies Record<
  SitesLocale,
  { title: string; description: string; download: string }
>;
