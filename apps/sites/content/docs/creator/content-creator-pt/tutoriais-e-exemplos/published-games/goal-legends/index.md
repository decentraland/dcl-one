> Disputas de pênaltis multijogador com partidas executadas pelo servidor, classificações persistentes, câmeras guiadas por fase e animações de bola sincronizadas.

# Goal Legends

![Pré-visualização de Goal Legends](/docs/_remote/aa680097357467a542aed322.webp)

Um jogo multijogador de disputa de pênaltis. Enfrente outro jogador ou o GL-Bot: escolha esquerda, centro ou direita para marcar ou defender. Cada rodada termina com uma repetição no estilo de uma transmissão de TV, com a multidão reagindo ao vivo.

* [Jogue no Decentraland](https://decentraland.org/jump?realm=goallegends.dcl.eth)

## Informações do projeto

**Mantido por:** Comunidade · carlosmu

[Ver fonte upstream](https://github.com/carlosmu/goal-legends-arena)

**Licença:** [Aviso Apache-2.0](https://github.com/carlosmu/goal-legends-arena/blob/master/LICENCE). Verifique os termos upstream antes de reutilizar; o código e os assets podem ter termos diferentes.


> [!WARNING]
> **Código mantido pela comunidade — reveja antes de usar**
> 
> Este projeto da comunidade é mantido fora das organizações GitHub da Decentraland Foundation e da DAO. O seu código-fonte e a sua implementação em produção podem mudar de forma independente.
> 
> A inclusão neste catálogo não implica uma auditoria de segurança nem uma garantia de segurança. Reveja o código, as dependências e os scripts de instalação antes de o executar ou reutilizar.
> 
> [Reportar uma preocupação no catálogo](https://github.com/decentraland/docs/issues/new). Para vulnerabilidades suspeitas, siga a política de segurança do projeto upstream. Não inclua detalhes da exploração nem credenciais em issues públicas.


## Principais funcionalidades

* **Servidor Multiplayer** executa as partidas
* **Armazenamento** para rankings diários/de todos os tempos de PvP/PvE e a bandeira do país de cada jogador.
* **Cena** **emotes**
* Props de bola sincronizados com o Animator para chutes, mergulhos, gols e defesas.
* **Câmeras virtuais** para as vistas de gameplay e uma cinemática do vencedor que todos veem.
* **AvatarShape** NPCs da multidão que reagem a cada gol e defesa.
* **Partículas** para os fogos de artifício
* Um portal com luzes e **`GltfNodeModifiers`** que te leva a outro World.

## Explorar o código

Estes links apontam para um snapshot específico da source para que as explicações continuem úteis à medida que o desenvolvimento upstream avança. São referências de aprendizagem, não uma avaliação de segurança.

### Alternar câmeras com as fases da partida

`initGameplayCamera` seleciona as câmeras de gameplay, vencedora ou padrão a partir do snapshot da partida do client. A câmera do vencedor orbita um pivô, enquanto os espectadores podem escolher uma câmera livre durante a partida.

**Contexto do projeto:** Siga isto em conjunto com `clientSnapshot`, o `GameState` enum, e o pivô nomeado da câmera da cena.

**Referências de origem:**

* [`initGameplayCamera`](https://github.com/carlosmu/goal-legends-arena/blob/5d42bba8f10ac2c3103a4e5804541c693e8528b3/src/client/gameplayCamera.ts#L109-L165)

### Coordene uma animação de prop com o estado do jogo

`tickAnimationManager` inicia um clip da bola quando a fase muda, escolhe animações de gol/defesa com base na direção do chute e oculta brevemente o prop enquanto troca de clips. Este caminho é específico do chutador bot; chutadores humanos carregam a bola em seus emotes.

**Contexto do projeto:** Depende do modelo da bola da cena e de clips de animação nomeados.

**Referências de origem:**

* [`tickAnimationManager`](https://github.com/carlosmu/goal-legends-arena/blob/5d42bba8f10ac2c3103a4e5804541c693e8528b3/src/client/animationManager.ts#L78-L114)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/published-games/goal-legends.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
