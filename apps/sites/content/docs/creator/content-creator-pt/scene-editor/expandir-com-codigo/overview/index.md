> Combine conteúdo criado no Scene Editor com o poder de escrever código.

# Combine com código



<div class="video-embed"><button type="button" class="video-play" data-video-src="https://www.youtube-nocookie.com/embed/55H37rygD7M" aria-label="Play YouTube video"><svg viewBox="0 0 64 64" width="64" height="64" aria-hidden="true"><circle cx="32" cy="32" r="30" fill="currentColor"/><path d="m26 20 20 12-20 12Z" fill="#181214"/></svg><span>Play video</span></button></div>
<p class="video-source"><a href="&lt;https://www.youtube.com/watch?v=55H37rygD7M>">Watch on YouTube</a></p>



O Creator Hub com código personalizado é uma combinação muito poderosa para criar conteúdo. Você pode usar o canvas para posicionar itens visualmente de forma intuitiva e, depois, escrever código que interaja com esses itens com total liberdade. Você pode até colocar um smart item, que tem seu próprio comportamento padrão, e escrever código que reaja quando o item é ativado.

Por exemplo, você pode aproveitar um smart item de alavanca existente, que já vem com seus sons, animações e estados, e escrever código que detecte quando a alavanca é puxada para executar sua própria lógica personalizada.

Veja [Referenciar itens no código](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/reference-items/) para saber como buscar itens por nome ou por tags a partir do seu código.

## Editar código

Você precisa instalar um editor de código na sua máquina para editar o código da sua cena. As opções recomendadas são:

* <img src="/docs/_remote/358795a1c08dfc479355a174.webp" alt="VS Code" data-size="line"> [Visual Studio Code](https://code.visualstudio.com/): Esta é a opção recomendada para desenvolvedores experientes.
* <img src="/docs/_remote/9510a847d1493f0d8b4cf7e3.webp" alt="Cursor" data-size="line"> [Cursor AI](https://www.cursor.com/): Este é um editor de código poderoso, integrado com IA. Ele permite escolher diferentes modelos de IA para ajudar você a escrever código; há um plano gratuito disponível, e modelos mais avançados exigem um plano pago. Esta é uma boa opção para desenvolvedores que estão começando em Decentraland ou TypeScript, ou se você quiser economizar tempo escrevendo código.


> [!WARNING]
> **📔 Nota**: Se você estiver no macOS, certifique-se de que o aplicativo do editor de código esteja na pasta Applications.


Depois de instalado, talvez você precise selecionar o seu Editor de código nas configurações do Creator Hub. Para fazer isso,

1. Clique no logotipo do Creator Hub no canto superior esquerdo da tela e selecione **Configurações**.
2. Abra o **EDITOR** tab.
3. Em **Editor de código de sua preferência**, selecione seu Editor de código. Você pode encontrar seu editor listado no dropdown, ou talvez precise selecionar **Escolher do seu dispositivo...** para encontrá-lo.

![A caixa de diálogo Preferências do App na aba EDITOR, mostrando o dropdown Editor de código de sua preferência definido como Visual Studio Code.](/docs/_remote/be0b7d493dc2e145af5207ac.webp)

## Abrir o código de uma cena

Depois de instalar um editor de código na sua máquina e selecioná-lo nas configurações do Creator Hub, você pode clicar no **< > CODE** botão para abri-lo no projeto da sua cena.

![](/docs/_remote/b1e812c7bf7175e15d141046.webp)

Isso abre uma janela separada com o editor de código. Na margem esquerda, você pode navegar pelos arquivos e pela estrutura de pastas do seu projeto.

![](/docs/_remote/89a525be913ad9dc95c15692.webp)

Adicione seu código personalizado no `index.ts` ficheiro em `/src`, dentro da `main()` função. Você pode, de outra forma, adicionar código personalizado fora dessa função ou criar novos `.ts` arquivos dentro da `/src` pasta, mas eles precisam ser referenciados de alguma forma dentro da `main()` função de `index.ts`.


> [!WARNING]
> **📔 Nota**: Se você tiver VS Code ou Cursor instalados, mas o **CODE** botão não os abrir, pode ser que o VS Code não esteja configurado corretamente na sua máquina para abrir via linha de comando. Na maioria dos casos, isso é tratado como parte da instalação padrão, mas, se não for, consulte [estas instruções da VS](https://code.visualstudio.com/docs/setup/mac#_launching-from-the-command-line) para habilitar o VS Code pela linha de comando.


Se você tiver uma janela de preview aberta executando sua cena, sempre que você alterar o código nos seus arquivos e salvar, a cena recarrega automaticamente com suas alterações.

## Usando IA

O Creator Hub tem um [AI Assistant](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/ai-assistant/) integrado que vê sua cena aberta e a edita para você, sem sair do aplicativo. É um recurso experimental, desativado por padrão.

Você também pode aproveitar assistentes de IA como Cursor ou Claude Code para ajudar a escrever o código da cena. Por exemplo, para usar o Cursor, faça o seguinte:

1. Abra o assistente de IA do Cursor clicando no **IA** botão no canto superior direito da tela
2. Lá, você pode solicitar ao assistente de IA que ajude você a escrever código. Seus prompts podem incluir links para páginas da documentação, caminhos para arquivos específicos no seu projeto, ou até imagens. Você também pode selecionar um modelo específico para usar no dropdown.

Para dar contexto de Decentraland ao seu assistente de IA, instale o SDK Skills oficial do Decentraland. Esses são conjuntos de instruções prontos, mantidos pela Decentraland Foundation, que ensinam ao seu agente de IA padrões verificados do SDK7 para cada tópico: criar cenas inteiras, adicionar modelos 3D, multiplayer, deploy e muito mais.

```bash
npx skills add decentraland/sdk-skills
```

Veja [Vibe Coding com IA](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/) para o guia completo, incluindo a lista completa de skills disponíveis.


> [!WARNING]
> **📔 Nota**: Projetos de cena mais antigos podem conter uma `/dclcontext` pasta, baixada pelo obsoleto `npx sdk-commands get-context-files` comando. Esse contexto não é mais mantido e foi substituído pelos SDK Skills. Você pode excluir a pasta com segurança.


## Controle de versão

Recomendamos que você crie um repositório para seu projeto no GitHub e o use para acompanhar as versões do seu projeto e trabalhar de forma colaborativa com outras pessoas.

Se você não estiver familiarizado com como fazer isso, veja [Início rápido para repositórios](https://docs.github.com/en/repositories/creating-and-managing-repositories/quickstart-for-repositories), ou use o [aplicativo GitHub Desktop](https://desktop.github.com/download/) para um fluxo mais simples baseado em UI.


> [!WARNING]
> **📔 Nota**: Envie a pasta inteira do projeto para um repositório no GitHub, mas certifique-se de que as pastas `/node_modules` ou `/bin` e o arquivo `package-lock.json` estejam todos incluídos no arquivo `.gitignore` , para evitar sincronizá-los. Isso deve ser o caso se você configurar o repositório como do tipo `node`. Esses arquivos são todos gerados automaticamente, e o conteúdo pode ser diferente em máquinas diferentes.


## Ver também

* [Vibe Coding com IA](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/): construa cenas descrevendo o que você quer para um assistente de IA.
* [Smart items - Noções básicas](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/)
* [Smart items - Avançado](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items-advanced/)
* [Estados e condições](/docs/creator/content-creator-pt/scene-editor/interatividade/states-and-conditions/)
* [Transformar qualquer item em smart item](/docs/creator/content-creator-pt/scene-editor/interatividade/make-any-item-smart/)
* [Início rápido do SDK](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/sdk-101/): siga este mini tutorial para um curso rápido.
* [Fluxo de desenvolvimento](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/dev-workflow/): leia isto para entender a criação de Scene do início ao fim.
* [Exemplos](/docs/creator/content-creator-pt/tutoriais-e-exemplos/examples/): mergulhe diretamente em scenes de exemplo funcionais.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scene-editor/expandir-com-codigo/overview.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
