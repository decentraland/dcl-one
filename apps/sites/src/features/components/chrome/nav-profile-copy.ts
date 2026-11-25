import type { SitesLocale, NavBarLabels } from "@dcl/one-ui";

type NavProfileCopy = {
  labels: Pick<
    NavBarLabels,
    | "account"
    | "userId"
    | "copyUserId"
    | "userIdCopied"
    | "copyUserIdError"
    | "credits"
    | "mana"
    | "polygonMana"
  >;
  actions: {
    viewProfile: string;
    myAssets: string;
    accountSettings: string;
    shopAuthorizations: string;
    logout: string;
    switchAccount: string;
  };
  walletNetwork: string;
};

export const navProfileCopy = {
  en: {
    labels: {
      account: (name) => `Account: ${name}`,
      userId: "User ID",
      copyUserId: "Copy user ID",
      userIdCopied: "User ID copied",
      copyUserIdError: "Could not copy user ID",
      credits: "Credits",
      mana: "MANA",
      polygonMana: "Polygon MANA",
    },
    actions: {
      viewProfile: "View Profile",
      myAssets: "My Assets",
      accountSettings: "Account Settings",
      shopAuthorizations: "Shop Authorizations",
      logout: "Logout",
      switchAccount: "Switch account",
    },
    walletNetwork: "Wallet network",
  },
  es: {
    labels: {
      account: (name) => `Cuenta: ${name}`,
      userId: "ID de usuario",
      copyUserId: "Copiar ID de usuario",
      userIdCopied: "ID de usuario copiado",
      copyUserIdError: "No se pudo copiar el ID de usuario",
      credits: "Créditos",
      mana: "MANA",
      polygonMana: "MANA en Polygon",
    },
    actions: {
      viewProfile: "Ver perfil",
      myAssets: "Mis activos",
      accountSettings: "Configuración de la cuenta",
      shopAuthorizations: "Autorizaciones de la tienda",
      logout: "Cerrar sesión",
      switchAccount: "Cambiar de cuenta",
    },
    walletNetwork: "Red de la billetera",
  },
  fr: {
    labels: {
      account: (name) => `Compte : ${name}`,
      userId: "Identifiant utilisateur",
      copyUserId: "Copier l’identifiant utilisateur",
      userIdCopied: "Identifiant utilisateur copié",
      copyUserIdError: "Impossible de copier l’identifiant utilisateur",
      credits: "Crédits",
      mana: "MANA",
      polygonMana: "MANA sur Polygon",
    },
    actions: {
      viewProfile: "Voir le profil",
      myAssets: "Mes actifs",
      accountSettings: "Paramètres du compte",
      shopAuthorizations: "Autorisations de la boutique",
      logout: "Se déconnecter",
      switchAccount: "Changer de compte",
    },
    walletNetwork: "Réseau du portefeuille",
  },
  zh: {
    labels: {
      account: (name) => `账户：${name}`,
      userId: "用户 ID",
      copyUserId: "复制用户 ID",
      userIdCopied: "已复制用户 ID",
      copyUserIdError: "无法复制用户 ID",
      credits: "积分",
      mana: "MANA",
      polygonMana: "Polygon 上的 MANA",
    },
    actions: {
      viewProfile: "查看个人资料",
      myAssets: "我的资产",
      accountSettings: "账户设置",
      shopAuthorizations: "商店授权",
      logout: "退出登录",
      switchAccount: "切换账户",
    },
    walletNetwork: "钱包网络",
  },
  ko: {
    labels: {
      account: (name) => `계정: ${name}`,
      userId: "사용자 ID",
      copyUserId: "사용자 ID 복사",
      userIdCopied: "사용자 ID를 복사했습니다",
      copyUserIdError: "사용자 ID를 복사할 수 없습니다",
      credits: "크레딧",
      mana: "MANA",
      polygonMana: "Polygon의 MANA",
    },
    actions: {
      viewProfile: "프로필 보기",
      myAssets: "내 자산",
      accountSettings: "계정 설정",
      shopAuthorizations: "상점 승인",
      logout: "로그아웃",
      switchAccount: "계정 전환",
    },
    walletNetwork: "지갑 네트워크",
  },
  ja: {
    labels: {
      account: (name) => `アカウント：${name}`,
      userId: "ユーザーID",
      copyUserId: "ユーザーIDをコピー",
      userIdCopied: "ユーザーIDをコピーしました",
      copyUserIdError: "ユーザーIDをコピーできませんでした",
      credits: "クレジット",
      mana: "MANA",
      polygonMana: "Polygon の MANA",
    },
    actions: {
      viewProfile: "プロフィールを表示",
      myAssets: "マイアセット",
      accountSettings: "アカウント設定",
      shopAuthorizations: "ショップの承認",
      logout: "ログアウト",
      switchAccount: "アカウントを切り替える",
    },
    walletNetwork: "ウォレットネットワーク",
  },
} satisfies Record<SitesLocale, NavProfileCopy>;
