> Ferramentas, add-ons, bibliotecas de assets e projetos de exemplo que aceleram seu fluxo de trabalho de criação no Decentraland.

# Recursos Úteis

Esta página reúne ferramentas, add-ons, bibliotecas de assets e projetos de exemplo que podem acelerar seu fluxo de trabalho ao criar experiências para a Decentraland.

## Decentraland SDK Skills

Dê ao seu assistente de codificação com IA orientações específicas da Decentraland para criar, iterar e fazer deploy de suas experiências rapidamente. As Decentraland SDK Skills funcionam com ferramentas como Claude Code, Cursor, Codex, Cline e Windsurf, e são mantidas atualizadas com as mudanças mais recentes do SDK.

🔗 [github.com/decentraland/sdk-skills](https://github.com/decentraland/sdk-skills)

Veja [Vibe Coding com IA](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/) para dicas sobre como usar SDK Skills no seu fluxo de trabalho com IA.

## Decentraland SDK7 Test Scenes

Use as Decentraland SDK7 Test Scenes para ver exemplos funcionais de recursos específicos do SDK. O repositório inclui muitas cenas de teste isoladas, e o mundo de teste ao vivo permite que você as experimente antes de usar sistemas semelhantes em suas próprias experiências.

🔗 [github.com/decentraland/sdk7-test-scenes](https://github.com/decentraland/sdk7-test-scenes/)

## Leaderboard do Multiplayer Server

Veja como o server Multiplayer da Decentraland pode ser usado para construir um leaderboard resiliente. O server controla o estado do jogo, valida cada pontuação, persiste os totais de cada jogador e transmite um leaderboard classificado que todo client da Decentraland pode renderizar.

🔗 [Cena de teste do leaderboard do server Multiplayer](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/90,-9-authoritative-server-leaderboard)

Veja [Servers autoritativos](/docs/creator/content-creator-pt/scenes-sdk7/rede/authoritative-servers/) para aprender como construir sobre o server autoritativo.

## Decentraland Tools for Blender

Acelere seu fluxo de trabalho com este add-on do Blender, que lida com grades de parcels, limites de wearables, otimização de texturas, geração de LOD e exportação de GLB com configurações da Decentraland integradas.

🔧 [extensions.blender.org/add-ons/decentraland-tools](https://extensions.blender.org/add-ons/decentraland-tools/)

## Assets de arte 3D

Uma boa experiência terá uma ótima arte 3D para acompanhá-la. Se você estiver animado para criar esses modelos 3D por conta própria, veja a [seção de Modelagem 3D](/docs/creator/content-creator-pt/modelacao-3d-e-animacoes/3d-models/). Mas, se você preferir se concentrar na programação ou no design de jogos, não precisa criar seus próprios assets!

Aqui estão algumas fontes de modelos 3D que você pode usar em uma scene da Decentraland:

* [IWB Catalog](https://dcl-iwb.co/)
* [Asset Ovi](https://assetovi.com/)
* [SketchFab](https://sketchfab.com/)
* [Clara.io](https://clara.io/)
* [Archive3D](https://archive3d.net/)
* [SketchUp 3D Warehouse](https://3dwarehouse.sketchup.com/)
* [Thingiverse](https://www.thingiverse.com/)
* [ShareCG](https://www.sharecg.com/)
* [CGTrader](https://cgtrader.com)

Você também pode usar ferramentas de IA generativa para gerar seus próprios modelos 3D. Confira:

* [Blender MCP](https://github.com/ahujasid/blender-mcp) + [Claude Desktop](https://claude.ai/download): conecte um assistente de IA diretamente ao [Blender](https://www.blender.org/), para que você possa criar e editar modelos 3D descrevendo o que deseja. Todas são ferramentas gratuitas.
* [Meshy](https://www.meshy.ai/)
* [Luma AI](https://lumalabs.ai/genie)
* [Tripo3D](https://www.tripo3d.ai/app)
* [Rodin](https://hyper3d.ai/rodin)


> [!WARNING]
> **📔 Nota**: Os modelos devem estar nos `.gltf` ou `.glb` formatos suportados e devem ter um número de triângulos, texturas e materiais que estejam em conformidade com o [limitações da cena](/docs/creator/content-creator-pt/scenes-sdk7/otimizacao/scene-limitations/). Se você estiver obtendo modelos de um site de terceiros, preste atenção às restrições de licença do conteúdo que baixar.


## Blender MCP Server

Use o Blender MCP para conectar um agente de IA, como o Claude Code, ao Blender. Ele pode ajudar a criar e editar modelos GLTF/GLB, materiais e texturas a partir de prompts de texto simples.

🔗 [blender.org/lab/mcp-server](https://www.blender.org/lab/mcp-server/)


> [!WARNING]
> **📔 Nota**: O Blender recomenda usar MCP em um ambiente seguro, sem arquivos sensíveis.


## Assets 3D de código aberto

Uma coleção curada de assets 3D de alta qualidade, incluindo props, ambientes e estruturas, totalmente gratuita e de código aberto. Construída sobre os padrões GLB e glTF.

🔗 [opensource3dassets.com](https://www.opensource3dassets.com/en)

## Genesis Plaza Assets

Reutilize quaisquer assets ou texturas da Genesis Plaza em suas próprias experiências. Você pode baixar o arquivo .blend ou usar a Blender Asset Library do Stom para arrastar e soltar a partir de um repositório. Você também pode baixar assets de paisagismo procedural correspondentes, como árvores e arbustos.

* 📂 [Baixe o arquivo .blend](https://drive.google.com/file/d/1beZHX1D8zR9iMCEmcPQU6etxpZXohZ5P/view?usp=sharing)
* 🐈‍⬛ Encontre o repositório do Stom em [github.com/stom66/dcl-genesis-plaza-assets](https://github.com/stom66/dcl-genesis-plaza-assets)
* ℹ️ [Assista a um vídeo sobre como usar o repositório](https://files.catbox.moe/eo5hhb.mp4)
* 🌺 Combine os assets procedurais da Decentraland [com estas árvores, arbustos e rochas](https://drive.google.com/drive/folders/1Hxoog-ov_igbNTU3H7EwOX634EVCbhYQ?usp=drive_link)

## Decentraland Scene Optimizer

Experimente esta ferramenta útil para extrair, deduplicar e comprimir texturas de arquivos GLB para otimizar ainda mais suas cenas.

🔗 [github.com/decentraland/SceneOptimizer](https://github.com/decentraland/SceneOptimizer)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/primeiros-passos/useful-resources.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
