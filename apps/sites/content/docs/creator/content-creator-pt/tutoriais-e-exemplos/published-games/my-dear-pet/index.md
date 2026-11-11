> Coleta de pets com aceleração e frenagem para movimento, além de animação com defasagem de fase de ovos colocados no editor.

# My Dear Pet

![Pré-visualização de My Dear Pet](/docs/_remote/3412460113d21fec792e322a.webp)

Um jogo de colecionar pets.

* [Jogue no Decentraland](https://decentraland.org/jump?realm=marshcolony.dcl.eth)

## Informações do projeto

**Mantido por:** Decentraland DAO

[Ver código-fonte](https://github.com/dcl-regenesislabs/marsh-colony)

**Licença:** Não identificado neste catálogo. Consulte o repositório upstream e o mantenedor para conhecer os termos aplicáveis antes de reutilizar.

## Explorar o código

Estes links apontam para um instantâneo específico do código-fonte para que as explicações continuem úteis à medida que o desenvolvimento upstream avança. Eles são referências de aprendizado, não uma avaliação de segurança.

### Dar aceleração e frenagem aos pets em movimento

`petMotionStep` mantém uma velocidade por Entity, limita a velocidade alvo usando a distância restante para parar e retorna um passo de movimento do tamanho de um frame. O chamador ainda fornece a rota e atualiza o Transform.

**Contexto do projeto:** Leia os valores de distância de chegada, aceleração e frenagem em `shared/config` junto com esta função auxiliar ao estudar a sensação do movimento.

**Referências do código-fonte:**

* [`petMotionStep`](https://github.com/dcl-regenesislabs/marsh-colony/blob/55dea4d3e13b3d821aa20bd277fbc4e0bc920b6f/src/client/petMotion.ts#L9-L35)

### Animar props colocados no editor com fases diferentes

`setupEggShake` encontra entidades chamadas egg, lembra suas rotações originais e aplica uma pequena rotação em onda senoidal com uma fase diferente para cada egg. Isso adiciona variação enquanto preserva a orientação criada para cada prop.

**Contexto do projeto:** Isso depende dos nomes de egg gerados a partir da cena do Creator Hub.

**Referências do código-fonte:**

* [`setupEggShake`](https://github.com/dcl-regenesislabs/marsh-colony/blob/55dea4d3e13b3d821aa20bd277fbc4e0bc920b6f/src/client/eggShake.ts#L29-L53)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/published-games/my-dear-pet.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
