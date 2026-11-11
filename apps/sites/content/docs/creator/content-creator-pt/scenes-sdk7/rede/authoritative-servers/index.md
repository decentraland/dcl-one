> Crie scenes multiplayer do Decentraland com um multiplayer server sem interface.

# Multiplayer Server

**📔 Nota**: O Multiplayer Server era anteriormente chamado de **Servidor Autoritativo**. Apenas o nome mudou; a feature é a mesma. A branch do SDK a instalar ainda se chama `auth-server`, veja [Configuração](#setup).

## Visão geral

Decentraland executa scenes localmente na máquina de um player. Por padrão, os players conseguem ver uns aos outros e interagir diretamente, mas cada um interage com o environment de forma independente. As mudanças no environment não são compartilhadas entre os players por padrão.

Permitir que todos os players vejam uma scene como se ela tivesse o mesmo conteúdo no mesmo state é extremamente importante para que os players interajam de maneiras mais significativas. Sem isso, se um player abrir uma door e entrar em uma house, os outros players verão essa door ainda fechada, e o primeiro player parecerá atravessar diretamente a door fechada para os outros players.

O **Multiplayer Server** é um processo de server sem interface que executa o código da sua scene, valida mudanças de state e transmite o resultado para todos os players conectados. Ele segue uma arquitetura de server autoritativo: em vez de confiar que cada client relate suas próprias ações, o server atua como a única source of truth. Isso o torna a abordagem recomendada para sincronizar scenes multiplayer.

O Multiplayer Server é ideal sempre que a fairness é importante para a game mechanics, pois você pode implementar validações anti-cheat elaboradas que rodam no side do server. Você também pode armazenar private keys e outras informações sensíveis no server, evitando precisar expô-las diretamente ao usuário.

Ter um Multiplayer Server também resolve um problema real: em uma configuração peer-to-peer, dois players controlando algo como uma floating platform podem produzir resultados conflitantes. Cada client define a platform para uma altura diferente, e ninguém tem autoridade para decidir qual está correto. O Multiplayer Server resolve toda mudança em um único lugar, de modo que todos os clients converjam para o mesmo state.

Ele também lhe dá um lugar para **persistir dados entre sessions**: leaderboards, progression do player, achievements desbloqueadas ou mudanças no environment, como doors abertas ou items colocados. Quando os players voltam, o mundo reflete o que aconteceu antes.

Decentraland hospeda e faz o deploy do server para você. Publicar sua scene pelo processo normal também publica o server de forma integrada, sem etapas extras nem necessidade de pagar por qualquer hospedagem.

## Configuração

### 1. Instale a versão auth-server do SDK

As APIs nativas do Multiplayer Server (`isServer`, `registerMessages`, `Storage`, `EnvVar`, etc.) estão disponíveis em uma branch separada do SDK. Execute os comandos a seguir para instalá-la no seu projeto em vez da branch padrão do SDK:

```bash
npm install @dcl/sdk@auth-server
npm install @dcl/js-runtime@auth-server
```

### 2. Configure scene.json

O Multiplayer Server é ativado pela flag `"authoritativeMultiplayer": true` em `scene.json`, no nível raiz. Você não precisa adicioná-la manualmente: ao usar a branch auth-server do SDK, ela é adicionada automaticamente na primeira vez que você build ou preview a scene. Apenas certifique-se de não removê-la; sem essa flag o server nunca roda e `isServer()` sempre retorna *false*.

Opcionalmente, você também pode adicionar o seguinte ao seu `scene.json` no nível raiz:

```json
{
  "logsPermissions": ["0xYourWalletAddress"]
}
```

Adicione `logsPermissions` para dar a essas wallets acesso aos logs e métricas do server após o deploy. World owners e colaboradores de deploy elegíveis também têm acesso, assim como os owners de Genesis City e os LAND managers delegados da base parcel da scene. Veja [Scene Insights](/docs/creator/content-creator-pt/scenes-sdk7/depuracao/scene-insights/) para permissões, login e comandos.

### 3. Execute o preview

Use o comando padrão de preview, sem necessidade de etapas extras. Ao usar a branch auth-server do SDK, o preview inicia automaticamente uma versão local do Multiplayer Server em segundo plano.

A session local do server não está conectada à produção, então você fica livre para testar coisas sem afetar os players que estão na sua scene publicada.

## Server / Client Branching

O código da scene na pasta do seu projeto `src` é executado tanto no server quanto no client. Use a `isServer()` function para dividir os caminhos de execução:

```typescript
import { isServer } from "@dcl/sdk/network"
import { initServer } from "./server/server"
import { initClient } from "./client/setup"
import { setupUi } from "./client/ui"

function main() {
  if (isServer()) {
    // Somente no server: lógica de jogo, validação, gerenciamento de state
    initServer()
    return
  } else {
    // Somente no client: UI, tratamento de input, envio de messages
    initClient()
    setupUi()
  }
}
```

O server executa sua scene sem interface e sem rendering. Ele tem acesso verificado a todas as posições dos players, wearables e outros dados por meio de `PlayerIdentityData` e é a única autoridade sobre o state do jogo.

## Components Sincronizados e Validação

### Sincronizando Entities para Todos os Clients

Use `syncEntity` para transmitir quaisquer mudanças nos Components indicados dessa Entity:

```typescript
import { isServer, syncEntity } from "@dcl/sdk/network"

if (isServer()) {
  syncEntity(
    entity,
    [Transform.componentId, GameState.componentId],
    /* enumId */ 1
  )
}
```

A sintaxe é idêntica à usada pelo [multiplayer serverless](/docs/creator/content-creator-pt/scenes-sdk7/rede/serverless-multiplayer/) feature, tornando trivial fazer upgrade de uma scene que usa essa arquitetura para o Multiplayer Server. Quando uma scene usa o Multiplayer Server, as state updates não são mais enviadas entre todos os players; em vez disso, todas as state updates agora são roteadas e validadas pelo server.


> [!WARNING]
> **📔 Nota**: Com o Multiplayer Server, o padrão ideal é fazer com que apenas o server chame `syncEntity`. Dessa forma, você não precisa se preocupar com a consistência do entity-id. Em vez disso, a Entity é instanciada e compartilhada pelo server, e todos os clients recebem sync sobre essa instância. Sempre proteja isso com `isServer()`. Isso é diferente de [multiplayer serverless](/docs/creator/content-creator-pt/scenes-sdk7/rede/serverless-multiplayer/), onde cada client chama `syncEntity` por conta própria.


### Validando mudanças

Use `validateBeforeChange()` para restringir qualquer state update em um Component específico de uma Entity. Ela permite executar uma função de validação personalizada, e as mudanças só são bem-sucedidas quando o teste de validação é atendido.

Se a validação retornar o valor *true*, então a mudança é aceita e propagada para todos os players. Se a validação retornar o valor *false*, então a mudança é rejeitada. Uma mudança rejeitada não será passada para outros players e será revertida para o player que tentou fazê-la.

#### Validar values

O caso mais simples é validar que o novo *value* sendo escrito está dentro de certos parâmetros. Por exemplo, aceite apenas mudanças para um `Transform` quando a nova posição Y estiver acima de 0:

```typescript
import { engine, Transform } from "@dcl/sdk/ecs"
import { Vector3 } from "@dcl/sdk/math"
import { isServer } from "@dcl/sdk/network"

const entity = engine.addEntity()
Transform.create(entity, { position: Vector3.create(10, 2, 10) })

if (isServer()) {
  Transform.validateBeforeChange(entity, (value) => {
    // Rejeite qualquer update que colocaria a Entity em Y = 0 ou abaixo
    return value.newValue.position.y > 0
  })
}
```

Porque `validateBeforeChange()` só tem significado no server; sempre proteja isso com `isServer()`. No client, a chamada não faz nada de útil.

Você pode usar isso para impedir mudanças que contrariem sua game logic, como mecanismos anti-cheat.

#### Validar proximidade ao player

Você também pode combinar `validateBeforeChange()` com posições de player verificadas pelo server para checar se um player está perto o suficiente de um object antes de permitir que ele interaja com ele. Por exemplo, quando um player tenta pegar um object mudando sua `Transform`, você pode rejeitar a mudança se o object estiver a mais de 5 metros do player:

```typescript
import { engine, Transform, PlayerIdentityData } from "@dcl/sdk/ecs"
import { Vector3 } from "@dcl/sdk/math"
import { isServer } from "@dcl/sdk/network"

const pickableEntity = engine.addEntity()
Transform.create(pickableEntity, { position: Vector3.create(8, 1, 8) })

if (isServer()) {
  Transform.validateBeforeChange(pickableEntity, (value) => {
    // Encontre o player que enviou esta mudança
    for (const [playerEntity, identity] of engine.getEntitiesWith(
      PlayerIdentityData
    )) {
      if (identity.address.toLowerCase() !== value.senderAddress.toLowerCase())
        continue

      const playerTransform = Transform.getOrNull(playerEntity)
      if (!playerTransform) return false

      // Obtenha a posição atual do object, antes da mudança
      const objectTransform = Transform.getOrNull(pickableEntity)
      if (!objectTransform) return false

      const distance = Vector3.distance(
        playerTransform.position,
        objectTransform.position
      )

      // Permita a mudança apenas se o player estiver a até 5 metros
      return distance <= 5
    }

    // Remetente não encontrado entre os players conectados — rejeitar
    return false
  })
}
```

Esse padrão é útil como um mecanismo anti-cheat: ele impede que os players alcancem objetos do outro lado da scene para pegar items com os quais não deveriam conseguir interagir.

#### Permitir mudanças apenas por admins

Você também pode validar com base em *quem* está enviando a mudança. Cada value recebido inclui um `senderAddress` field com o wallet address do remetente. Use isso para permitir mudanças apenas de certos players. Por exemplo, para permitir que apenas admins da scene modifiquem um `VideoPlayer` component:

```typescript
import { engine, VideoPlayer } from "@dcl/sdk/ecs"
import { isServer } from "@dcl/sdk/network"
import { isPreview } from "@dcl/asset-packs/dist/admin-toolkit-ui/fetch-utils"
import { getSceneAdmins } from "@dcl/asset-packs/dist/admin-toolkit-ui/ModerationControl/api"

// Cache de endereços de wallet de admins, atualizado a partir da lista de admins da scene
let adminAddresses = new Set<string>()

async function updateAdminAddresses() {
  if (isPreview()) return
  try {
    const [error, response] = await getSceneAdmins()
    if (error) {
      console.error("[SERVER] Erro ao buscar a lista de admins:", error)
      adminAddresses = new Set()
      return
    }
    adminAddresses = new Set(
      (response ?? []).map((admin) => admin.admin.toLowerCase())
    )
    console.log(
      "[SERVER] Cache de endereços de admin atualizada:",
      Array.from(adminAddresses)
    )
  } catch (error) {
    console.error("[SERVER] Erro ao atualizar os endereços de admin:", error)
    adminAddresses = new Set()
  }
}

export async function main() {
  const videoEntity = engine.addEntity()
  VideoPlayer.create(videoEntity, { src: "videos/intro.mp4" })

  if (isServer()) {
    // Preencha o cache antes de conectar a validação
    await updateAdminAddresses()

    VideoPlayer.validateBeforeChange(videoEntity, (value) => {
      // Sempre permita mudanças enquanto estiver rodando em preview, para facilitar o teste local
      if (isPreview()) return true

      const senderAddress = value.senderAddress.toLowerCase()
      if (!adminAddresses.has(senderAddress)) {
        console.log(
          "[SERVER] Mudança não autorizada de VideoPlayer bloqueada de:",
          senderAddress
        )
        return false
      }
      return true
    })
  }
}
```

Veja [Admin da Scene](/docs/creator/content-creator-pt/scene-editor/operar-ao-vivo/scene-admin/) para mais contexto sobre como os players se tornam admins em uma scene.

#### Envie intenção, nunca resultados

Sempre que scores, prêmios ou resources compartilhados estiverem em jogo, projete suas messages de client em torno do que o player está **tentando fazer**, e não em torno do resultado. Um client envia `claimPoint` ou `harvest`. Ele nunca envia "minha pontuação agora é 40".

O server então verifica a message: quem a enviou, se o jogo está em uma phase que permite isso, se o player está perto o suficiente e se ainda resta algo para reivindicar. Só então ele calcula o novo value e o grava. O server é o único writer do state do jogo.

Um client que consegue nomear sua própria score pode nomear qualquer score.

#### Permitir mudanças apenas pelo server

O caso mais rigoroso é aceitar apenas writes que se originam do próprio server, rejeitando qualquer mudança vinda de um client. Esse é o padrão ideal para state que deve ser totalmente autoritativo: scores, game phase, Entities spawned, etc.

Cada value recebido inclui um `senderAddress` field. Quando o remetente é o server, esse field corresponde à constant `AUTH_SERVER_PEER_ID`, exportada de `@dcl/sdk/network/message-bus-sync`.

O exemplo abaixo define uma pequena `protectServerEntity()` helper que aplica essa verificação a um ou mais Components em uma determinada Entity. É uma forma conveniente de proteger múltiplos Components (como `Transform` e `GltfContainer`) em uma única chamada:

```typescript
import { engine, Entity, Transform, GltfContainer } from "@dcl/sdk/ecs"
import { Vector3 } from "@dcl/sdk/math"
import { isServer } from "@dcl/sdk/network"
import { AUTH_SERVER_PEER_ID } from "@dcl/sdk/network/message-bus-sync"

type ComponentWithValidation = {
  validateBeforeChange: (
    entity: Entity,
    cb: (value: { senderAddress: string }) => boolean
  ) => void
}

function protectServerEntity(
  entity: Entity,
  components: ComponentWithValidation[]
) {
  for (const component of components) {
    component.validateBeforeChange(entity, (value) => {
      return value.senderAddress === AUTH_SERVER_PEER_ID
    })
  }
}

if (isServer()) {
  // Depois de criar uma Entity gerenciada pelo server:
  const entity = engine.addEntity()
  Transform.create(entity, { position: Vector3.create(10, 5, 10) })
  GltfContainer.create(entity, { src: "assets/model.glb" })
  protectServerEntity(entity, [Transform, GltfContainer])
}
```


> [!WARNING]
> **📔 Nota**: Sempre chame `protectServerEntity()` dentro de um `isServer()` bloco. Ele envolve `validateBeforeChange()`, que só tem significado no server — chamá-lo em um client produz erros.


#### Componentes personalizados

Você também pode aplicar `validateBeforeChange()` em Components personalizados definidos pela scene.

```typescript
import { engine, Schemas } from "@dcl/sdk/ecs"
import { isServer } from "@dcl/sdk/network"
import { AUTH_SERVER_PEER_ID } from "@dcl/sdk/network/message-bus-sync"

export const GameState = engine.defineComponent("game:State", {
  phase: Schemas.String,
  score: Schemas.Int,
  timeRemaining: Schemas.Int,
})

// Somente o server pode modificar esse Component
if (isServer()) {
  GameState.validateBeforeChange((value) => {
    return value.senderAddress === AUTH_SERVER_PEER_ID
  })
}
```

## Messages

Components sincronizados são ótimos para state que todos os players devem ver continuamente: coisas como posições, scores ou game phase. Mas nem tudo se encaixa nesse modelo. Às vezes, um player só precisa dizer ao server "eu cliquei neste button" ou "quero entrar no jogo", e o server precisa responder com uma resposta única como "a rodada começou" ou "aqui estão suas stats". Isso são events, não state contínuo.

É para isso que servem as mensagens. Use `registerMessages()` para uma comunicação tipada e validada por schema entre clients e server. Messages são de envio único: um client envia uma para o server, o server a processa e, opcionalmente, envia uma de volta. Elas não criam nenhum state persistente por conta própria.

### Defina Messages

Defina todas as messages em um arquivo compartilhado que tanto o server quanto o client importam. Assim, ambos os lados sempre concordam sobre quais messages existem e quais dados elas carregam. Cada message é um `Schemas.Map` que descreve seu payload:

```typescript
import { Schemas } from "@dcl/sdk/ecs"
import { registerMessages } from "@dcl/sdk/network"

export const Messages = {
  // Client → Server
  playerReady: Schemas.Map({ displayName: Schemas.String }),
  playerAction: Schemas.Map({ action: Schemas.String, targetId: Schemas.Int }),
  giveCoins: Schemas.Map({ to: Schemas.String, amount: Schemas.Int }),

  // Server → Client
  gameStarted: Schemas.Map({ roundNumber: Schemas.Int }),
  gameEnded: Schemas.Map({ winnerId: Schemas.String }),
}

export const room = registerMessages(Messages)
```


> [!WARNING]
> **📔 Nota**: Chame `registerMessages()` uma vez, no nível superior de um módulo compartilhado, como no exemplo acima. Assim, ela é executada quando o módulo carrega pela primeira vez, tanto no server quanto no client. Não a chame condicionalmente nem dentro de functions; ambos os lados sempre devem registrar as mesmas messages.


### Enviar mensagens

Os clients só podem enviar messages ao server. Não há messaging direto de client para client. O server pode fazer broadcast para todos os clients ou direcionar players específicos por address.

```typescript
import { room } from "./shared/messages"

// Client → Server (broadcast, o server recebe)
room.send("playerReady", { displayName: "Alice" })

// Server → todos os clients
room.send("gameStarted", { roundNumber: 1 })

// Server → um client específico (por wallet address)
room.send("gameEnded", { winnerId: "Alice" }, { to: [playerAddress] })
```


> [!WARNING]
> **📔 Nota**Messages têm um tamanho máximo de cerca de 13 KB. Messages maiores são descartadas silenciosamente pelo transporte, sem gerar qualquer erro. Mantenha os payloads pequenos; se precisar compartilhar dados maiores, divida-os em várias messages ou use Components sincronizados.


### Receber mensagens

```typescript
import { room } from "./shared/messages"

// Client recebe do server
room.onMessage("gameStarted", (data) => {
  console.log(`Rodada ${data.roundNumber} iniciada!`)
})

// Server recebe do client
room.onMessage("playerReady", (data, context) => {
  if (!context) return
  const senderAddress = context.from // wallet address verificado
  console.log(`[Server] ${data.displayName} está pronto (${senderAddress})`)
})
```

No server, cada message recebida inclui um `context` object com o wallet address verificado do remetente. Use isso para saber qual player enviou a message (nunca confie na identidade informada pelo próprio payload).

`context.from` chega exatamente como a rede o reporta, sem alteração de maiúsculas ou minúsculas. Coloque-o em minúsculas uma vez (`context.from.toLowerCase()`) e use esse value para cada map, set e chamada de Storage, para que os dados de um player não acabem divididos entre duas keys.

### Aguarde a sync do State antes de enviar

Os clients devem aguardar até que o state da scene seja sincronizado antes de enviar sua primeira message, para evitar race conditions na entrada:

```typescript
import { engine } from "@dcl/sdk/ecs"
import { isStateSyncronized } from "@dcl/sdk/network"
import { room } from "./shared/messages"

engine.addSystem(() => {
  if (!isStateSyncronized()) return

  // Seguro para enviar messages agora
  room.send("playerReady", { displayName: "Alice" })
})
```


> [!NOTE]
> **💡 Dica**: `isStateSyncronized()` só informa que o state do client está sincronizado; isso não garante que o server tenha terminado de inicializar. Para uma verificação de prontidão mais robusta, faça o server transmitir uma message de "ready" ou heartbeat, e faça os clients esperarem por ela antes de enviar messages de gameplay.


### Tipos de Schema disponíveis

Todos os payloads de messages e Components personalizados usam `Schemas` para serialização binária. Aqui está uma referência rápida dos tipos disponíveis:

```typescript
import { Schemas } from "@dcl/sdk/ecs"

// Tipos básicos
Schemas.String // "hello"
Schemas.Int // 42
Schemas.Float // 3.14
Schemas.Boolean // true / false
Schemas.Int64 // Date.now()

// Tipos de vetor
Schemas.Vector3 // { x: 1, y: 2, z: 3 }
Schemas.Quaternion // { x, y, z, w }

// Tipos complexos
Schemas.Array(Schemas.String) // ["a", "b", "c"]
Schemas.Entity // referência de Entity
Schemas.Optional(Schemas.String) // "hello" ou undefined
Schemas.Optional(Schemas.Int) // 42 ou undefined

// Objetos aninhados
Schemas.Map({
  name: Schemas.String,
  health: Schemas.Int,
  position: Schemas.Vector3,
  playerId: Schemas.Optional(Schemas.String),
})
```


> [!WARNING]
> **📔 Nota**: Messages *deve* ser definido usando `Schemas.Map(...)`. Não podes enviar objetos JavaScript simples; eles falharão na serialização binária.


## Server a Ler as Posições dos Jogadores

O server pode ler **verificadas** as posições dos jogadores; os clients não as conseguem falsificar. Esta é a base do anti-cheat baseado em posição:

```typescript
import { engine, PlayerIdentityData, Transform } from "@dcl/sdk/ecs"

engine.addSystem(() => {
  for (const [entity, identity] of engine.getEntitiesWith(PlayerIdentityData)) {
    const transform = Transform.getOrNull(entity)
    if (!transform) continue

    const address = identity.address
    const position = transform.position
    // Esta posição é verificada pelo server — nunca confies na posição reportada pelo client
  }
})
```


> [!WARNING]
> **📔 Nota**: Usa sempre `PlayerIdentityData` + `Transform` no server para obter as posições dos jogadores. Nunca confies nos valores reportados pelo próprio client.


## Armazenamento de Dados

Mantém os dados persistentes entre reinícios do server. Storage é **apenas do server**, protege sempre as chamadas com `isServer()`. O server pode tanto escrever como ler estes dados.

```typescript
import { Storage } from "@dcl/sdk/server"
```

Os dados podem ser armazenados em dois níveis:

* **World**: Usa isto para dados relevantes para todos os jogadores, como leaderboards ou alterações persistentes do ambiente.
* **Player**: Usa isto para dados específicos do jogador, como guardar progresso ou preferências desse jogador.


> [!NOTE]
> **💡 Dica**: Os valores de Storage são JSON. Podes armazenar um número, uma string, um array ou um objeto simples tal como está e lê-lo novamente como o mesmo valor. Não precisas de `JSON.stringify()` ou `JSON.parse()`. Duas exceções: um `Date` é armazenado como o seu texto ISO (`"2026-10-27T13:05:12.000Z"`), e as propriedades do objeto definidas como `undefined` são descartadas. Um valor que guardaste anteriormente com `JSON.stringify()` é uma string, por isso é lido novamente como essa string.
> 
> Durante o desenvolvimento local, o storage é escrito em `node_modules/@dcl/sdk-commands/.runtime-data/server-storage.json`.



> [!WARNING]
> **📔 Nota**: Como as chamadas de Storage reportam problemas:
> 
> * `Storage.set()` e `Storage.player.set()` resolvem para um **boolean**. `false` significa que o valor foi **não** persistido, por exemplo após um erro de network ou quando correm demasiados requests ao mesmo tempo (ver abaixo). Verifica sempre o resultado: um discarded `false` é um save perdido silenciosamente.
> * `set()` lança um `TypeError` quando o valor não pode ser armazenado como JSON inalterado: `undefined`, `NaN` ou `Infinity`, uma função, um `Map` ou um `Set`, entre outros. Para remover uma chave, usa `delete()`.
> * `get()` resolve para o valor armazenado, ou para `null` apenas quando a chave não existe. Quando a leitura falha, lança uma exceção, por isso uma falha nunca é confundida com uma chave em falta.
> * O `delete()` as variantes lançam uma exceção quando o request falha. Resolvem para `false` apenas quando a chave não existia.


### Guarda nos checkpoints, não em cada alteração

Storage é persistência duradoura para dados que têm de sobreviver a reinícios do server e a redeploys. Não é um datastore em tempo real. Mantém o estado de trabalho do jogo na memória do server. Isso é mais rápido e é o padrão certo para um server. Escreve em Storage apenas quando realmente precisares, em checkpoints significativos.


> [!WARNING]
> **⚠️ Aviso**: O runtime do server permite um máximo de **40 host calls em curso** ao mesmo tempo, partilhadas por *tudo* o que a scene pede ao runtime para fazer: cada request de storage, `signedFetch`, e outras APIs do runtime contam todas para o mesmo limite. Calls excessivas são **enfileiradas**. São rejeitadas imediatamente com um `demasiados host calls concorrentes` erro. O SDK capta a rejeição e resolve o `Storage.set` promise para `false` em vez de lançar uma exceção. Se o teu código descartar esse boolean, o save falhado fica invisível e os teus dados persistidos acabam desatualizados ou perdidos. Verifica o resultado de cada escrita (ver a Nota acima).


Bons momentos para persistir:

* Fim do jogo, ou o fim de uma ronda.
* Um jogador sai.
* Um save debounced periódico (por exemplo, uma vez a cada 30 segundos), não uma vez por frame.

Persiste apenas os dados que têm de sobreviver a um reinício ou redeploy. Tudo o resto pode viver na memória.

O exemplo abaixo mantém as pontuações na memória e escreve-as apenas num checkpoint, em vez de a cada ponto marcado:

```typescript
import { Storage } from "@dcl/sdk/server"

// O estado de trabalho vive na memória, atualizado em cada evento
const scores: Record<string, number> = {}

function onPointScored(address: string) {
  scores[address] = (scores[address] ?? 0) + 1
  // Sem chamada a Storage aqui — apenas atualiza a memória
}

// Persiste apenas num checkpoint, como quando um jogador sai
async function onPlayerLeave(address: string) {
  const saved = await Storage.player.set(address, "score", scores[address] ?? 0)
  if (!saved) {
    // A escrita não foi persistida — mantém o valor na memória e tenta novamente mais tarde
    console.error(`Failed to save score for ${address}`)
  }
}
```

Para scenes com muitos jogadores em simultâneo, uma abordagem mais robusta acompanha quais chaves têm alterações não guardadas (um "dirty set") e volta a tentar as escritas falhadas na próxima flush:

```typescript
import { engine } from "@dcl/sdk/ecs"
import { Storage } from "@dcl/sdk/server"

// Estado de trabalho na memória, atualizado em cada evento
const scores: Record<string, number> = {}
const dirty = new Set<string>()

function onPointScored(address: string) {
  scores[address] = (scores[address] ?? 0) + 1
  dirty.add(address) // Marca para flush posterior, sem chamada a Storage aqui
}

// Faz flush às chaves dirty periodicamente (por exemplo, a cada ~30s) e nos checkpoints
async function flush() {
  for (const address of dirty) {
    const ok = await Storage.player.set(address, "score", scores[address])
    if (ok) {
      dirty.delete(address) // Guardado com sucesso
    }
    // Se ok for false, a chave continua dirty e volta a tentar no próximo flush
  }
}

// Executa o flush num timer usando um acumulador dt
let flushTimer = 0
engine.addSystem((dt) => {
  flushTimer += dt
  if (flushTimer > 30) {
    flushTimer = 0
    flush()
  }
})
```

### World Storage — Partilhado por Todos os Jogadores

```typescript
import { Storage } from "@dcl/sdk/server"

type Entry = { name: string; score: number }

// Escrever — set() resolve para false se o valor NÃO foi persistido
const ok = await Storage.set("leaderboard", [
  { name: "Alice", score: 100 },
  { name: "Bob", score: 85 },
])
if (!ok) {
  console.error("Failed to save leaderboard — retry later")
}

// Ler: null quando a chave não existe
const leaderboard = (await Storage.get<Entry[]>("leaderboard")) ?? []

// Eliminar
await Storage.delete("leaderboard")
```

Também podes gerir o storage da scene pela linha de comando, usando `npx sdk-commands storage scene`:

```bash
# Definir um valor
npx sdk-commands storage scene set high_score --value 100

# Obter um valor
npx sdk-commands storage scene get high_score

# Eliminar um valor
npx sdk-commands storage scene delete high_score

# Eliminar todos os dados de storage da scene
npx sdk-commands storage scene clear --confirm
```

### Player Storage — Por endereço de wallet

```typescript
import { Storage } from "@dcl/sdk/server"

type Progress = { level: number; coins: number }

// Escrever — set() resolve para false se o valor NÃO foi persistido
const ok = await Storage.player.set(playerAddress, "progress", {
  level: 5,
  coins: 250,
})
if (!ok) {
  console.error(`Failed to save progress for ${playerAddress} — retry later`)
}

// Ler: null quando este jogador não tem progresso guardado
const progress = (await Storage.player.get<Progress>(playerAddress, "progress")) ?? { level: 1, coins: 0 }

// Eliminar
await Storage.player.delete(playerAddress, "progress")
```

Também podes gerir o player storage pela linha de comando, usando `npx sdk-commands storage player`:

```bash
# Definir um valor para um jogador específico
npx sdk-commands storage player set level --value 10 --address 0x1234...

# Obter um valor para um jogador específico
npx sdk-commands storage player get level --address 0x1234...

# Eliminar um valor para um jogador específico
npx sdk-commands storage player delete level --address 0x1234...

# Eliminar todos os dados de um jogador específico
npx sdk-commands storage player clear --address 0x1234... --confirm

# Eliminar todos os dados do player (todos os players)
npx sdk-commands storage player clear --confirm
```

Cada valor de scene pode ter até 512 KB, e todos os valores de scene de um World partilham 10 MB. Cada valor de player pode ter até 100 KB, com 1 MB por player.

### Carrega um save sem o sobrescrever

Nunca atribuas dados por defeito quando um jogador entra e depois os guardes: se os dados guardados existirem mas ainda não tiverem sido carregados, os valores por defeito substituem-nos. Lê primeiro e só cria defaults quando `get()` resolve para `null`. Se a leitura lançar uma exceção, deixa o jogador por carregar e tenta novamente mais tarde.

```typescript
import { Storage } from "@dcl/sdk/server"

type Save = { coins: number }
const saves: Record<string, Save> = {}

async function loadOrCreate(address: string): Promise<Save> {
  if (saves[address]) return saves[address] // Já carregado
  // Lança uma exceção se a leitura falhar, por isso uma falha nunca se transforma num save novo
  const stored = await Storage.player.get<Save>(address, "save")
  saves[address] = stored ?? { coins: 0 } // Defaults apenas quando não existe save
  return saves[address]
}
```

Chama-o dentro de um `try/catch` no teu join handler. Se lançar uma exceção, informa o client para que possa voltar a tentar e ignora as ações desse jogador até o save carregar.

### Progresso enquanto ninguém está na scene

Não mantenhas um timer a correr para coisas que crescem com o tempo, como crops, energia ou reposição de loja. O Multiplayer Server pára alguns minutos depois de o último jogador sair (ver [Aguarda que o server arranque](#wait-for-the-server-to-start-up)), por isso o timer também pára. Em vez disso, guarda o momento em que o progresso foi atualizado pela última vez. Quando um jogador regressar, calcula o progresso a partir do tempo que passou e avança o tempo guardado.

Usa o `Date.now()` do server para isto, nunca uma hora enviada por um client: um jogador pode adiantar o relógio para saltar a espera.

### Evita perder dados com escritas concorrentes

Storage não tem forma de atualizar uma chave apenas se ninguém a tiver alterado desde que a leste (sem "compare-and-set"). Por isso, se լeres uma chave, alterares o valor e o voltares a escrever, podes perder dados:

* Duas instâncias do server podem escrever ao mesmo tempo. Isto acontece enquanto uma versão antiga e uma nova da tua scene se sobrepõem durante um redeploy.
* Uma leitura pode devolver um valor mais antigo do que o último escrito. Storage já forneceu um valor antigo de uma chave durante uma hora ou mais, mesmo com `Storage.get(key, { fresh: true })`, e através de reinícios do server. A `fresh` opção apenas ignora a cache do próprio SDK.
* Uma listagem com `Storage.getValues()` também pode estar desatualizada: pode omitir uma chave escrita recentemente ou ainda mostrar uma que eliminaste.

Para dados que apenas crescem, como um log de resultados ou feedback de players, usa este padrão em vez de reescrever uma chave:

1. Escreve cada snapshot para uma **nova chave** que se ordena por tempo e não pode colidir, por exemplo `results-1-20261027-130512-k3fz`: um prefixo, um número de parte, a hora UTC e alguns caracteres aleatórios.
2. Para ler, lista cada chave da parte com `Storage.getValues({ prefix, offset })`, página a página até teres `pagination.total` entradas e funde-as pelo id da row.
3. Escreve as rows fundidas como uma nova chave. Lê essa chave de volta e, apenas se corresponder, elimina as chaves que fundiste. Nunca elimines uma chave que outro writer ainda possa alterar com base nas tuas próprias leituras.

Uma race deixa então uma chave extra, que a próxima escrita funde, em vez de uma row perdida. Quem ler os dados, incluindo tu quando os descarregares da [storage UI](#access-stored-data), tem de ler cada chave da parte e remover ids duplicados.

```typescript
import { Storage } from "@dcl/sdk/server"

type Row = { id: string; text: string }
const PREFIX = "results-1-"

// Cada chave que começa com o prefixo, da mais antiga para a mais recente
async function listAll(prefix: string) {
  const entries: { key: string; value: unknown }[] = []
  for (;;) {
    const page = await Storage.getValues({ prefix, offset: entries.length })
    entries.push(...page.data)
    if (page.data.length === 0 || entries.length >= page.pagination.total) break
  }
  return entries.sort((a, b) => (a.key < b.key ? -1 : 1))
}

async function appendRows(newRows: Row[]): Promise<boolean> {
  const versions = await listAll(PREFIX)
  // Funde por row id, a primeira encontrada vence
  const rows = new Map<string, Row>()
  for (const { value } of versions) for (const row of value as Row[]) if (!rows.has(row.id)) rows.set(row.id, row)
  for (const row of newRows) if (!rows.has(row.id)) rows.set(row.id, row)
  const merged = [...rows.values()]

  // Uma nova chave sempre: hora UTC mais caracteres aleatórios
  // "2026-10-27T13:05:12.123Z" torna-se "20261027-130512"
  const stamp = new Date().toISOString().slice(0, 19).replace(/[-:]/g, "").replace("T", "-")
  const key = `${PREFIX}${stamp}-${Math.random().toString(36).slice(2, 6)}`
  if (!(await Storage.set(key, merged))) return false // Não foi guardado, tenta novamente mais tarde

  // Elimina as chaves fundidas apenas depois de a nova ser lida de volta
  const check = await Storage.get<Row[]>(key, { fresh: true })
  if (JSON.stringify(check) !== JSON.stringify(merged)) return true
  for (const version of versions) {
    try {
      await Storage.delete(version.key)
    } catch {
      // Mantida no lugar: a próxima escrita volta a fundi-la
    }
  }
  return true
}
```

Um valor pode conter no máximo 512 KB, por isso começa uma nova parte (`results-2-`) antes de uma parte se aproximar desse limite. O [exemplo de Scene Feedback](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0%2C-1-scene-feedback) usa este padrão para os seus ficheiros CSV e passa para uma nova parte aos 400 KB.

### Acede aos dados armazenados

Podes ver e editar os dados armazenados em tempo real no teu server através da storage UI, introduzindo este link:

[decentraland.org/storage](https://decentraland.org/storage)

Também podes aceder a esta página através do Creator Hub. Abre a **Manage** tab, clica nos três pontos ao lado de um local onde publicaste conteúdo e seleciona **View Storage**.

Aí podes ver uma lista de todos os worlds e LAND onde podes publicar scenes.

Abre a tua scene e depois a **Scene** ou **Player** tab.

No **Scene** tab, verás uma lista de todas as variáveis armazenadas. A partir daqui podes editar ou remover qualquer uma destas variáveis ao clicar no ícone do lápis ou do lixo.

![Activate stream](/docs/_remote/76bca9abaef9d73bc207163d.webp)

No **Player** tab, verás uma lista de todos os players que têm dados armazenados no teu server. Podes procurá-los por address ou nome e depois ver todos os dados associados. Também podes editar ou remover estes dados ao clicar no ícone do lápis ou do lixo.

### Alterar a estrutura dos dados

Os dados armazenados em produção são **não são apagados quando publicas uma nova versão da tua scene**. Isto é ótimo para leaderboards, progresso do jogador e alterações persistentes do ambiente que os jogadores esperam que sobrevivam a pequenas atualizações da tua scene.

O reverso é que os dados que estão no storage foram escritos por uma versão mais antiga do teu código. Se o teu novo código esperar uma shape diferente, fazer o parse ou ler esses dados antigos pode falhar de formas subtis. Um campo que renomeaste vai faltar. Um campo que antes era uma string e agora é um objeto vai lançar uma exceção quando tentares aceder a uma propriedade nele. Um player que não faz login há meses pode carregar dados anteriores a uma estrutura que o teu código já não sabe como gerir.


> [!WARNING]
> **📔 Nota**: Alterações de schema não afetam apenas a primeira leitura após um deploy. Os dados armazenados vivem até serem sobrescritos ou eliminados, por isso um valor em formato antigo pode surgir a qualquer momento, muitas vezes de um player que regressa e de quem já te tinhas esquecido.


#### Boas práticas

* *Lê sempre de forma defensiva*. Trata tudo o que sai do storage como input não confiável, mesmo que tenhas sido tu a escrevê-lo. O tipo que passas a `get<T>()` não é verificado, por isso confirma que os campos existem e têm o tipo certo antes de os leres, e tem um default sensato pronto quando não existirem:

  ```typescript
  import { Storage } from "@dcl/sdk/server"

  const saved = await Storage.player.get<any>(playerAddress, "progress")
  const progress = {
    level: typeof saved?.level === "number" ? saved.level : 1,
    coins: typeof saved?.coins === "number" ? saved.coins : 0,
  }
  ```
* *Adiciona campos, não os renomes nem removas*. A alteração de schema mais segura é uma aditiva: introduz um novo campo com um default e deixa os campos existentes em paz. Os dados antigos simplesmente não terão o novo campo, o que o teu parsing defensivo já trata. Renomear um campo obriga todos os registos antigos a falhar.
* *Versiona os teus objetos armazenados*. Inclui um `version` campo desde o primeiro dia. Quando leres os dados, faz branch com base na version e migra as shapes mais antigas para a atual antes de as usares. Isto mantém o resto do teu código a funcionar com uma única shape atual:

  ```typescript
  type ProgressV2 = { version: 2; level: number; coins: number; xp: number }

  function migrate(raw: any): ProgressV2 {
    const version = raw?.version ?? 1
    if (version === 1) {
      // a v1 não tinha campo xp — define-o por defeito
      return {
        version: 2,
        level: raw.level ?? 1,
        coins: raw.coins ?? 0,
        xp: 0,
      }
    }
    return raw as ProgressV2
  }
  ```
* *Escreve de volta o valor migrado*. Depois de atualizares um registo na memória, guarda-o de volta para que a próxima leitura já esteja no novo formato. Com o tempo, isto esvazia o conjunto de registos com a shape antiga sem precisares de um script de migração único.
* *Para breaking changes, usa uma nova chave*. Se a nova estrutura for realmente incompatível e migrar não compensar, escreve numa nova chave de storage (por exemplo `progress_v2`) e ignora a antiga. A chave antiga fica inofensivamente no storage e evitas qualquer caminho de leitura que tenha de a interpretar. Podes limpar as chaves antigas mais tarde através do [storage UI](https://decentraland.org/storage) ou dos `npx sdk-commands storage` commands.
* *Testa com dados reais de produção*. Antes de implantar uma mudança estrutural, puxe alguns registros reais da UI de armazenamento e execute seu novo código de parsing sobre eles. Os casos extremos que causam problemas geralmente são registros que você não sabia que existiam.
* *Deixe uma saída de emergência*. Tenha em mente que você pode editar ou excluir registros individuais da UI de armazenamento ou via `npx sdk-commands storage`. Se um único jogador acabar preso em um estado ruim após uma mudança de schema, você pode corrigir o registro dele diretamente sem fazer redeploy.

## Variáveis de Ambiente

Configure sua scene sem codificar valores diretamente no código. Variáveis de ambiente são úteis para dados sensíveis e também para flags de recurso ou parâmetros que podem ser alterados facilmente sem republicar sua scene.

Variáveis de ambiente são **apenas do server**. Proteja-as com `isServer()`. O server pode ler variáveis de ambiente, mas não alterar seus valores.

`EnvVar.get()` retorna um `Promise<string>` e se resolve para uma string vazia quando a variável não estiver definida, então sempre forneça um valor de fallback para valores ausentes:

```typescript
import { EnvVar } from "@dcl/sdk/server"
import { isServer } from "@dcl/sdk/network"

export async function main() {
  if (!isServer()) return

  const maxPlayers = parseInt((await EnvVar.get("MAX_PLAYERS")) || "4")
  const gameDuration = parseInt((await EnvVar.get("GAME_DURATION")) || "300")
  const debugMode = ((await EnvVar.get("DEBUG")) || "false") === "true"
}
```

### Dados sensíveis

Variáveis de ambiente são especialmente úteis para armazenar chaves privadas, códigos de resgate de recompensas e outros dados sensíveis que seriam arriscados expor no código compilado da scene pública.

Você pode armazenar chaves privadas no storage do server e fazer com que apenas o server as leia com `isServer()`. Dessa forma, os dados sensíveis nunca passam pela máquina do jogador.

### Desenvolvimento Local

Para usar variáveis de ambiente enquanto executa seu projeto localmente, crie um arquivo `.env` na raiz do seu projeto:

```
MAX_PLAYERS=8
GAME_DURATION=300
DEBUG=true
```

Importante: Adicione `.env` ao seu `.gitignore`, para que esses valores potencialmente sensíveis nunca sejam enviados para os content servers públicos.

### Alterar variáveis de ambiente

A forma mais fácil de alterar os valores das suas variáveis de ambiente é pela UI de armazenamento.

Você pode acessar os dados armazenados pelo storage da scene inserindo este link:

[decentraland.org/storage](https://decentraland.org/storage)

Também podes aceder a esta página através do Creator Hub. Abre a **Manage** tab, clica nos três pontos ao lado de um local onde publicaste conteúdo e seleciona **View Storage**.

Aí podes ver uma lista de todos os worlds e LAND onde podes publicar scenes.

Abre a tua scene e depois a **Environment** aba. Você deverá ver todas as variáveis de ambiente no projeto.

![Activate stream](/docs/_remote/13189fe9bb9d6d0cc3454293.webp)

Observe que você não pode ler os valores de nenhuma dessas variáveis de ambiente (isso é para proteger dados sensíveis), mas pode excluir ou sobrescrever qualquer uma delas. Basta clicar no ícone de lápis ou na lixeira.

Você também pode gerenciar variáveis de ambiente via linha de comando, usando `npx sdk-commands storage env`:

```bash
# Definir uma variável
npx sdk-commands storage env set MAX_PLAYERS --value 8

# Excluir uma variável
npx sdk-commands storage env delete OLD_VAR

# Excluir todas as variáveis de ambiente
npx sdk-commands storage env clear --confirm
```

Você também pode direcionar um ambiente específico com a flag `--target` a:

```bash
# Fazer deploy para staging
npx sdk-commands storage env set MY_KEY --value my_value --target https://storage.decentraland.zone

# Fazer deploy para um server de desenvolvimento local
npx sdk-commands storage env set MY_KEY --value my_value --target http://localhost:8000
```

As variáveis de ambiente implantadas têm precedência sobre os valores `.env` .

## Estrutura de Projeto Recomendada

Separar o código de server, client e shared mantém a base de código legível à medida que ela cresce:

```
src/
├── index.ts              # Ponto de entrada — ramificação isServer()
├── client/
│   ├── setup.ts          # Handlers de input, remetentes de mensagens
│   └── ui.tsx            # UI React ECS (lê estado sincronizado)
├── server/
│   ├── server.ts         # Game loop, handlers de mensagens, mutações de estado
│   └── gameState.ts      # Funções auxiliares para o estado do server
└── shared/
    ├── schemas.ts        # Definições de Component + validateBeforeChange
    └── messages.ts       # registerMessages() — importado por ambos os lados
```


> [!NOTE]
> **💡 Dica**: Mantenha todas as `registerMessages()` chamadas e definições personalizadas de Component em `shared/`. Tanto o server quanto o client importam de lá, garantindo que sempre concordem sobre os schemas de mensagens.


## Melhores Práticas de Performance

Cada mudança de Component envia os dados do *inteiro* Component pela rede. Isso é diferente do que o Colyseus faz, que envia apenas diffs. Ao projetar Components personalizados, tenha isso em mente. A solução ideal pode ser armazenar dados em Components separados, com base na frequência de mudança.

### ❌ Evite Components monolíticos

```typescript
import { engine, Schemas } from "@dcl/sdk/ecs"

// RUIM — alterar a pontuação também envia o array de posições
const GameState = engine.defineComponent("GameState", {
  playerAScore: Schemas.Int,
  playerBScore: Schemas.Int,
  timer: Schemas.Int,
  playerPositions: Schemas.Array(Schemas.Vector3), // payload grande
})
```

### ✅ Prefira Components atômicos

```typescript
import { engine, Schemas } from "@dcl/sdk/ecs"

// BOM — cada atualização é pequena e independente
const PlayerScore = engine.defineComponent("PlayerScore", {
  playerA: Schemas.Int,
  playerB: Schemas.Int,
})

const GameTimer = engine.defineComponent("GameTimer", {
  secondsLeft: Schemas.Int,
})
```

*Regra geral*: agrupe campos que mudam juntos e com frequência semelhante. Separe dados que mudam rápido (timers, posições) de dados que mudam lentamente (pontuações, configuração).

### Limite mensagens frequentes

Evite enviar mensagens a cada frame. Agrupe ou limite quando possível:

```typescript
import { engine } from "@dcl/sdk/ecs"
import { room } from "./shared/messages"

let lastSend = 0
engine.addSystem((dt) => {
  lastSend += dt
  if (lastSend > 0.1) {
    // a cada 100 ms
    room.send("position", transform.position)
    lastSend = 0
  }
})
```

Por exemplo, se o server controla um timer de contagem regressiva, não é necessário enviar atualizações para todos os jogadores a cada segundo. O ideal é que cada client calcule a passagem do tempo por conta própria, e que o server transmita seu estado atual a cada 30 segundos mais ou menos, para garantir consistência.

## Limites de Recursos do Server

O Multiplayer Server executa cada scene em um isolate sandboxed com limites rígidos de recursos. Ultrapassar esses limites pode descartar dados silenciosamente ou encerrar o server para todos na scene, então projete sua scene para ficar bem dentro deles.

### Memória

O isolate tem um limite de **256 MB** de memória. Se for excedido, o isolate é descartado e o server é encerrado para todos os players conectados. Mantenha o estado de trabalho enxuto e elimine dados por player quando os players saírem.

### CPU

Cada turno de execução tem um orçamento de tempo de relógio:

* **Execução síncrona**: 10 segundos por turno. Um loop sem limite que exceda isso encerra o isolate.
* **Assentamento do turno assíncrono**: 60 segundos. Se uma cadeia de `Promise` ou `await` demorar mais do que isso para se resolver, o isolate é encerrado.

Distribua o trabalho pesado por vários ticks usando um acumulador de `dt` em `engine.addSystem()`. Nunca execute loops síncronos sem limite no server.

### Taxa de mensagens de entrada

Cada peer conectado pode enviar até aproximadamente **300 mensagens por 1.000 ms**. Quadros de dados excessivos são descartados (não enfileirados). Nunca envie mensagens a cada frame do client. Veja [Melhores Práticas de Performance](#performance-best-practices) para padrões de limitação.

### Tamanho da mensagem

* Pacotes de entrada têm limite de **128 KB** por pacote. Pacotes grandes demais são descartados por completo.
* Mensagens de scene para comms têm limite de aproximadamente **30 KB**. Para a camada de transporte prática, mantenha as mensagens sincronizadas bem abaixo de **13 KB** (veja a seção [Messages](#messages) ).

### Fetch externo

Chamadas simultâneas `signedFetch` são limitadas a **32** em voo. Fetches adicionais entram em fila até que uma vaga abra. Cada tentativa de fetch tem um timeout de **15 segundos** . As respostas de fetch têm limite de **10 MB**. As conexões WebSocket são limitadas a sockets **32** simultâneos, com tamanho máximo de mensagem de **1 MB** por mensagem.

### Chamadas de host em voo

O limite de 40 chamadas descrito em [Armazenamento de Dados](#data-storage) se aplica a todas as chamadas de host no isolate inteiro, incluindo Storage, `signedFetch`, e outras APIs de runtime. Chamadas em excesso são rejeitadas imediatamente e não entram em fila.

## Armadilhas Comuns

### Esquecer validação em estado apenas do server

Sem `validateBeforeChange`, os clients podem escrever em qualquer Component:

```typescript
import { engine, Schemas } from "@dcl/sdk/ecs"
import { isServer } from "@dcl/sdk/network"
import { AUTH_SERVER_PEER_ID } from "@dcl/sdk/network/message-bus-sync"

// ❌ RUIM — clients podem trapacear
const Score = engine.defineComponent("Score", { value: Schemas.Int })

// ✅ BOM — apenas do server
if (isServer()) {
  Score.validateBeforeChange((v) => v.senderAddress === AUTH_SERVER_PEER_ID)
}
```

### Confiar em valores fornecidos pelo client

Nunca deixe um client ditar seus próprios valores para dados importantes como saúde, pontuação ou posição:

```typescript
import { room } from "./shared/messages"

// ❌ RUIM
room.onMessage("setHealth", (data) => {
  player.health = data.health // o client controla o valor!
})

// ✅ BOM — o server calcula o resultado
room.onMessage("takeDamage", (data) => {
  const damage = calculateDamage(data.source)
  player.health = Math.max(0, player.health - damage)
})
```

### Confiar em quantias e endereços nas mensagens

Uma mensagem de um client é um pedido, não um fato. Verifique cada campo antes de usá-lo:

* **Quantias**: `Schemas.Int` decodifica o valor como um inteiro de 32 bits, mas ele ainda pode ser zero ou negativo. Rejeite quantias de zero ou menos, ou um player poderia "enviar" `-500` moedas e recebê-las em vez disso. Verifique também se o remetente tem o suficiente.
* **Endereços**: um endereço de wallet que o client envia deve ser `0x` seguido por 40 caracteres hexadecimais. Rejeite qualquer outra coisa antes que isso chegue aos seus maps ou Storage.

```typescript
import { room } from "./shared/messages"

const ADDRESS = /^0x[0-9a-f]{40}$/
const balances: Record<string, number> = {}

room.onMessage("giveCoins", (data, context) => {
  if (!context) return
  const from = context.from.toLowerCase()
  const to = String(data.to).toLowerCase()
  if (!ADDRESS.test(to) || to === from) return
  if (!Number.isInteger(data.amount) || data.amount <= 0) return // Rejeita negativos e zero
  if ((balances[from] ?? 0) < data.amount) return // Moedas insuficientes
  balances[from] -= data.amount
  balances[to] = (balances[to] ?? 0) + data.amount
})
```

### Enviar mensagens antes da sincronização do estado

Os clients devem esperar até que o estado esteja sincronizado antes de interagir:

```typescript
import { engine } from "@dcl/sdk/ecs"
import { isStateSyncronized } from "@dcl/sdk/network"

engine.addSystem(() => {
  if (!isStateSyncronized()) return
  // seguro enviar mensagens
})
```

### Aguarda que o server arranque

O server fica ativo apenas enquanto houver pelo menos um player na scene. Depois que o último player sai, o server permanece ligado por cerca de dois minutos e então desliga. A próxima visita inicia a frio uma nova instância, o que leva aproximadamente **15 segundos em produção**. A visualização local inicia o server instantaneamente, então problemas de cold-start quase sempre aparecem apenas depois da publicação.

O código da sua scene deve estar preparado para esperar o server entrar no ar. Mensagens enviadas antes de o server terminar de iniciar são perdidas silenciosamente. Mostre ao player uma mensagem de "server acordando" enquanto espera e adicione lógica de retry para as solicitações iniciais ao server.


> [!NOTE]
> **Dica:** Uma forma confiável de detectar a prontidão do server é um heartbeat: faça o server escrever `Date.now()` em um campo de Component sincronizado a cada 2 segundos, e faça o client monitorar quando viu pela última vez o valor mudar. Se nenhuma mudança chegar em até 6 segundos, trate o server como offline. Isso é mais robusto do que `isStateSyncronized()`, que apenas confirma que o transporte está conectado, não que o server está em execução.


## Exemplo Completo

Um contador multiplayer mínimo: clique em um botão, o server incrementa uma pontuação sincronizada. O server persiste o contador em `Storage` para que o valor sobreviva a reinicializações do server. Lembre-se de que o server desliga quando não há players na scene, então sem storage a contagem seria redefinida para zero toda vez que a scene ficasse vazia de players.

```typescript
import { engine, Schemas } from "@dcl/sdk/ecs"
import { registerMessages, isServer, syncEntity } from "@dcl/sdk/network"
import { AUTH_SERVER_PEER_ID } from "@dcl/sdk/network/message-bus-sync"
import { pointerEventsSystem } from "@dcl/sdk/ecs"
import { Storage } from "@dcl/sdk/server"

// 1. Defina mensagens (shared)
const Messages = {
  increment: Schemas.Map({}),
  stateUpdate: Schemas.Map({
    count: Schemas.Int,
    lastPlayer: Schemas.String,
  }),
}

// 2. Defina um Component apenas do server (shared)
const Counter = engine.defineComponent("Counter", {
  value: Schemas.Int,
  lastPlayer: Schemas.String,
})

// 3. Crie a room
const room = registerMessages(Messages)

export async function main() {
  if (isServer()) {
    // === SERVER ===

    // Somente o server pode modificar esse Component
    Counter.validateBeforeChange(
      (v) => v.senderAddress === AUTH_SERVER_PEER_ID
    )

    // Restaure o contador do storage caso o server tenha reiniciado
    let initialCount = 0
    let initialPlayer = "none"
    let loaded = false
    try {
      initialCount = (await Storage.get<number>("counter")) ?? 0
      initialPlayer = (await Storage.get<string>("lastPlayer")) ?? "none"
      loaded = true
    } catch (error) {
      console.log("[Server] Não foi possível carregar o contador:", error)
    }

    const counterEntity = engine.addEntity()
    syncEntity(counterEntity, [Counter.componentId], 1)
    Counter.create(counterEntity, {
      value: initialCount,
      lastPlayer: initialPlayer,
    })

    room.onMessage("increment", async (_data, context) => {
      if (!context) return
      if (!loaded) return // Nunca salve por cima de um contador que não carregou
      const from = context.from.toLowerCase()
      const counter = Counter.getMutable(counterEntity)
      counter.value += 1
      counter.lastPlayer = from

      // Persista no storage para que o valor sobreviva a reinicializações do server
      const saved =
        (await Storage.set("counter", counter.value)) &&
        (await Storage.set("lastPlayer", from))
      if (!saved) console.log("[Server] Contador não salvo, o próximo clique o salva novamente")

      room.send("stateUpdate", {
        count: counter.value,
        lastPlayer: from,
      })
    })
  } else {
    // === CLIENT ===
    const button = engine.addEntity()
    // ... adicione Transform, MeshRenderer, etc.

    pointerEventsSystem.onPointerDown(button, () => {
      room.send("increment", {})
    })

    room.onMessage("stateUpdate", (data) => {
      console.log(`Count: ${data.count} (último clique por ${data.lastPlayer})`)
    })
  }
}
```

## Testando Localmente

A visualização padrão cuida de tudo. Ao usar a branch auth-server do SDK, o server local inicia automaticamente em segundo plano junto com a visualização do client.

Para testar interações multiplayer localmente, abra a visualização em duas janelas separadas; cada janela é tratada como um player separado. Conecte cada janela com um endereço diferente. Ambos os clients se conectarão à mesma instância local do server.

Usando o Creator Hub, ative **Multi-Instance Preview** no submenu **Desktop Client** de **Play Options**, depois clique no botão **Play** uma segunda vez para abrir uma segunda janela do explorer do Decentraland. Você deve se conectar nas duas janelas com endereços diferentes. As mesmas sessões permanecerão abertas enquanto a scene recarrega.

Como alternativa, você pode abrir uma segunda janela do explorer do Decentraland escrevendo o seguinte em uma URL do navegador:

> `decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&multi-instance=true`

### Dicas de debug

* *Prefixe seus logs* com `[SERVER]` ou `[CLIENT]` para diferenciá-los no terminal:

  ```typescript
  import { isServer } from "@dcl/sdk/network"

  if (isServer()) {
    console.log("[SERVER] Starting...")
  } else {
    console.log("[CLIENT] Starting...")
  }
  ```
* *Verifique a sincronização do Component* no client registrando contagens de entities:

  ```typescript
  import { engine } from "@dcl/sdk/ecs"

  engine.addSystem(() => {
    const entities = Array.from(engine.getEntitiesWith(MyComponent))
    console.log("[CLIENT] Synced entities:", entities.length)
  })
  ```

## Debug em Produção

Use [Scene Insights](/docs/creator/content-creator-pt/scenes-sdk7/depuracao/scene-insights/) para pesquisar logs publicados do server, acompanhá-los ao vivo e obter CPU, memória e outras métricas. Logs e métricas são retidos por **7 dias**, incluindo reinicializações e redeploys.

Atualize para o SDK mais recente, `auth-server` e então execute estes comandos no projeto da sua scene:

```bash
npx sdk-commands scene-insights login
npx sdk-commands scene-insights tail
```

Entre com uma wallet autorizada no navegador. A sessão somente leitura é reutilizada para comandos subsequentes por até 12 horas. Pressione **Ctrl+C** para parar de acompanhar os logs.

O [Guia do Scene Insights](/docs/creator/content-creator-pt/scenes-sdk7/depuracao/scene-insights/) cobre pesquisas históricas, seleção de scene, permissões de acesso, métricas, debug assistido por IA e limites de coleção.

### Ver dados de storage

Você pode acessar os dados armazenados pelo storage da scene inserindo este link:

[decentraland.org/storage](https://decentraland.org/storage)

Também podes aceder a esta página através do Creator Hub. Abre a **Manage** tab, clica nos três pontos ao lado de um local onde publicaste conteúdo e seleciona **View Storage**.

Aí podes ver uma lista de todos os worlds e LAND onde podes publicar scenes.

Abra os dados do world ou do player para ver as informações armazenadas para cada um.

Por exemplo, se um player específico tiver um problema ao jogar sua scene, você poderia localizar esse player pelo endereço e ver quais dados estão armazenados para ele para entender a situação. Talvez ele tenha encontrado um caso extremo em que acabou com dados contraditórios. Você pode até limpar ou editar os dados desse player nesta página para restaurá-lo a um estado estável.

## Controle de Versão

Cada versão publicada da sua scene recebe seu próprio hash ID exclusivo, e cada hash é pareado com sua própria instância de server. Isso significa que o código do client e o código do server sempre avançam juntos; não existe uma janela em que um client executando lógica antiga fala com um server executando lógica nova (ou vice-versa).

Quando você publica uma atualização:

* *Players já na scene* continuam vendo a versão antiga da scene até saírem e voltarem. Os clients deles permanecem conectados à instância de server correspondente ao hash antigo.
* *Novos jogadores chegando* após a atualização, carregam a nova versão da scene e se conectam à nova instância do servidor.

Isso garante que o estado de client e server nunca saia de sync por causa de uma mudança de schema ou de um Component renomeado. Uma atualização nunca pode quebrar a sessão de um jogador que já esteja na sua scene.

A contrapartida é que, por uma pequena janela logo após um deploy, os jogadores podem acabar divididos entre duas instâncias diferentes de server. Um jogador que já estava lá e um jogador que acabou de chegar podem não se ver nem conseguir interagir pela scene, mesmo estando na mesma scene, até que os jogadores mais antigos saiam e entrem novamente.


> [!NOTE]
> **💡 Dica**: Dados armazenados pelo [Storage](#data-storage) serviço (como placares, progresso do jogador ou alterações persistentes no ambiente) é *não* limpos entre versões. O armazenamento é persistido no nível da localização e compartilhado entre todas as instâncias de server que apontam para a mesma scene, então novas versões retomam exatamente de onde a anterior parou.


## Migrando do Colyseus

Se você tem uma scene existente construída em Colyseus, a tabela abaixo mapeia padrões comuns do Colyseus para os equivalentes no SDK7:

| Colyseus                             | Servidor Multijogador do SDK7                           |
| ------------------------------------ | ------------------------------------------------------- |
| `room.send(type, data)`              | `room.send(type, data)` — mesma API                     |
| `room.onMessage(type, cb)`           | `room.onMessage(type, cb)` — mesma API                  |
| `room.state.players` (schema)        | `syncEntity` + Components personalizados                |
| serialização JSON                    | serialização binária (automática via `Schemas`)         |
| Aplicação de servidor separada       | Mesma base de código — `isServer()` ramificação         |
| Hospedagem personalizada do servidor | Integrado: o Preview executa o servidor automaticamente |

Principais diferenças a ter em mente:

* *Serialização*: O Colyseus envia diffs JSON; o SDK envia o Component completo a cada alteração. Mantenha os Components pequenos (veja [Melhores Práticas de Performance](#performance-best-practices)).
* *Modelo de estado*: O Colyseus usa uma árvore de estado mutável com diffing automático. O SDK usa Components ECS sincronizados via `syncEntity` e protegidos com `validateBeforeChange`.
* *Hospedagem*: Não há implantação separada do servidor. O Multiplayer Server é implantado automaticamente junto com a scene.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/rede/authoritative-servers.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
