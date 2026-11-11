> Entrega de encomendas cronometrada com áreas de coleta e entrega, um ponteiro de direção montado no avatar e pontuação separada da animação de entrega.

# Pigeon Deluxe

![Pré-visualização do Pigeon Deluxe](/docs/_remote/5140367a547cfe389db77f7e.webp)

Um jogo de entregas cronometradas em que os jogadores coletam encomendas, levam-nas até outra plataforma e ganham tempo extra pelas entregas.

* [Jogar no Decentraland](https://decentraland.org/jump?realm=pigeondeluxe.dcl.eth)

## Informações do projeto

**Mantido por:** Comunidade · PepeGawd

[Ver fonte upstream](https://github.com/PepeGawd/Pigeon-Deluxe)

**Licença:** Não identificado neste catálogo. Consulte o repositório upstream e o mantenedor sobre os termos aplicáveis antes de reutilizar.


> [!WARNING]
> **Código mantido pela comunidade — revise antes de usar**
> 
> Este projeto da comunidade é mantido fora das organizações GitHub da Decentraland Foundation e da DAO. Seu código-fonte e implantação em produção podem mudar de forma independente.
> 
> A inclusão neste catálogo não implica uma auditoria de segurança nem garante segurança. Revise o código, as dependências e os scripts de instalação antes de executá-lo ou reutilizá-lo.
> 
> [Reportar uma preocupação do catálogo](https://github.com/decentraland/docs/issues/new). Para vulnerabilidades suspeitas, siga a política de segurança do projeto upstream. Não inclua detalhes do exploit nem credenciais em issues públicas.


## Explorar o código

Estes links apontam para um snapshot específico da fonte, para que as explicações continuem úteis à medida que o desenvolvimento upstream avança. Eles são referências de aprendizado, não uma avaliação de segurança.

### Apontar para um objetivo enquanto anexado a um avatar

`initParcelDirectionPointer` e o seu sistema de atualização anexam uma seta acima da etiqueta de nome do avatar, calculam a direção no espaço mundial até o alvo ativo e convertem essa rotação para o espaço local do pai. A rotação avança usando `dt`, então a seta gira em direção ao objetivo ao longo do tempo.

**Contexto do projeto:** O alvo vem do sistema de encomendas e a seta usa o `FaceDirection1.glb` ativo da cena.

**Referências de origem:**

* [`initParcelDirectionPointer` e o seu sistema de atualização](https://github.com/PepeGawd/Pigeon-Deluxe/blob/1c24bcb3c82d73dbb1aad8b1035f3f7edab5d315/src/Game/parcelDirectionPointer.ts#L26-L96)

### Separar a pontuação da animação de entrega

`startNewRound` e `handleSpawnAreaEnter` atribuem funções de coleta e entrega às plataformas, acompanham se uma encomenda está sendo carregada e concedem pontos e tempo assim que a plataforma de entrega é alcançada. A próxima rodada começa quando a animação de soltar termina.

**Contexto do projeto:** Leia isto junto com o temporizador de jogo da cena e as funções auxiliares de anexação de encomendas para ver como o estado da jogabilidade e o feedback visual se encaixam.

**Referências de origem:**

* [`startNewRound` e `handleSpawnAreaEnter`](https://github.com/PepeGawd/Pigeon-Deluxe/blob/1c24bcb3c82d73dbb1aad8b1035f3f7edab5d315/src/Game/parcelCollectDeliver.ts#L363-L433)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/published-games/pigeon-deluxe.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
