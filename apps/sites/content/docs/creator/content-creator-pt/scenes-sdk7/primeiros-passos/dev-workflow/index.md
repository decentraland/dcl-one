> Fluxo de trabalho recomendado para criar uma scene do Decentraland com o Creator Hub, da ideação até os testes e a publicação.

# Fluxo de Trabalho de Desenvolvimento

Este documento descreve os passos recomendados para desenvolver uma scene para o Decentraland, desde a ideação até à publicação e além.

## Como crio uma scene no Decentraland?

1. **Instale o** [**Creator Hub**](https://decentraland.org/download/creator-hub), a aplicação de desktop oficial para criar, pré-visualizar e publicar scenes. É a ferramenta recomendada para criadores de todos os níveis de experiência.
2. **Se usar um assistente de programação com IA** (como Claude Code, Cursor ou Copilot), instale o oficial [Decentraland SDK Skills](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/) para que conheça os padrões verificados do SDK7: `npx skills add decentraland/sdk-skills`
3. **A** [**CLI**](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/using-the-cli/) é uma alternativa para utilizadores avançados e fluxos de trabalho automatizados.

As secções abaixo aprofundam cada etapa desse fluxo de trabalho.

## Instale o Creator Hub

Certifique-se de que tem o Decentraland Creator Hub instalado.

* [Guia de Instalação](/docs/creator/content-creator-pt/scene-editor/primeiros-passos/editor-installation/)

Se pretende trabalhar com código, certifique-se também de que instala [Visual Studio Code](https://code.visualstudio.com/) ou [Cursor AI](https://www.cursor.com/).


> [!NOTE]
> **💡 Dica**: Também pode usar assistentes de IA para gerar código da scene a partir de descrições em linguagem natural. Veja [Vibe Coding com IA](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/vibe-coding/) como começar a desenvolver com assistência de IA.


## Desenhe a sua experiência

Pense em quanto espaço precisa ocupar, que tipo de distribuição quer, que tipos de mecânicas pretende que os jogadores possam realizar, etc. Os documentos seguintes podem servir de guia:

* [Guia de UX & UI](/docs/creator/content-creator-pt/scenes-sdk7/conceber-a-experiencia/ux-ui-guide/)
* [Constrangimentos de design para jogos](/docs/creator/content-creator-pt/scenes-sdk7/conceber-a-experiencia/design-games/)
* [Diretrizes de MVP da Scene](/docs/creator/content-creator-pt/scenes-sdk7/conceber-a-experiencia/mvp-guidelines/)

## Onde publicar

No Decentraland, o conteúdo é publicado em parcelas de terra adjacentes num espaço finito. Os jogadores podem caminhar livremente de uma para a outra. Cada scene é o seu próprio pequeno mundo contido; os itens de uma scene não podem estender-se para outra scene, e o código de cada scene é isolado em sandbox de todos os outros.

A permissão para publicar em cada um destes é controlada por tokens. Não precisa de land para desenvolver uma scene, mas precisará de acesso a land quando estiver pronto para publicar.

Em alternativa, tem a opção de publicar em Decentraland [Worlds](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing-options/#decentraland-worlds), que são scenes autónomas e isoladas.

As opções seguintes estão disponíveis:

* Alugar LAND
* Comprar LAND
* Obter permissões de um proprietário de land
* Publicar num Decentraland World, consulte [worlds](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing-options/#decentraland-worlds) para saber mais.

Veja [Opções de publicação](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing-options/) para mais detalhes.

## Templates e exemplos

Ao criar uma nova scene, escolha entre vários templates base de scenes que incluem algum código básico e modelos 3D. Use-os para começar mais rapidamente.

* [Scenes de exemplo](/docs/creator/content-creator-pt/tutoriais-e-exemplos/examples/): aqui encontra uma coleção selecionada de scenes em funcionamento e repositórios, cada um mostrando mecânicas que pode reutilizar ou consultar.
* [Bibliotecas auxiliares](https://studios.decentraland.org/resources?sdk_version=SDK7&resource_type=Library): estas podem simplificar muitas tarefas comuns.

## Assets artísticos

Se é um artista experiente ou tem acesso a alguém que o seja, pode criar `.gltf` ou `.glb` modelos personalizados para a sua scene. Veja [Essenciais de modelos 3D](/docs/creator/content-creator-pt/modelacao-3d-e-animacoes/3d-models/) para dicas sobre como criar modelos 3D para o Decentraland.

No entanto, não precisa de criar os seus próprios assets. Veja [Recursos úteis](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/useful-resources/) para bibliotecas de assets e ferramentas de IA generativa que pode usar para obter modelos 3D, bem como outras ferramentas que podem acelerar o seu fluxo de trabalho.

## Executar uma pré-visualização local

Para executar uma pré-visualização da sua scene, abra-a no Creator Hub e clique no **Play** botão. Em alternativa, se estiver a trabalhar na linha de comandos, execute `npm run start` na pasta raiz do seu projeto.

* [Pré-visualize a sua scene](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/preview-scene/) para mais detalhes.
* Consulte o [Depurar uma scene](/docs/creator/content-creator-pt/scenes-sdk7/depuracao/debug-in-preview/) para dicas sobre como depurar quaisquer problemas.
* Ative **Comprimir assets** no **Desktop Client** submenu de **Play Options** (ou execute `npm run start -- --local-ab` a partir da CLI) para carregar os seus modelos 3D como pacotes de assets otimizados. A pré-visualização é mais fluida e mostra os modelos exatamente como ficarão quando forem publicados. Veja [Pré-visualização com assets otimizados](/docs/creator/content-creator-pt/scenes-sdk7/primeiros-passos/preview-scene/#preview-with-optimized-assets).


> [!NOTE]
> **💡 Dica**: Ao usar o Creator Hub, sempre que fizer uma alteração na sua scene, a pré-visualização é atualizada automaticamente. Mesmo durante a execução.


## Publicar no Decentraland

Quando estiver satisfeito com a sua scene, chegou a hora de publicar no Decentraland. Para isso, precisa de possuir LAND, um Decentraland NAME ou um nome ETH ENS, ou ter permissões concedidas por alguém que as tenha.

Veja [publicação](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing/) para instruções sobre como fazer isso.

Em alternativa, pode publicar em [Worlds](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing-options/#decentraland-worlds), um espaço 3D pessoal que não requer LAND.

## Promova

Agora que a sua scene está disponível, espalhe a palavra! Aqui estão algumas formas de o fazer:

* Partilhe-a nas redes sociais (#DCL)
* Anuncie-a no [Discord](https://dcl.gg/discord)
* Submeta-a para ser destacada em [events.decentraland.org](https://events.decentraland.org/)
* Organize um evento na sua scene
* Adicione um spawn point numa área de grande tráfego que ligue à sua scene

## Itere

Depois de a sua scene estar em funcionamento há algum tempo e de ter recebido feedback dos jogadores, estará numa excelente posição para a iterar!

Atualize o seu conteúdo com melhorias e novas funcionalidades, fazendo deploy de novas versões da sua scene nas mesmas coordenadas.

## Retribuir

Se criar uma scene, jogo ou aplicação de que se orgulhe, considere torná-la open source! Assim, outros podem aprender com o seu código e desenvolver o seu trabalho. Também pode partilhar o projeto completo no [Awesome Repository](https://github.com/decentraland-scenes/Awesome-Repository).

Se criar uma funcionalidade reutilizável, pode querer transformá-la numa biblioteca que outros possam importar para os seus projetos.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/primeiros-passos/dev-workflow.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
