> O que pode ver na pré-visualização de uma cena

# Pré-visualize a sua cena

Depois que você tiver [criado uma nova cena](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/sdk-101/#create-your-first-scene) ou baixado um [exemplo de cena](/docs/creator/content-creator-pt/tutoriais-e-exemplos/examples/) você pode visualizá-la localmente.

## Usando o Scene Editor no Creator Hub

Certifique-se de que você tenha [instalado o Creator Hub](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/editor-installation/).

1. Abra o projeto da sua cena.
2. Clique no **Play** botão no canto superior direito. Isso abrirá uma nova janela com o Decentraland Desktop Explorer, executando apenas a sua cena. Lá você pode se mover pela cena e interagir com itens interativos.

![](/docs/_remote/ca7e6550cdb1bb947ce74b28.webp)

### Escolha onde visualizar

Clique na seta ao lado do **Play** botão para abrir o **Play Options** menu. O painel principal é onde você escolhe o que executa sua cena:

* **Desktop Client**: Abre a pré-visualização no Decentraland Desktop Explorer. Esta é a opção padrão.
* **Web (Bevy)**: Abre a pré-visualização no seu navegador, usando o cliente Bevy Web. Equivalente ao sinalizador `--web` da CLI.
* **Mostrar código QR para mobile**: Exibe um código QR que abre a pré-visualização da sua cena no [aplicativo mobile do Decentraland](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/overview/). Escaneie o código com um telefone na mesma rede Wi-Fi que o seu computador. Clique na área escura ao redor do pop-up para fechá-lo. Veja [Pré-visualização no mobile](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/preview-on-mobile/) para detalhes.

### Opções do Desktop Client

Selecione **Desktop Client** no **Play Options** menu e, em seguida, passe o mouse sobre ele para abrir um submenu com as configurações que afetam apenas o cliente desktop:

* **Ativar terrenos de paisagem**: Alterna a paisagem ao redor da cena. Isso está ativado por padrão; desative para reduzir o uso de memória da cena. Fica esmaecido quando as próprias configurações da cena já ocultam o terreno.
* **Pré-visualização multi-instância**: Permite que várias janelas do Explorer sejam executadas ao mesmo tempo, para que você possa [testar uma cena multiplayer localmente](#test-a-multiplayer-scene-locally). Com isso ativado, cada janela pede que você faça login, para que você possa se conectar como um jogador diferente em cada uma. Só fica visível quando seu projeto usa SDK 7.20.4 ou posterior.
* **Ativar servidor MCP**: Inicia o Explorer com o servidor de automação MCP ativado, para que agentes de IA possam ver e controlar a pré-visualização em execução. Só fica visível quando seu projeto usa SDK 7.25.0 ou posterior. Veja [Vibe Coding com IA](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/#let-the-ai-see-your-scene-in-world) para o fluxo completo.
* **Comprimir assets**: Pré-visualiza a cena com pacotes de assets gerados localmente, correspondendo à forma como ela é renderizada em produção após a [conversão de asset bundles](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/performance-optimization/#asset-bundle-conversion). A primeira execução converte todos os assets, o que pode levar vários minutos em cenas grandes. Só fica visível quando sua plataforma e a versão do SDK do seu projeto dão suporte a isso. Veja [Pré-visualização com assets otimizados](#preview-with-optimized-assets).

A saída do console da cena não precisa de uma opção de menu. O Creator Hub abre isso para você sempre que executa uma pré-visualização. Veja [O console do Creator Hub](#the-creator-hub-console).


> [!NOTE]
> **Dica:** Você também pode visualizar sua cena diretamente no aplicativo mobile do Decentraland. Use a **Mostrar código QR para mobile** opção no Creator Hub, ou execute `npm run start -- --mobile` pela CLI. Veja [Pré-visualização no mobile](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/preview-on-mobile/) para detalhes.


## Usando a CLI

Para visualizar uma cena, execute o seguinte comando na pasta principal da cena:

```bash
npm run start
```

Quaisquer dependências ausentes são instaladas e então a CLI cria um servidor web local no seu sistema e inicia a cena no Decentraland Desktop client por meio de um `decentraland://` deeplink. O cliente Desktop é o destino padrão da pré-visualização.

Para visualizar em uma aba do navegador em vez disso, adicione `-- --web` (ou `-- --bevy-web`) para abrir a cena no cliente Bevy Web em `decentraland.org/bevy-web/`.

Sempre que você fizer alterações na cena, a pré-visualização recarrega e atualiza automaticamente, então não há necessidade de executar o comando novamente.


> [!WARNING]
> **📔 Observação**: Algumas cenas dependem de comunicação com um servidor externo para executar lógica personalizada ou armazenar e recuperar dados. Ao visualizar uma dessas cenas, provavelmente você também terá que executar o servidor localmente em outra porta. Verifique o readme da cena para obter instruções sobre como iniciar o servidor e a cena.


### Parâmetros do comando de pré-visualização

Você pode adicionar as seguintes flags ao `npm run start` comando para alterar seu comportamento:

* `-- --web` (alias `-- --bevy-web`) Abre a pré-visualização no cliente de navegador Bevy Web em `decentraland.org/bevy-web/` em vez do Desktop Explorer. Navegadores baseados em Chromium (Chrome 142+) exigem a permissão Local Network Access para que a página hospedada alcance o seu servidor local de pré-visualização — quando o navegador pedir acesso aos aplicativos do seu dispositivo, clique em "Allow".
* `-- --mobile` (alias `-- -m`) Mostra um código QR no terminal que abre sua cena no [aplicativo mobile do Decentraland](/docs/creator/content-creator-pt/criar-para-mobile/cliente-movel/overview/) em um telefone conectado à mesma rede Wi-Fi. Veja [Pré-visualização no mobile](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/preview-on-mobile/).
* `-- --skip-build` Pule a build e sirva apenas os arquivos no modo de pré-visualização.
* `-- --port` (alias `-- -p`) para atribuir uma porta específica para executar a cena. Caso contrário, será usada qualquer porta disponível.
* `-- --no-browser` (alias `-- -b`) para impedir que a pré-visualização abra uma nova aba do navegador.
* `-- -w` ou `-- --no-watch` para não monitorar alterações no sistema de arquivos e evitar hot-reload sempre que o código da cena mudar.
* `-- --ci` Para executar o parcel previewer em um servidor unix remoto.
* `-- --multi-instance` Permite executar várias instâncias do Explorer simultaneamente.
* `-- --asset-bundles` Pré-visualize com asset bundles otimizados. O Decentraland Explorer converte os assets da cena em asset bundles por conta própria durante a pré-visualização, correspondendo à forma como a cena é renderizada em produção. Equivalente à opção **Comprimir assets** no Creator Hub. Disponível apenas com o Desktop Client (não com o cliente Bevy Web). Veja [Pré-visualização com assets otimizados](#preview-with-optimized-assets).
* `-- --no-client` Suprime todos os auto-lançamentos (deeplink do Explorer desktop, abertura do navegador, QR mobile). O observador de arquivos ainda notifica um Explorer desktop se ele se conectar por conta própria. Útil quando uma ferramenta externa controla o processo do Explorer.
* `-- --mcp` Ativa o servidor MCP no Explorer (encaminhado como parâmetro de deeplink).
* `-- --mcp-port` Porta para o servidor MCP no Explorer (encaminhado como parâmetro de deeplink). Por exemplo: `npm run start -- --mcp --mcp-port 3001`.


> [!WARNING]
> **📔 Observação**: Os parâmetros precisam ser adicionados com dois grupos de hífens, por exemplo `npm run start -- --web3`.


## Pré-visualização com assets otimizados

Quando você publica uma cena, os servidores do Decentraland convertem todos os seus modelos 3D em asset bundles, um formato otimizado que é muito mais leve para carregar e renderizar. Veja [Conversão de asset bundles](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/performance-optimization/#asset-bundle-conversion). Por padrão, as pré-visualizações locais pulam esta etapa e carregam os modelos brutos em vez disso.

Ative assets otimizados para executar essa mesma conversão localmente quando você visualizar sua cena. Isso tem dois benefícios:

* A pré-visualização carrega mais rápido e roda com mais fluidez, especialmente em cenas com modelos 3D pesados.
* Você vê os modelos exatamente como os jogadores verão após a publicação, então pode identificar quaisquer problemas com a compressão dos assets antes de publicar.

Esta opção afeta apenas a sua pré-visualização local. A cena publicada sempre é convertida nos servidores, independentemente de você usar esta opção ou não.

Para ativá-la:

* **No Creator Hub**: abra **Play Options** ao lado do **Play** botão, passe o mouse sobre **Desktop Client**, e marque **Comprimir assets**.
* **Pela CLI**: execute `npm run start -- --asset-bundles`.

![](/docs/_remote/b1d24b74a3fe9e049ed203e0.webp)

Com esta opção ativada, o Decentraland Explorer converte os modelos 3D da cena em asset bundles na sua máquina antes de carregá-los. A conversão é quase instantânea para a maioria das cenas, mas a primeira pré-visualização pode demorar mais se a cena tiver assets muito pesados. Os modelos convertidos são armazenados em cache, então nas pré-visualizações seguintes apenas os assets novos ou modificados precisam ser convertidos.


> [!NOTE]
> **💡 Dica**: Se a conversão de um asset falhar por qualquer motivo, a pré-visualização volta a carregar os modelos 3D brutos, como em uma pré-visualização normal.


## Enviar uma cena ao Decentraland

Quando você estiver satisfeito com sua cena, pode enviá-la e publicá-la no Decentraland. Para isso, você deve possuir LAND, um Decentraland NAME, ou um nome ENS em ETH, ou ter permissões concedidas por alguém que tenha isso. Veja [publicação](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing/) para instruções sobre como fazer isso.

## Tamanho da cena na pré-visualização

O tamanho da cena mostrado na pré-visualização é baseado na configuração da cena.

Edite isso na segunda aba do menu da cena no Scene Editor.

![](/docs/_remote/5bcc0f6dfa70093b896862ae.webp)

Use os menus suspensos e clique em **Aplicar layout** para alterar as dimensões da sua cena. Você também pode clicar em cada parcela individual para desativá-la no seu layout.

![](/docs/_remote/c2905f95052e96703e78829e.webp)

Você também pode editar o *scene.json* arquivo para listar várias parcelas no campo "parcels". Veja [definir parcelas pela linha de comando](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/scene-metadata/#scene-parcels) para mais detalhes.


> [!NOTE]
> **💡 Dica**: Enquanto a pré-visualização está em execução, as coordenadas das parcelas não precisam corresponder às que sua cena realmente usará, desde que sejam adjacentes e estejam organizadas no mesmo formato. Você terá que substituí-las pelas coordenadas reais depois, quando você [implantar a cena](#upload-a-scene-to-decentraland).


## Ver o console da cena

Abra o console clicando no ![](/docs/_remote/92e8eb247fcf289c2feff8f8.webp) ícone no canto superior direito. Aqui você pode ver quaisquer mensagens de erro e também qualquer texto que sua cena imprima no console via `console.log()`.

Você também pode abri-lo pressionando a **\`** tecla no seu teclado. Você também pode pressionar Shift + **\`** para abrir o console ainda mais, caso você precise ver mais texto.

## O console do Creator Hub

O Creator Hub mostra a mesma saída sem sair do editor, e faz isso por padrão. Execute uma pré-visualização e uma **CONSOLE** aba aparece ao lado de **SISTEMA DE ARQUIVOS** e **PACOTES DE ASSETS** no painel na parte inferior do editor.

Para impedir que o console seja aberto, desmarque **Abrir console de depuração** em **Play Options** na **EDITOR** aba das configurações do Creator Hub.

Para tirar o console do caminho, clique no ícone de pop-out em seu cabeçalho, rotulado **Abrir console em uma janela separada**. O console abre como uma janela própria intitulada **Console**, que você pode mover para um segundo monitor e deixar visível enquanto edita.

A janela separada tem um botão **Limpar console** e um botão **Reencaixar no editor** que retorna o console ao painel inferior. Enquanto isso, a **CONSOLE** aba no editor mostra **Console aberto em uma janela separada** e oferece um botão **Reencaixar aqui** que faz a mesma coisa.


> [!NOTE]
> **💡 Dica**: O console continua coletando a saída enquanto está destacado, então reencaixá-lo não perde nada. Quando nenhuma pré-visualização está em execução, ele mostra **Execute uma cena para ver a saída de depuração**.


## Teste uma cena multiplayer localmente

Se você iniciar uma pré-visualização de cena e abri-la em duas (ou mais) janelas diferentes do Explorer, cada janela aberta será interpretada como um jogador separado, e um servidor de comunicação simulado manterá esses jogadores sincronizados.

Interaja com a cena em uma janela e depois mude para a outra para ver que os efeitos dessa interação também ficam visíveis lá.

Usando o Creator Hub, ative **Pré-visualização multi-instância** no **Desktop Client** submenu de **Play Options**, depois clique no **Play** botão uma segunda vez para abrir uma segunda janela do Decentraland Explorer. Você deve se conectar em ambas as janelas com endereços diferentes. As mesmas sessões permanecerão abertas enquanto a cena recarrega.

![](/docs/_remote/ca7e6550cdb1bb947ce74b28.webp)

Como alternativa, você pode abrir uma segunda janela do Decentraland Explorer escrevendo o seguinte em uma URL do navegador:

> `decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&multi-instance=true`

### Avançado: iteração rápida com asset bundles remotos


> [!NOTE]
> **💡 Dica**: Para a maioria das cenas, a forma mais simples de visualizar com asset bundles é ativar [assets otimizados](#preview-with-optimized-assets), que converte seus modelos localmente e sempre reflete a versão mais recente. O modo abaixo é uma alternativa que reutiliza os bundles já publicados nos servidores.


Para cenas pesadas com muitos modelos 3D, você pode acelerar o carregamento e o recarregamento da cena reutilizando os [asset bundles](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/performance-optimization/#asset-bundle-conversion) que já estão publicados nos servidores do Decentraland, em vez de carregar os modelos 3D brutos e não otimizados. Isso é especialmente útil ao iterar em mudanças apenas de código.

Para ativar esse modo, inicie o Decentraland Desktop client com os seguintes argumentos:

```bash
npm run start -- --realm http://127.0.0.1:8000/ --position 0,0 --local-scene true --debug --skip-version-check true --lsd-use-remote-ab <ab-source>
```

O `<ab-source>` argumento muda dependendo de onde a cena já está publicada:

* **Na Genesis City**: `--lsd-remote-ab-server Genesis`
* **Em um World**: `--lsd-remote-ab-world <world-name>.dcl.eth`

Por exemplo, para visualizar uma cópia local de uma cena que já foi implantada em um World:

```bash
npm run start -- --realm http://127.0.0.1:8000/ --position 0,0 --local-scene true --debug --skip-version-check true --lsd-use-remote-ab --lsd-remote-ab-world myworld.dcl.eth
```

Em ambos os casos, `--realm http://127.0.0.1:8000/` direciona o cliente para o seu servidor local de pré-visualização (execute `npm run start` primeiro para iniciá-lo), e `--local-scene true` instrui o cliente a carregar o código da cena a partir daí.


> [!WARNING]
> **📔 Importante**: Ao usar esse modo, é recomendável que **todos** os seus assets já estejam publicados, com seus asset bundles totalmente processados pelos servidores de conteúdo. Se você adicionou novos assets, você perderá o carregamento otimizado, pois eles serão carregados como arquivos glTF brutos, como acontece quando você executa uma pré-visualização normalmente. Mas, se você modificou localmente um asset que já estava publicado, mantendo o mesmo nome de arquivo, então verá a versão publicada antiga desse asset.
> 
> Nesse caso, publique a cena novamente primeiro, aguarde alguns minutos para que os asset bundles sejam gerados (veja [Conversão de asset bundles](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/performance-optimization/#asset-bundle-conversion)), e então retome o uso desse modo para iteração apenas de código.



---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/primeiros-passos/preview-scene.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
