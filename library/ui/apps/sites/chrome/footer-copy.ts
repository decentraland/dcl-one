// Shared footer copy used by Sites pages and the download confirmation.
const en = {
  newsletter: {
    title: 'Get the weekly highlights in your inbox',
    placeholder: 'Enter Your Email',
    cta: 'Subscribe',
    emailLabel: 'Email',
    sending: 'Sending…',
    subscribed: 'Subscribed!',
    invalidEmail: 'Enter a valid email address.',
    failed: 'Something went wrong. Please try again.'
  },
  connect: 'Connect',
  menu: 'Menu',
  getting_started: {
    title: 'Getting Started',
    what_is: 'About Decentraland',
    download: 'Download Decentraland',
    system_requirements: 'System Requirements',
    faqs: 'FAQs',
    contact_support: 'Contact Support'
  },
  resources: {
    title: 'Resources',
    marketplace: 'Shop',
    creator_hub: 'Creator Hub',
    docs: 'Docs',
    blog: 'Blog',
    vote: 'Vote'
  },
  language: {
    change: 'Change language',
    label: 'Language'
  },
  legal: {
    privacy: 'Privacy Policy',
    terms: 'Terms of Use',
    content: 'Content Policy',
    ethics: 'Code of Ethics'
  }
}

export type SitesFooterCopy = typeof en

export const sitesFooterCopy = {
  en,
  es: {
    newsletter: {
      title: 'Recibí las novedades en tu inbox',
      placeholder: 'Ingresá tu email',
      cta: 'Suscribirse',
      emailLabel: 'Correo electrónico',
      sending: 'Enviando…',
      subscribed: '¡Te suscribiste!',
      invalidEmail: 'Ingresá un correo electrónico válido.',
      failed: 'No se pudo completar la suscripción. Volvé a intentarlo.'
    },
    connect: 'Conectar',
    menu: 'Menú',
    getting_started: {
      title: 'Primeros pasos',
      what_is: 'Acerca de Decentraland',
      download: 'Descargar Decentraland',
      system_requirements: 'Requisitos del sistema',
      faqs: 'Preguntas frecuentes',
      contact_support: 'Contactar soporte'
    },
    resources: {
      title: 'Recursos',
      marketplace: 'Comprar',
      creator_hub: 'Creator Hub',
      docs: 'Documentación',
      blog: 'Blog',
      vote: 'Votar'
    },
    language: {
      change: 'Cambiar idioma',
      label: 'Idioma'
    },
    legal: {
      privacy: 'Política de privacidad',
      terms: 'Términos de uso',
      content: 'Política de contenido',
      ethics: 'Código de ética'
    }
  },
  fr: {
    newsletter: {
      title: 'Recevez les dernières nouvelles',
      placeholder: 'Entrez votre email',
      cta: "S'abonner",
      emailLabel: 'Adresse e-mail',
      sending: 'Envoi…',
      subscribed: 'Vous êtes abonné !',
      invalidEmail: 'Saisissez une adresse e-mail valide.',
      failed: 'Une erreur est survenue. Réessayez.'
    },
    connect: 'Connecter',
    menu: 'Menu',
    getting_started: {
      title: 'Pour commencer',
      what_is: 'À propos de Decentraland',
      download: 'Télécharger Decentraland',
      system_requirements: 'Configuration requise',
      faqs: 'FAQ',
      contact_support: 'Contacter le support'
    },
    resources: {
      title: 'Ressources',
      marketplace: 'Marketplace',
      creator_hub: 'Creator Hub',
      docs: 'Documentation',
      blog: 'Blog',
      vote: 'Voter'
    },
    language: {
      change: 'Changer de langue',
      label: 'Langue'
    },
    legal: {
      privacy: 'Politique de confidentialité',
      terms: 'Conditions d’utilisation',
      content: 'Politique de contenu',
      ethics: 'Code d’éthique'
    }
  },
  zh: {
    newsletter: {
      title: '订阅精选资讯',
      placeholder: '输入您的邮箱',
      cta: '订阅',
      emailLabel: '电子邮箱',
      sending: '正在发送…',
      subscribed: '订阅成功！',
      invalidEmail: '请输入有效的电子邮箱地址。',
      failed: '订阅失败，请重试。'
    },
    connect: '关注我们',
    menu: '菜单',
    getting_started: {
      title: '快速入门',
      what_is: '关于 Decentraland',
      download: '下载 Decentraland',
      system_requirements: '系统要求',
      faqs: '常见问题',
      contact_support: '联系支持'
    },
    resources: {
      title: '资源',
      marketplace: '市场',
      creator_hub: '创作者中心',
      docs: '文档',
      blog: '博客',
      vote: '投票'
    },
    language: {
      change: '更改语言',
      label: '语言'
    },
    legal: {
      privacy: '隐私政策',
      terms: '使用条款',
      content: '内容政策',
      ethics: '道德准则'
    }
  },
  ko: {
    newsletter: {
      title: '주요 소식을 받아보세요',
      placeholder: '이메일을 입력하세요',
      cta: '구독',
      emailLabel: '이메일 주소',
      sending: '전송 중…',
      subscribed: '구독이 완료됐어요!',
      invalidEmail: '유효한 이메일 주소를 입력하세요.',
      failed: '구독을 완료하지 못했어요. 다시 시도해 주세요.'
    },
    connect: '팔로우',
    menu: '메뉴',
    getting_started: {
      title: '시작하기',
      what_is: 'Decentraland 소개',
      download: 'Decentraland 다운로드',
      system_requirements: '시스템 요구 사항',
      faqs: '자주 묻는 질문',
      contact_support: '고객 지원'
    },
    resources: {
      title: '리소스',
      marketplace: '마켓플레이스',
      creator_hub: '크리에이터 허브',
      docs: '문서',
      blog: '블로그',
      vote: '투표'
    },
    language: {
      change: '언어 변경',
      label: '언어'
    },
    legal: {
      privacy: '개인정보 처리방침',
      terms: '이용약관',
      content: '콘텐츠 정책',
      ethics: '윤리 강령'
    }
  },
  ja: {
    newsletter: {
      title: '最新情報をお届けします',
      placeholder: 'メールアドレスを入力',
      cta: '登録',
      emailLabel: 'メールアドレス',
      sending: '送信中…',
      subscribed: '登録が完了しました！',
      invalidEmail: '有効なメールアドレスを入力してください。',
      failed: '登録できませんでした。もう一度お試しください。'
    },
    connect: 'フォロー',
    menu: 'メニュー',
    getting_started: {
      title: 'はじめに',
      what_is: 'Decentralandについて',
      download: 'Decentralandをダウンロード',
      system_requirements: 'システム要件',
      faqs: 'よくある質問',
      contact_support: 'サポートに連絡'
    },
    resources: {
      title: 'リソース',
      marketplace: 'マーケットプレイス',
      creator_hub: 'クリエイターハブ',
      docs: 'ドキュメント',
      blog: 'ブログ',
      vote: '投票'
    },
    language: {
      change: '言語を変更',
      label: '言語'
    },
    legal: {
      privacy: 'プライバシーポリシー',
      terms: '利用規約',
      content: 'コンテンツポリシー',
      ethics: '倫理規範'
    }
  }
} satisfies Record<string, SitesFooterCopy>
