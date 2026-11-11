> Crie scenes do Decentraland com assistentes de IA usando o Creator Hub e o SDK Skills oficial do Decentraland (npx skills add decentraland/sdk-skills).

# Vibe Coding com IA

Crie cenas de Decentraland descrevendo o que você quer. Um assistente de IA lida com o código SDK7, a arquitetura ECS e a estrutura do projeto para você.

Seja você um criador de primeira viagem ou um desenvolvedor experiente, o "vibe coding" assistido por IA permite ir de uma ideia a uma cena em execução em minutos, em vez de horas.


> [!NOTE]
> **💡 Dica**: Você não precisa saber TypeScript para começar. Assistentes de IA podem gerar código de cena funcional a partir de descrições em linguagem natural.


## O que é Vibe Coding?

Vibe coding significa criar cenas conversando com um assistente de IA em vez de escrever cada linha de código manualmente. Você descreve o que quer — "uma taverna medieval com portas clicáveis e música de fundo" — e a IA escreve código SDK7 correto e pronto para deploy.

Essa abordagem funciona em qualquer nível de habilidade:

* **Iniciantes e não desenvolvedores** — Vá do zero a uma cena funcional sem escrever código manualmente.
* **Desenvolvedores experientes** — Pule o boilerplate. Deixe a IA lidar com a sincronização multiplayer, o esqueleto da UI e a configuração de deploy enquanto você se concentra nas decisões criativas.
* **Equipes e estúdios** — Prototipe rapidamente conceitos de cena antes de comprometer recursos completos de desenvolvimento.

## Use o Assistente de IA no Creator Hub

A maneira mais rápida de começar é o [Assistente de IA](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/ai-assistant/) integrado do Creator Hub. Ele vê sua cena aberta, edita entities, components, smart items e código para você, e pode executar a preview para verificar o próprio trabalho. Ele instala automaticamente no seu projeto as skills abaixo, então não há nada a configurar além de entrar em um CLI de codificação.

É um recurso experimental, desativado por padrão.

## Combine um editor de código com IA

Use um editor de código de IA de uso geral como [Cursor](https://www.cursor.com/) ou VS Code com GitHub Copilot ou Claude AI. A Decentraland fornece uma pasta de contexto para que essas ferramentas entendam o SDK.

1. Abra o Creator Hub e crie ou abra uma cena.
2. Clique no **< > CODE** botão para abrir seu editor de código.
3. Use o assistente de IA integrado do editor (o chat do Cursor, o Copilot etc.) para gerar ou modificar código.

## Instale Skills para qualquer agente de IA

Skills são conjuntos de instruções prontos que ensinam seu agente de IA a trabalhar com o SDK da Decentraland. Cada skill cobre um tópico específico, como criar cenas, adicionar modelos 3D ou configurar multiplayer, então a IA já conhece os padrões, APIs e restrições corretos sem que você precise explicá-los. Instalar skills significa menos erros e melhores resultados desde o primeiro prompt.

```bash
# Escolha quais skills de Decentraland instalar com um seletor interativo
npx skills add decentraland/sdk-skills

# Ou instale todas as skills de Decentraland
npx skills add decentraland/sdk-skills --all

# Ou escolha skills específicas
npx skills add decentraland/sdk-skills --skill create-scene

# Instalar globalmente (disponível em todos os projetos)
npx skills add decentraland/sdk-skills -g
```

Isso copia os arquivos da skill para a configuração do seu agente, para que ele conheça os padrões e restrições da Decentraland.

## Atualizando Skills

Novas skills são adicionadas ao longo do tempo, e as existentes são aprimoradas. Para obter as versões mais recentes, execute novamente o comando de instalação com `--all`:

```bash
# Atualize cada skill instalada e baixe quaisquer novas
npx skills add decentraland/sdk-skills --all
```

Executar `add` novamente faz uma nova busca no repositório, então isso atualiza as skills que você já tem e instala quaisquer que tenham sido adicionadas desde a sua primeira instalação. Se você instalou skills globalmente, adicione `-g` a este comando também.


> [!WARNING]
> **📔 Nota**: Não use `npx skills update` para isso. Ele apenas atualiza as skills já presentes na sua máquina, então quaisquer skills adicionadas ao repositório após sua última instalação são ignoradas silenciosamente. Use sempre `npx skills add decentraland/sdk-skills --all` em vez disso.


## Depure uma scene multiplayer implantada

A skill atualizada `authoritative-server`  pode pesquisar logs persistentes do servidor, acompanhar logs por um período limitado e recuperar CPU, memória e outras métricas quando você pedir para investigar uma scene implantada. Atualize suas skills e o `auth-server`  do SDK primeiro; veja [Scene Insights](/docs/creator/content-creator-pt/scenes-sdk7/depuracao/scene-insights/) para configuração, permissões e limites.

Por exemplo, peça:

> Investigue erros e crescimento de CPU ou memória na minha scene multiplayer implantada nas últimas duas horas. Use Scene Insights e correlacione os logs e métricas com o código-fonte.

Você aprova o login da wallet por conta própria no navegador. O assistente pode então reutilizar a sessão somente leitura; ele não precisa da sua chave privada nem do arquivo de sessão.

## Skills de IA disponíveis

Quando você instala skills no seu agente, os recursos a seguir ficam disponíveis:

| Skill                  | O que faz                                                                           |
| ---------------------- | ----------------------------------------------------------------------------------- |
| `sdk-scenes`           | Ponto de entrada com diretrizes para o agente e índice de todas as skills temáticas |
| `create-scene`         | Estruture um novo projeto de scene SDK7 do zero                                     |
| `migrate-sdk6-to-sdk7` | Migre uma scene legada do SDK6 para SDK7                                            |
| `add-3d-models`        | Adicione modelos 3D (`.glb`/`.gltf`) com posicionamento, escala e colliders         |
| `add-interactivity`    | Eventos de ponteiro, triggers, raycasts                                             |
| `build-ui`             | UI 2D em screen-space com React-ECS — HUDs, menus, diálogos                         |
| `animations-tweens`    | Animações de modelos GLTF com Animator, tweens do SDK                               |
| `multiplayer-sync`     | Multiplayer peer-to-peer usando networking CRDT                                     |
| `authoritative-server` | Servidor de Multiplayer sem interface para scenes validadas pelo servidor (BETA)    |
| `audio-video`          | Efeitos sonoros, música, streaming de áudio e players de vídeo                      |
| `audio-analysis`       | Dados de amplitude e frequência em tempo real para scenes reativas ao áudio         |
| `deploy-scene`         | Faça deploy de scenes para Genesis City (baseado em LAND)                           |
| `deploy-worlds`        | Faça deploy de scenes para Worlds (espaços 3D pessoais)                             |
| `optimize-scene`       | Otimização de desempenho, limites da scene, melhores práticas                       |
| `camera-control`       | Detecção do modo de câmera, câmera cinematográfica, câmeras virtuais                |
| `composites`           | Referência do formato de arquivo Composite para conteúdo estático da scene          |
| `lighting-environment` | Iluminação dinâmica, sombras, skybox, fog, configurações de ambiente                |
| `particle-system`      | Efeitos de partículas — fogo, fumaça, faíscas, neve, fogos de artifício             |
| `npcs`                 | Personagens não jogáveis — biblioteca NPC Toolkit e abordagens manuais              |
| `player-avatar`        | Posição do player, perfil, personalização do avatar, attachments                    |
| `player-physics`       | Forças físicas — impulsos, knockback, forças contínuas                              |
| `nft-blockchain`       | Exibição de NFT e interações com blockchain/cripto                                  |
| `advanced-rendering`   | Billboard, TextShape, materiais PBR, materiais de vídeo                             |
| `advanced-input`       | Leitura de entrada em nível de sistema e controle de movimento do player            |
| `scene-runtime`        | APIs de runtime transversais — trabalho assíncrono, HTTP, mensagens                 |
| `script-components`    | Classes de Script component para o Creator Hub                                      |
| `game-design`          | Padrões de game design, limites da scene, orçamentos de desempenho                  |
| `unity-explorer-mcp`   | Controle um Explorer em execução para testar e verificar uma scene no mundo         |

Observação: algumas dessas skills envolvem buscar modelos 3D ou outros assets em catálogos de assets gratuitos. O agente de IA deve sempre obter confirmação do usuário antes de baixar quaisquer novos assets para um projeto de scene.

## Deixe a IA ver sua scene no mundo

Normalmente a IA escreve código e *você*  executa a preview, olha o resultado e informa o que está errado. O client desktop da Decentraland pode fechar esse ciclo: ele vem com um **servidor MCP**  opcional que permite a um agente de IA olhar e controlar diretamente o Explorer em execução. O agente tira suas próprias capturas de tela, lê a saída do console da scene, anda com o player pelo cenário, clica em objetos e verifica se a scene realmente faz o que foi solicitada para construir — depois corrige o que encontrar e olha de novo.

Isso transforma o vibe coding de "descrever, esperar, revisar" em um ciclo que o agente pode executar quase todo sozinho.


> [!NOTE]
> **💡 Dica**: Instale a `unity-explorer-mcp` skill antes de tentar isso. Ela ensina ao agente todo o fluxo de trabalho — como iniciar o client, como enquadrar capturas de tela úteis, como cruzar o que ele vê com o estado real da scene e como se recuperar quando a scene para de carregar.
> 
> ```bash
> npx skills add decentraland/sdk-skills --skill unity-explorer-mcp
> ```
> 


### O que você precisa

* O **client desktop da Decentraland**  instalado (o mesmo que o Creator Hub abre para previews).
* Um agente de IA que possa se conectar a servidores MCP via HTTP — Claude Code, Cursor, Cline, VS Code com uma extensão compatível com MCP e outros.
* Um SDK atualizado na sua scene: execute `npm i @dcl/sdk@latest` se a flag `--mcp`  abaixo for rejeitada como uma opção desconhecida.

### 1. Inicie a scene com o servidor MCP ativado

Na pasta da sua scene:

```bash
npm run start -- --mcp
```

Isso faz o que `npm run start` sempre faz — serve sua scene em `http://127.0.0.1:8000` e faz hot-reload sempre que você salva um arquivo — e também inicia o client desktop com o servidor MCP ouvindo em `http://127.0.0.1:8123/unity-explorer-mcp`.

Faça login quando o client abrir. O agente só pode começar a trabalhar depois que você passar pela tela de login e o mundo tiver carregado.

### 2. Conecte seu agente de IA ao servidor

No **Claude Code**, registre-o uma vez:

```bash
claude mcp add --transport http --scope user explorer http://127.0.0.1:8123/unity-explorer-mcp
```

No **qualquer outro cliente MCP**, adicione o servidor da maneira documentada por esse cliente, usando estes detalhes:

| Configuração | Valor                                                                                                      |
| ------------ | ---------------------------------------------------------------------------------------------------------- |
| Transporte   | HTTP transmitível (não stdio — não há comando para executar, o servidor vive dentro do client em execução) |
| URL          | `http://127.0.0.1:8123/unity-explorer-mcp`                                                                 |
| Autenticação | Nenhuma                                                                                                    |
| Nome         | `explorer`                                                                                                 |

Muitos clients usam um arquivo de configuração JSON para isso (`.cursor/mcp.json`, `mcp.json` e semelhantes — consulte a documentação do seu client para os nomes exatos das chaves):

```json
{
  "mcpServers": {
    "explorer": {
      "type": "http",
      "url": "http://127.0.0.1:8123/unity-explorer-mcp"
    }
  }
}
```

Reinicie ou recarregue seu client de IA depois de registrar o servidor, para que ele pegue a conexão. Se o agente disser que as ferramentas do Explorer não estão disponíveis, a causa usual é que o client não estava em execução quando o agente começou — reconecte o servidor (no Claude Code, execute `/mcp`) com o Explorer aberto.


> [!NOTE]
> **Isso é seguro?** O servidor só roda enquanto você inicia o client com `--mcp`, aceita conexões apenas da sua própria máquina (`127.0.0.1`), e rejeita solicitações vindas de páginas da web. Nada é exposto à internet, e ele fica completamente desligado em uma inicialização normal do client.


### 3. Peça o que você quer e deixe-o verificar

Com o servidor conectado, peça o trabalho do jeito que você normalmente faria — a diferença é que agora o agente pode conferir os próprios resultados:

> "Adicione um baú do tesouro no centro do parcel que se abre quando clicado, depois vá até ele e clique para confirmar que a tampa anima."

> "A placa de neon está escura demais. Tire uma captura de tela, ajuste a intensidade emissiva e me mostre um antes/depois."

> "Algo está errado com o elevador. Suba na plataforma, observe os logs e me diga por que ele não se move."

Nos bastidores, o agente pode:

* **Ver** — tirar capturas de tela, ler a `console.log`  da scene e os erros, verificar se a scene carregou ou travou, listar as entities da scene e inspecionar seus components, e ler a posição do player e da câmera.
* **Controlar** — mover e teleportar o player, andar em uma direção por meio de colisões reais, apontar a câmera, posicionar uma câmera livre para uma tomada específica, alternar modos de câmera, clicar em objetos da scene, enviar mensagens de chat e `/commands`, acionar emotes e recarregar a scene.
* **Medir** — ler contagens ao vivo do conteúdo da scene (triangles, entities, textures, materials e mais) em relação aos limites da Decentraland, classificar quais modelos 3D mais contribuem para essa carga a partir de qualquer ponto de vista e amostrar a taxa de quadros real que o player experimenta em qualquer local.

### Use isso para otimizar sua scene

Como o agente pode medir além de ver, você pode entregar diretamente a ele o trabalho de desempenho. Ele pode verificar sua scene em relação aos [limites da scene](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/scene-limitations/), identificar quais modelos 3D são os mais pesados, medir a taxa de quadros em pontos específicos e, o mais importante, medir novamente após fazer alterações para provar que elas ajudaram:

> "Verifique se minha scene está dentro dos limites de conteúdo da Decentraland e me diga qual orçamento está mais próximo do limite."

> "A scene parece travada perto da fonte. Fique lá, meça a taxa de quadros e me diga quais modelos custam mais a partir desse ponto."

> "Otimize a scene: descubra o que está derrubando a taxa de quadros, corrija e me mostre medições antes e depois."

Para melhores resultados, instale a `optimize-scene` skill `unity-explorer-mcp` junto com [Performance Optimization](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/performance-optimization/#optimize-with-ai)  para saber mais sobre esse fluxo de trabalho, incluindo como combiná-lo com o servidor MCP do Blender para automatizar correções nos próprios modelos 3D.

### Dicas

* **Peça provas, não alegações.** "Verifique com uma captura de tela" ou "confirme pelos logs" é o que faz esse fluxo valer a pena. Um bom agente cruza ambos: os pixels podem parecer corretos enquanto o estado subjacente está quebrado, e o contrário também.
* **Capturas de tela custam tokens.** Cada captura de tela que o agente analisa consome parte do seu contexto. Se você quer uma varredura visual longa — uma animação ao longo do tempo, um tour por muitos pontos — peça para capturar quadros em arquivos e ler apenas os que importam. A `unity-explorer-mcp` skill vem com um script que faz exatamente isso.
* **Salve uma vez, não cinco vezes seguidas.** Salvamentos rápidos em sequência podem fazer o client carregar um pacote parcialmente escrito e derrubar a scene por completo, o que exige reiniciar o client para se recuperar. Peça ao agente para agrupar suas edições em um único salvamento.
* **Mantenha o client aberto.** Se você o fechar, a conexão cai e o agente perde os olhos. Reiniciá-lo com o mesmo comando o traz de volta.
* **Teleports se comportam de forma diferente no desenvolvimento local de scenes.** Mover-se entre parcels com `/goto`  é proibido ali, então o agente deve reposicionar o player dentro da scene em vez de teleportar.

## Dicas para Prompting Eficiente

Obter os melhores resultados da IA tem a ver com dar prompts claros e específicos. Aqui vão algumas dicas:

### Seja específico sobre o que você quer

Em vez de:

> "Melhore minha scene"

Tente:

> "Adicione uma porta na posição (8, 0, 8) que se abre com uma animação de rotação quando clicada e reproduz um efeito sonoro de rangido"

### Faça referência a itens existentes

> "Faça o botão vermelho na mesa acionar o elevador para subir"

### Peça uma coisa de cada vez

Divida solicitações complexas em etapas:

1. "Adicione uma UI de placar no canto superior direito"
2. "Adicione um contador que aumenta quando o player clica no alvo"
3. "Exiba o valor do contador no placar"

### Itere e refine

Depois de cada alteração:

1. Visualize a scene (clique em **Play**  no Creator Hub, ou `npm run start`  na linha de comando)
2. Verifique o que funciona e o que não funciona
3. Diga à IA o que ajustar: "Mova o NPC 2 metros para a esquerda e faça-o encarar o player"

### Exemplos de prompts

Para ser executado em um clone novo de [sdk7-scene-template](https://github.com/decentraland/sdk7-scene-template/).

#### Exemplo de prompt 1

```
Quero que você descarte o código atual da scene e faça um pequeno jogo de labirinto simples, em 1 parcel, as paredes podem ser cubos e devem ter colisão.

O jogo deve começar quando o player realmente entrar no labirinto no ponto A e terminar quando ele sair no ponto B (a única saída do labirinto).

Quero que você adicione 3 portas diferentes no labirinto, que precisem ser interagidas com o input de ponteiro para abrir e passar.

Você deve verificar que o player realmente pode jogar e vencer
```

#### Exemplo de prompt 2

```
Quero que você descarte o código atual da scene e construa um jogo de plataforma simples.

Você precisa verificar que o jogo pode ser vencido antes de considerá-lo concluído.
```

## No que a IA pode ajudar

* Estruturar novas scenes a partir de uma descrição
* Adicionar e posicionar modelos 3D
* Escrever handlers de clique e interatividade
* Construir UI (HUDs, menus, diálogos)
* Configurar sincronização multiplayer
* Configurar o Servidor Multiplayer para anti-cheat
* Adicionar áudio, vídeo e streaming
* Criar animações e tweens
* Otimizar o desempenho da scene
* Preparar scenes para deploy
* Depurar problemas em código existente
* Testar e verificar visualmente uma scene em um Explorer em execução (veja [Deixe a IA ver sua scene no mundo](#let-the-ai-see-your-scene-in-world))

## Limitações

Embora as ferramentas de IA sejam poderosas, tenha isto em mente:

* **Sempre visualize** — o código gerado por IA pode não parecer exatamente como você espera. Execute uma preview para verificar.
* **Os limites da scene ainda se aplicam** — a IA não pode contornar os [limites da scene](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/scene-limitations/) (contagens de triangles, tamanhos de arquivo, limites de parcel).
* **Lógica de jogo complexa** — Para mecânicas de jogo intrincadas, talvez você precise orientar a IA passo a passo ou refinar a saída manualmente.
* **Modelos 3D personalizados** — A IA pode referenciar assets gratuitos existentes ou carregar modelos que você fornecer, mas não pode criar modelos 3D do zero (a menos que você use outras ferramentas como o servidor MCP oficial do Blender ao mesmo tempo).

## Próximos passos

* [Início rápido do SDK](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/sdk-101/) — Aprenda os fundamentos do SDK7
* [Combine com Código](/docs/creator/content-creator-pt/scene-editor/expandir-com-codigo/overview/) — Misture edição visual com código
* [Servidor Multiplayer](/docs/creator/content-creator-pt/scenes-sdk7/rede/authoritative-servers/) — multiplayer com autoridade no servidor
* [Exemplos de scenes](/docs/creator/content-creator-pt/tutoriais-e-exemplos/examples/) — Navegue por mecânicas de scene e repositórios reutilizáveis
* [Recursos úteis](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/useful-resources/) — Mais ferramentas de IA, bibliotecas de assets e add-ons para acelerar seu fluxo de trabalho


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
