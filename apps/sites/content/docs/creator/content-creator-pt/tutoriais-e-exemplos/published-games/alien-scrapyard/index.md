> Rodadas cooperativas de construção com puzzles de construção definidos por dados e câmeras cinematográficas guiadas por fase.

# Alien Scrapyard

![Pré-visualização de Alien Scrapyard](/docs/_remote/c94532935e9e0629a121ed5f.webp)

Um jogo de construção cooperativo e cronometrado, em que os jogadores trabalham em conjunto para montar tecnologia alienígena e desbloquear novas pontuações e formas.

* [Jogue no Decentraland](https://decentraland.org/jump?realm=alienscrapyard.dcl.eth)

## Informações do projeto

**Mantido por:** Comunidade · procesista

[Ver fonte upstream](https://github.com/procesista/Alienscrap)

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

### Representar quebra-cabeças de construção como dados

As definições de slots descrevem a peça necessária, a posição, a escala e o rótulo de cada peça. Os helpers de Template recentram e dimensionam os layouts, expõem uma sequência de quebra-cabeças e procuram slots individuais. Isto é útil para estudar como adicionar novas construções através de dados.

**Contexto do projeto:** Leia-o em conjunto com as constantes partilhadas de scene-center e part-type.

**Referências de origem:**

* [Definições de slots](https://github.com/procesista/Alienscrap/blob/be0b1b93580452f9cc778dc36554fe9354ee0ea2/src/shared/templates.ts#L3-L39)
* [Helpers de Template](https://github.com/procesista/Alienscrap/blob/be0b1b93580452f9cc778dc36554fe9354ee0ea2/src/shared/templates.ts#L140-L184)

### Ligar uma câmara cinematográfica ao estado da ronda

O sistema cinematográfico ativa planos para as fases de contagem decrescente e de desempenho, lida com jogadores que entram a meio da ronda e liberta a câmara quando o snapshot está desatualizado. A atualização final acrescenta saídas por mudança de ronda e por tempo decorrido antes de mover a câmara ao longo do seu plano.

**Contexto do projeto:** O exemplo depende do snapshot do client, das constantes de timing partilhadas e dos helpers de visibilidade do HUD.

**Referências de origem:**

* [O sistema cinematográfico](https://github.com/procesista/Alienscrap/blob/be0b1b93580452f9cc778dc36554fe9354ee0ea2/src/client/cinematic.ts#L93-L129)
* [A atualização final](https://github.com/procesista/Alienscrap/blob/be0b1b93580452f9cc778dc36554fe9354ee0ea2/src/client/cinematic.ts#L158-L176)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/published-games/alien-scrapyard.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
