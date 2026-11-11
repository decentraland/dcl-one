> Ligar a outras cenas ou sites externos

# Ligações externas

Você pode adicionar links da sua scene para outro conteúdo, seja para outras scenes ou para websites externos.

## Use o Scene Editor no Creator Hub

A maneira mais fácil de adicionar um link externo ou um teleporte é usar o Scene Editor. Use o **Teleport** [Smart Item](/docs/creator/content-creator-pt/scene-editor/interatividade/smart-items/) para adicionar um teleporte para outra scene em Genesis City, ou use qualquer um dos **Social Links** smart items para adicionar links para sites externos.

![](/docs/_remote/71809788940cd245e36af334.webp)

## Teleports

Para teleportar um jogador para outra scene, chame a seguinte função, indicando as coordenadas para onde você quer que os jogadores sejam teleportados.

```ts
import { teleportTo } from "~system/RestrictedActions"

(...)

teleportTo({ worldCoordinates: { x: -51, y: 1 } })
```

Os jogadores recebem uma tela de confirmação antes de serem teleportados; essa tela exibe informações do `arquivo scene.json`, incluindo o `nome da scene`, `descrição` e `navmapThumbnail`. Veja [metadados da scene](/docs/creator/content-creator-pt/scenes-sdk7/tipos-de-projetos/scene-metadata/) para detalhes sobre como definir esses dados.

Tenha em mente que os teleports levam você a uma scene nas coordenadas indicadas, mas não necessariamente a essas mesmas coordenadas. Isso significa que, ao viajar para uma scene que tem múltiplos parcels, os jogadores podem não aterrissar nas mesmas coordenadas especificadas, mas sim em um dos spawn points designados pelo criador da scene.

Para mover um jogador para outro conjunto de coordenadas dentro da scene atual, use a `movePlayerTo()` função em vez disso. Veja [Mover um Jogador](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/move-player/).

## Teleport para um WORLD

Para enviar um jogador para uma scene que não está publicada no mapa aberto de Genesis City, mas sim para um [Decentraland WORLD](/docs/creator/content-creator-pt/scenes-sdk7/publicacao/publishing-options/#decentraland-worlds), passe um `realm` para `teleportTo()`.

```ts
import { teleportTo } from "~system/RestrictedActions"

(...)

teleportTo({ realm: 'mannakia.dcl.eth' })
```

`realm` aceita tanto um nome de World como `mannakia.dcl.eth`ou uma URL de realm.

Sem `worldCoordinates`, o jogador aterrissa em um dos spawn points do destino. Adicione coordenadas para escolher um Parcel específico dentro desse World:

```ts
// Aterrisse no parcel 12,34 de um World específico
teleportTo({
	realm: 'mannakia.dcl.eth',
	worldCoordinates: { x: 12, y: 34 },
})
```

Os jogadores recebem uma tela de confirmação antes de mudarem de realm. Ela pergunta *"Tem certeza de que deseja entrar neste World?"* e mostra o destino (o nome do World, ou o host da URL do realm). O jogador só é movido se aceitar.

Para enviar um jogador de volta para Genesis City a partir de um World, não inclua o `realm` campo, que por padrão aponta para Genesis City.


> [!WARNING]
> **📔 Nota**: O `realm` campo requer `@dcl/sdk` versão 7.28.0 ou mais recente.


### changeRealm está obsoleto

As scenes mais antigas usavam `changeRealm()` para alcançar um World. Ainda funciona, mas use `teleportTo({ realm })` em vez disso. `changeRealm()` também mostra uma tela de confirmação; seu campo opcional `message` apenas substitui o texto padrão do prompt.

`changeRealm()` é resolvido assim que o jogador aceita o prompt, e não quando o novo realm realmente entra no ar. Uma `teleportTo()` chamada logo depois disso seria executada contra o **realm** antigo e colocaria o jogador no lugar errado. `teleportTo({ realm })` faz as duas etapas em uma única chamada, então essa lacuna não existe.

## Links externos

Para adicionar um link para um website externo, use o `comando openExternalUrl().` import { openExternalUrl } from '\~system/RestrictedActions'

```ts
openExternalUrl({ url: 'google.com' })

Para evitar qualquer uso abusivo deste recurso para enviar spam aos jogadores, só é possível chamar o
```

openExternalUrl `a partir de um clique explícito ou de um evento de botão em uma entity. Não é possível chamar esta função como resultado de um timer, de uma área de colisão ou de um evento global de clique. Veja` Button events [para detalhes sobre como fazer isso.](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/) Quando

é chamado, os jogadores recebem uma tela de confirmação, onde são informados para onde o link os levará e onde podem aceitar ou recusar visitar o link. `a partir de um clique explícito ou de um evento de botão em uma entity. Não é possível chamar esta função como resultado de um timer, de uma área de colisão ou de um evento global de clique. Veja` O link é aberto em uma nova aba, mantendo a aba original em Decentraland.

Se os jogadores marcarem a caixa de seleção

trust this domain *, eles não serão solicitados novamente durante a sessão, desde que o link venha da mesma scene e aponte para o mesmo domínio.* Copiar para a área de transferência

## Para copiar uma string para a área de transferência do jogador, use

copyToClipboard() `. Depois disso, quando o jogador fizer`colar *no chat do Decentraland ou em qualquer outro aplicativo no computador dele, ele estará colando a sua string.* import { copyToClipboard } from "\~system/RestrictedActions"

```ts
copyToClipboard( { text: 'My text to copy' } )

copyToClipboard( { text: 'My text to copy' } )
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/interatividade/external-links.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
