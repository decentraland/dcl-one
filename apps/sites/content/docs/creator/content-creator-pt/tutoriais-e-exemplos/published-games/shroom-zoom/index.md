> Platforming com raycasts de salto ativados por gatilho, impulsos do jogador e sprites colecionáveis que seguem ou orbitam seu dono.

# Shroom Zoom

![Pré-visualização de Shroom Zoom](/docs/_remote/abaf092d80ad05ebe9c8fc1b.webp)

Um jogo de plataforma e contra-relógio com salto duplo, planagem, colecionáveis e exploração.

* [Jogue no Decentraland](https://decentraland.org/jump?realm=shroomzoom.dcl.eth)

## Informações do projeto

**Mantido por:** Comunidade · CanessaDCL

[Ver fonte upstream](https://github.com/CanessaDCL/dcl-shroomzoom)

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
* Impulsos de física para cogumelos saltitantes

## Explorar o código

Estes links apontam para um snapshot específico da source para que as explicações continuem úteis à medida que o desenvolvimento upstream avança. São referências de aprendizagem, não uma avaliação de segurança.

### Combine um trigger próximo com uma verificação de contacto precisa

As callbacks de BounceTrigger permitem raycasting enquanto o jogador está dentro de um trigger e removem esse sistema na saída. A lógica de raycast e salto verifica impactos para baixo contra a Entity de cogumelo configurada e aplica um impulso ao jogador perto da sua superfície.

**Contexto do projeto:** Leia-o em conjunto com BounceZoom ou BounceRegular para ver as dimensões do trigger da Scene e as definições de impulso.

**Referências de origem:**

* [callbacks de BounceTrigger](https://github.com/CanessaDCL/dcl-shroomzoom/blob/042968e3c13b5d1093d2922d5713570ad4cb0284/src/bounce/class.bounce.ts#L75-L101)
* [A lógica de raycast e salto](https://github.com/CanessaDCL/dcl-shroomzoom/blob/042968e3c13b5d1093d2922d5713570ad4cb0284/src/bounce/class.bounce.ts#L103-L164)

### Transforme os itens recolhidos numa comitiva animada

Os helpers de alvo dos followers calculam offsets em torno do jogador e posições numa órbita. followSpritesSystem interpola cada sprite em direção ao seu alvo, mudando para orbitação quando a coleção está completa e rodando os sprites em direção ao respetivo proprietário.

**Contexto do projeto:** Os conjuntos de coleção da Scene, as Entities follower e os timestamps de órbita por jogador fornecem o estado para este comportamento visual.

**Referências de origem:**

* [helpers de alvo dos followers](https://github.com/CanessaDCL/dcl-shroomzoom/blob/042968e3c13b5d1093d2922d5713570ad4cb0284/src/spriteFollowers.ts#L400-L437)
* [followSpritesSystem](https://github.com/CanessaDCL/dcl-shroomzoom/blob/042968e3c13b5d1093d2922d5713570ad4cb0284/src/spriteFollowers.ts#L678-L706)


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/tutoriais-e-exemplos/published-games/shroom-zoom.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
