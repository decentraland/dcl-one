> Aplique forças e impulsos ao avatar do jogador

# Física do Player

Você pode aplicar forças físicas ao avatar do jogador a partir do código da sua Scene. Isso permite criar mecânicas de gameplay como launch pads, zonas de vento, efeitos de knockback, explosões e muito mais.

Há dois tipos de força que você pode aplicar:

* **Impulso**: Um impulso instantâneo de uso único. Use isto para efeitos discretos e súbitos, como lançar o jogador no ar ou empurrá-lo para trás.
* **Força contínua**: Um empurrão sustentado aplicado a cada tick enquanto estiver ativo. Use isto para efeitos contínuos como zonas de vento, correntes de água ou campos gravitacionais.

Ambos são aplicados através do `Physics` helper, importado de `@dcl/sdk/ecs`.


> [!WARNING]
> **📔 Nota**: Estas forças afetam apenas o avatar do jogador local. Os outros jogadores veem as alterações nas posições dos outros jogadores, mas as forças em si não são sincronizadas com os outros jogadores no multiplayer. A física de cada jogador é executada localmente na sua própria instância.


## Aplicar um impulso

Use `Physics.applyImpulseToPlayer()` para dar ao jogador um empurrão de uso único numa direção específica.

* `vetor`: Direção e força combinadas — o comprimento do vetor codifica a magnitude do impulso.

```ts
import { Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

// Lançar o jogador diretamente para cima
Physics.applyImpulseToPlayer(Vector3.create(0, 50, 0))
```

Você também pode passar uma direção e uma magnitude separadamente. Nesse caso, o vetor de direção é normalizado automaticamente:

* `direção`: Direção do empurrão — normalizada automaticamente antes da escala.
* `magnitude`: Força do impulso.

```ts
import { Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

// Lançar o jogador para cima com uma magnitude de 50
Physics.applyImpulseToPlayer(Vector3.create(0, 1, 0), 50)
```

Se você chamar `applyImpulseToPlayer()` várias vezes dentro do mesmo frame, os impulsos são acumulados — são somados e aplicados como um único impulso combinado.

### Quão forte é um impulso

O avatar tem massa 1, portanto a magnitude de um impulso é a variação da velocidade do avatar, em metros por segundo. Um impulso de `50` adiciona 50 m/s nessa direção.

* A velocidade adicionada por um impulso desvanece ao longo do tempo. No ar, desvanece a uma taxa de 1.5 por segundo, então sobra cerca de um quinto após um segundo. Desvanece mais rápido enquanto o avatar está no chão.
* A velocidade adicionada por impulsos e forças é limitada a 50 m/s.
* Um impulso ascendente primeiro cancela qualquer velocidade descendente que o avatar tenha devido à queda, então uma plataforma de lançamento funciona da mesma forma quer o jogador passe por cima dela, quer caia sobre ela de cima.

### Disparar um impulso na entrada do jogador

Um padrão comum é disparar um impulso quando o jogador entra numa área. Use uma TriggerArea para detetar quando o jogador entra:

```ts
import { engine, Physics, TriggerArea, triggerAreaEventsSystem, ColliderLayer, MeshRenderer, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const launchPad = engine.addEntity()
Transform.create(launchPad, { position: Vector3.create(8, 0, 8) })
MeshRenderer.setBox(launchPad)
TriggerArea.setBox(launchPad, ColliderLayer.CL_PLAYER)

triggerAreaEventsSystem.onTriggerEnter(launchPad, (result) => {
	if (result.trigger?.entity !== engine.PlayerEntity) return
	Physics.applyImpulseToPlayer(Vector3.create(0, 50, 0))
})
```

## Aplicar um impulso de knockback

Use `Physics.applyKnockbackToPlayer()` para empurrar o jogador para longe de um ponto com um único impulso. Isto é ideal para explosões, efeitos de impacto e outras rajadas direcionais de uso único. A direção é calculada automaticamente do ponto de origem até à posição atual do jogador.

```ts
import { Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const explosionPosition = Vector3.create(8, 1, 8)

// Afastar o jogador do centro da explosão
Physics.applyKnockbackToPlayer(explosionPosition, 40)
```

Você pode limitar a área de efeito com um `raio`, e controlar como a magnitude diminui com a distância usando a `KnockbackFalloff` opção:

* `fromPosition`: Origem do knockback no espaço mundial (centro da explosão, posição do inimigo, etc.).
* `magnitude`: Força base do impulso.
* `raio`: Distância máxima do efeito (padrão: `Infinity`).
* `falloff`: Como a magnitude diminui com a distância (padrão: `CONSTANT`).

```ts
import { Physics, KnockbackFalloff } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

Physics.applyKnockbackToPlayer(
	Vector3.create(8, 1, 8),
	40,                          // magnitude
	10,                          // raio: sem efeito para além de 10 metros
	KnockbackFalloff.LINEAR      // a magnitude desvanece até 0 na borda do raio
)
```

O `KnockbackFalloff` enum controla como a força diminui com a distância:

* `KnockbackFalloff.CONSTANT` — mesma magnitude a qualquer distância dentro do raio (padrão)
* `KnockbackFalloff.LINEAR` — diminuição linear suave até 0 na borda do raio
* `KnockbackFalloff.INVERSE_SQUARE` — queda acentuada fisicamente realista

Se o jogador estiver exatamente na posição de origem, é empurrado diretamente para cima. Uma magnitude negativa puxa o jogador em direção ao ponto em vez de o empurrar para longe.

Os mesmos `KnockbackFalloff` valores estão disponíveis para `applyRepulsionForceToPlayer()`.

## Aplicar uma força contínua

Use `Physics.applyForceToPlayer()` para aplicar uma força sustentada ao jogador. Ao contrário de um impulso, esta força é aplicada a cada tick enquanto permanecer ativa.

* `source`: Uma Entity que identifica a origem desta força — use-a para atualizar ou remover a força mais tarde.
* `vetor`: Direção e força combinadas — o comprimento do vetor codifica a magnitude da força.

A posição da Entity de origem não é relevante — é usada apenas como identificador.

```ts
import { engine, Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const windZoneEntity = engine.addEntity()

// Empurrar o jogador continuamente para a direita
Physics.applyForceToPlayer(windZoneEntity, Vector3.create(10, 0, 0))
```

O vetor de força está sempre em **espaço mundial**. Se precisar de uma direção relativa a uma Entity rodada, use `Transform.localToWorldDirection()` para a converter primeiro — veja [Converter uma direção local para o espaço mundial](#convert-a-local-direction-to-world-space).

Tal como com `applyImpulseToPlayer()`, também pode passar um `direção` e `magnitude` separadamente — veja [Aplicar um impulso](#apply-an-impulse):

```ts
Physics.applyForceToPlayer(windZoneEntity, Vector3.create(0, 1, 0), 50)
```

Se você chamar `applyForceToPlayer()` novamente com a mesma Entity de origem, substitui a força anterior dessa origem. Se várias fontes de força forem acumuladas, os seus vetores são somados a cada tick.

### Remover uma força contínua

Para parar de aplicar uma força, chame `Physics.removeForceFromPlayer()` com uma referência à Entity de origem que foi usada para criar a força. Se a Entity não estiver atualmente a aplicar uma força, esta chamada é ignorada com segurança.

* `source`: A Entity usada quando a força foi aplicada.

```ts
Physics.removeForceFromPlayer(windZoneEntity)
```

### Aplicar uma força por uma duração limitada

Use `Physics.applyForceToPlayerForDuration()` para aplicar uma força durante um período de tempo específico. A duração é em segundos. A força é removida automaticamente quando o tempo expira. Chamar isto novamente com a mesma Entity de origem reinicia o temporizador.

* `source`: Uma Entity que identifica esta origem de força.
* `duração`: Quanto tempo a força dura, em segundos.
* `vetor`: Direção e força combinadas — o comprimento do vetor codifica a magnitude da força.

```ts
import { engine, Physics } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const gustEntity = engine.addEntity()

// Aplicar uma força ascendente forte durante 1.5 segundos
Physics.applyForceToPlayerForDuration(gustEntity, 1.5, Vector3.create(0, 50, 0))
```

O `direção` + `magnitude` A sobrecarga também está disponível — veja [Aplicar um impulso](#apply-an-impulse):

```ts
Physics.applyForceToPlayerForDuration(gustEntity, 1.5, Vector3.create(0, 1, 0), 50)
```

### Exemplo de zona de vento

Este exemplo cria um túnel de vento que empurra o jogador enquanto ele estiver dentro dele e para quando ele sair:

```ts
import { engine, Physics, TriggerArea, triggerAreaEventsSystem, ColliderLayer, MeshRenderer, MeshCollider, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const windTunnel = engine.addEntity()
Transform.create(windTunnel, {
	position: Vector3.create(8, 1, 8),
	scale: Vector3.create(4, 3, 4),
})
MeshRenderer.setBox(windTunnel)
TriggerArea.setBox(windTunnel, ColliderLayer.CL_PLAYER)

triggerAreaEventsSystem.onTriggerEnter(windTunnel, (result) => {
	if (result.trigger?.entity !== engine.PlayerEntity) return
	Physics.applyForceToPlayer(windTunnel, Vector3.create(15, 0, 0))  // empurrar lateralmente ao longo do eixo X
})

triggerAreaEventsSystem.onTriggerExit(windTunnel, (result) => {
	if (result.trigger?.entity !== engine.PlayerEntity) return
	Physics.removeForceFromPlayer(windTunnel)
})
```

## Aplicar uma força de repulsão

Use `Physics.applyRepulsionForceToPlayer()` para empurrar continuamente o jogador para longe de um ponto fixo no espaço. Isto é adequado para efeitos como um campo de repulsão magnética, uma barreira de força ou um objeto flutuante que mantém os jogadores à distância. Ao contrário de `applyKnockbackToPlayer()`, esta força é recalculada a cada tick à medida que o jogador se move. A magnitude enfraquece com a distância com base no raio e no falloff — veja [Aplicar um impulso de knockback](#apply-a-knockback-impulse) para as `KnockbackFalloff` opções. Uma magnitude negativa inverte o efeito, puxando continuamente o jogador para o ponto como um vórtice ou um poço gravitacional.


> [!WARNING]
> **📔 Nota**: A origem da repulsão é o `fromPosition` vetor que você passa — **não** a posição da `source` Entity. A `source` Entity é usada apenas como identificador para que você possa atualizar ou remover a força mais tarde. A sua posição, rotação e escala são completamente ignoradas.


* `source`: Uma Entity usada como identificador para esta força — a sua posição não é usada.
* `fromPosition`: O ponto no espaço mundial de onde o jogador é empurrado para longe.
* `magnitude`: Força base da força. Valores negativos atraem em vez de repelir.
* `raio`: Distância máxima do efeito (padrão: `Infinity`).
* `falloff`: Como a magnitude diminui com a distância (padrão: `CONSTANT`).

```ts
import { engine, Physics, Transform, timers } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

const repulsionSource = engine.addEntity()
Transform.create(repulsionSource, { position: Vector3.create(8, 1, 8) })

// Passar explicitamente a posição da Entity como origem da repulsão
Physics.applyRepulsionForceToPlayer(
	repulsionSource,
	Transform.get(repulsionSource).position,  // origem da repulsão — passada explicitamente, não lida automaticamente
	50,                                         // magnitude
	10,                                         // raio de efeito em metros
)

// Remover a força após meio segundo
timers.setTimeout(() => {
    Physics.removeForceFromPlayer(repulsionSource)
}, 500)
```

## Forças durante a planagem

Enquanto o jogador está a planar, as forças contínuas comportam-se de forma diferente:

* Forças contínuas — de `applyForceToPlayer()`, `applyForceToPlayerForDuration()`, ou `applyRepulsionForceToPlayer()` — são **1.5 vezes mais fortes**, uma vez que o planador aberto apanha o fluxo de ar. As zonas de vento e as correntes parecem mais responsivas para um jogador em planagem.
* O componente ascendente de uma força contínua pode **elevar** um jogador em planagem. O limite de velocidade de queda do planador apenas limita a velocidade com que o jogador desce; não cancela o movimento ascendente, portanto uma corrente de vento inclinada ou vertical empurra o jogador ao longo de toda a direção da força.

Impulsos de uso único — de `applyImpulseToPlayer()` ou `applyKnockbackToPlayer()` — não são afetados pela planagem. Comportam-se da mesma forma quer o planador esteja aberto quer fechado.

Pode combinar estes comportamentos para criar mecânicas como correntes térmicas ascendentes que transportam jogadores em planagem para cima, ou corredores de vento que são mais fáceis de atravessar com o planador aberto. Para ajustar a velocidade de queda ou a velocidade para a frente do planador, use o `AvatarLocomotionSettings` componente, veja [Definições de Locomotion](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/locomotion/#locomotion-settings).

## Converter uma direção local para o espaço mundial

Use `Transform.localToWorldDirection()` para transformar um vetor de direção do espaço de coordenadas local de uma Entity para o espaço mundial, tendo em conta toda a hierarquia parental. Isto é útil ao aplicar forças relativas a uma Entity rodada — por exemplo, empurrar o jogador para longe de uma face específica de um obstáculo em rotação.

* `Entity`: A Entity de origem cujo espaço local define a direção.
* `localDirection`: Vetor de direção nas coordenadas locais da Entity.

Devolve o vetor de direção em coordenadas mundiais.

Isto aplica **apenas rotação** — não tem em conta translação nem escala — tornando-o diretamente adequado para vetores de direção passados para funções de força e impulso.

```ts
import { Physics, Transform } from '@dcl/sdk/ecs'
import { Vector3 } from '@dcl/sdk/math'

// Empurrar o jogador na direção para onde aponta o eixo +Z local da Entity no espaço mundial
const worldDir = Transform.localToWorldDirection(myEntity, Vector3.create(0, 0, 1))
Physics.applyImpulseToPlayer(worldDir, 20)
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/interatividade/player-physics.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
