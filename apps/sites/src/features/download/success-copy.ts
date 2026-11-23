import type { SitesLocale } from "@dcl/one-ui/apps/sites/chrome/sites-locale";

// Copy from decentraland/sites; only the install page is included.
export const successCopy = {
  en: {
    downloading: "Downloading Decentraland...",
    success: {
      title: "You're almost done!",
      subtitle:
        "Open the file you just downloaded to finish installing Decentraland and start {action}",
      subtitle_action_creating: "creating",
      step: "Step",
      footer_prefix: "Trouble with the download? Let's try that again:",
      started: {
        title: "Your download has started!",
        subtitle: "Follow these 3 steps to install Decentraland and jump in.",
      },
      footer_retry: "Download didn't start? {link}",
      footer_retry_link: "Try again",
      footer_help: "Need help? Go to {link}",
      footer_help_link: "decentraland.org/help",
      event:
        "After the download is complete, go back to the {event} event page so you can jump in. Redirecting in {seconds} seconds.",
      steps: {
        windows: {
          step1: {
            title: "Find your file",
            text: "Click the download icon in your browser toolbar, then open the Decentraland installer (.exe).",
          },
          step2: {
            title: "Install",
            text: "Follow the Setup Wizard and click Next until it finishes. If Windows asks for permission, click Yes.",
          },
          step3: {
            title: "Jump in!",
            text: "Once installed, Decentraland opens on its own. See you in-world!",
          },
        },
        macOS: {
          step1: {
            title: "Find your file",
            text: "Click the download icon in your browser toolbar, then open Decentraland-Installer.dmg.",
          },
          step2: {
            title: "Install",
            text: "Double-click the Decentraland icon in the window that opens. When the macOS security message appears, click Open.",
          },
          step3: {
            title: "Jump in!",
            text: "Once installed, Decentraland opens on its own. See you in-world!",
          },
        },
      },
    },
  },
  es: {
    downloading: "Descargando Decentraland...",
    success: {
      title: "¡Ya casi está!",
      subtitle:
        "Abre el archivo que acabas de descargar para terminar de instalar Decentraland y comenzar a {action}",
      step: "Paso",
      started: {
        title: "¡Tu descarga ha comenzado!",
        subtitle: "Sigue estos 3 pasos para instalar Decentraland y entrar.",
      },
      footer_retry: "¿No empezó la descarga? {link}",
      footer_retry_link: "Inténtalo de nuevo",
      footer_help: "¿Necesitas ayuda? Ve a {link}",
      footer_help_link: "decentraland.org/help",
      event:
        "Después de que se complete la descarga, vuelve a la página del evento {event} para poder entrar. Redireccionando en {seconds} segundos.",
      steps: {
        windows: {
          step1: {
            title: "Encuentra tu archivo",
            text: "Haz clic en el icono de descarga de la barra de herramientas de tu navegador y abre el instalador de Decentraland (.exe).",
          },
          step2: {
            title: "Instala",
            text: "Sigue el Asistente de Instalación y haz clic en Siguiente hasta que termine. Si Windows pide permiso, haz clic en Sí.",
          },
          step3: {
            title: "¡Entra!",
            text: "Una vez instalado, Decentraland se abre solo. ¡Nos vemos dentro!",
          },
        },
        macOS: {
          step1: {
            title: "Encuentra tu archivo",
            text: "Haz clic en el icono de descarga de la barra de herramientas de tu navegador y abre Decentraland-Installer.dmg.",
          },
          step2: {
            title: "Instala",
            text: "Haz doble clic en el icono de Decentraland en la ventana que se abre. Cuando aparezca el mensaje de seguridad de macOS, haz clic en Abrir.",
          },
          step3: {
            title: "¡Entra!",
            text: "Una vez instalado, Decentraland se abre solo. ¡Nos vemos dentro!",
          },
        },
      },
    },
  },
  fr: {
    downloading: "Téléchargement de Decentraland...",
    success: {
      title: "Vous y êtes presque !",
      subtitle:
        "Ouvrez le fichier que vous venez de télécharger pour terminer l'installation de Decentraland et commencer à {action}",
      step: "Étape",
      started: {
        title: "Votre téléchargement a commencé !",
        subtitle:
          "Suivez ces 3 étapes pour installer Decentraland et vous lancer.",
      },
      footer_retry: "Le téléchargement n'a pas démarré ? {link}",
      footer_retry_link: "Réessayer",
      footer_help: "Besoin d'aide ? Rendez-vous sur {link}",
      footer_help_link: "decentraland.org/help",
      event:
        "Une fois le téléchargement terminé, retournez sur la page de l'événement {event} pour plonger. Redirection dans {seconds} secondes.",
      steps: {
        windows: {
          step1: {
            title: "Trouvez votre fichier",
            text: "Cliquez sur l'icône de téléchargement dans la barre d'outils de votre navigateur, puis ouvrez le programme d'installation de Decentraland (.exe).",
          },
          step2: {
            title: "Installez",
            text: "Suivez l'assistant d'installation et cliquez sur Suivant jusqu'à la fin. Si Windows demande une autorisation, cliquez sur Oui.",
          },
          step3: {
            title: "C'est parti !",
            text: "Une fois installé, Decentraland s'ouvre tout seul. À très vite dans Decentraland !",
          },
        },
        macOS: {
          step1: {
            title: "Trouvez votre fichier",
            text: "Cliquez sur l'icône de téléchargement dans la barre d'outils de votre navigateur, puis ouvrez Decentraland-Installer.dmg.",
          },
          step2: {
            title: "Installez",
            text: "Double-cliquez sur l'icône Decentraland dans la fenêtre qui s'ouvre. Lorsque le message de sécurité de macOS apparaît, cliquez sur Ouvrir.",
          },
          step3: {
            title: "C'est parti !",
            text: "Une fois installé, Decentraland s'ouvre tout seul. À très vite dans Decentraland !",
          },
        },
      },
    },
  },
  zh: {
    downloading: "正在下载 Decentraland...",
    success: {
      title: "即将完成！",
      subtitle: "打开刚下载的文件完成 Decentraland 安装并开始{action}",
      step: "步骤",
      started: {
        title: "下载已开始！",
        subtitle: "按照以下 3 个步骤安装 Decentraland，马上进入。",
      },
      footer_retry: "下载没有开始？{link}",
      footer_retry_link: "重试",
      footer_help: "需要帮助？请访问 {link}",
      footer_help_link: "decentraland.org/help",
      event:
        "下载完成后，返回 {event} 活动页面即可进入。{seconds} 秒后自动跳转。",
      steps: {
        windows: {
          step1: {
            title: "找到文件",
            text: "点击浏览器工具栏中的下载图标，然后打开 Decentraland 安装程序（.exe）。",
          },
          step2: {
            title: "安装",
            text: "按照安装向导操作，点击“下一步”直至完成。如果 Windows 请求权限，请点击“是”。",
          },
          step3: {
            title: "进入世界！",
            text: "安装完成后，Decentraland 会自动打开。我们在世界里见！",
          },
        },
        macOS: {
          step1: {
            title: "找到文件",
            text: "点击浏览器工具栏中的下载图标，然后打开 Decentraland-Installer.dmg。",
          },
          step2: {
            title: "安装",
            text: "在打开的窗口中双击 Decentraland 图标。出现 macOS 安全提示时，点击“打开”。",
          },
          step3: {
            title: "进入世界！",
            text: "安装完成后，Decentraland 会自动打开。我们在世界里见！",
          },
        },
      },
    },
  },
  ko: {
    downloading: "Decentraland 다운로드 중...",
    success: {
      title: "거의 다 됐어요!",
      subtitle:
        "방금 다운로드한 파일을 열어 Decentraland 설치를 완료하고 {action}을 시작하세요",
      step: "단계",
      started: {
        title: "다운로드가 시작되었습니다!",
        subtitle: "아래 3단계를 따라 Decentraland를 설치하고 바로 입장하세요.",
      },
      footer_retry: "다운로드가 시작되지 않았나요? {link}",
      footer_retry_link: "다시 시도",
      footer_help: "도움이 필요하신가요? 다음 페이지로 이동하세요: {link}",
      footer_help_link: "decentraland.org/help",
      event:
        "다운로드가 완료되면 {event} 이벤트 페이지로 돌아가서 입장하세요. {seconds}초 후 리디렉션됩니다.",
      steps: {
        windows: {
          step1: {
            title: "파일 찾기",
            text: "브라우저 도구 모음의 다운로드 아이콘을 클릭한 다음 Decentraland 설치 프로그램(.exe)을 여세요.",
          },
          step2: {
            title: "설치",
            text: "설치 마법사에서 완료될 때까지 '다음'을 클릭하세요. Windows에서 권한을 요청하면 '예'를 클릭하세요.",
          },
          step3: {
            title: "입장하세요!",
            text: "설치가 끝나면 Decentraland가 자동으로 열립니다. 월드에서 만나요!",
          },
        },
        macOS: {
          step1: {
            title: "파일 찾기",
            text: "브라우저 도구 모음의 다운로드 아이콘을 클릭한 다음 Decentraland-Installer.dmg를 여세요.",
          },
          step2: {
            title: "설치",
            text: "열린 창에서 Decentraland 아이콘을 더블클릭하세요. macOS 보안 메시지가 나타나면 '열기'를 클릭하세요.",
          },
          step3: {
            title: "입장하세요!",
            text: "설치가 끝나면 Decentraland가 자동으로 열립니다. 월드에서 만나요!",
          },
        },
      },
    },
  },
  ja: {
    downloading: "Decentralandをダウンロード中...",
    success: {
      title: "もう少しで完了です！",
      subtitle:
        "ダウンロードしたファイルを開いてDecentralandのインストールを完了し、{action}を始めましょう",
      step: "ステップ",
      started: {
        title: "ダウンロードを開始しました！",
        subtitle:
          "次の3ステップでDecentralandをインストールして、世界に飛び込みましょう。",
      },
      footer_retry: "ダウンロードが始まりませんか？{link}",
      footer_retry_link: "もう一度試す",
      footer_help: "お困りですか？{link}をご覧ください",
      footer_help_link: "decentraland.org/help",
      event:
        "ダウンロード完了後、{event}のイベントページに戻って入場してください。{seconds}秒後にリダイレクトされます。",
      steps: {
        windows: {
          step1: {
            title: "ファイルを探す",
            text: "ブラウザのツールバーにあるダウンロードアイコンをクリックし、Decentralandインストーラー（.exe）を開きます。",
          },
          step2: {
            title: "インストール",
            text: "セットアップウィザードの指示に従い、完了するまで「次へ」をクリックします。Windowsが許可を求めたら「はい」をクリックします。",
          },
          step3: {
            title: "いざ、出発！",
            text: "インストールが完了すると、Decentralandが自動的に起動します。ワールドでお会いしましょう！",
          },
        },
        macOS: {
          step1: {
            title: "ファイルを探す",
            text: "ブラウザのツールバーにあるダウンロードアイコンをクリックし、Decentraland-Installer.dmgを開きます。",
          },
          step2: {
            title: "インストール",
            text: "開いたウィンドウでDecentralandのアイコンをダブルクリックします。macOSのセキュリティメッセージが表示されたら、「開く」をクリックします。",
          },
          step3: {
            title: "いざ、出発！",
            text: "インストールが完了すると、Decentralandが自動的に起動します。ワールドでお会いしましょう！",
          },
        },
      },
    },
  },
} as const satisfies Record<SitesLocale, unknown>;
