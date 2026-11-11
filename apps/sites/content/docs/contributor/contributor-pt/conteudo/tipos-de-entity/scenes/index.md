# Cenas

Scenes são as [entidades](/docs/contributor/contributor-pt/conteudo/entities/) que produzem comportamento em diferentes partes de Decentraland. Elas podem abranger vários parcels de LAND, e os players que caminharem por elas em um World Explorer executarão código especial fornecido pela scene.

Você pode encontrar um exemplo completo da vida real em [o manifest desta scene](https://peer.decentraland.org/content/contents/bafkreidihplrc5cxarjkji2enmzhicpjoa2vrlqyyffnholmxb3o2xft3u) no servidor de conteúdo da Foundation.

### Ponteiros <a href="#pointers" id="pointers"></a>

Scenes têm um ou mais associados [pointers](/docs/contributor/contributor-pt/conteudo/pointers/) na forma de strings de coordenadas de parcelas, como `"0,0"`. Toda parcela que executar a scene será listada no [`pointers` array](/docs/contributor/contributor-pt/conteudo/entities/#properties) do manifest da entity, e todos esses ponteiros serão resolvidos pelo servidor de conteúdo para essa scene.

Em outras palavras, você pode escolher as coordenadas x/y de uma parcela e localizar a scene em execução consultando o servidor de conteúdo pelo `"<x>,<y>"` ponteiro. É assim que os World Explorers decidem se devem habilitar o comportamento de uma scene à medida que o player navega pelo mundo.

Um típico `pointers` array se parece com isto:

```json
{
  "pointers": [
    "-113,-134",
    "-113,-133",
    "-113,-132",
  ],
  // ... outras propriedades da entity
}
```

Esses dados também estão disponíveis no `metadata.scene.parcels` campo detalhado abaixo, caso você queira salvar apenas o objeto específico da scene no manifest.

### Campos de Metadata

Há várias propriedades especiais para [entidades](/docs/contributor/contributor-pt/conteudo/entities/) do tipo `scene` localizadas no `metadata` campo de nível superior.

| Campo                 | Valor                                                                                                                                |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| `owner`               | Informações sobre o mantenedor da scene.                                                                                             |
| `main`                | O [nome de arquivo interno](/docs/contributor/contributor-pt/conteudo/entities/#files) para o arquivo JavaScript principal desta scene. |
| `tags`                | Um array de rótulos de string descritivos desta scene.                                                                               |
| `display`             | Informações sobre a scene para os Explorers mostrarem aos players (veja abaixo).                                                     |
| `scene`               | As parcelas em que esta scene está ativa e sua localização central (veja abaixo).                                                    |
| `contact`             | Nome e endereço de email para entrar em contato com os criadores ou mantenedores da scene (veja abaixo).                             |
| `spawnPoints`         | Locais e ângulos de câmera para players entrando nesta scene (veja abaixo).                                                          |
| `requiredPermissions` | Permissões recomendadas para os Explorers solicitarem aos players (veja abaixo).                                                     |
| `featureToggles`      | Configurações para feature flags                                                                                                     |


> [!NOTE]
> Lembre-se de que entities e seus metadata podem ter campos personalizados. Você pode encontrar algumas propriedades por aí que não estão listadas aqui.


#### Exibição

Em `metadata.display`, você encontrará propriedades úteis para informar os players sobre a scene antes de entrarem nela.

| Campo             | Valor                                                                                                                                               |
| ----------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| `title`           | O nome de exibição desta scene.                                                                                                                     |
| `description`     | Uma descrição estendida desta scene.                                                                                                                |
| `favicon`         | *Obsoleto* O [nome de arquivo interno](/docs/contributor/contributor-pt/conteudo/entities/#files) para um ícone, exibido quando esta scene está ativa. |
| `navmapThumbnail` | O [nome de arquivo interno](/docs/contributor/contributor-pt/conteudo/entities/#files) para a miniatura desta scene para o mapa-múndi.                 |

Um exemplo:

```json
{
  "title": "my_cool_scene",
  "favicon": "favicon.png",
  "navmapThumbnail": "thumbnail.png"
}
```

#### Cena

O `metadata.scene` a propriedade é um objeto que descreve a posição desta scene no mapa-múndi.

| Campo     | Valor                                                      |
| --------- | ---------------------------------------------------------- |
| `parcels` | Um array de ponteiros de parcelas que executam esta scene. |
| `base`    | O ponteiro da parcela para o ponto de origem desta scene.  |

Na prática, fica assim:

```json
{
  "parcels": [
    "17,-112", 
    "17,-113"
  ],
  "base": "17,-112"
}
```

O `base` campo está sempre incluído no `parcels` array.

#### Contato

O `metadata.contact` o objeto contém as informações de contato do proprietário ou mantenedor da scene.

| Campo   | Valor                                         |
| ------- | --------------------------------------------- |
| `name`  | Um nome para este contato.                    |
| `email` | (Opcional) Um endereço de email para contato. |

Em JSON:

```json
{
  "name": "cool_scene_maker",
  "email": "hello@decentraland.org"
}
```

Embora o `name` campo seja preenchido em toda scene, você verá que, na prática, o `email` campo frequentemente é uma string vazia. Esta é uma escolha de privacidade feita pelo criador da scene.

#### Pontos de spawn

O `metadata.spawnPoints` campo define um array de pontos e direções de câmera que determinam onde os players que entram na scene devem aparecer e para onde devem olhar inicialmente. Deve haver pelo menos um ponto de spawn definido.

Cada item no array tem várias propriedades:

| Campo          | Valor                                                                                      |
| -------------- | ------------------------------------------------------------------------------------------ |
| `name`         | Um rótulo para identificar este ponto de spawn.                                            |
| `position`     | A posição inicial em um `{ x, y, z }` objeto com coordenadas de ponto flutuante.           |
| `cameraTarget` | A direção inicial da câmera em um `{ x, y, z }` objeto com coordenadas de ponto flutuante. |
| `default`      | Se este ponto de spawn é usado, salvo indicação em contrário.                              |

Por exemplo:

```json
[
  {
    "name": "spawn1",
    "position": { "x": 10.02, "y": 5.27, "z": 15.23  },
    "cameraTarget": { "x": 10.02, "y": 6.27, "z": 31.23 },
    "default": true
  }
]
```


> [!NOTE]
> **📔 Nota**: A Spawn Area e o Camera Target podem ser definidos usando o Creator Hub. Para mais informações, vá para a [Spawn Area](https://github.com/decentraland/docs/tree/main/creator/scene-editor/build/spawn-areas.md) docs.


#### Permissões Requeridas <a href="#permissions" id="permissions"></a>

Em `metadata.requiredPermissions` você encontrará um array de strings bem conhecidas que declaram quais permissões devem ser solicitadas ao player antes que o World Explorer permita certas ações. Isso é responsabilidade do Explorer; em outros clients construídos para outros propósitos, podem optar por ignorar isso.

Este é o conjunto atual de permissões suportadas:

| Permissão                           | Propósito                                                                      |
| ----------------------------------- | ------------------------------------------------------------------------------ |
| `USE_FETCH`                         | Permite que a scene faça solicitações HTTP externas.                           |
| `USE_WEBSOCKET`                     | Permite que a scene use a API WebSocket para estabelecer conexões externas.    |
| `OPEN_EXTERNAL_LINK`                | Permite que a scene abra uma URL (em uma aba do navegador ou em uma web view). |
| `USE_WEB3_API`                      | Permite que a scene se comunique com uma wallet.                               |
| `ALLOW_TO_TRIGGER_AVATAR_EMOTE`     | Permite que a scene anime o avatar do player com uma emote.                    |
| `ALLOW_TO_MOVE_PLAYER_INSIDE_SCENE` | Permite que a scene altere a posição do player.                                |

#### Alternâncias de recursos

O `metadata.featureToggles` campo permite que uma scene indique se certos recursos do World Explorer devem ser habilitados ou desabilitados.

O campo contém um objeto da forma `{ [featureName]: 'enabled' | 'disabled' }`, como este:

```json
{
  "voiceChat": "enabled"
}
```

Atualmente, `voiceChat` é a única feature flag com suporte comum.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-pt/conteudo/tipos-de-entity/scenes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
