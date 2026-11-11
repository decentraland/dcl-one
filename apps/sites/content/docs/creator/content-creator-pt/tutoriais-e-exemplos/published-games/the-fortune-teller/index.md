> Leitura de sorte social com transições explícitas de estado da conversa e um rig de câmera cinematográfica baseado em pivô.

# The Fortune Teller

![A pré-visualização do Fortune Teller](/docs/_remote/2aaa323560ef094922b15d6a.webp)

Uma experiência de roleplay social em que os jogadores podem receber leituras de fortuna ou fazer leituras de fortuna para amigos.

* [Jogue no Decentraland](https://decentraland.org/jump?realm=fortunes.dcl.eth)

## Informações do projeto

**Mantido por:** Comunidade · carlosmu

[Ver fonte upstream](https://github.com/carlosmu/circus_fortune_teller)

**Licença:** Não identificado neste catálogo. Consulte o repositório upstream e o mantenedor para os termos aplicáveis antes de reutilizar.


> [!WARNING]
> **Código mantido pela comunidade — reveja antes de usar**
> 
> Este projeto da comunidade é mantido fora das organizações GitHub da Decentraland Foundation e da DAO. O seu código-fonte e a sua implementação em produção podem mudar de forma independente.
> 
> A inclusão neste catálogo não implica uma auditoria de segurança nem uma garantia de segurança. Reveja o código, as dependências e os scripts de instalação antes de o executar ou reutilizar.
> 
> [Reportar uma preocupação no catálogo](https://github.com/decentraland/docs/issues/new). Para vulnerabilidades suspeitas, siga a política de segurança do projeto upstream. Não inclua detalhes da exploração nem credenciais em issues públicas.


## Explorar o código

Estes links apontam para um snapshot específico da source para que as explicações continuem úteis à medida que o desenvolvimento upstream avança. São referências de aprendizagem, não uma avaliação de segurança.

### Representar uma leitura como transições de estado explícitas

A máquina de estados da fortuna lista os passos permitidos, desde a seleção de categoria e baralho até a revelação. `tryTransition` verifica a próxima etapa, atualiza a sessão e notifica os listeners de entrada de estado.

**Contexto do projeto:** Leia-o em conjunto com os tipos de sessão e os handlers de ação para ver como uma interação em várias etapas é organizada.

**Referências de origem:**

* [máquina de estados da fortuna](https://github.com/carlosmu/circus_fortune_teller/blob/f8db0c5630a03208d328a96737224e1293df60c7/src/fortuneFsm/machine.ts#L6-L60)

### Construir um rig de câmera em torno de um objeto da cena

`ensureHostPivotRig` e `startHostCinematicCamera` vincule uma câmera virtual a um pivot sob a mesa, escolha os ângulos inicial e final do lado do jogador e ative a câmera.

**Contexto do projeto:** O rig depende da Entity da mesa, dos valores de configuração e do System de atualização neste módulo; essas partes explicam como enquadrar uma interação no contexto da sua cena.

**Referências de origem:**

* [`ensureHostPivotRig` e `startHostCinematicCamera`](https://github.com/carlosmu/circus_fortune_teller/blob/f8db0c5630a03208d328a96737224e1293df60c7/src/cinematicCamera.ts#L141-L197)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/published-games/the-fortune-teller.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
