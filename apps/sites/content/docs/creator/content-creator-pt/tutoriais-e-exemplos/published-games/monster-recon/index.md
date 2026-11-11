> Jogo da memória seguido por perseguições de monstro baseadas em gatilhos, com progressão de colecionáveis, classificações e texto bitmap personalizado.

# Monster Recon

![Prévia de Monster Recon](/docs/_remote/276e8747b4e8b433f9cfa9f6.webp)

Um jogo de memória em que você limpa tabuleiros com tempo limitado e depois persegue o monstro que revelou antes que ele escape. Colete achados raros no seu Codex e suba nos rankings semanal e geral.

* [Jogar no Decentraland](https://decentraland.org/jump?realm=monsterrecon.dcl.eth)

## Informações do projeto

**Mantido por:** Comunidade · carlosmu

[Ver código-fonte upstream](https://github.com/carlosmu/dcl-monster-recon)

**Licença:** Não identificado neste catálogo. Consulte o repositório upstream e o mantenedor para conhecer os termos aplicáveis antes de reutilizar.


> [!WARNING]
> **Código mantido pela comunidade — revise antes de usar**
> 
> Este projeto da comunidade é mantido fora das organizações GitHub da Decentraland Foundation e da DAO. Seu código-fonte e a implantação ao vivo podem mudar de forma independente.
> 
> A inclusão neste catálogo não implica uma auditoria de segurança nem garante a segurança. Revise o código, as dependências e os scripts de instalação antes de executar ou reutilizá-lo.
> 
> [Reportar uma preocupação no catálogo](https://github.com/decentraland/docs/issues/new). Para vulnerabilidades suspeitas, siga a política de segurança do projeto upstream. Não inclua detalhes da exploração nem credenciais em issues públicas.


## Principais recursos

* Servidor Multiplayer
* Armazenamento para progresso, melhores tempos, tabelas de classificação e recompensas semanais.
* Câmeras virtuais
* Emotes de cena
* Áreas de gatilho
* Partículas
* Tweens
* TextShapes
* Tabela de classificação 3D (com fotos de perfil dos jogadores)
* Fonte bitmap personalizada para UI
* Feed ao vivo sincronizado das capturas de outros jogadores

## Explorar o código

Estes links apontam para um snapshot específico da fonte, para que as explicações continuem úteis à medida que o desenvolvimento upstream avança. São referências de aprendizado, não uma avaliação de segurança.

### Crie uma perseguição a partir de marcadores da cena e áreas de gatilho

`spawnHop` combina um tween de balanço, gatilho esférico, som e sprite de billboard em um marcador selecionado. O manipulador de captura verifica se a entidade que entrou é o jogador local antes de fazer a limpeza e chamar o callback de recompensa.

**Contexto do projeto:** Leia isto em conjunto com os nomes dos marcadores, o atlas de sprites e os callbacks fornecidos a `startPrizeChase`.

**Referências da fonte:**

* [`spawnHop`](https://github.com/carlosmu/dcl-monster-recon/blob/eb0bc6fa87b9ebf98c8c068ebb9bba686a9467cb/src/prizeChase.ts#L216-L271)
* [manipulador de captura](https://github.com/carlosmu/dcl-monster-recon/blob/eb0bc6fa87b9ebf98c8c068ebb9bba686a9467cb/src/prizeChase.ts#L183-L214)

### Renderizar texto bitmap personalizado em ReactEcs

`BitmapTextLine` organiza imagens de glifos individuais usando UVs do atlas, offsets de glifos e kerning. Os helpers de medição e quebra de linha anteriores dividem o texto em linhas pela largura medida.

**Contexto do projeto:** O renderer precisa de métricas de bitmap-font e assets de textura compatíveis.

**Referências da fonte:**

* [`BitmapTextLine`](https://github.com/carlosmu/dcl-monster-recon/blob/eb0bc6fa87b9ebf98c8c068ebb9bba686a9467cb/src/bitmapFont.tsx#L268-L327)
* [helpers de medição e quebra de linha](https://github.com/carlosmu/dcl-monster-recon/blob/eb0bc6fa87b9ebf98c8c068ebb9bba686a9467cb/src/bitmapFont.tsx#L229-L266)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/published-games/monster-recon.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
