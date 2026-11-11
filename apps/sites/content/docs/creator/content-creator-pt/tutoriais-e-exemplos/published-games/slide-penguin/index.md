> Deslizamento do pinguim com varredura cronometrada que modifica a desaceleração e a pontuação com base na distância até o alvo.

# Slide Penguin

Deslize um pinguim em direção a um alvo e use varridas cronometradas para afetar a rapidez com que ele desacelera.

Nota de deployment: Atualmente não está implantado.

* [Jogue no Decentraland](https://decentraland.org/jump?position=-140%2C101)

## Informações do projeto

**Mantido por:** Comunidade · ToxSam

[Ver fonte upstream](https://github.com/ToxSam/SlidePenguin)

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

### Transforme a varredura num modificador de movimento temporário

`handleSwipe` aplica um cooldown à redução de desaceleração e registra quando esse efeito expira. `penguinMovementSystem` restaura a desaceleração base após essa janela, atualiza a velocidade e avança o Transform usando o tempo do frame.

**Contexto do projeto:** Siga o projeto `PenguinMovement` e `SwipeBox` components e constantes de ajuste para entender a conexão entre input e movimento.

**Referências de origem:**

* [`handleSwipe`](https://github.com/ToxSam/SlidePenguin/blob/19978bf4ca9e793ce87cd5637efc1abf3a3b686c/src/systems.ts#L519-L598)
* [`penguinMovementSystem`](https://github.com/ToxSam/SlidePenguin/blob/19978bf4ca9e793ce87cd5637efc1abf3a3b686c/src/systems.ts#L864-L913)

### Pontue quão perto um objeto cai de um alvo

`calculateDistance2D` e `calculateScore` ignore a altura, normalize a distância horizontal em relação a uma distância máxima e mapeie-a para uma pontuação arredondada limitada por `MAX_SCORE`.

**Contexto do projeto:** Este pequeno módulo é um ponto de partida útil para entender a pontuação por proximidade; suas entradas são a posição final do pinguim e a posição do alvo.

**Referências de origem:**

* [`calculateDistance2D` e `calculateScore`](https://github.com/ToxSam/SlidePenguin/blob/19978bf4ca9e793ce87cd5637efc1abf3a3b686c/src/scoring.ts#L5-L39)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/published-games/slide-penguin.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
