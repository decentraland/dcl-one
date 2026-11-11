> Uma cena multijogador de Festive Trail com avatares personalizados, integração com o Marketplace, uma roda de cores conduzida por raycast e câmeras de acompanhamento.

# Slay the Steps

![Pré-visualização de Slay the Steps](/docs/_remote/009ced18986123a766b6a4fb.webp)

Uma experiência de Multiplayer Server criada pelo Studio para o Decentraland Festive Trail 2025. Inclui avatars personalizados, integração com a Marketplace API e um seletor personalizado de roda de cores. O seu código separa módulos de client, server e partilhados, e inclui os assets de origem juntamente com a cena implantável.

* [Jogue no Decentraland](https://decentraland.org/jump?realm=slaythesteps.dcl.eth)

## Informações do projeto

**Mantido por:** Comunidade · stom66

[Ver fonte upstream](https://github.com/stom66/dcl-slay-the-steps)

**Licença:** [CC BY-NC-ND 4.0](https://github.com/stom66/dcl-slay-the-steps/blob/main/LICENSE.md). Verifique os termos upstream antes de reutilizar; o código e os assets podem ter termos diferentes.


> [!WARNING]
> **Código mantido pela comunidade — reveja antes de usar**
> 
> Este projeto da comunidade é mantido fora das organizações GitHub da Decentraland Foundation e da DAO. O seu código-fonte e a sua implementação em produção podem mudar de forma independente.
> 
> A inclusão neste catálogo não implica uma auditoria de segurança nem uma garantia de segurança. Reveja o código, as dependências e os scripts de instalação antes de o executar ou reutilizar.
> 
> [Reportar uma preocupação no catálogo](https://github.com/decentraland/docs/issues/new). Para vulnerabilidades suspeitas, siga a política de segurança do projeto upstream. Não inclua detalhes da exploração nem credenciais em issues públicas.


## Principais funcionalidades

* Cinematográfico **câmaras virtuais**
* **AvatarShapes** para mostrar wearables
* **Multijogador** para sincronizar rondas/roupas do jogador
* Seletor de cores 3D personalizado

## Explorar o código

Estes links apontam para um snapshot específico da source para que as explicações continuem úteis à medida que o desenvolvimento upstream avança. São referências de aprendizagem, não uma avaliação de segurança.

### Transforma um acerto do pointer 3D numa seleção de cor

`GetLocalMousePosition` faz raycasts a partir da câmara e converte o impacto na posição local da roda. `SampleColorWheel` mapeia o seu ângulo e raio para tonalidade e saturação, invoca um callback e tinge o controle deslizante de valor.

**Contexto do projeto:** A implementação usa os meshes da roda do projeto, a função auxiliar de conversão HSV e os nomes dos nós de material.

**Referências de origem:**

* [`GetLocalMousePosition`](https://github.com/stom66/dcl-slay-the-steps/blob/9f6026c826fb61d25eeea6a98f22a4dd16b29a15/dcl/src/client/colorPickers.ts#L306-L335)
* [`SampleColorWheel`](https://github.com/stom66/dcl-slay-the-steps/blob/9f6026c826fb61d25eeea6a98f22a4dd16b29a15/dcl/src/client/colorPickers.ts#L203-L238)

### Seguir um alvo em movimento com uma câmara virtual

O sistema de posição da câmara e a cadeia de tween calculam periodicamente uma posição relativa ao jogador e ao alvo e depois fazem tween em direção a ela. `TrackEntity` cria a câmara com `lookAtEntity` e torna-a ativa.

**Contexto do projeto:** Ler `ResetCamera` e a vinculação de eventos do jogo para o fim do ciclo de vida do tracking.

**Referências de origem:**

* [sistema de posição da câmara e cadeia de tween](https://github.com/stom66/dcl-slay-the-steps/blob/9f6026c826fb61d25eeea6a98f22a4dd16b29a15/dcl/src/client/cameraController.ts#L46-L96)
* [`TrackEntity`](https://github.com/stom66/dcl-slay-the-steps/blob/9f6026c826fb61d25eeea6a98f22a4dd16b29a15/dcl/src/client/cameraController.ts#L99-L137)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/published-games/slay-the-steps.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
