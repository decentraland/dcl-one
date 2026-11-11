> Obtenha dados dos jogadores à medida que interagem com a sua cena.

# Dados do utilizador

## Posição e rotação do jogador

Use a `PlayerEntity` e as `CameraEntity` para saber a posição e a rotação do jogador, verificando os seus `Transform` components.

```ts
function getPlayerPosition() {
	if (!Transform.has(engine.PlayerEntity)) return
	if (!Transform.has(engine.CameraEntity)) return

	//posição do jogador
	const playerPos = Transform.get(engine.PlayerEntity).position

	//rotação do jogador
	const playerRot = Transform.get(engine.PlayerEntity).rotation

	//posição da camera
	const CameraPos = Transform.get(engine.CameraEntity).position

	//rotação da camera
	const CameraRot = Transform.get(engine.CameraEntity).rotation

	console.log('playerPos: ', playerPos)
	console.log('playerRot: ', playerRot)
	console.log('cameraPos: ', CameraPos)
	console.log('cameraRot: ', CameraRot)
}

engine.addSystem(getPlayerPosition)
```

* **Posição de PlayerEntity**: A posição do avatar, à altura do peito. Aproximadamente a 0,88 metros acima do chão.
* **Rotação de PlayerEntity**: A direção para a qual o avatar está virado, expressa como um quaternion.
* **Posição de CameraEntity**:
  * Em 1.ª pessoa: Igual à posição do avatar, mas ao nível dos olhos. Aproximadamente a 1,75 metros acima do chão.
  * Em 3.ª pessoa: Pode variar consoante os movimentos da camera.
* **Rotação de PlayerEntity**:
  * Em 1.ª pessoa: Semelhante à direção para a qual o avatar está virado, expressa como um quaternion. Pode ser arredondada ligeiramente de forma diferente da rotação do jogador.
  * Em 3.ª pessoa: Pode variar consoante os movimentos da camera.


> [!WARNING]
> **📔 Nota**: Evita referir-te à `engine.PlayerEntity` ou dos `engine.CameraEntity` no carregamento inicial da scene, porque isso pode resultar em erros se os entities ainda não tiverem sido inicializados. Para evitar este problema, usa-os dentro da `main()` função, ou numa function chamada indiretamente por `main()`. Também podes encapsular o comportamento num async [`executeTask` bloco](/docs/creator/content-creator-pt/scenes-sdk7/padroes-de-programacao/async-functions/#the-executetask-function).
> 
> Outra opção é referir estes entities dentro de um system. Aí estarão sempre disponíveis, porque a primeira execução do system é chamada quando a scene já está devidamente inicializada.


## Obter todos os players

Todos os players na scene têm um `Transform` component. Este component é apenas de leitura nos avatars. Para obter as posições de todos os players, [itera por todos os entities com](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/user-data/) um `PlayerIdentityData` componente.

```ts
import { PlayerIdentityData } from '@dcl/sdk/ecs'

for (const [entity, data, transform] of engine.getEntitiesWith(
	PlayerIdentityData,
	Transform
)) {
	console.log('Dados do player: ', { entity, data, transform })
}
```

O código acima itera por todos os entities com um `Transform` e uma `PlayerIdentityData` component e regista os seus dados. Podes usar este mesmo método para obter quaisquer dados disponíveis de todos os players.

Consulte [Ouvintes de eventos](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/event-listeners/#player-enters-or-leaves-scene) para aprender como detetar e reagir quando novos players entram na scene.

## Obter dados do player

Use `getPlayer()` para obter dados sobre o player atual, ou sobre qualquer outro player na scene.

```ts
import { getPlayer } from '@dcl/sdk/src/players'

export function main() {
	createCube(5, 1, 5)

	let myPlayer = getPlayer()

	if (myPlayer) {
		console.log('Nome : ', myPlayer.name)
		console.log('UserId : ', myPlayer.userId)
	}
}
```

`getPlayer()` devolve o seguinte:

* `name`: *(string)* O nome de utilizador do player, tal como os outros o veem no mundo
* `userId`: *(string)* Uma string que identifica o player. Para players ligados com uma wallet, este é o endereço da wallet, escrito em minúsculas. Para contas de convidado, é um identificador gerado localmente.
* `isGuest`: *(boolean)* Indica se o player tem uma public key. *Verdadeiro* se o player for uma conta de convidado sem public key.
* `posição`: *(Vector3)* A posição do avatar na scene.
* `avatar`: Um objeto aninhado com dados sobre o avatar base do player e a sua aparência.
* `wearables`: Uma array de identificadores para cada um dos wearables que o player está atualmente a usar. Por exemplo `urn:decentraland:off-chain:base-avatars:green_hoodie`. Todos os wearables têm um identificador semelhante, mesmo que sejam NFTs.
* `emotes`: Uma array de identificadores para cada um dos emotes que o player tem atualmente equipados na roda de acesso rápido.
* `entity`: Uma referência para o entity do player. Isto pode ser útil para passar para outras functions, ou para lhe adicionar components personalizados.

O `avatar` object tem a seguinte informação aninhada:

* `bodyShapeUrn`: Um identificador para a forma corporal geral do avatar. Seja `urn:decentraland:off-chain:base-avatars:BaseFemale` para feminino ou `urn:decentraland:off-chain:base-avatars:BaseMale` para masculino.
* `skinColor`: Cor da pele do jogador como um `Color3`
* `eyesColor`: Cor dos olhos do jogador como um `Color3`
* `hairColor`: Cor do cabelo do jogador como um `Color3`
* `name`: O nome do jogador.


> [!WARNING]
> **📔 Nota**: Os dados do player podem não estar disponíveis no primeiro frame da scene, dependendo dos tempos de carregamento. Deves verificar se os dados foram devolvidos e, caso contrário, tentar novamente alguns milissegundos depois.


```ts
import { getPlayer } from '@dcl/sdk/src/players'

export function main() {
	createCube(5, 1, 5)

	let myPlayer = getPlayer()

	if (myPlayer) {
		console.log('É convidado: ', myPlayer.isGuest)
		console.log('Nome : ', myPlayer.name)
		console.log('UserId : ', myPlayer.userId)
		console.log('Forma do avatar : ', myPlayer.position)
		console.log('Forma do avatar : ', myPlayer.avatar?.bodyShapeUrn)
		console.log('Cor dos olhos do avatar : ', myPlayer.avatar?.eyesColor)
		console.log('Cor do cabelo do avatar : ', myPlayer.avatar?.hairColor)
		console.log('Wearables ativos : ', myPlayer.wearables)
		console.log('Emotes disponíveis : ', myPlayer.emotes)
	}
}
```

Para obter os dados de um player específico na scene, diferente do player atual, executa `getPlayer()` com um object com um `userId` propriedade.

```ts
import { getPlayer } from '@dcl/sdk/src/players'

for (const [entity, data, transform] of engine.getEntitiesWith(
	PlayerIdentityData,
	Transform
)) {
	let player = getPlayer({ userId: data.address })
	console.log('PLAYER : ', player?.name)
}
```

O snippet acima itera por todos os entities com um `PlayerIdentityData` component, ou seja, todos os entities de avatar na scene. Depois executa o `getPlayer()` para esse entity.

`getPlayer()` só pode obter dados de players que estão atualmente na mesma scene, não precisam necessariamente de estar no alcance visual, mas devem estar ligados à mesma ilha de comms. Para experimentar isto em preview, abre um segundo separador e inicia sessão com uma conta diferente, e faz com que ambos os players fiquem dentro da scene.


> [!WARNING]
> **📔 Nota**: Os User IDs devem estar sempre em minúsculas. Se copiares um endereço de wallet, certifica-te de que todos os caracteres estão em minúsculas.


## Dados de qualquer player

Para obter informações de qualquer player, faz uma [chamada REST API](/docs/creator/content-creator-pt/scenes-sdk7/rede/network-connections/#call-a-rest-api) para os content servers.

Estas informações são expostas no seguinte URL, acrescentando o user id do player ao parâmetro de URL.

`https://peer.decentraland.org/lambdas/profile/<player user id>`


> [!NOTE]
> **💡 Dica**: Experimenta o URL num browser para veres como a resposta está estruturada.


A seguinte informação está disponível nesta API:

* `displayName`: *(string)* O nome de utilizador do player, tal como os outros o veem no mundo
* `userId`: *(string)* Uma string UUID que identifica o player. Se o player tiver uma public key, este campo terá o mesmo valor que a public key.
* `hasConnectedWeb3`: *(boolean)* Indica se o player tem uma public key. *Verdadeiro* se o player tiver uma.
* `publicKey`: *(string)* A public key da wallet Ethereum do player. Se o player iniciar sessão como convidado, sem wallet associada, este campo será `null`.
* `avatar`: Um objeto aninhado com dados sobre a aparência do player.
* `version`: *(number)* Um número de versão que aumenta em um sempre que o player altera alguma das suas definições. Usa isto se encontrares dados em conflito, para saber qual é a versão mais recente.


> [!WARNING]
> **📔 Nota**: Para quaisquer transações Ethereum com o player, usa sempre o `publicKey` campo, em vez do `userId`, para evitar lidar com wallets inexistentes.


O `avatar` object tem a seguinte informação aninhada:

* `wearables`: `WearableId[]` Uma array de identificadores para cada um dos wearables que o player está atualmente a usar. Por exemplo `urn:decentraland:off-chain:base-avatars:green_hoodie`. Todos os wearables têm um identificador semelhante, mesmo que sejam NFTs.
* `bodyShape`: Um identificador para a forma corporal geral do avatar. Seja `urn:decentraland:off-chain:base-avatars:BaseFemale` para feminino ou `urn:decentraland:off-chain:base-avatars:BaseMale` para masculino.
* `skin`: Um objecto com um `color` campo, que contém a cor da pele do player como `{ r, g, b, a }` números de 0 a 1.
* `hair`: Um objecto com um `color` campo, que contém a cor do cabelo do player como `{ r, g, b, a }` números de 0 a 1.
* `eyes`: Um objecto com um `color` campo, que contém a cor dos olhos do player como `{ r, g, b, a }` números de 0 a 1.
* `snapshots`: Um objeto aninhado com representações base64 de imagens .jpg do player em várias resoluções.
  * `face256`: *string* O rosto do player como uma imagem de 256x256 píxeis.
  * `body`: *string* A imagem em resolução total do player em posição ereta, com 512x1024 píxeis.


> [!CAUTION]
> **❗Aviso** Os snapshots do avatar serão descontinuados no futuro e deixarão de ser devolvidos como parte dos dados de um avatar. A abordagem recomendada é usar `AvatarTexture` em vez disso, vê [Retratos de Avatar](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/materials/#avatar-portraits).


Ao contrário de `getPlayer()`, esta opção não está limitada apenas aos players que estão atualmente na mesma scene, nem sequer no mesmo server. Com esta abordagem podes obter dados de qualquer player que se tenha ligado aos servers no passado.

Se souberes a que server o player que queres consultar está ligado, podes obter dados mais atualizados enviando os teus pedidos para esse server específico. Por exemplo, se o player mudar de roupa, esta informação ficará disponível instantaneamente no server do player, mas provavelmente demorará alguns minutos a propagar-se para o `peer.decentraland.org` server.

`https://<player server>/lambdas/profile/<player user id>`


> [!NOTE]
> **💡 Dica**: Podes obter o server do player atual chamando `getRealm()` e lendo o campo `realmInfo.baseUrl` da resposta.


Este exemplo combina `myProfile.userId` e `getRealm()` para obter os dados do player diretamente do server em que o player está:

```ts
import { getRealm } from '~system/Runtime'
import { myProfile } from '@dcl/sdk/network'

async function fetchPlayerData() {
	const { realmInfo } = await getRealm({})
	if (!realmInfo) return

	const url = `${realmInfo.baseUrl}/lambdas/profile/${myProfile.userId}`
	console.log('a usar o URL: ', url)

	try {
		const response = await fetch(url)
		const json = await response.json()

		console.log('resposta completa: ', json)
		console.log('o player está a usar :', json.avatars[0].avatar.wearables)
	} catch {
		console.log('ocorreu um erro ao tentar obter os dados do player')
	}
}

fetchPlayerData()
```

## Componentes de dados do player

Em vez de usar `getPlayer()`, podes ler os dados diretamente de uma série de components que guardam os dados em cada entity de player. Os seguintes components existem:

* `PlayerIdentityData`: Armazena o endereço do player e uma `isGuest` propriedade para assinalar contas de convidado.
* `AvatarBase`: Armazena dados sobre o avatar base, incluindo:
  * `name`: O nome do jogador.
  * `bodyShapeUrn`: Os ids correspondentes ao tipo de corpo masculino ou feminino.
  * `skinColor`: Cor da pele do jogador como um `Color3`
  * `eyesColor`: Cor dos olhos do jogador como um `Color3`
  * `hairColor`: Cor do cabelo do jogador como um `Color3`
  * `talking`: *true* enquanto o jogador estiver falando no chat de voz, *false* quando estiver em silêncio, *undefined* quando o client não reporta atividade de voz para ele. Vê [Detectar quando um jogador está falando](#detect-when-a-player-is-speaking).
* `AvatarEquippedData`: A lista de wearables e emotes equipados.
  * `wearableUrns`: A lista de wearables que o jogador atualmente tem equipados.
  * `emoteUrns`: A lista de emotes que o jogador atualmente tem equipados na roda de acesso rápido.
* `AvatarEmoteCommand`: Informação sobre os emotes que o player está atualmente a reproduzir. Inclui:
  * `emoteUrn`: O URN do último emote reproduzido pelo player, desde que entrou na scene
  * `loop`: Verdadeiro se o emote estiver em loop
  * `timestamp`: A hora em que este emote foi desencadeado

```ts
for (const [entity, data, base, attach, transform] of engine.getEntitiesWith(
	PlayerIdentityData,
	AvatarBase,
	AvatarEquippedData,
	Transform
)) {
	console.log('DADOS DO PLAYER: ', { entity, data, transform, base, attach })
}
```


> [!WARNING]
> **📔 Nota**: Todos estes components são apenas de leitura. Não podes alterar os seus valores a partir da scene.


## Detectar quando um jogador está falando

Ler `talking` no `AvatarBase` component para saber se alguém está a falar em voice chat neste momento. Funciona para o player que está a executar a scene e para todos os outros players próximos.

```ts
import { AvatarBase, engine } from '@dcl/sdk/ecs'

// regista sempre que o player local começa ou deixa de falar
AvatarBase.onChange(engine.PlayerEntity, (base) => {
	console.log('a falar: ', base?.talking)
})
```

`talking` tem três estados, não dois:

| Valor       | Significado                                                                                                             |
| ----------- | ----------------------------------------------------------------------------------------------------------------------- |
| `true`      | O player está a falar neste momento.                                                                                    |
| `false`     | O player não está a falar.                                                                                              |
| `undefined` | O client não está a reportar atividade de voz para este player, por exemplo porque o voice chat próximo está desligado. |

Esse terceiro estado é importante: `undefined` significa "sem dados", não "silencioso". Verifica-o explicitamente em vez de tratares qualquer valor não-`true` como silencioso.

```ts
import { AvatarBase, engine, PlayerIdentityData } from '@dcl/sdk/ecs'

function whoIsSpeaking() {
	for (const [player, base] of engine.getEntitiesWith(AvatarBase)) {
		if (base.talking === undefined) continue // sem dados de voz para este player
		if (base.talking) console.log(`${base.name} está a falar`)
	}
}

engine.addSystem(whoIsSpeaking)
```

Para colocar um marcador sobre a cabeça de um player que está a falar, anexa-o com [`AvatarAttach`](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/entity-positioning/#attach-to-other-players) usando o endereço de `PlayerIdentityData`. Não faças parent da tua entity à entity do player: isso não funciona, e o marcador acaba na origem da scene.


> [!WARNING]
> **📔 Nota**: `AvatarBase.talking` reporta atividade real de voice chat e não podes escrever-lhe. É uma coisa diferente de `AvatarShape.talking`, que é um flag que a tua scene define para desenhar barras verdes de voz ao lado de um NPC avatar [NPC avatar](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/avatares/npc-avatars/) que criaste.


## Obter experiências portáteis

As experiências portáteis são essencialmente scenes que não estão limitadas a parcels de land. Os players podem levá-las consigo para onde quer que vão em Decentraland, adicionando uma nova camada de conteúdo sobre o mundo. Smart Wearables são exemplos de experiências portáteis. Podes querer saber se um player está a usar uma destas, uma vez que um smart wearable pode permitir aos players ter habilidades que poderiam ser consideradas batota num jogo competitivo. Por exemplo, num jogo de plataformas, um player que use um jetpack tem uma vantagem muito injusta sobre os outros.

Como criador de scene, podes querer limitar o que os players que usam experiências portáteis podem fazer na tua scene. Usa `getPortableExperiencesLoaded()` para verificar se o player tem atualmente alguma experiência portátil ativada.

```ts
import { getPortableExperiencesLoaded } from '~system/PortableExperiences'

executeTask(async () => {
	let portableExperiences = await getPortableExperiencesLoaded({})
	console.log(portableExperiences.loaded)
})
```

`getPortableExperiencesLoaded()` devolve uma array de objects, cada um destes objects inclui um atributo `id` . No caso dos wearables, o id é o URN do wearable.

## Obter informação detalhada sobre os wearables de um player

O `getPlayer()` A function devolve apenas uma lista de wearable ids, sem informação sobre cada wearable. Talvez queiras verificar qualquer wearable de uma categoria específica (ex.: hats), ou qualquer wearable de uma raridade específica (ex.: Mythic); para isso, vais precisar de obter informação mais detalhada sobre os wearables do player.

Faz uma [chamada REST API](/docs/creator/content-creator-pt/scenes-sdk7/rede/network-connections/#call-a-rest-api) para o seguinte URL, para obter uma lista completa e atualizada de todos os wearables que podem ser usados atualmente, com detalhes sobre cada um.

`${playerRealm.realmInfo.baseUrl}/lambdas/collections/wearables-by-owner/${userData.userId}?includeDefinitions`


> [!WARNING]
> **📔 Nota**: Para construir este URL, tens de obter o realm (provavelmente com `getRealm()`) e o id do player (provavelmente com `getPlayer()`)


Esta funcionalidade pode ser usada em conjunto com a obtenção de informação sobre o player, para por exemplo só permitir que os players entrem num local se estiverem a usar qualquer wearable da coleção de halloween, ou qualquer wearable que tenha raridade *legendary* .


> [!NOTE]
> **💡 Dica**: Experimenta o URL num browser para veres como a resposta está estruturada.


```ts
import { getPlayer } from '@dcl/sdk/src/players'
import { getRealm } from '~system/Runtime'

async function fetchWearablesData() {
	try {
		let userData = getPlayer()
		const realm = await getRealm({})
		if (!userData || !realm.realmInfo) return

		const url =
			`${realm.realmInfo?.baseUrl}/lambdas/collections/wearables-by-owner/${userData.userId}?includeDefinitions`.toString()
		console.log('a usar o URL: ', url)

		let response = await fetch(url)
		let json = await response.json()

		console.log('resposta completa: ', json)
	} catch {
		console.log('ocorreu um erro ao tentar obter os dados dos wearables')
	}
}

executeTask(fetchWearablesData)
```


> [!NOTE]
> **💡 Dica**: Podes obter ainda mais informação sobre wearables específicos a partir da [API seguinte](https://decentraland.github.io/catalyst-api-specs/#tag/Lambdas/operation/searchWearables).


## Verificar o modo da camera do player

Os players podem estar a usar uma camera em 1.ª ou 3.ª pessoa ao explorar Decentraland. Verifica qual destas o player está a usar verificando o valor `CameraMode` component do `engine.CameraEntity` Entity.

```ts
function checkCameraMode() {
	if (!CameraMode.has(engine.CameraEntity)) return

	let cameraEntity = CameraMode.get(engine.CameraEntity)

	if (cameraEntity.mode == CameraType.CT_THIRD_PERSON) {
		console.log('O jogador está a usar a câmara em 3.ª pessoa')
	} else {
		console.log('O jogador está a usar a câmara em 1.ª pessoa')
	}
}

engine.addSystem(checkCameraMode)
```


> [!WARNING]
> **📔 Nota**: A informação da camera só está disponível para o player atual que está a executar a scene. Não podes consultar os dados da camera de nenhum outro player.


O modo da camera usa um valor do `CameraType` enum. Os seguintes valores são possíveis:

* `CameraType.CT_FIRST_PERSON`
* `CameraType.CT_THIRD_PERSON`

O `CameraMode` component do `engine.CameraEntity` é apenas de leitura, não podes forçar o player a mudar de modo de camera através disto.


> [!NOTE]
> **💡 Dica**: Para alterar o modo de camera do player, usa uma [área modificadora de camera](/docs/creator/content-creator-pt/scenes-sdk7/fundamentos-de-conteudo-3d/camera/#1st-and-3rd-person-camera-modes).


Saber o modo da camera pode ser muito útil para afinar a mecânica da tua scene para se ajustar melhor ao que é mais confortável ao usar este modo. Por exemplo, alvos pequenos são mais difíceis de clicar em 3.ª pessoa.


> [!WARNING]
> **📔 Nota**: Evita referir-te à `engine.CameraEntity` no carregamento inicial da scene, porque isso pode resultar em erros se os entities ainda não tiverem sido inicializados. Para evitar este problema, usa-os dentro da `main()` função, ou numa function chamada indiretamente por `main()`. Também podes encapsular o comportamento num async [`executeTask` bloco](/docs/creator/content-creator-pt/scenes-sdk7/padroes-de-programacao/async-functions/#the-executetask-function).
> 
> Outra opção é referir este entity dentro de um system. Ele estará sempre disponível, porque a primeira execução do system é chamada quando a scene já está devidamente inicializada.


## Verificar se o player tem o cursor bloqueado

Os jogadores podem alternar entre dois modos de cursor: *cursor travado* modo para controlar a câmera ou *cursor destravado* modo para mover o cursor livremente pela UI.

Os jogadores destravam o cursor clicando no *botão direito do mouse* ou pressionando a *Esc* tecla, e travam o cursor novamente clicando em qualquer lugar da tela.

Verifica o `PointerLock` component da [camera entity](/docs/creator/content-creator-pt/scenes-sdk7/arquitetura/entities-components/#reserved-entities) da scene para descobrir qual é o modo atual do cursor.

```ts
export function main() {
	const isLocked = PointerLock.get(engine.CameraEntity).isPointerLocked
	console.log(isLocked)
}
```

Consulte [Ouvintes de eventos](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/event-listeners/#player-locks-or-unlocks-cursor) para veres como reagir facilmente a alterações no estado do cursor.

Também podes forçar o estado do cursor do player escrevendo no `isPointerLocked` campo do `PointerLock` componente na `engine.CameraEntity`. Veja [Bloquear ou desbloquear o cursor](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#lock-or-unlock-the-cursor) para exemplos.


> [!WARNING]
> **📔 Nota**: Evita referir-te à `engine.CameraEntity` no carregamento inicial da scene, porque isso pode resultar em erros se os entities ainda não tiverem sido inicializados. Para evitar este problema, usa-os dentro da `main()` função, ou numa function chamada indiretamente por `main()`. Também podes encapsular o comportamento num async [`executeTask` bloco](/docs/creator/content-creator-pt/scenes-sdk7/padroes-de-programacao/async-functions/#the-executetask-function).
> 
> Outra opção é referir o entity dentro de um system. Ele estará sempre disponível, porque a primeira execução do system é chamada quando a scene já está devidamente inicializada.


## Verificar a posição do cursor do player

Use a `PrimaryPointerInfo` component no `engine.RootEntity` para obter a posição do cursor do player. Isto pode ser usado para mecânicas como interações de arrastar e largar, gestos de swipe, etc.

```ts
import { PrimaryPointerInfo } from '@dcl/sdk/ecs'

function CursorSystem() {
	const pointerInfo = PrimaryPointerInfo.get(engine.RootEntity)
	console.log(pointerInfo)
}

engine.addSystem(CursorSystem)
```


> [!WARNING]
> **📔 Nota**: Evita referir-te à `engine.RootEntity` no carregamento inicial da scene, porque isso pode resultar em erros se os entities ainda não tiverem sido inicializados. Para evitar este problema, refere sempre o entity dentro de um system. Ele estará sempre disponível, porque a primeira execução do system é chamada quando a scene já está devidamente inicializada.


O `PrimaryPointerInfo` component devolve um object com as seguintes propriedades:

* `screenCoordinates`: *(Vector2)* A posição do cursor na scene, expressa em píxeis. A origem é o canto superior esquerdo do ecrã, com `y` a crescer para baixo — o mesmo espaço de coordenadas de `UiTransform` e `UiCanvasInformation`. Quando o cursor está bloqueado, isto reporta o centro do ecrã.
* `screenDelta`: *(Vector2)* A alteração na posição do cursor desde o último frame, expressa em píxeis. Vê [Movimento do rato](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/mouse-movement/) para detalhes e exemplos.
* `worldRayDirection`: *(Vector3)* Um vector que representa a direção do raio da camera para o cursor. A origem é a posição da camera. Usa isto para calcular a posição do cursor no mundo.
* `pointerType`: 0 para `nenhum`, 1 para `mouse`


> [!NOTE]
> **💡 Dica**: Ao contrário das outras propriedades, `screenDelta` continua a reportar o movimento do mouse mesmo enquanto o cursor está [bloqueado](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/eventos-de-botao/click-events/#lock-or-unlock-the-cursor). Isto torna-o ideal para interações em tempo real como gestos de arrastar e controlos de camera personalizados, vê [Movimento do rato](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/mouse-movement/).



> [!NOTE]
> **Dica:** Para reagir a eventos simples de hover em elementos de UI, talvez seja mais fácil usar o `onMouseEnter` e `onMouseLeave` eventos, veja [Eventos de Botão de UI](/docs/creator/content-creator-pt/scenes-sdk7/ui-2d/ui_button_events/#hover-feedback).


O `PrimaryPointerInfo` componente é somente leitura, você não pode forçar o jogador a mudar a posição do cursor.

O exemplo a seguir mostra como exibir a posição do cursor em um elemento de UI.

***File ui.tsx:***

```tsx
import { UiEntity, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'
import {cursorXpos, cursorYpos} from './index'

export const uiMenu = () => (
  <UiEntity
    uiTransform={{
			width: '100%',
			height: '100px',
			justifyContent: 'center',
			alignItems: 'center',
    }}
    uiText={{ value: `Posição do cursor: `+  cursorXpos + `,` + cursorYpos, fontSize: 40 }}
    uiBackground={{ color: Color4.create(0.5, 0.8, 0.1, 0.6) }}
  />
)
```

***File index.ts:***

```ts
import { engine, PrimaryPointerInfo } from '@dcl/sdk/ecs'
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
  ReactEcsRenderer.setUiRenderer(uiMenu)
}

export let cursorXpos: number | undefined = undefined
export let cursorYpos: number | undefined = undefined

function CursorSystem() {
  const pointerInfo = PrimaryPointerInfo.get(engine.RootEntity)
  console.log(pointerInfo)

  cursorXpos = pointerInfo.screenCoordinates?.x
  cursorYpos = pointerInfo.screenCoordinates?.y
}

engine.addSystem(CursorSystem)
```

Você pode usar o `worldRayDirection` para definir o `direction` campo de um raycast para saber se uma Entity está na linha de visão do cursor. Veja [Raycasting](/docs/creator/content-creator-pt/scenes-sdk7/interatividade/raycasting/) para mais detalhes.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-pt/scenes-sdk7/interatividade/user-data.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
