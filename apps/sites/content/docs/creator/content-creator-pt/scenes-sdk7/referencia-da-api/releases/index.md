> O que mudou no SDK e no Creator Hub ao longo do último ano, filtrado para as mudanças que afetam os criadores.

# Notas de Versão

Esta página lista as alterações no [SDK](#sdk) e o [Creator Hub](#creator-hub) do último ano que afetam como você constrói cenas. Trabalhos internos, como ferramentas de build, testes, rastreamento e refatorações, foram omitidos. Cada item links para seu pull request no GitHub, onde você pode encontrar mais detalhes.

Para as notas completas e sem filtro, veja as [releases do SDK](https://github.com/decentraland/js-sdk-toolchain/releases) e o [releases do Creator Hub](https://github.com/decentraland/creator-hub/releases) no GitHub.


> [!NOTE]
> As releases do Creator Hub agrupam as alterações dos pacotes `@dcl/inspector` (o Scene Editor) e `@dcl/asset-packs` (smart items). Essas alterações estão listadas sob a versão do Creator Hub que as lançou primeiro.


## SDK

### 7.31.0 (22 de out de 2026)

* **Fontes personalizadas**: a nova `fontSrc` propriedade renderiza texto com uma `.ttf` arquivo de fonte da sua cena, em `TextShape`, `UiText`, `UiInput` e `UiDropdown` e no React ECS `Label`, `Button`, `Input` e `Dropdown`. Veja [Fontes personalizadas](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/text/#custom-fonts) e [Fontes personalizadas na UI](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/ui_text/#custom-fonts). [#1656](https://github.com/decentraland/js-sdk-toolchain/pull/1656)
* **Publicar scenes grandes**: `npm run deploy` faz o upload de cenas de 50 MiB ou mais em lotes, então cenas com mais de cerca de 200 MB podem ser publicadas, e a página de deploy no navegador mostra o progresso do upload e permite cancelar. Veja [Publicar scenes grandes](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing/#publish-large-scenes). [#1635](https://github.com/decentraland/js-sdk-toolchain/pull/1635)
* **Correções**: a pré-visualização local serve cada Entity da cena uma única vez, então cenas que se estendem por muitos parcels carregam na pré-visualização [#1650](https://github.com/decentraland/js-sdk-toolchain/pull/1650).

### 7.30.0 e 7.30.1 (8 de out de 2026)

* **Idioma do jogador**: `getPlayerLanguage()` retorna o idioma definido no cliente do jogador, como uma tag BCP 47, e `onPlayerLanguageChanged` reporta alterações feitas no meio da sessão. Veja [Obter idioma do jogador](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/runtime-data/#get-player-language). [#1622](https://github.com/decentraland/js-sdk-toolchain/pull/1622)
* **Aguardar o fechamento de um painel do Explorer**: a nova `openExplorerUiAndWait()` helper em `@dcl/sdk/explorer-ui` abre um painel e é resolvido assim que o jogador o fecha, com um limite de tempo opcional. Veja [Aguardar até o jogador fechar um painel](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/event-listeners/#wait-until-the-player-closes-a-panel). [#1543](https://github.com/decentraland/js-sdk-toolchain/pull/1543)
* **Correções**: o watcher do bundle ignora `node_modules` [#1640](https://github.com/decentraland/js-sdk-toolchain/pull/1640); a pré-visualização verifica novamente a cena do editor em cada rebuild do watch [#1644](https://github.com/decentraland/js-sdk-toolchain/pull/1644).

### 7.29.0 (15 de set de 2026)

* **Ordem de empilhamento entre módulos de UI**: uma opção de renderer `zIndex` decide qual módulo de UI é desenhado na frente de qual. Sem isso, os módulos se empilham na ordem em que aparecem primeiro, com a UI principal ao fundo. Veja [Ordem de empilhamento entre módulos de UI](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/onscreen-ui/#stacking-order-between-ui-modules). [#1607](https://github.com/decentraland/js-sdk-toolchain/pull/1607)
* **Correções**: remover um manipulador de evento de pointer agora remove a entrada que ele criou, então um botão de UI desativado deixa de anunciar um click que já não pode tratar, e registrar manipuladores novamente não acumula entradas duplicadas. [#1577](https://github.com/decentraland/js-sdk-toolchain/pull/1577)

### 7.28.0 (10 de set de 2026)

* **Eventos de pointer usam a distância da câmera**: `maxCameraDistance` substitui `maxPlayerDistance` nos eventos de pointer. O campo antigo ainda funciona, mas está obsoleto. Dicas de hover e clicks agora dependem de quão longe a câmera está do item, o que corresponde ao que o jogador vê. [#1560](https://github.com/decentraland/js-sdk-toolchain/pull/1560)
* **componente AvatarNametag**: personalize o rótulo de nome exibido sobre os avatares. [#1600](https://github.com/decentraland/js-sdk-toolchain/pull/1600)
* **Teleportar para um realm**: `teleportTo()` pode apontar para um realm específico. [#1601](https://github.com/decentraland/js-sdk-toolchain/pull/1601)
* **Correções**: uma UI desativada `Button` não escurece mais as cores da cena [#1578](https://github.com/decentraland/js-sdk-toolchain/pull/1578); a entrada de pointer de uma Entity não oculta mais a de outra no mesmo frame [#1576](https://github.com/decentraland/js-sdk-toolchain/pull/1576); parâmetros de `ActionCallback` em smart items Script resolvem para `undefined` em vez de falhar [#1605](https://github.com/decentraland/js-sdk-toolchain/pull/1605); composites são instanciados corretamente na RootEntity [#1587](https://github.com/decentraland/js-sdk-toolchain/pull/1587); campos opcionais de componentes definidos como `0` ou `false` agora são salvos [#1582](https://github.com/decentraland/js-sdk-toolchain/pull/1582); componentes definidos a partir de um esquema primitivo ou de array podem ser lidos novamente [#1593](https://github.com/decentraland/js-sdk-toolchain/pull/1593); várias correções de robustez de serialização e networking [#1592](https://github.com/decentraland/js-sdk-toolchain/pull/1592), [#1595](https://github.com/decentraland/js-sdk-toolchain/pull/1595), [#1568](https://github.com/decentraland/js-sdk-toolchain/pull/1568), [#1570](https://github.com/decentraland/js-sdk-toolchain/pull/1570).

### 7.27.0 (27 de ago de 2026)

* **Melhorias nos helpers de eventos**: os sistemas de helper de pointer, trigger, áudio e outros eventos agora têm nomes únicos e comportamento consistente. [#1536](https://github.com/decentraland/js-sdk-toolchain/pull/1536)
* **Estado da tela de carregamento**: `EngineInfo` informa se a tela de carregamento ainda está visível. [#1555](https://github.com/decentraland/js-sdk-toolchain/pull/1555)
* **Builds mais rápidos**: os arquivos do projeto são processados em paralelo ao fazer build e deploy. [#1481](https://github.com/decentraland/js-sdk-toolchain/pull/1481)
* **Correções**: eventos de click são detectados para todos os jogadores, não apenas o primeiro [#1468](https://github.com/decentraland/js-sdk-toolchain/pull/1468); timers continuam funcionando depois que uma callback lança erro [#1470](https://github.com/decentraland/js-sdk-toolchain/pull/1470); entidades de UI do React ECS desmontadas são liberadas, corrigindo um vazamento de memória [#1471](https://github.com/decentraland/js-sdk-toolchain/pull/1471); subscriptions de observable não interferem mais entre si [#1467](https://github.com/decentraland/js-sdk-toolchain/pull/1467); a etapa de login ao fazer deploy encaminha os headers corretamente [#1551](https://github.com/decentraland/js-sdk-toolchain/pull/1551); IDs de Entity reservados pelo renderer nunca são gerados nem reciclados [#1544](https://github.com/decentraland/js-sdk-toolchain/pull/1544), [#1460](https://github.com/decentraland/js-sdk-toolchain/pull/1460).

### 7.26.0 (13 de ago de 2026)

* **componente TouchScreenControls**: mostre, oculte e personalize os controles na tela no mobile. Veja [Controles na tela](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/touch-screen-controls/). [#1441](https://github.com/decentraland/js-sdk-toolchain/pull/1441), [#1510](https://github.com/decentraland/js-sdk-toolchain/pull/1510)
* **Abra a UI do Explorer a partir da sua cena**: a `openExplorerUi()` ação restrita abre o mapa, a mochila, as configurações e outras telas do Explorer, e `ExplorerUiEventsResult` reporta quando elas se fecham. [#1503](https://github.com/decentraland/js-sdk-toolchain/pull/1503), [#1511](https://github.com/decentraland/js-sdk-toolchain/pull/1511)
* **Eventos de áudio e estado do emote**: a nova `audioEventSystem` reage a eventos de reprodução de áudio, e `AvatarEmoteCommand` expõe o `estado do emote`. [#1530](https://github.com/decentraland/js-sdk-toolchain/pull/1530)
* **Padrões de tela da UI**: a UI usa uma tela virtual padrão e uma área de recuo da tela, e a proporção de pixels do dispositivo não afeta mais o layout. [#1489](https://github.com/decentraland/js-sdk-toolchain/pull/1489)
* **Pré-visualização web do Bevy**: a pré-visualização no navegador usa o Explorer do Bevy Web em vez do Explorer web legado. [#1502](https://github.com/decentraland/js-sdk-toolchain/pull/1502)
* **Assets otimizados na pré-visualização**: a pré-visualização pode carregar seus modelos como asset bundles, correspondendo à aparência que terão quando publicados. Veja [Pré-visualização com assets otimizados](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/preview-scene/#preview-with-optimized-assets). [#1539](https://github.com/decentraland/js-sdk-toolchain/pull/1539)
* **Correções**: `npm run start` permanece ativa quando o build inicial falha, então você pode corrigir o erro e ela faz rebuild [#1501](https://github.com/decentraland/js-sdk-toolchain/pull/1501); portas ocupadas são detectadas corretamente [#1542](https://github.com/decentraland/js-sdk-toolchain/pull/1542).

### 7.24.5 (15 de jul de 2026)

* **Alvo do Billboard**: uma opção de renderer `Billboard` pode ficar de frente para uma Entity alvo em vez da câmera. [#1445](https://github.com/decentraland/js-sdk-toolchain/pull/1445)
* **Bundles de cena menores**: uma segunda cópia de `@dcl/ecs` não é mais incluída em bundle em todas as cenas. [#1450](https://github.com/decentraland/js-sdk-toolchain/pull/1450)
* **Correção**: `MessageBus` as mensagens são enviadas em ordem. [#1462](https://github.com/decentraland/js-sdk-toolchain/pull/1462)

### 7.24.4 (3 de jul de 2026)

* **InteractableArea para React ECS**: novo componente de UI para áreas interativas na tela. [#1411](https://github.com/decentraland/js-sdk-toolchain/pull/1411)

### 7.24.3 (1 de jul de 2026)

* **Chega de limite de parcel 45x45** para cenas. [#1429](https://github.com/decentraland/js-sdk-toolchain/pull/1429)
* **Salto duplo e planar**: `AvatarLocomotionSettings` tem campos para ativar ou desativar o salto duplo e o planear. [#1402](https://github.com/decentraland/js-sdk-toolchain/pull/1402)
* **Correção**: a tela virtual da UI considera a proporção de pixels do dispositivo. [#1433](https://github.com/decentraland/js-sdk-toolchain/pull/1433)

### 7.24.2 (25 de jun de 2026)

* **Correções**: um rearmamento de `setTimeout` não trava mais a cena após um frame longo [#1420](https://github.com/decentraland/js-sdk-toolchain/pull/1420); React ECS mantém uma propriedade opcional definida pela primeira vez [#1422](https://github.com/decentraland/js-sdk-toolchain/pull/1422); erros de "Premature close" durante deploy e outras requests da CLI desapareceram [#1428](https://github.com/decentraland/js-sdk-toolchain/pull/1428).

### 7.24.1 (12 de jun de 2026)

* **Camada de colisão do jogador**: uma nova camada de colisão para o jogador principal. [#1414](https://github.com/decentraland/js-sdk-toolchain/pull/1414)
* **Correção**: ao retriggerar um `AudioSource` já não toca sons duplicados. [#1382](https://github.com/decentraland/js-sdk-toolchain/pull/1382)

### 7.24.0 (4 de jun de 2026)

* **ScreenInsetArea**: componente do React ECS que mantém a UI dentro da safe area no mobile. Veja [Safe Area no mobile](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/safe-area/). [#1386](https://github.com/decentraland/js-sdk-toolchain/pull/1386)
* **Fornecedor de composite**: `getCompositeProvider()` e `setCompositeProvider()` no engine, para controle avançado de como os composites são carregados. [#1333](https://github.com/decentraland/js-sdk-toolchain/pull/1333)

### 7.23.2 (13 de mai de 2026)

* **Correção**: as trigger areas não enviam mais mensagens de rede repetidamente enquanto um jogador permanece dentro delas. [#1397](https://github.com/decentraland/js-sdk-toolchain/pull/1397)

### 7.23.1 (5 de mai de 2026)

* **Análise de áudio**: leia dados em tempo real de fontes de áudio em reprodução para acionar efeitos. [#1256](https://github.com/decentraland/js-sdk-toolchain/pull/1256)

### 7.23.0 (5 de mai de 2026)

* **componente AvatarLocomotionSettings**: controle as configurações de caminhada, corrida e salto do jogador. [#1389](https://github.com/decentraland/js-sdk-toolchain/pull/1389)
* **ação de input Modifier**: `IA_MODIFIER` está disponível na lista de comandos de input. [#1390](https://github.com/decentraland/js-sdk-toolchain/pull/1390)

### 7.22.5 (23 de abr de 2026)

* **Bundles menores para cenas apenas com código**: a biblioteca de smart items só é incluída em bundle quando a cena usa um composite. [#1381](https://github.com/decentraland/js-sdk-toolchain/pull/1381)

### 7.22.4 (16 de abr de 2026)

* **Sourcemaps excluídos dos deployments** via `.dclignore`. [#1373](https://github.com/decentraland/js-sdk-toolchain/pull/1373)
* **Explosões do sistema de partículas** propriedade atualizada. [#1375](https://github.com/decentraland/js-sdk-toolchain/pull/1375)

### 7.22.3 (8 de abr de 2026)

* **componente ParticleSystem**: emite partículas a partir de uma Entity. Veja [Sistema de Partículas](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/particle-system/). [#1369](https://github.com/decentraland/js-sdk-toolchain/pull/1369)
* **Menos tráfego de rede**: escrever um valor inalterado em um componente mutável não envia mais uma mensagem. [#1359](https://github.com/decentraland/js-sdk-toolchain/pull/1359)
* **Erros de deploy mais claros** quando o World não existe, você não tem permissões ou um catalyst não pode ser alcançado. [#1361](https://github.com/decentraland/js-sdk-toolchain/pull/1361), [#1370](https://github.com/decentraland/js-sdk-toolchain/pull/1370)

### 7.22.2 (26 de mar de 2026)

* **Interações de proximidade**: reaja à aproximação do jogador a uma Entity. Veja [Eventos de Proximidade](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/proximity-events/). [#1356](https://github.com/decentraland/js-sdk-toolchain/pull/1356)

### 7.22.1 (26 de mar de 2026)

* **Página de login atualizada** usada ao fazer deploy pela CLI. [#1362](https://github.com/decentraland/js-sdk-toolchain/pull/1362)

### 7.22.0 (25 de mar de 2026)

* **Força e impulso de física**: empurre o jogador com forças e impulsos. Veja [Física do Jogador](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/player-physics/). [#1338](https://github.com/decentraland/js-sdk-toolchain/pull/1338)
* **Worlds com múltiplas cenas**: faça deploy de várias cenas para o mesmo World. [#1355](https://github.com/decentraland/js-sdk-toolchain/pull/1355)
* **Deploy a partir de CI** pipelines. [#1357](https://github.com/decentraland/js-sdk-toolchain/pull/1357)
* **Correções**: reordenar listas com chave no React ECS [#1351](https://github.com/decentraland/js-sdk-toolchain/pull/1351); tipos de ação são excluídos da sincronização de rede [#1354](https://github.com/decentraland/js-sdk-toolchain/pull/1354).

### 7.21.0 (10 de mar de 2026)

* **Desativar salto duplo e planar** na sua cena. [#1323](https://github.com/decentraland/js-sdk-toolchain/pull/1323)
* **modo Tween MoveRotateScale**: anime posição, rotação e escala em um único tween. [#1339](https://github.com/decentraland/js-sdk-toolchain/pull/1339)
* **Detecção de plataforma**: saiba se a cena roda em desktop, web ou mobile. Veja [Detectar a plataforma](/docs/creator/content-creator-pt/criar-para-mobile/desenvolver/detect-platform/). [#1340](https://github.com/decentraland/js-sdk-toolchain/pull/1340)

### 7.20.4 (3 de mar de 2026)

* **Pré-visualização com múltiplas instâncias**: execute várias instâncias de jogador contra uma pré-visualização para testar multiplayer. [#1336](https://github.com/decentraland/js-sdk-toolchain/pull/1336)
* **Correções**: colisões de IDs de Entity [#1332](https://github.com/decentraland/js-sdk-toolchain/pull/1332); `UiInput` não ecoa mais seu valor de volta [#1331](https://github.com/decentraland/js-sdk-toolchain/pull/1331).

### 7.20.2 e 7.20.3 (23 de fev de 2026)

* **O QR mobile inclui a posição** de spawn. [#1328](https://github.com/decentraland/js-sdk-toolchain/pull/1328)
* **Correções**: valores com timestamps idênticos (por exemplo, resultados de eventos de pointer) são ordenados corretamente [#1322](https://github.com/decentraland/js-sdk-toolchain/pull/1322); endpoint da pré-visualização mobile corrigido ([release 7.20.3](https://github.com/decentraland/js-sdk-toolchain/releases/tag/7.20.3)).

### 7.20.1 (16 de fev de 2026)

* **Fluxo melhor ao fazer deploy para um World com múltiplas cenas.** [#1317](https://github.com/decentraland/js-sdk-toolchain/pull/1317)

### 7.20.0 (5 de fev de 2026)

* **Pré-carregamento de assets**: marque assets para carregar antes de serem necessários. [#1287](https://github.com/decentraland/js-sdk-toolchain/pull/1287)

### 7.19.0 (29 de jan de 2026)

* **Funções helper do SDK**: atalhos para tarefas comuns, como atrasar uma instrução. [#1275](https://github.com/decentraland/js-sdk-toolchain/pull/1275)
* **Distância máxima do jogador para eventos de pointer**: limite a distância que um jogador pode estar para interagir. [#1293](https://github.com/decentraland/js-sdk-toolchain/pull/1293)

### 7.18.0 (26 de jan de 2026)

* **Vários renderizadores de UI**: `addUiRenderer()` e `removeUiRenderer()` no renderer do React ECS. [#1285](https://github.com/decentraland/js-sdk-toolchain/pull/1285)
* **Fator de escala automático da UI** no React ECS. [#1290](https://github.com/decentraland/js-sdk-toolchain/pull/1290)
* **Instâncias de Script a partir de código**: chame os componentes Script dos seus smart items de qualquer lugar do seu código, e acione ações configuradas no editor com `ActionCallback`. [#1284](https://github.com/decentraland/js-sdk-toolchain/pull/1284)
* **Arquivos Markdown excluídos dos deployments.** [#1282](https://github.com/decentraland/js-sdk-toolchain/pull/1282)

### 7.17.0 (5 de jan de 2026)

* **Erros de build mostram a saída da verificação de tipos.** [#1263](https://github.com/decentraland/js-sdk-toolchain/pull/1263)
* **A pré-visualização mobile imprime coordenadas e um deep link.** [#1268](https://github.com/decentraland/js-sdk-toolchain/pull/1268)

### 7.16.0 (30 de dez de 2025)

* **A visibilidade se propaga para filhos**: ocultar uma Entity com `VisibilityComponent` também oculta seus filhos. [#1262](https://github.com/decentraland/js-sdk-toolchain/pull/1262)
* **Preview no mobile**: execute a pré-visualização no aplicativo mobile. [#1261](https://github.com/decentraland/js-sdk-toolchain/pull/1261)
* **Pré-visualização do navegador Bevy Web** suportada. [#1241](https://github.com/decentraland/js-sdk-toolchain/pull/1241)
* **`npm run start` abre o novo Explorer para desktop por padrão.** [#1264](https://github.com/decentraland/js-sdk-toolchain/pull/1264)
* **Correção**: o template de projeto de wearable inteligente volta a ser baixado. [#1255](https://github.com/decentraland/js-sdk-toolchain/pull/1255)

### 7.15.0 e 7.15.1 (8 de dez de 2025)

* **Áudio espacial** para `AudioSource` e `VideoPlayer`. [#1243](https://github.com/decentraland/js-sdk-toolchain/pull/1243)
* **suporte ao componente Script** para smart items com código. [#1245](https://github.com/decentraland/js-sdk-toolchain/pull/1245)
* **Correções**: o Windows solicita baixar o Desktop Client quando ele está ausente [#1244](https://github.com/decentraland/js-sdk-toolchain/pull/1244); smart items são resolvidos corretamente a partir de `node_modules` [#1258](https://github.com/decentraland/js-sdk-toolchain/pull/1258).

### 7.12.1 (3 de nov de 2025)

* **`comando` code-to-composite**: transforme código de cena em um composite que o editor pode abrir. [#1232](https://github.com/decentraland/js-sdk-toolchain/pull/1232)

### 7.12.0 (30 de out de 2025)

* **ação restrita CopyToClipboard**. [#1234](https://github.com/decentraland/js-sdk-toolchain/pull/1234)

### 7.11.0 (15 de out de 2025)

* **Erros mais claros do content server** ao fazer deploy. [#1217](https://github.com/decentraland/js-sdk-toolchain/pull/1217)
* **`dclcontext` e pastas de thumbnails excluídas dos deployments.** [#1227](https://github.com/decentraland/js-sdk-toolchain/pull/1227), [#1229](https://github.com/decentraland/js-sdk-toolchain/pull/1229)

### 7.10.6 (9 de out de 2025)

* **Modos contínuos do Tween**: continue girando ou se movendo indefinidamente. [#1221](https://github.com/decentraland/js-sdk-toolchain/pull/1221)

### 7.10.5 (6 de out de 2025)

* **`get-context-files` code-to-composite**: baixa os documentos de contexto de IA para a sua cena para que assistentes de codificação conheçam o SDK. [#1220](https://github.com/decentraland/js-sdk-toolchain/pull/1220)

### 7.10.3 (29 de set de 2025)

* **Componentes de trigger area**: detecta jogadores entrando, permanecendo e saindo de uma região. Veja [Áreas de Trigger](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/trigger-areas/). [#1210](https://github.com/decentraland/js-sdk-toolchain/pull/1210)
* **componente Tags**: rotule Entities e consulte-as por tag. [#1216](https://github.com/decentraland/js-sdk-toolchain/pull/1216)

## Creator Hub

### 0.52.0 (20 de out de 2026)

* **asset pack Avatar Animations**: 45 animações de avatar prontas para usar, além das 32 integradas que toda cena já pode reproduzir. Passe o mouse sobre um cartão para assistir, arraste um para copiar o arquivo dele para a sua cena e então reproduza-o a partir do código. Veja [Animações de avatar](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/about-editor/#avatar-animations). [#1689](https://github.com/decentraland/creator-hub/pull/1689)
* **Mencione assets em um prompt do AI Assistant**: dê um duplo clique em um asset em **Local Assets** para mencioná-lo na sua próxima mensagem, junto com o que você tiver selecionado. Veja [Aponte para um asset no seu prompt](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/ai-assistant/#point-at-an-asset-in-your-prompt). [#1689](https://github.com/decentraland/creator-hub/pull/1689)
* **Mais de 400 novos assets, e categorias Buildings e Materials**: modelos de Genesis Plaza e das cenas template do Decentraland, além de smart items Water Plane e Water Cylinder que espirram, e um Floor Pressure Button que é pressionado enquanto um jogador estiver em cima dele. Veja [Água](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/#water). [#1696](https://github.com/decentraland/creator-hub/pull/1696)
* **Painel de assets recolhível**: uma seta no lado direito da barra de separadores do painel encolhe-o até ficar só com os separadores, dando à vista 3D a altura total da janela. Veja [Adicionar itens](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/about-editor/#add-items). [#1691](https://github.com/decentraland/creator-hub/pull/1691)
* **Configuração do AI Assistant a partir de um terminal**: Claude e Cursor são agora instalados e iniciados sessão a partir de um terminal, tal como o Gemini. Cada ecrã de configuração mostra o comando de instalação do fornecedor para o teu sistema operativo e liga para o respetivo guia de instalação, e **Verificar novamente** encontra uma instalação nova sem reiniciar. Veja [Ligar uma CLI de programação](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/ai-assistant/#connect-a-coding-cli). [#1684](https://github.com/decentraland/creator-hub/pull/1684)
* **Uma ligação partilhável após publicar**: o ecrã de sucesso mostra e copia uma ligação web que qualquer pessoa pode abrir num browser, em vez de uma que precise do cliente de desktop. Veja [Publica a tua cena](/docs/creator/content-creator-pt/scene-editor/publicar/publish-scene/). [#1688](https://github.com/decentraland/creator-hub/pull/1688)
* **O card Collections abre o app de criadores**: os wearables e emotes **Collections** card no **Mais** agora abre o novo app de criadores em vez do Builder legado. [#1687](https://github.com/decentraland/creator-hub/pull/1687)
* **Correções**: uma falha de publicação que indica que a miniatura da cena "tem de ser um ficheiro incluído na implementação", o que fazia com que as cenas afetadas se corrigissem automaticamente no próximo guardado da miniatura [#1657](https://github.com/decentraland/creator-hub/pull/1657); um layout redimensionado no separador Layout mantém-se, por isso publicar já não repõe a cena para um único parcel no renderizador Bevy [#1682](https://github.com/decentraland/creator-hub/pull/1682); fiabilidade do AI Assistant, com um turno silencioso a mostrar agora um erro, perguntas expiradas a serem fechadas, **Parar** a terminar realmente um turno no Windows, e uma mensagem enviada a meio de um turno a voltar para o compositor em vez de se perder [#1681](https://github.com/decentraland/creator-hub/pull/1681); orientação de início de sessão quando um agente indica que não tens sessão iniciada [#1685](https://github.com/decentraland/creator-hub/pull/1685).

### 0.51.0 (8 de out. de 2026)

* **Anexar ficheiros aos prompts do AI Assistant**: arrasta ficheiros para a caixa de mensagem, escolhe-os com o botão do clipe ou cola uma imagem da área de transferência. São suportadas imagens, modelos 3D, áudio e qualquer outro ficheiro, até 8 por mensagem. Veja [Anexar ficheiros a um prompt](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/ai-assistant/#attach-files-to-a-prompt). [#1670](https://github.com/decentraland/creator-hub/pull/1670)
* **Itens rápidos a partir de arrastos de media**: arrastar uma imagem, som ou vídeo de **Local Assets** para o canvas cria agora um item pronto que já aponta para esse ficheiro. Veja [Importar assets personalizados](/docs/creator/content-creator-pt/scene-editor/construir/import-items/). [#1616](https://github.com/decentraland/creator-hub/pull/1616)
* **O botão da barra de ferramentas tem o rótulo Reproduzir novamente**, e o seu menu está dividido num painel principal que escolhe onde fazer a pré-visualização, mais um **Cliente de desktop** submenu para as opções que só se aplicam aí. A consola de depuração abre agora por predefinição, pelo que já não é uma opção de menu. Veja [Pré-visualiza a tua cena](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/preview-scene/). [#1671](https://github.com/decentraland/creator-hub/pull/1671), [#1656](https://github.com/decentraland/creator-hub/pull/1656)
* **Escreve um nível de zoom exato no UI Editor**: a percentagem de zoom é agora um campo. Clica nele, escreve um valor de 10 a 200 e prime Enter. Veja [UI Editor](/docs/creator/content-creator-pt/scene-editor/construir/ui-editor/). [#1659](https://github.com/decentraland/creator-hub/pull/1659)
* **Silenciar o áudio da cena enquanto editas**: um botão de altifalante na barra de ferramentas do editor silencia todas as fontes de áudio e streams, no renderizador Bevy. Veja [Renderizador da cena](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/about-editor/#scene-renderer). [#1669](https://github.com/decentraland/creator-hub/pull/1669)
* **Opção Scene Inset renomeada**: "Gameplay Safe Area" é agora "Interactable Safe Area" no UI Editor. Escreve o mesmo `screenInset: 'interactable'` valor de sempre. Veja [UI Editor](/docs/creator/content-creator-pt/scene-editor/construir/ui-editor/). [#1673](https://github.com/decentraland/creator-hub/pull/1673)
* **Correções**: duplicação de Trigger Area e perda de campos de edição de forma [#1658](https://github.com/decentraland/creator-hub/pull/1658); a pré-visualização do MobileHUD é enquadrada contra o ecrã móvel em vez da GUI ativa [#1648](https://github.com/decentraland/creator-hub/pull/1648); renomear uma cena já não perde toques de teclas, e o cabeçalho do editor atualiza [#1651](https://github.com/decentraland/creator-hub/pull/1651); a consola de depuração já não desaparece depois de **Recarregar a cena a partir do disco** [#1652](https://github.com/decentraland/creator-hub/pull/1652); limpeza do desfazer no editor Bevy e aviso de dependência [#1669](https://github.com/decentraland/creator-hub/pull/1669); o logótipo da Decentraland é renderizado na dica de ferramenta do smart item [#1677](https://github.com/decentraland/creator-hub/pull/1677); as entities eliminadas continuam a poder ser restauradas com desfazer [#1679](https://github.com/decentraland/creator-hub/pull/1679); os rótulos longos das caixas de verificação quebram linha [#1676](https://github.com/decentraland/creator-hub/pull/1676).

### 0.50.0 (23 de set. de 2026)

* **Assets largados na pasta do projeto aparecem por si próprios**: ficheiros copiados para a `assets` pasta do projeto a partir de fora do editor aparecem em **Local Assets** sem premir **Atualizar**. Veja [Importar assets personalizados](/docs/creator/content-creator-pt/scene-editor/construir/import-items/). [#1606](https://github.com/decentraland/creator-hub/pull/1606)
* **O UI Editor está estável**: já não está atrás de uma definição experimental. Abre-o a partir do alternador 2D/3D. A tua cena precisa de `@dcl/sdk` 7.26.0 ou posterior. Veja [UI Editor](/docs/creator/content-creator-pt/scene-editor/construir/ui-editor/). [#1643](https://github.com/decentraland/creator-hub/pull/1643)
* **O AI Assistant está estável** e ativo por predefinição, com o seu próprio **AI** separador nas definições. Veja [AI Assistant](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/ai-assistant/). [#1646](https://github.com/decentraland/creator-hub/pull/1646)
* **Otimizar modelos 3D a partir do editor**: uma **Otimizar** ação na barra de título e no **Local Assets** painel comprime os teus modelos, com um **Reverter** para os repor. Veja [Otimizar modelos](https://github.com/decentraland/docs/tree/main/creator/scene-editor/build/optimize-models.md). [#1566](https://github.com/decentraland/creator-hub/pull/1566)
* **smart item Trigger Area**: coloca uma caixa invisível, escolhe se reage apenas ao jogador ou a toda a gente, e descreve em linguagem simples o que deve acontecer quando alguém entra ou sai. Veja [Trigger Area](/docs/creator/content-creator-pt/scene-editor/interatividade/trigger-area/). [#1604](https://github.com/decentraland/creator-hub/pull/1604)
* **MobileHUD personalizável no UI Editor**: escolhe o botão principal, mostra ou oculta os outros botões, o joystick e a mira, e dá-lhes ícones personalizados, com uma pré-visualização em direto no canvas. Veja [Personalizar os controlos móveis](/docs/creator/content-creator-pt/scene-editor/construir/ui-editor/#customize-the-mobile-controls). [#1615](https://github.com/decentraland/creator-hub/pull/1615)
* **Miniaturas da cena em 16:9**: a pré-visualização da miniatura ocupa toda a largura, imagens com outra proporção são recusadas, e a miniatura gerada automaticamente é capturada em 16:9. Veja [Miniatura da cena](/docs/creator/content-creator-pt/scene-editor/configurar/scene-settings/#scene-thumbnail). [#1601](https://github.com/decentraland/creator-hub/pull/1601)
* **As definições dos Componentes acompanham o SDK**: o inspetor expõe agora a velocidade de reprodução de vídeo, a posição inicial e o áudio espacial, a quebra de texto e o tamanho da caixa, o tom do áudio e os modos de mover e rodar de Material, com uma forma de limpar cores e texturas. As propriedades definidas a partir de código já não se perdem quando editas um componente. Veja [Components](/docs/creator/content-creator-pt/scene-editor/construir/components/). [#1470](https://github.com/decentraland/creator-hub/pull/1470)
* **Consola de depuração destacável**: destaca a consola para a sua própria janela e mantém-na num segundo monitor. Veja [A consola do Creator Hub](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/preview-scene/#the-creator-hub-console). [#1642](https://github.com/decentraland/creator-hub/pull/1642)
* **O AI Assistant pode usar as ferramentas de cena do Creator Hub no Cursor e no Gemini**, e não apenas Claude e Codex. Veja [AI Assistant](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/ai-assistant/). [#1614](https://github.com/decentraland/creator-hub/pull/1614)
* **Correções**: **Mostrar código QR para o telemóvel** funciona independentemente do cliente de pré-visualização selecionado, e a pré-visualização Bevy (Web) reutiliza o seu separador [#1608](https://github.com/decentraland/creator-hub/pull/1608); publicar já não espera pela geração de LOD, por isso o **A otimizar** passo desapareceu [#1603](https://github.com/decentraland/creator-hub/pull/1603); o UI Editor já não deixa um órfão `./ui` importação em cenas mais antigas [#1609](https://github.com/decentraland/creator-hub/pull/1609); o renderizador Bevy já não mostra o aviso de pré-visualização de "code elements" [#1607](https://github.com/decentraland/creator-hub/pull/1607); o Admin Toolkit já não altera a ordem Z do restante UI da cena [#1612](https://github.com/decentraland/creator-hub/pull/1612); a sobreposição dos atalhos de teclado funciona no Bevy, o scroll repõe-se na navegação, e os alternadores dos ícones da árvore de entities comportam-se [#1617](https://github.com/decentraland/creator-hub/pull/1617); refinamento do compositor do AI Assistant, do menu suspenso do chat e das notificações [#1632](https://github.com/decentraland/creator-hub/pull/1632), [#1621](https://github.com/decentraland/creator-hub/pull/1621).

### 0.49.0 (16 de set. de 2026)

* **smart item Locomotion Settings**: altera a velocidade a que os jogadores se movem e a altura dos saltos, com controlos deslizantes em vez de código. Veja [Definições de locomotion](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/#locomotion-settings). [#1584](https://github.com/decentraland/creator-hub/pull/1584)
* **O AI Assistant suporta Cursor e Gemini**, juntamente com Claude e Codex. Veja [AI Assistant](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/ai-assistant/). [#1591](https://github.com/decentraland/creator-hub/pull/1591)
* **Escreve valores exatos de Transform**: prime uma tecla da ferramenta, um eixo e um número para mover, rodar ou dimensionar por uma quantidade precisa em vez de arrastar. **F** já não é um atalho para o gizmo livre. Veja [Escrever uma quantidade exata](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/scene-editor-essentials/#type-an-exact-amount). [#1599](https://github.com/decentraland/creator-hub/pull/1599)
* **Correções**: o UI Editor já não obscurece a vista com contentores de ecrã inteiro, e os valores de pré-visualização da GUI aninhada e o posicionamento livre funcionam [#1590](https://github.com/decentraland/creator-hub/pull/1590); o componente Animator é apresentado no editor Bevy [#1587](https://github.com/decentraland/creator-hub/pull/1587); o hot-reload do editor Bevy é acionado por eventos de build reais [#1592](https://github.com/decentraland/creator-hub/pull/1592); os turnos de seguimento do AI Assistant voltam a funcionar com Codex [#1600](https://github.com/decentraland/creator-hub/pull/1600); os menus dropdown e kebab já não são cortados [#1595](https://github.com/decentraland/creator-hub/pull/1595); Create Scene com um clique, os nomes das entities mantêm espaços consecutivos, o menu do card da cena já não reorganiza a grelha, as dicas de ferramenta dos assets aparecem corretamente nos resultados da pesquisa, e podes arrastar a partir da árvore Local Assets [#1593](https://github.com/decentraland/creator-hub/pull/1593).

### 0.48.0 (10 de set. de 2026)

* **Pré-visualização móvel do UI Editor**: uma moldura de telemóvel real com uma área segura precisa, usando as mesmas predefinições de dispositivo que o client, mais guias opcionais do HUD (joystick, botões, chat) que podes ativar ou desativar. [#1579](https://github.com/decentraland/creator-hub/pull/1579)
* **Redesign do AI Assistant**: painel mais limpo, botão de fechar fiável, mensagem de introdução persistente e melhores definições. Veja [AI Assistant](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/ai-assistant/). [#1586](https://github.com/decentraland/creator-hub/pull/1586)
* **Analytics mostra o tempo de jogo mediano** em vez da média. [#1564](https://github.com/decentraland/creator-hub/pull/1564)
* **Alterações**: o renderizador Bevy está rotulado como "experimental" [#1583](https://github.com/decentraland/creator-hub/pull/1583); o campo de definições da cena é agora "Creator name" [#1561](https://github.com/decentraland/creator-hub/pull/1561); o aviso de atraso do pacote de assets no ecrã de publicação desapareceu [#1582](https://github.com/decentraland/creator-hub/pull/1582).
* **Correção**: no Windows, os turnos do AI Assistant já não falham quando a linha de comandos fica demasiado longa. [#1588](https://github.com/decentraland/creator-hub/pull/1588)

### 0.47.0 (4 de set. de 2026)

* **Scene Agent**: um assistente de IA integrado no editor. Lê a tua cena, escreve código SDK7, cria e edita entities, components, smart items e scripts em direto na viewport, e pode executar a pré-visualização para verificar o seu trabalho. Controla a tua CLI Claude ou Codex instalada, por isso não há nenhuma API key para configurar. [#1499](https://github.com/decentraland/creator-hub/pull/1499)
* **Os smart items funcionam com components Script**: o comportamento de cada smart item vive agora como código num `.ts` ficheiro dentro da pasta do item, com as suas funções principais expostas como ações. Podes lê-lo e ajustá-lo. [#1354](https://github.com/decentraland/creator-hub/pull/1354)
* **Entradas de slider para smart items Script**: um parâmetro numérico pode declarar um intervalo e é apresentado como um slider. Veja [Usar o Script Component](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/script-component/). [#1513](https://github.com/decentraland/creator-hub/pull/1513)
* **UI Editor**: a posição de um nó (fixo ou em fluxo) é independente da forma como organiza os seus filhos, e os nós têm ícones ao estilo do Figma. [#1552](https://github.com/decentraland/creator-hub/pull/1552)
* **Uma lista de tutoriais partilhada** na barra lateral Scenes e na página Learn. [#1551](https://github.com/decentraland/creator-hub/pull/1551)
* **Correções**: os Macs Intel voltam a arrancar [#1545](https://github.com/decentraland/creator-hub/pull/1545); o UI Editor lembra-se do modo 2D ou 3D [#1532](https://github.com/decentraland/creator-hub/pull/1532); as animações importadas têm por predefinição um weight de 1 [#1557](https://github.com/decentraland/creator-hub/pull/1557); o componente Script aparece logo a seguir a GLTF no painel de entities [#1556](https://github.com/decentraland/creator-hub/pull/1556); os custom items carregam sob o renderizador Bevy [#1555](https://github.com/decentraland/creator-hub/pull/1555).

### 0.46.0 (27 de ago. de 2026)

* **Contentor de ecrã inteiro** no UI Editor: um contentor que preenche todo o ecrã com um clique. [#1540](https://github.com/decentraland/creator-hub/pull/1540)

### 0.45.0 (27 de ago. de 2026)

* **UI Editor (experimental)**: desenha visualmente a interface no ecrã da tua cena num canvas 2D com uma árvore de nodes e um painel de propriedades. O teu `@dcl/react-ecs` código continua a ser a única fonte de verdade. Ativa-o em Settings > Experimental. Requer SDK 7.26.0 ou posterior. Veja [UI Editor](/docs/creator/content-creator-pt/scene-editor/construir/ui-editor/). [#1523](https://github.com/decentraland/creator-hub/pull/1523), [#1522](https://github.com/decentraland/creator-hub/pull/1522), [#1521](https://github.com/decentraland/creator-hub/pull/1521), [#1520](https://github.com/decentraland/creator-hub/pull/1520), [#1524](https://github.com/decentraland/creator-hub/pull/1524)
* **Áreas de Modificador de Avatar e Câmera**: altera a forma como os avatares aparecem e como a câmara se comporta dentro de uma região. Adiciona-as como components ou como smart items. Veja [Áreas de Modificador](/docs/creator/content-creator-pt/scene-editor/interatividade/modifier-areas/). [#1471](https://github.com/decentraland/creator-hub/pull/1471)
* **Workshops do Friendzone Buildathon 2026** em Learn > Videos. [#1510](https://github.com/decentraland/creator-hub/pull/1510)
* **Correções**: publicar para LAND já não fica preso em "Publishing…" [#1496](https://github.com/decentraland/creator-hub/pull/1496); o botão de confirmar funciona ao publicar para um World a partir do editor Bevy [#1493](https://github.com/decentraland/creator-hub/pull/1493); Analytics lista todas as cenas implementadas [#1492](https://github.com/decentraland/creator-hub/pull/1492); novos NAMEs e permissões de World aparecem sem reiniciar [#1509](https://github.com/decentraland/creator-hub/pull/1509); a caixa de diálogo de atualização do Windows "Creator Hub cannot be closed" desapareceu [#1497](https://github.com/decentraland/creator-hub/pull/1497); o separador More deixa de mostrar o card Legacy Web Editor e liga Names ao marketplace [#1495](https://github.com/decentraland/creator-hub/pull/1495).

### 0.44.2 (14 de ago. de 2026)

* **Analytics**: visitantes, retenção, visitas e interação para cada cena de World e Genesis City que possuas ou na qual colabores, separados por desktop e mobile. Veja [Scene Analytics](/docs/creator/content-creator-pt/scenes-sdk7/outros/scene-analytics/). [#1447](https://github.com/decentraland/creator-hub/pull/1447)
* **Ecrã inicial redesenhado.** [#1481](https://github.com/decentraland/creator-hub/pull/1481)
* **Correções**: as importações de GLB já não ficam presas num indicador de carregamento [#1487](https://github.com/decentraland/creator-hub/pull/1487); o servidor de multiplayer local volta a arrancar nas pré-visualizações da cena [#1455](https://github.com/decentraland/creator-hub/pull/1455).

### 0.44.0 (4 de ago. de 2026)

* **Renderizador Bevy (experimental)**: um renderizador alternativo para a viewport do editor. [#1330](https://github.com/decentraland/creator-hub/pull/1330), [#1363](https://github.com/decentraland/creator-hub/pull/1363)
* **Opção de pré-visualização Optimize Assets**: pré-visualiza com pacotes de assets para que os modelos se vejam como ficarão depois de publicados. Veja [Pré-visualização com assets otimizados](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/preview-scene/#preview-with-optimized-assets). [#1396](https://github.com/decentraland/creator-hub/pull/1396)
* **Alternador Explorer MCP** nas opções de pré-visualização. [#1440](https://github.com/decentraland/creator-hub/pull/1440)
* **Pesquisa na árvore de entities.** [#1385](https://github.com/decentraland/creator-hub/pull/1385)
* **Renomear a pasta de um projeto** a partir da vista Scenes. [#1357](https://github.com/decentraland/creator-hub/pull/1357)
* **definição da cena "Hide Landscape Terrain".** [#1383](https://github.com/decentraland/creator-hub/pull/1383)
* **Admin tools redesenhadas** para o novo design da Decentraland. [#1360](https://github.com/decentraland/creator-hub/pull/1360)
* **Removida a opção de publicação para Test Server.** [#1441](https://github.com/decentraland/creator-hub/pull/1441)
* **Correções**: a pré-visualização por QR no mobile já não abre o cliente de desktop [#1452](https://github.com/decentraland/creator-hub/pull/1452); os alternadores de bloquear e ocultar na árvore de entities podem voltar a ser desativados [#1429](https://github.com/decentraland/creator-hub/pull/1429); Custom Items incluem assets do componente Placeholder [#1287](https://github.com/decentraland/creator-hub/pull/1287); o painel de administração do DCL Cast abre automaticamente, mantém o estado dos slides e replica para quem entra mais tarde [#1358](https://github.com/decentraland/creator-hub/pull/1358); as admin tools saltam a inicialização no servidor autoritativo [#1381](https://github.com/decentraland/creator-hub/pull/1381).

### 0.43.0 (6 de jul. de 2026)

* **Início de sessão mais simples**: uma única confirmação no browser em vez de escrever um código. [#1338](https://github.com/decentraland/creator-hub/pull/1338)
* **Correções**: erro claro quando uma implementação excede o tempo limite [#1355](https://github.com/decentraland/creator-hub/pull/1355); contagem correta de parcelas para cenas que abrangem coordenadas negativas e positivas [#1352](https://github.com/decentraland/creator-hub/pull/1352); mensagem "Scene already imported" ao importar novamente [#1350](https://github.com/decentraland/creator-hub/pull/1350).

### 0.42.0 (26 de jun. de 2026)

* **componente Particle System** no editor. Veja [Sistema de Partículas](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/particle-system/). [#1311](https://github.com/decentraland/creator-hub/pull/1311)
* **ação Spawn Entity**: cria um smart item a partir de uma cadeia de ações, em vez de pré-colocar cópias ocultas. [#1322](https://github.com/decentraland/creator-hub/pull/1322)
* **Copiar e colar valores de components** a partir do menu de três pontos de um component. [#1321](https://github.com/decentraland/creator-hub/pull/1321)
* **Até 100 colaboradores** por World. [#1310](https://github.com/decentraland/creator-hub/pull/1310)
* **Os colaboradores podem retirar da publicação cenas de World.** [#1209](https://github.com/decentraland/creator-hub/pull/1209)
* **Alterações**: a classificação etária Teen é removida [#1329](https://github.com/decentraland/creator-hub/pull/1329); um aviso explica que a transmissão em direto não é suportada no modo de jogador único [#1336](https://github.com/decentraland/creator-hub/pull/1336).
* **Correções**: Wind Area e Impulse Area reagem apenas ao jogador atual [#1323](https://github.com/decentraland/creator-hub/pull/1323); a UI das admin tools adapta-se a ecrãs móveis [#1256](https://github.com/decentraland/creator-hub/pull/1256); controlos para vários components [#1282](https://github.com/decentraland/creator-hub/pull/1282); erros de arranque do inspetor [#1334](https://github.com/decentraland/creator-hub/pull/1334).

### 0.41.1 (29 de abr. de 2026)

* **Botão Atualizar** na barra superior reinicia o editor, útil depois de editar a cena com ferramentas de IA. [#1312](https://github.com/decentraland/creator-hub/pull/1312)
* **A consola faz auto-scroll** e suporta copiar texto. [#1305](https://github.com/decentraland/creator-hub/pull/1305)

### 0.41.0 (27 de abr. de 2026)

* **Sessão de depuração móvel**: emparelha com o cliente móvel via QR e transmite o estado das entities, logs da consola e dados de desempenho para o editor, com navegação por ticks e comandos de pausar, retomar e recarregar. [#1306](https://github.com/decentraland/creator-hub/pull/1306)
* **Speaker Showcase e painel de apresentação para DCL Cast**: gere os speakers em direto e os slides a partir das Admin Tools. [#1213](https://github.com/decentraland/creator-hub/pull/1213)
* **Erros ao nível da entity** na árvore de entities: um ícone vermelho assinala entities com valores de components inválidos. [#1258](https://github.com/decentraland/creator-hub/pull/1258)
* **"Disable Nearby Voice Chat"** restrição da cena. [#1289](https://github.com/decentraland/creator-hub/pull/1289)
* **botão "View storage"** para Worlds, para ver e editar dados do servidor autoritativo. [#1307](https://github.com/decentraland/creator-hub/pull/1307)
* **smart items Wind Area e Impulse Area** que empurram o jogador. [#1296](https://github.com/decentraland/creator-hub/pull/1296), [#1308](https://github.com/decentraland/creator-hub/pull/1308)
* **smart items de recompensa**: Balões, dois dispensadores de Wearables e um dispensador de resgate Clap que distribuem wearables de uma campanha de recompensas. [#1124](https://github.com/decentraland/creator-hub/pull/1124)
* **Correções**: os valores de rotação já não derivam ao sair do campo [#1266](https://github.com/decentraland/creator-hub/pull/1266); a caixa de verificação "Fixed Skybox" já não alterna com outras caixas de verificação [#1254](https://github.com/decentraland/creator-hub/pull/1254); publicar para um World só retira da publicação cenas cujas parcelas se sobrepõem [#1271](https://github.com/decentraland/creator-hub/pull/1271); os duplicados aparecem junto do original na árvore [#1150](https://github.com/decentraland/creator-hub/pull/1150); modelo atualizado da trigger area [#1285](https://github.com/decentraland/creator-hub/pull/1285).

### 0.39.1 (30 de mar. de 2026)

* **"Show in Places"** caixa de verificação nas definições de World. [#1264](https://github.com/decentraland/creator-hub/pull/1264)
* **Correções**: a pré-visualização mostra o erro quando o código da cena está quebrado em vez de falhar em silêncio [#1238](https://github.com/decentraland/creator-hub/pull/1238); sinalizador de multi-scene validado no ficheiro de implementação [#1270](https://github.com/decentraland/creator-hub/pull/1270).

### 0.39.0 (26 de mar. de 2026)

* **Correções**: atualização do instalador do Windows [#1252](https://github.com/decentraland/creator-hub/pull/1252); implementação multi-scene [#1262](https://github.com/decentraland/creator-hub/pull/1262); smart items que não estavam a funcionar [#1260](https://github.com/decentraland/creator-hub/pull/1260); trigger e click areas mostram uma malha do ponteiro [#1255](https://github.com/decentraland/creator-hub/pull/1255).

### 0.38.0 (20 de mar. de 2026)

* **Correções**: os asset packs voltam a fazer scroll [#1239](https://github.com/decentraland/creator-hub/pull/1239); ficheiros de áudio mantidos ao criar um custom smart item numa cena nova [#1234](https://github.com/decentraland/creator-hub/pull/1234).

### 0.37.0 (16 de mar. de 2026)

* **Consola de depuração incorporada** como um separador no painel de Assets. [#1200](https://github.com/decentraland/creator-hub/pull/1200)
* **Correções**: os custom items mantêm os seus ficheiros de textura [#1217](https://github.com/decentraland/creator-hub/pull/1217); os gizmos funcionam na primeira seleção [#1218](https://github.com/decentraland/creator-hub/pull/1218); a ação Show Image assume por predefinição alinhamento ao centro [#1221](https://github.com/decentraland/creator-hub/pull/1221); o texto da fonte de vídeo já não menciona apenas Vimeo [#1216](https://github.com/decentraland/creator-hub/pull/1216).

### 0.36.0 (10 de mar. de 2026)

* **Correções**: seleção do spawn point na árvore de entities [#1199](https://github.com/decentraland/creator-hub/pull/1199); "Restricted" removido das classificações de conteúdo de World [#1205](https://github.com/decentraland/creator-hub/pull/1205); Manage Published lista apenas Worlds com cenas publicadas [#1208](https://github.com/decentraland/creator-hub/pull/1208); erro de implementação quando o limite de World é excedido [#1210](https://github.com/decentraland/creator-hub/pull/1210).

### 0.35.0 (5 de mar. de 2026)

* **Visualização dos spawn points** no editor. Veja [Spawn Areas](/docs/creator/content-creator-pt/scene-editor/construir/spawn-areas/). [#1129](https://github.com/decentraland/creator-hub/pull/1129)
* **Catálogo dinâmico de assets**: os itens mais recentes são carregados em runtime, com uma verificação de compatibilidade do SDK. [#1172](https://github.com/decentraland/creator-hub/pull/1172)
* **Pré-visualização com múltiplas instâncias** alternador para testar multiplayer. Requer SDK 7.20.4 ou posterior. [#1187](https://github.com/decentraland/creator-hub/pull/1187)
* **Separador Access das permissões de World**, e as definições de Multiplayer e Skybox de volta em World Settings. [#1186](https://github.com/decentraland/creator-hub/pull/1186), [#1189](https://github.com/decentraland/creator-hub/pull/1189)
* **Atualização do UI** das cores, estilos de components e do inspetor de entities. [#1178](https://github.com/decentraland/creator-hub/pull/1178)
* **Correções**: TextShape inicializa corretamente [#1194](https://github.com/decentraland/creator-hub/pull/1194) e renderiza sem corte [#1185](https://github.com/decentraland/creator-hub/pull/1185); mosaicos de Ground duplicados [#1179](https://github.com/decentraland/creator-hub/pull/1179); atualizações do layout da cena no modo avançado [#1181](https://github.com/decentraland/creator-hub/pull/1181); o destaque da seleção já não atrasa nem torna os objetos pretos [#1196](https://github.com/decentraland/creator-hub/pull/1196); projetos geridos carregam [#1182](https://github.com/decentraland/creator-hub/pull/1182); a UI de administração da cena adapta-se a todos os ecrãs [#1190](https://github.com/decentraland/creator-hub/pull/1190).

### 0.34.0 (25 de fev. de 2026)

* **Worlds com múltiplas cenas**: publica várias cenas para um World. [#1089](https://github.com/decentraland/creator-hub/pull/1089), [#1163](https://github.com/decentraland/creator-hub/pull/1163)
* **Gerir Worlds**: definições de World e uma página de gestão. [#1132](https://github.com/decentraland/creator-hub/pull/1132), [#1049](https://github.com/decentraland/creator-hub/pull/1049), [#1112](https://github.com/decentraland/creator-hub/pull/1112)
* **Permissões de colaborador** para Worlds. [#1128](https://github.com/decentraland/creator-hub/pull/1128), [#1083](https://github.com/decentraland/creator-hub/pull/1083), [#1125](https://github.com/decentraland/creator-hub/pull/1125)
* **Retirar da publicação uma cena de um World.** [#1123](https://github.com/decentraland/creator-hub/pull/1123)
* **componente Placeholder**: entities invisíveis para sit spots, trigger areas e semelhantes, sem afetar as métricas da cena. [#1154](https://github.com/decentraland/creator-hub/pull/1154)
* **Ações Log e Delete** para smart items. [#1131](https://github.com/decentraland/creator-hub/pull/1131)
* **Correções**: as áreas de trigger só são ativadas para o seu próprio jogador [#1142](https://github.com/decentraland/creator-hub/pull/1142); os devtools para o tráfego web da cena voltam a funcionar [#1109](https://github.com/decentraland/creator-hub/pull/1109); duração da Virtual Camera e auto-seleção em "Change camera" [#1160](https://github.com/decentraland/creator-hub/pull/1160); gizmo de escala [#1082](https://github.com/decentraland/creator-hub/pull/1082); smart item Photo Wall [#1110](https://github.com/decentraland/creator-hub/pull/1110); o aviso "Custom Code" está mais preciso [#1171](https://github.com/decentraland/creator-hub/pull/1171); problemas de acesso ao World [#1133](https://github.com/decentraland/creator-hub/pull/1133); miniaturas da cena [#1168](https://github.com/decentraland/creator-hub/pull/1168); o ícone Guardar não utilizado e o aviso "Slow connection" foram removidos [#1152](https://github.com/decentraland/creator-hub/pull/1152), [#1169](https://github.com/decentraland/creator-hub/pull/1169).

### 0.33.1 (18 fev. 2026)

* **Pré-visualização na app móvel**, configurável nas definições de pré-visualização. [#1121](https://github.com/decentraland/creator-hub/pull/1121)
* **Tooltips do componente Script** a partir dos comentários no ficheiro de script, e um template melhorado. [#1130](https://github.com/decentraland/creator-hub/pull/1130), [#1019](https://github.com/decentraland/creator-hub/pull/1019)
* **Seleção de fonte** no componente TextShape. [#1096](https://github.com/decentraland/creator-hub/pull/1096)
* **Correções**: gizmo de rotação [#1081](https://github.com/decentraland/creator-hub/pull/1081); edição de smart items de luz [#1139](https://github.com/decentraland/creator-hub/pull/1139); erro falso "signal is aborted" ao publicar [#1144](https://github.com/decentraland/creator-hub/pull/1144); o gizmo já não muda para movimento livre após duplicar [#1088](https://github.com/decentraland/creator-hub/pull/1088); smart items Clap Meter e Photo Wall [#1101](https://github.com/decentraland/creator-hub/pull/1101).

### 0.33.0 (4 fev. 2026)

* **Métodos de Script como ações**: o dropdown de tipo de Action inclui métodos dos componentes Script. [#1026](https://github.com/decentraland/creator-hub/pull/1026), [#1071](https://github.com/decentraland/creator-hub/pull/1071)
* **`isAdmin` função** para saber se o jogador é um administrador da cena. [#1037](https://github.com/decentraland/creator-hub/pull/1037)
* **página de armazenamento do Worlds.** [#1036](https://github.com/decentraland/creator-hub/pull/1036)
* **Modo offline** com uma página "Sem ligação". [#991](https://github.com/decentraland/creator-hub/pull/991)
* **Correções**: a pesquisa de assets considera tags e descrições [#1022](https://github.com/decentraland/creator-hub/pull/1022); o gizmo de escala responde ao arrastar para a esquerda e para a direita e não permite definir o tamanho para 0 [#1065](https://github.com/decentraland/creator-hub/pull/1065); escalamento proporcional com valores abaixo de 1 [#1076](https://github.com/decentraland/creator-hub/pull/1076); o estado de bloqueio não é herdado pelos filhos [#1085](https://github.com/decentraland/creator-hub/pull/1085); Photo Mural suporta imagens de Worlds [#1074](https://github.com/decentraland/creator-hub/pull/1074); itens personalizados só de imagem criam a sua pasta [#1033](https://github.com/decentraland/creator-hub/pull/1033); valores decimais nos sliders [#1075](https://github.com/decentraland/creator-hub/pull/1075).

### 0.32.1 (21 jan. 2026)

* **Novas ações**: Freeze Player, Unfreeze Player, Face Item, Change Collisions, Change Skybox, Reset Skybox, mais um smart item de Camera para "Change Camera". [#997](https://github.com/decentraland/creator-hub/pull/997), [#989](https://github.com/decentraland/creator-hub/pull/989)
* **Alterar vários States de uma vez** ao selecionar vários itens. [#975](https://github.com/decentraland/creator-hub/pull/975)
* **Atalho para republish**: fazer redeploy para o mesmo World ou coordenadas sem passar pelos menus de configuração. [#1003](https://github.com/decentraland/creator-hub/pull/1003)
* **Novos smart items com script**: Clap Meter, Wearables Scanner, Zombie e Photo Mural. O Padlock pode acionar "Open" noutra entidade. [#1008](https://github.com/decentraland/creator-hub/pull/1008)
* **`ActionRef` e `ActionCallback` tipos de input** para Scripts, para lidar com as ações de outro item a partir do código. [#1054](https://github.com/decentraland/creator-hub/pull/1054)
* **Tooltips** em componentes, ações e smart items. [#1009](https://github.com/decentraland/creator-hub/pull/1009)
* **página Manage Projects.** [#1032](https://github.com/decentraland/creator-hub/pull/1032)
* **Correções**: os modelos de ecrã de vídeo são de dois lados [#1007](https://github.com/decentraland/creator-hub/pull/1007); as métricas da cena contam triângulos, materiais e texturas corretamente [#1020](https://github.com/decentraland/creator-hub/pull/1020).

### 0.32.0 (5 jan. 2026)

* **Editar um componente em várias entities ao mesmo tempo**: Tag, Material, AudioSource, AudioStream, MeshCollider, TextShape, Visibility e LightSource. [#949](https://github.com/decentraland/creator-hub/pull/949), [#950](https://github.com/decentraland/creator-hub/pull/950), [#958](https://github.com/decentraland/creator-hub/pull/958), [#959](https://github.com/decentraland/creator-hub/pull/959), [#960](https://github.com/decentraland/creator-hub/pull/960), [#963](https://github.com/decentraland/creator-hub/pull/963), [#964](https://github.com/decentraland/creator-hub/pull/964)
* **Campo Creator** no inspetor da entidade. [#944](https://github.com/decentraland/creator-hub/pull/944)
* **Nova UI de definições da app.** [#988](https://github.com/decentraland/creator-hub/pull/988)
* **Limpar Assets Não Utilizados**: selecionar tudo, estado intermédio e undo. [#993](https://github.com/decentraland/creator-hub/pull/993)
* **Mais componentes** disponíveis no editor. [#999](https://github.com/decentraland/creator-hub/pull/999)
* **Correções**: os itens ocultos continuam invisíveis [#974](https://github.com/decentraland/creator-hub/pull/974); materiais transparentes [#976](https://github.com/decentraland/creator-hub/pull/976); o indicador de eixo mostra o norte [#978](https://github.com/decentraland/creator-hub/pull/978); deploy com uma identidade expirada [#981](https://github.com/decentraland/creator-hub/pull/981); o modal de publish já não fecha ao clicar no backdrop [#982](https://github.com/decentraland/creator-hub/pull/982); apresentação de erros de deployment [#985](https://github.com/decentraland/creator-hub/pull/985); sobreposição do dropdown de smart item [#995](https://github.com/decentraland/creator-hub/pull/995); undo já não cria um template indelével [#998](https://github.com/decentraland/creator-hub/pull/998); desempenho do gizmo de posição [#972](https://github.com/decentraland/creator-hub/pull/972).

### 0.31.0 e 0.31.1 (16 dez. 2025)

* **Gizmos de posição planares.** [#943](https://github.com/decentraland/creator-hub/pull/943)
* **Pan da câmara** e um **auxiliar de eixos** na viewport. [#952](https://github.com/decentraland/creator-hub/pull/952), [#951](https://github.com/decentraland/creator-hub/pull/951)
* **DCL Cast** smart item para fazer streaming de vídeo para uma cena. [#957](https://github.com/decentraland/creator-hub/pull/957)
* **Novas ações** em smart items. [#955](https://github.com/decentraland/creator-hub/pull/955)
* **Desseleciona ao clicar numa área vazia.** [#961](https://github.com/decentraland/creator-hub/pull/961)
* **feedback "Link copied"** no botão de partilha. [#965](https://github.com/decentraland/creator-hub/pull/965)
* **Correções**: importar vários ficheiros [#931](https://github.com/decentraland/creator-hub/pull/931); posição final ao escalar vários objetos [#962](https://github.com/decentraland/creator-hub/pull/962).

### 0.30.0 (9 dez. 2025)

* **componente Script**: anexar código personalizado a qualquer item. Consulte [Usar o Script Component](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/script-component/). [#886](https://github.com/decentraland/creator-hub/pull/886)
* **Prima F** para centrar a câmara na entidade selecionada. [#948](https://github.com/decentraland/creator-hub/pull/948)
* **Aviso quando uma cena tem código personalizado.** [#903](https://github.com/decentraland/creator-hub/pull/903)
* **O separador de assets personalizados mostra um tutorial** quando está vazio. [#925](https://github.com/decentraland/creator-hub/pull/925)
* **Correções**: rodar vários itens em conjunto [#946](https://github.com/decentraland/creator-hub/pull/946); Add Component oculto nas entities da cena, da câmara e do jogador [#923](https://github.com/decentraland/creator-hub/pull/923); erros mais claros quando uma cena falha ao carregar [#921](https://github.com/decentraland/creator-hub/pull/921); o Windows pede para descarregar o launcher [#899](https://github.com/decentraland/creator-hub/pull/899); todos os duplicados permanecem selecionados [#929](https://github.com/decentraland/creator-hub/pull/929); orientação do placeholder do video player [#909](https://github.com/decentraland/creator-hub/pull/909); numeração de duplicados [#933](https://github.com/decentraland/creator-hub/pull/933); a pasta Custom Items é criada quando está em falta [#940](https://github.com/decentraland/creator-hub/pull/940); Clean Unused Assets remove pastas vazias [#924](https://github.com/decentraland/creator-hub/pull/924); publicar cenas com código personalizado [#941](https://github.com/decentraland/creator-hub/pull/941); campos de checkbox na vista básica do smart item [#922](https://github.com/decentraland/creator-hub/pull/922).

### 0.29.3 (20 nov. 2025)

* **Correções**: as cenas recém-importadas já não ficam presas a carregar [#908](https://github.com/decentraland/creator-hub/pull/908); já não há várias janelas do Finder ou do Explorer [#906](https://github.com/decentraland/creator-hub/pull/906); fluxo de deployment [#896](https://github.com/decentraland/creator-hub/pull/896); smart items de ecrã de vídeo [#898](https://github.com/decentraland/creator-hub/pull/898).

### 0.29.1 e 0.29.2 (13 nov. 2025)

* **Correções**: rendering do Video Player e a sua entrada na lista de componentes, com arrastar e largar [#872](https://github.com/decentraland/creator-hub/pull/872), [#873](https://github.com/decentraland/creator-hub/pull/873); smart item NFT [#885](https://github.com/decentraland/creator-hub/pull/885); Clean Unused Assets já não danifica modelos ou miniaturas [#887](https://github.com/decentraland/creator-hub/pull/887), [#882](https://github.com/decentraland/creator-hub/pull/882); o editor fica esbatido enquanto as dependências são atualizadas [#891](https://github.com/decentraland/creator-hub/pull/891).

### 0.29.0 (5 nov. 2025)

* **Limpar Assets Não Utilizados** ferramenta. [#857](https://github.com/decentraland/creator-hub/pull/857)
* **painel de ajuda da cena**: o ficheiro de ajuda markdown de uma cena é mostrado no editor, útil para templates. [#833](https://github.com/decentraland/creator-hub/pull/833)
* **Atalho para abrir a pasta da cena** no Finder ou no Explorer. [#853](https://github.com/decentraland/creator-hub/pull/853)
* **Correções**: downloads de templates [#845](https://github.com/decentraland/creator-hub/pull/845); campo de contract do smart item NFT [#839](https://github.com/decentraland/creator-hub/pull/839); as entities da câmara e do jogador não podem ser parented [#855](https://github.com/decentraland/creator-hub/pull/855).

### 0.28.0 (23 out. 2025)

* **Lights e modificadores de nodes** no editor. Veja [Lights](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/lights/). [#813](https://github.com/decentraland/creator-hub/pull/813)
* **Correção**: os itens duplicados podem ser movidos. [#815](https://github.com/decentraland/creator-hub/pull/815)

### 0.27.5 (15 out. 2025)

* **Marcação de entities.** [#790](https://github.com/decentraland/creator-hub/pull/790)
* **ficheiros de contexto de IA** são transferidos para cada cena quando é criada ou atualizada, para que os assistentes de código conheçam o SDK. [#817](https://github.com/decentraland/creator-hub/pull/817)
* **Submeter um evento** cartão na secção More. [#811](https://github.com/decentraland/creator-hub/pull/811)
* **Correções**: o peso da animação aceita decimais [#826](https://github.com/decentraland/creator-hub/pull/826); miniaturas de som e de modelos [#822](https://github.com/decentraland/creator-hub/pull/822), [#804](https://github.com/decentraland/creator-hub/pull/804); debugger de pré-visualização em branco [#821](https://github.com/decentraland/creator-hub/pull/821); os assets atualizam-se após a importação [#809](https://github.com/decentraland/creator-hub/pull/809); deploys através do servidor linker [#812](https://github.com/decentraland/creator-hub/pull/812).

### 0.25.3 (6 out. 2025)

* **Chrome DevTools** disponível no Creator Hub para depurar cenas. [#766](https://github.com/decentraland/creator-hub/pull/766)
* **Correção**: criar novas cenas. [#794](https://github.com/decentraland/creator-hub/pull/794)

### 0.24.1 (17 set. 2025)

* **Definição predefinida do editor de código**: escolha VS Code, Cursor ou outro editor instalado. [#751](https://github.com/decentraland/creator-hub/pull/751)
* **Eliminar ficheiros do disco** ao eliminar um projeto, como opção. [#787](https://github.com/decentraland/creator-hub/pull/787)
* **Correções**: as texturas podem ser removidas e carregadas a partir de URLs [#772](https://github.com/decentraland/creator-hub/pull/772), [#779](https://github.com/decentraland/creator-hub/pull/779); redimensionamento do canvas ao eliminar uma entidade [#770](https://github.com/decentraland/creator-hub/pull/770).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/referencia-da-api/releases.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
