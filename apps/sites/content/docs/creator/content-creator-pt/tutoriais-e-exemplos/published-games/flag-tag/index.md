> Jogo de manter a posse multijogador com rodadas cronometradas, classificações persistentes, projéteis em pool e um orçamento de iluminação baseado em proximidade.

# Flag Tag

![Pré-visualização de Flag Tag](/docs/_remote/a8bf7285858279143edb58ed.webp)

Um jogo de keep-away em Multiplayer Server 24/7. Os jogadores competem para segurar uma flag durante rondas de cinco minutos, usando boomerangs e banana traps, enquanto classificações diárias, mensais e de todos os tempos acompanham os resultados.

* [Jogue no Decentraland](https://decentraland.org/jump?realm=flagtag.dcl.eth)

## Informações do projeto

**Mantido por:** Comunidade · iillee

[Ver fonte upstream](https://github.com/iillee/flagtag)

**Licença:** Não identificado neste catálogo. Consulte o repositório upstream e o mantenedor para os termos aplicáveis antes de reutilizar.


> [!WARNING]
> **Código mantido pela comunidade — reveja antes de usar**
> 
> Este projeto da comunidade é mantido fora das organizações GitHub da Decentraland Foundation e da DAO. O seu código-fonte e a sua implementação em produção podem mudar de forma independente.
> 
> A inclusão neste catálogo não implica uma auditoria de segurança nem uma garantia de segurança. Reveja o código, as dependências e os scripts de instalação antes de o executar ou reutilizar.
> 
> [Reportar uma preocupação no catálogo](https://github.com/decentraland/docs/issues/new). Para vulnerabilidades suspeitas, siga a política de segurança do projeto upstream. Não inclua detalhes da exploração nem credenciais em issues públicas.


## Principais funcionalidades

* Servidor multiplayer
* Armazenamento para progressão do jogador, desbloqueios
* Mascaramento de Avatar para animações personalizadas

## Explorar o código

Estes links apontam para um snapshot específico da source para que as explicações continuem úteis à medida que o desenvolvimento upstream avança. São referências de aprendizagem, não uma avaliação de segurança.

### Reutilizar visuais de projectile por variante de modelo

O projectile pool pré-cria dez entities ocultas para cada cor de boomerang. As funções auxiliares de acquire e release encontram uma entity disponível e devolvem-na à sua posição oculta depois de parar o seu som.

**Contexto do projeto:** O código de flight deve posicionar as entities adquiridas e devolvê-las quando terminar; o pool depende dos assets de boomerang do projeto e do sound helper.

**Referências de origem:**

* [O projectile pool](https://github.com/iillee/flagtag/blob/d5d9c8fbf0f69eadb4d79c3ae4054e8947f256b6/src/systems/projectile/pool.ts#L11-L37)
* [Funções auxiliares de acquire e release](https://github.com/iillee/flagtag/blob/d5d9c8fbf0f69eadb4d79c3ae4054e8947f256b6/src/systems/projectile/pool.ts#L48-L66)

### Limitar as luzes ativas à volta do jogador

proximityLightSystem verifica periodicamente, seleciona as luzes mais próximas com comparações de distância ao quadrado e arrays reutilizáveis, e altera apenas as luzes cujo estado ativo difere. A scene limita o conjunto ativo a oito e controla-o através do seu auxiliar de dia/noite.

**Contexto do projeto:** Leia o setup e as constantes no mesmo file para entender a conversão de posição no mundo e as configurações de intensidade específicas do dispositivo.

**Referências de origem:**

* [proximityLightSystem](https://github.com/iillee/flagtag/blob/d5d9c8fbf0f69eadb4d79c3ae4054e8947f256b6/src/systems/proximityLights.ts#L109-L168)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/published-games/flag-tag.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
