> 헤드리스 멀티플레이어 서버를 사용해 멀티플레이어 Decentraland 씬을 구축합니다.

# 멀티플레이어 서버

**📔 참고**: 멀티플레이어 서버는 이전에 **권한 있는 서버**. 이름만 변경되었고, 기능은 동일합니다. 설치할 SDK 브랜치 이름은 여전히 `auth-server`, 다음을 참조하세요 [설정](#setup).

## 개요

Decentraland는 플레이어의 기기에서 장면을 로컬로 실행합니다. 기본적으로 플레이어들은 서로를 보고 직접 상호작용할 수 있지만, 각자는 환경과 독립적으로 상호작용합니다. 기본적으로 환경의 변경 사항은 플레이어 간에 공유되지 않습니다.

모든 플레이어가 같은 상태의 같은 콘텐츠를 가진 장면을 볼 수 있게 하는 것은 플레이어들이 더 의미 있는 방식으로 상호작용하기 위해 매우 중요합니다. 이 기능이 없으면 플레이어가 문을 열고 집 안으로 들어가도 다른 플레이어에게는 그 문이 여전히 닫혀 있는 것으로 보이며, 첫 번째 플레이어는 다른 플레이어에게 닫힌 문을 그대로 통과해 걷는 것처럼 보이게 됩니다.

다음 **멀티플레이어 서버** 는 장면 코드를 실행하고 상태 변경을 검증한 뒤 그 결과를 연결된 모든 플레이어에게 브로드캐스트하는 헤드리스 서버 프로세스입니다. 이것은 권한 있는 서버 아키텍처를 따릅니다. 각 클라이언트가 자신의 행동을 보고하도록 신뢰하는 대신, 서버가 단일한 진실의 원천 역할을 합니다. 따라서 멀티플레이어 장면 동기화에 권장되는 방식입니다.

멀티플레이어 서버는 공정성이 게임 메커니즘에서 중요할 때 이상적입니다. 서버 측에서 실행되는 정교한 치트 방지 검증을 구현할 수 있기 때문입니다. 또한 비밀 키나 기타 민감한 정보를 서버에 저장할 수 있어, 이를 사용자에게 직접 노출할 필요가 없습니다.

멀티플레이어 서버를 사용하면 또 다른 실제 문제도 해결됩니다. 피어 투 피어 환경에서는 떠 있는 플랫폼 같은 것을 두 플레이어가 동시에 조작할 때 서로 충돌하는 결과가 발생할 수 있습니다. 각 클라이언트가 플랫폼 높이를 서로 다르게 설정하면 어느 쪽이 올바른지 결정할 권한이 없습니다. 멀티플레이어 서버는 모든 변경 사항을 한곳에서 해결하므로 모든 클라이언트가 동일한 상태로 수렴합니다.

또한 다음을 위한 장소도 제공합니다 **세션 간 데이터 유지**: 순위표, 플레이어 진행도, 잠금 해제된 업적, 또는 열린 문이나 배치된 아이템 같은 환경 변경 사항. 플레이어가 돌아오면 세계는 이전에 일어난 일을 반영합니다.

Decentraland가 서버를 호스팅하고 배포해 줍니다. 일반적인 절차로 장면을 게시하면 별도의 단계나 호스팅 비용 없이 서버도 함께 매끄럽게 게시됩니다.

## 설정

### 1. auth-server SDK 버전 설치

기본 멀티플레이어 서버 API(`isServer`, `registerMessages`, `Storage`, `EnvVar`등)는 별도의 SDK 브랜치에서 사용할 수 있습니다. 표준 SDK 브랜치 대신 다음 명령을 실행하여 프로젝트에 설치하세요:

```bash
npm install @dcl/sdk@auth-server
npm install @dcl/js-runtime@auth-server
```

### 2. scene.json 구성

멀티플레이어 서버는 다음 플래그로 활성화됩니다 `"authoritativeMultiplayer": true` 다음에서 `scene.json`, 루트 수준에서. 수동으로 추가할 필요는 없습니다. SDK의 auth-server 브랜치를 사용할 때는 장면을 처음 빌드하거나 미리 볼 때 자동으로 추가됩니다. 제거하지 않도록 주의하세요. 이 플래그가 없으면 서버는 절대 실행되지 않고 `isServer()` 는 항상 다음을 반환합니다 *false*.

선택적으로 다음도 추가할 수 있습니다 `scene.json` 루트 수준에서:

```json
{
  "logsPermissions": ["0xYourWalletAddress"]
}
```

추가하세요 `logsPermissions` 를 추가하면 배포 후 이 지갑들이 서버 로그와 메트릭에 접근할 수 있습니다. 장면의 기본 파셀에 대해 제네시스 시티 소유자와 위임된 LAND 관리자도, 적격한 배포 협업자와 함께 접근할 수 있습니다. 권한, 로그인 및 명령은 다음을 참조하세요 [Scene Insights](/docs/creator/content-creator-ko/sdk7/debugging/scene-insights/) 를 참조하세요.

### 3. 미리 보기 실행

표준 미리 보기 명령을 사용하면 되며, 추가 단계는 필요 없습니다. SDK의 auth-server 브랜치를 사용할 때 미리 보기는 자동으로 백그라운드에서 멀티플레이어 서버의 로컬 버전을 시작합니다.

서버의 로컬 세션은 프로덕션의 세션과 연결되지 않으므로, 게시된 장면의 플레이어에게 영향을 주지 않고 자유롭게 테스트할 수 있습니다.

## 서버 / 클라이언트 분기

프로젝트의 `src` 폴더의 장면 코드는 서버와 클라이언트 양쪽에서 실행됩니다. 다음 함수를 사용해 실행 경로를 분기하세요: `isServer()` 함수를 사용하여 실행 경로를 분기하세요:

```typescript
import { isServer } from "@dcl/sdk/network"
import { initServer } from "./server/server"
import { initClient } from "./client/setup"
import { setupUi } from "./client/ui"

function main() {
  if (isServer()) {
    // 서버 전용: 게임 로직, 검증, 상태 관리
    initServer()
    return
  } else {
    // 클라이언트 전용: UI, 입력 처리, 메시지 전송
    initClient()
    setupUi()
  }
}
```

서버는 렌더링 없이 헤드리스로 장면을 실행합니다. 서버는 다음을 통해 모든 플레이어 위치, 착용 아이템 및 기타 데이터에 대한 검증된 접근 권한을 가집니다 `PlayerIdentityData` 그리고 게임 상태에 대한 유일한 권한을 가집니다.

## 동기화된 컴포넌트와 검증

### 모든 클라이언트에 엔티티 동기화

다음을 사용하세요 `syncEntity` 를 사용하여 해당 엔티티의 지정된 컴포넌트에서 발생한 변경 사항을 브로드캐스트하세요:

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

구문은 다음에서 사용되는 것과 동일합니다 [서버리스 멀티플레이어](/docs/creator/content-creator-ko/sdk7/networking/serverless-multiplayer/) 기능과 동일하여, 이 아키텍처를 사용하는 장면을 멀티플레이어 서버로 쉽게 업그레이드할 수 있습니다. 장면이 멀티플레이어 서버를 사용하면 상태 업데이트는 더 이상 모든 플레이어 간에 직접 전송되지 않고, 모든 상태 업데이트가 이제 서버를 통해 라우팅되고 검증됩니다.


> [!WARNING]
> **📔 참고**: 멀티플레이어 서버에서는 이상적인 패턴이 서버만 다음을 호출하도록 하는 것입니다 `syncEntity`. 그렇게 하면 엔티티 ID 일관성에 대해 걱정할 필요가 없습니다. 대신 엔티티는 서버에 의해 인스턴스화되고 공유되며, 모든 클라이언트는 해당 인스턴스에 대해 동기화됩니다. 항상 다음으로 보호하세요 `isServer()`. 이는 다음과 다릅니다 [서버리스 멀티플레이어](/docs/creator/content-creator-ko/sdk7/networking/serverless-multiplayer/), 여기서는 모든 클라이언트가 스스로 다음을 호출합니다 `syncEntity` 을(를) 직접


### 변경 사항 검증

다음을 사용하세요 `validateBeforeChange()` 를 사용하여 엔티티의 특정 컴포넌트에서 상태 업데이트를 제한할 수 있습니다. 이를 통해 사용자 정의 검증 함수를 실행할 수 있으며, 검증 테스트를 통과한 경우에만 변경이 성공합니다.

검증이 값 *true*을 반환하면 변경이 수락되어 모든 플레이어에게 전파됩니다. 검증이 값 *false*을 반환하면 변경이 거부됩니다. 거부된 변경은 다른 플레이어에게 전달되지 않으며 변경을 시도한 플레이어에게는 되돌려집니다.

#### 값 검증

가장 간단한 경우는 새로 *값* 이 쓰여지는 값이 특정 매개변수 내에 있는지 검증하는 것입니다. 예를 들어, 다음에 대한 변경만 허용할 수 있습니다 `Transform` 새 Y 위치가 0보다 클 때:

```typescript
import { engine, Transform } from "@dcl/sdk/ecs"
import { Vector3 } from "@dcl/sdk/math"
import { isServer } from "@dcl/sdk/network"

const entity = engine.addEntity()
Transform.create(entity, { position: Vector3.create(10, 2, 10) })

if (isServer()) {
  Transform.validateBeforeChange(entity, (value) => {
    // 엔티티가 Y = 0 이하에 위치하게 되는 업데이트는 거부
    return value.newValue.position.y > 0
  })
}
```

왜냐하면 `validateBeforeChange()` 는 서버에서만 의미가 있으므로 항상 다음으로 감싸세요 `isServer()`. 클라이언트에서는 이 호출이 아무런 유용한 작업도 하지 않습니다.

이를 사용해 게임 로직에 어긋나는 변경을 차단할 수 있으며, 이는 치트 방지 메커니즘으로도 활용됩니다.

#### 플레이어와의 근접성 검증

다음을 결합할 수 있습니다 `validateBeforeChange()` 서버가 검증한 플레이어 위치와 함께 사용하여, 플레이어가 객체와 충분히 가까울 때만 상호작용을 허용하도록 확인할 수 있습니다. 예를 들어 플레이어가 객체의 `Transform`를 변경하여 아이템을 집으려 할 때, 객체가 플레이어로부터 5미터보다 멀리 있으면 변경을 거부할 수 있습니다:

```typescript
import { engine, Transform, PlayerIdentityData } from "@dcl/sdk/ecs"
import { Vector3 } from "@dcl/sdk/math"
import { isServer } from "@dcl/sdk/network"

const pickableEntity = engine.addEntity()
Transform.create(pickableEntity, { position: Vector3.create(8, 1, 8) })

if (isServer()) {
  Transform.validateBeforeChange(pickableEntity, (value) => {
    // 이 변경을 보낸 플레이어 찾기
    for (const [playerEntity, identity] of engine.getEntitiesWith(
      PlayerIdentityData
    )) {
      if (identity.address.toLowerCase() !== value.senderAddress.toLowerCase())
        continue

      const playerTransform = Transform.getOrNull(playerEntity)
      if (!playerTransform) return false

      // 변경 전 객체의 현재 위치 가져오기
      const objectTransform = Transform.getOrNull(pickableEntity)
      if (!objectTransform) return false

      const distance = Vector3.distance(
        playerTransform.position,
        objectTransform.position
      )

      // 플레이어가 5미터 이내일 때만 변경 허용
      return distance <= 5
    }

    // 연결된 플레이어 목록에서 보낸 사람을 찾지 못함 — 거부
    return false
  })
}
```

이 패턴은 치트 방지 메커니즘으로 유용합니다. 플레이어가 장면 전체를 가로질러 상호작용할 수 없는 객체를 가져가려는 것을 막아줍니다.

#### 관리자만 변경 허용

다음을 기준으로도 검증할 수 있습니다 *누가* 변경을 보내는지. 들어오는 모든 값에는 `senderAddress` 필드가 있으며, 보낸 사람의 지갑 주소가 들어 있습니다. 이를 사용하여 특정 플레이어의 변경만 허용할 수 있습니다. 예를 들어, 장면 관리자만 다음을 수정하도록 허용하려면 `VideoPlayer` 컴포넌트:

```typescript
import { engine, VideoPlayer } from "@dcl/sdk/ecs"
import { isServer } from "@dcl/sdk/network"
import { isPreview } from "@dcl/asset-packs/dist/admin-toolkit-ui/fetch-utils"
import { getSceneAdmins } from "@dcl/asset-packs/dist/admin-toolkit-ui/ModerationControl/api"

// 장면 관리자 목록에서 새로고침되는 관리자 지갑 주소 캐시
let adminAddresses = new Set<string>()

async function updateAdminAddresses() {
  if (isPreview()) return
  try {
    const [error, response] = await getSceneAdmins()
    if (error) {
      console.error("[SERVER] 관리자 목록을 가져오는 중 오류 발생:", error)
      adminAddresses = new Set()
      return
    }
    adminAddresses = new Set(
      (response ?? []).map((admin) => admin.admin.toLowerCase())
    )
    console.log(
      "[SERVER] 관리자 주소 캐시가 업데이트됨:",
      Array.from(adminAddresses)
    )
  } catch (error) {
    console.error("[SERVER] 관리자 주소를 업데이트하는 중 오류 발생:", error)
    adminAddresses = new Set()
  }
}

export async function main() {
  const videoEntity = engine.addEntity()
  VideoPlayer.create(videoEntity, { src: "videos/intro.mp4" })

  if (isServer()) {
    // 검증을 연결하기 전에 캐시 채우기
    await updateAdminAddresses()

    VideoPlayer.validateBeforeChange(videoEntity, (value) => {
      // 미리 보기에서 실행할 때는 항상 변경을 허용하여 로컬 테스트를 쉽게 함
      if (isPreview()) return true

      const senderAddress = value.senderAddress.toLowerCase()
      if (!adminAddresses.has(senderAddress)) {
        console.log(
          "[SERVER] 권한 없는 VideoPlayer 변경이 차단됨:",
          senderAddress
        )
        return false
      }
      return true
    })
  }
}
```

다음을 참조하세요 [장면 관리자](/docs/creator/content-creator-ko/scene-editor/operate-live/scene-admin/) 플레이어가 장면에서 관리자가 되는 방식에 대한 자세한 맥락을 보려면.

#### 의도만 보내고, 결과는 보내지 마세요

점수, 보상 또는 공유 자원이 걸려 있을 때는 클라이언트 메시지를 플레이어가 무엇을 **하려는지**에 맞춰 설계하고, 결과 중심으로 설계하지 마세요. 클라이언트는 `claimPoint` 또는 `harvest`를 보냅니다. "내 점수는 이제 40이다"와 같은 메시지는 보내지 않습니다.

그런 다음 서버가 메시지를 검사합니다. 누가 보냈는지, 게임이 이를 허용하는 단계인지, 플레이어가 충분히 가까운지, 그리고 아직 가져갈 것이 남아 있는지 등을 확인합니다. 그 후에야 새 값을 계산하여 저장합니다. 서버는 게임 상태를 기록하는 유일한 주체입니다.

자신의 점수를 스스로 정할 수 있는 클라이언트는 어떤 점수든 정할 수 있습니다.

#### 서버만 변경 허용

가장 엄격한 경우는 서버 자체에서 시작된 쓰기만 허용하고 클라이언트에서 오는 모든 변경을 거부하는 것입니다. 이는 완전히 권한 있는 상태에 대한 기본 패턴입니다. 예: 점수, 게임 단계, 생성된 엔티티 등.

들어오는 모든 값에는 `senderAddress` 필드가 포함됩니다. 보낸 사람이 서버인 경우 이 필드는 상수 `AUTH_SERVER_PEER_ID`와 일치하며, 다음에서 내보내집니다 `@dcl/sdk/network/message-bus-sync`.

아래 예제는 작은 `protectServerEntity()` 도우미를 정의합니다. 이 도우미는 주어진 엔티티의 하나 이상의 컴포넌트에 이 검사를 적용합니다. 다음과 같은 여러 컴포넌트를 보호하는 편리한 방법입니다 `Transform` 및 `GltfContainer`)을 하나의 호출로 보호할 수 있습니다:

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
  // 서버가 관리하는 엔티티를 만든 후:
  const entity = engine.addEntity()
  Transform.create(entity, { position: Vector3.create(10, 5, 10) })
  GltfContainer.create(entity, { src: "assets/model.glb" })
  protectServerEntity(entity, [Transform, GltfContainer])
}
```


> [!WARNING]
> **📔 참고**: 항상 다음을 호출하세요 `protectServerEntity()` 내부에서 `isServer()` 블록을 사용하세요. 이것은 `validateBeforeChange()`를 감싸며, 이것은 서버에서만 의미가 있습니다 — 클라이언트에서 호출하면 오류가 발생합니다.


#### 사용자 정의 컴포넌트

다음에도 적용할 수 있습니다 `validateBeforeChange()` 장면에서 정의한 사용자 정의 컴포넌트에

```typescript
import { engine, Schemas } from "@dcl/sdk/ecs"
import { isServer } from "@dcl/sdk/network"
import { AUTH_SERVER_PEER_ID } from "@dcl/sdk/network/message-bus-sync"

export const GameState = engine.defineComponent("game:State", {
  phase: Schemas.String,
  score: Schemas.Int,
  timeRemaining: Schemas.Int,
})

// 이 컴포넌트는 서버만 수정할 수 있습니다
if (isServer()) {
  GameState.validateBeforeChange((value) => {
    return value.senderAddress === AUTH_SERVER_PEER_ID
  })
}
```

## 메시지

동기화된 컴포넌트는 모든 플레이어가 계속해서 보아야 하는 상태에 적합합니다. 예를 들어 위치, 점수, 게임 단계 같은 것들입니다. 하지만 모든 것이 이 모델에 맞는 것은 아닙니다. 때로는 플레이어가 서버에 "이 버튼을 클릭했다" 또는 "게임에 참가하고 싶다"라고만 알리면 되고, 서버는 "라운드가 시작됨" 또는 "여기 당신의 통계입니다" 같은 일회성 응답으로 답하면 됩니다. 이것들은 지속적인 상태가 아니라 이벤트입니다.

바로 이것이 메시지의 용도입니다. 다음을 사용하세요 `registerMessages()` 를 사용하여 클라이언트와 서버 간의 타입 지정 및 스키마 검증된 통신을 수행하세요. 메시지는 일회성 전송입니다. 클라이언트가 서버에 하나를 보내면 서버가 이를 처리하고 필요에 따라 하나를 다시 보냅니다. 메시지는 그 자체로 지속적인 상태를 직접 생성하지 않습니다.

### 메시지 정의

서버와 클라이언트가 모두 import 하는 공유 파일에 모든 메시지를 정의하세요. 이렇게 하면 양쪽이 항상 어떤 메시지가 존재하고 어떤 데이터를 담는지 일치시킬 수 있습니다. 각 메시지는 다음과 같습니다 `Schemas.Map` 로 자신의 페이로드를 설명합니다:

```typescript
import { Schemas } from "@dcl/sdk/ecs"
import { registerMessages } from "@dcl/sdk/network"

export const Messages = {
  // 클라이언트 → 서버
  playerReady: Schemas.Map({ displayName: Schemas.String }),
  playerAction: Schemas.Map({ action: Schemas.String, targetId: Schemas.Int }),
  giveCoins: Schemas.Map({ to: Schemas.String, amount: Schemas.Int }),

  // 서버 → 클라이언트
  gameStarted: Schemas.Map({ roundNumber: Schemas.Int }),
  gameEnded: Schemas.Map({ winnerId: Schemas.String }),
}

export const room = registerMessages(Messages)
```


> [!WARNING]
> **📔 참고**: 호출하세요 `registerMessages()` 한 번, 공유 모듈의 최상위 수준에서, 위 예제처럼 호출하세요. 이렇게 하면 모듈이 처음 로드될 때 서버와 클라이언트 양쪽에서 실행됩니다. 조건부로 또는 함수 내부에서 호출하지 마세요. 양쪽 모두 항상 동일한 메시지를 등록해야 합니다.


### 메시지 전송

클라이언트는 서버에만 메시지를 보낼 수 있습니다. 클라이언트 간 직접 메시징은 없습니다. 서버는 모든 클라이언트에게 브로드캐스트하거나 주소로 특정 플레이어를 지정할 수 있습니다.

```typescript
import { room } from "./shared/messages"

// 클라이언트 → 서버(브로드캐스트, 서버가 수신)
room.send("playerReady", { displayName: "Alice" })

// 서버 → 모든 클라이언트
room.send("gameStarted", { roundNumber: 1 })

// 서버 → 특정 클라이언트 1명(지갑 주소로)
room.send("gameEnded", { winnerId: "Alice" }, { to: [playerAddress] })
```


> [!WARNING]
> **📔 참고**: 메시지의 최대 크기는 약 13KB입니다. 더 큰 메시지는 오류 없이 전송 계층에서 조용히 삭제됩니다. 페이로드는 작게 유지하세요. 더 큰 데이터를 공유해야 한다면 여러 메시지로 나누거나 동기화된 컴포넌트를 사용하세요.


### 메시지 수신

```typescript
import { room } from "./shared/messages"

// 클라이언트가 서버로부터 수신
room.onMessage("gameStarted", (data) => {
  console.log(`라운드 ${data.roundNumber} 시작!`)
})

// 서버가 클라이언트로부터 수신
room.onMessage("playerReady", (data, context) => {
  if (!context) return
  const senderAddress = context.from // 검증된 지갑 주소
  console.log(`[Server] ${data.displayName} 준비 완료 (${senderAddress})`)
})
```

서버에서는 수신된 모든 메시지에 `context` 객체가 포함되며, 여기에는 보낸 사람의 검증된 지갑 주소가 들어 있습니다. 이를 사용해 어떤 플레이어가 메시지를 보냈는지 파악하세요(페이로드의 자가 보고된 신원은 절대 신뢰하지 마세요).

`context.from` 는 네트워크가 보고한 그대로 정확히 도착하며 대소문자가 변경되지 않습니다. 한 번만 소문자로 바꾸고(`context.from.toLowerCase()`) 그 값을 모든 map, set 및 Storage 호출에 사용하세요. 그래야 한 플레이어의 데이터가 두 개의 키로 분리되는 일이 없습니다.

### 전송하기 전에 상태 동기화를 기다리기

클라이언트는 참가 시 경쟁 상태를 피하기 위해 첫 메시지를 보내기 전에 장면 상태가 동기화될 때까지 기다려야 합니다:

```typescript
import { engine } from "@dcl/sdk/ecs"
import { isStateSyncronized } from "@dcl/sdk/network"
import { room } from "./shared/messages"

engine.addSystem(() => {
  if (!isStateSyncronized()) return

  // 이제 메시지를 보내도 안전함
  room.send("playerReady", { displayName: "Alice" })
})
```


> [!NOTE]
> **💡 팁**: `isStateSyncronized()` 는 클라이언트의 상태가 동기화되었음을 알려줄 뿐, 서버가 초기화를 끝냈다는 것을 보장하지는 않습니다. 더 강력한 준비 상태 확인을 위해 서버가 "ready" 또는 하트비트 메시지를 브로드캐스트하고, 클라이언트가 게임플레이 메시지를 보내기 전에 이를 기다리게 하세요.


### 사용 가능한 스키마 타입

모든 메시지 페이로드와 사용자 정의 컴포넌트는 다음을 사용합니다 `Schemas` 를 사용한 이진 직렬화를 합니다. 사용 가능한 타입의 간단한 참조는 다음과 같습니다:

```typescript
import { Schemas } from "@dcl/sdk/ecs"

// 기본 타입
Schemas.String // "hello"
Schemas.Int // 42
Schemas.Float // 3.14
Schemas.Boolean // true / false
Schemas.Int64 // Date.now()

// 벡터 타입
Schemas.Vector3 // { x: 1, y: 2, z: 3 }
Schemas.Quaternion // { x, y, z, w }

// 복합 타입
Schemas.Array(Schemas.String) // ["a", "b", "c"]
Schemas.Entity // 엔티티 참조
Schemas.Optional(Schemas.String) // "hello" 또는 undefined
Schemas.Optional(Schemas.Int) // 42 또는 undefined

// 중첩 객체
Schemas.Map({
  name: Schemas.String,
  health: Schemas.Int,
  position: Schemas.Vector3,
  playerId: Schemas.Optional(Schemas.String),
})
```


> [!WARNING]
> **📔 참고**: 메시지 *반드시* 다음을 사용하여 정의되어야 함 `Schemas.Map(...)`. 일반 JavaScript 객체를 보낼 수 없습니다. 바이너리 직렬화에 실패합니다.


## 서버가 플레이어 위치 읽기

서버는 다음을 읽을 수 있습니다: **검증된** 플레이어 위치입니다. 클라이언트는 이를 위조할 수 없습니다. 이것이 위치 기반 안티치트의 기반입니다:

```typescript
import { engine, PlayerIdentityData, Transform } from "@dcl/sdk/ecs"

engine.addSystem(() => {
  for (const [entity, identity] of engine.getEntitiesWith(PlayerIdentityData)) {
    const transform = Transform.getOrNull(entity)
    if (!transform) continue

    const address = identity.address
    const position = transform.position
    // 이 위치는 서버에서 검증된 것입니다 — 클라이언트가 보고한 위치를 절대 신뢰하지 마세요
  }
})
```


> [!WARNING]
> **📔 참고**: 항상 다음을 사용하세요 `PlayerIdentityData` + `Transform` 서버에서 플레이어 위치를 가져오세요. 클라이언트 자체가 보고한 값은 절대 신뢰하지 마세요.


## 데이터 저장

서버 재시작 후에도 데이터를 유지합니다. Storage는 **서버 전용**, 호출은 항상 다음으로 감싸세요 `isServer()`. 서버는 이 데이터를 쓰고 읽을 수 있습니다.

```typescript
import { Storage } from "@dcl/sdk/server"
```

데이터는 두 수준에서 저장할 수 있습니다:

* **월드**: 리더보드나 지속적인 환경 변경처럼 모든 플레이어와 관련된 데이터에 사용하세요.
* **플레이어**: 해당 플레이어의 진행 상황이나 선호도처럼 플레이어별 데이터에 사용하세요.


> [!NOTE]
> **💡 팁**: Storage 값은 JSON입니다. 숫자, 문자열, 배열 또는 일반 객체는 그대로 저장하고, 같은 값으로 다시 읽을 수 있습니다. 필요하지 않습니다 `JSON.stringify()` 또는 `JSON.parse()`. 두 가지 예외가 있습니다: `Date` 는 ISO 텍스트(`"2026-10-27T13:05:12.000Z"`)로 저장되며, 값이 `undefined` 로 설정된 객체 속성은 제거됩니다. 이전에 `JSON.stringify()` 로 저장한 값은 문자열이므로, 읽어오면 그 문자열 그대로 반환됩니다.
> 
> 로컬 개발 중에는 storage가 다음에 기록됩니다: `node_modules/@dcl/sdk-commands/.runtime-data/server-storage.json`.



> [!WARNING]
> **📔 참고**: Storage 호출이 문제를 보고하는 방식:
> 
> * `Storage.set()` 및 `Storage.player.set()` 다음으로 resolve됩니다: **부울**. `false` 는 값이 **저장되지** 않았음을 의미합니다. 예를 들어 네트워크 오류 후나 요청이 너무 많이 동시에 실행될 때입니다(아래 참조). 결과를 항상 확인하세요: 버려진 `false` 은 조용히 사라진 저장입니다.
> * `set()` 다음을 던집니다: `TypeError` 값을 변경 없이 JSON으로 저장할 수 없을 때: `undefined`, `NaN` 또는 `Infinity`, 함수, 또는 `Map` 또는 `Set`, 등이 있습니다. 키를 제거하려면 다음을 사용하세요: `delete()`.
> * `get()` 는 저장된 값으로 resolve되거나, `null` 는 키가 존재하지 않을 때만 반환됩니다. 읽기에 실패하면 예외를 던지므로, 실패를 누락된 키로 착각하지 않습니다.
> * 다음 `delete()` 변형은 요청이 실패하면 예외를 던집니다. 이들은 `false` 키가 존재하지 않았을 때만 반환됩니다.


### 모든 변경마다가 아니라 체크포인트에서 저장하세요

Storage는 서버 재시작과 재배포를 견뎌야 하는 데이터의 영속 저장소입니다. 실시간 데이터 저장소는 아닙니다. 작업 중인 게임 상태는 서버 메모리에 보관하세요. 더 빠르고, 서버에 맞는 올바른 패턴입니다. 꼭 필요할 때만, 의미 있는 체크포인트에서 Storage에 쓰세요.


> [!WARNING]
> **⚠️ 경고**: 서버 런타임은 최대 **동시에 진행 중인 호스트 호출 40개** 까지 허용하며, 다음 전체에 공유됩니다 *모든 것* 씬이 런타임에 요청하는 모든 것: 모든 storage 요청, `signedFetch`, 그리고 다른 런타임 API도 모두 같은 제한에 포함됩니다. 초과 호출은 **대기열에 들어가지 않습니다**. 즉시 다음과 함께 거절됩니다: `동시 호스트 호출이 너무 많음` 오류. SDK는 거절을 잡아 `Storage.set` 프로미스를 `false` 으로 해결하므로 예외를 던지지 않습니다. 코드가 그 부울 값을 버리면 실패한 저장은 보이지 않게 되고, 영속 데이터는 오래되거나 사라질 수 있습니다. 모든 쓰기의 결과를 확인하세요(위의 참고 참조).


저장하기 좋은 시점:

* 게임 오버 또는 라운드 종료 시.
* 플레이어가 떠날 때.
* 주기적인 디바운스 저장(예: 30초마다 한 번)으로, 매 프레임이 아닙니다.

재시작이나 재배포 후에도 남아야 하는 데이터만 저장하세요. 나머지는 모두 메모리에 둘 수 있습니다.

아래 예제는 점수를 메모리에 유지하고, 점수가 들어올 때마다가 아니라 체크포인트에서만 씁니다:

```typescript
import { Storage } from "@dcl/sdk/server"

// 작업 상태는 메모리에 있으며, 모든 이벤트마다 갱신됩니다
const scores: Record<string, number> = {}

function onPointScored(address: string) {
  scores[address] = (scores[address] ?? 0) + 1
  // 여기서는 Storage 호출 없음 — 메모리만 갱신
}

// 플레이어가 떠날 때처럼 체크포인트에서만 저장
async function onPlayerLeave(address: string) {
  const saved = await Storage.player.set(address, "score", scores[address] ?? 0)
  if (!saved) {
    // 쓰기가 영속화되지 않았습니다 — 값을 메모리에 유지하고 나중에 다시 시도하세요
    console.error(`${address}의 점수 저장에 실패했습니다`)
  }
}
```

동시에 많은 플레이어가 있는 씬에서는, 더 견고한 접근 방식은 어떤 키에 저장되지 않은 변경 사항이 있는지("dirty set")를 추적하고 다음 flush에서 실패한 쓰기를 다시 시도하는 것입니다:

```typescript
import { engine } from "@dcl/sdk/ecs"
import { Storage } from "@dcl/sdk/server"

// 작업 상태는 메모리에 있으며, 모든 이벤트마다 갱신됩니다
const scores: Record<string, number> = {}
const dirty = new Set<string>()

function onPointScored(address: string) {
  scores[address] = (scores[address] ?? 0) + 1
  dirty.add(address) // 나중에 flush하도록 표시, 여기서는 Storage 호출 없음
}

// dirty 키를 주기적으로(예: 약 30초마다) 그리고 체크포인트에서 flush합니다
async function flush() {
  for (const address of dirty) {
    const ok = await Storage.player.set(address, "score", scores[address])
    if (ok) {
      dirty.delete(address) // 성공적으로 저장됨
    }
    // ok가 false이면, 키는 dirty 상태로 남고 다음 flush에서 다시 시도합니다
  }
}

// dt 누적값을 사용해 타이머로 flush를 실행합니다
let flushTimer = 0
engine.addSystem((dt) => {
  flushTimer += dt
  if (flushTimer > 30) {
    flushTimer = 0
    flush()
  }
})
```

### 월드 Storage — 모든 플레이어가 공유

```typescript
import { Storage } from "@dcl/sdk/server"

type Entry = { name: string; score: number }

// 쓰기 — 값이 영속화되지 않으면 set()은 false로 resolve됩니다
const ok = await Storage.set("leaderboard", [
  { name: "Alice", score: 100 },
  { name: "Bob", score: 85 },
])
if (!ok) {
  console.error("리더보드를 저장하지 못했습니다 — 나중에 다시 시도하세요")
}

// 읽기: 키가 존재하지 않을 때는 null
const leaderboard = (await Storage.get<Entry[]>("leaderboard")) ?? []

// 삭제
await Storage.delete("leaderboard")
```

명령줄을 사용해서도 scene storage를 관리할 수 있습니다: `npx sdk-commands storage scene`:

```bash
# 값 설정
npx sdk-commands storage scene set high_score --value 100

# 값 가져오기
npx sdk-commands storage scene get high_score

# 값 삭제
npx sdk-commands storage scene delete high_score

# 모든 scene storage 데이터 삭제
npx sdk-commands storage scene clear --confirm
```

### 플레이어 Storage — 지갑 주소별

```typescript
import { Storage } from "@dcl/sdk/server"

type Progress = { level: number; coins: number }

// 쓰기 — 값이 영속화되지 않으면 set()은 false로 resolve됩니다
const ok = await Storage.player.set(playerAddress, "progress", {
  level: 5,
  coins: 250,
})
if (!ok) {
  console.error(`${playerAddress}의 진행 상황 저장에 실패했습니다 — 나중에 다시 시도하세요`)
}

// 읽기: 이 플레이어에게 저장된 진행 상황이 없으면 null
const progress = (await Storage.player.get<Progress>(playerAddress, "progress")) ?? { level: 1, coins: 0 }

// 삭제
await Storage.player.delete(playerAddress, "progress")
```

명령줄을 사용해서도 player storage를 관리할 수 있습니다: `npx sdk-commands storage player`:

```bash
# 특정 플레이어의 값 설정
npx sdk-commands storage player set level --value 10 --address 0x1234...

# 특정 플레이어의 값 가져오기
npx sdk-commands storage player get level --address 0x1234...

# 특정 플레이어의 값 삭제
npx sdk-commands storage player delete level --address 0x1234...

# 특정 플레이어의 모든 데이터 삭제
npx sdk-commands storage player clear --address 0x1234... --confirm

# 모든 플레이어 데이터 삭제(전체 플레이어)
npx sdk-commands storage player clear --confirm
```

각 scene 값은 최대 512 KB까지 가능하며, World의 모든 scene 값은 10 MB를 공유합니다. 각 플레이어 값은 최대 100 KB이며, 플레이어당 1 MB입니다.

### 저장을 덮어쓰지 않고 불러오기

플레이어가 입장할 때 기본 데이터를 할당한 뒤 저장하지 마세요: 저장된 데이터는 존재하지만 아직 불러오지 않았다면, 기본값이 그것을 덮어씁니다. 먼저 읽고, 다음 경우에만 기본값을 만드세요 `get()` 다음으로 resolve됩니다: `null`. 읽기에서 예외가 발생하면 플레이어를 로드하지 않은 상태로 두고 나중에 다시 시도하세요.

```typescript
import { Storage } from "@dcl/sdk/server"

type Save = { coins: number }
const saves: Record<string, Save> = {}

async function loadOrCreate(address: string): Promise<Save> {
  if (saves[address]) return saves[address] // 이미 로드됨
  // 읽기에 실패하면 예외를 던지므로, 실패가 새 저장으로 바뀌지 않습니다
  const stored = await Storage.player.get<Save>(address, "save")
  saves[address] = stored ?? { coins: 0 } // 저장이 없을 때만 기본값
  return saves[address]
}
```

다음 안에서 호출하세요: `try/catch` 입장 핸들러에서. 예외가 발생하면 클라이언트에 알려 다시 시도할 수 있게 하고, 저장이 로드될 때까지 해당 플레이어의 동작은 무시하세요.

### 씬에 아무도 없을 때의 진행

작물, 에너지, 상점 재입고처럼 시간이 지나며 증가하는 것에 타이머를 계속 돌리지 마세요. Multiplayer Server는 마지막 플레이어가 떠난 뒤 몇 분 후에 멈춥니다( [서버가 시작되기를 기다리기](#wait-for-the-server-to-start-up)), 따라서 타이머도 멈춥니다. 대신 진행 상황이 마지막으로 업데이트된 시간을 저장하세요. 플레이어가 돌아오면 지난 시간으로부터 진행 상황을 계산하고, 저장된 시간을 앞으로 이동시키세요.

서버의 다음을 사용하세요 `Date.now()` 이 용도로 사용하세요. 클라이언트가 보낸 시간은 절대 사용하지 마세요: 플레이어가 자신의 시계를 앞으로 설정해 대기 시간을 건너뛸 수 있습니다.

### 동시 쓰기에서 데이터 손실을 피하기

Storage에는 읽은 이후 아무도 변경하지 않았을 때만 키를 업데이트하는 방법이 없습니다("compare-and-set" 없음). 따라서 키를 읽고, 값을 바꾼 뒤 다시 쓰면 데이터를 잃을 수 있습니다:

* 두 서버 인스턴스가 동시에 쓸 수 있습니다. 이는 재배포 중 오래된 버전과 새 버전의 씬이 겹치는 동안 발생합니다.
* 읽기가 마지막으로 작성된 값보다 더 오래된 값을 반환할 수 있습니다. Storage는 다음이 있어도 키의 오래된 값을 한 시간 이상 제공한 적이 있습니다: `Storage.get(key, { fresh: true })`, 그리고 서버 재시작을 거쳐서도 그렇습니다.  `fresh` 옵션은 SDK 자체 캐시만 건너뜁니다.
* 다음으로 만든 목록도 `Storage.getValues()` 오래될 수 있습니다: 최근에 작성된 키가 빠지거나, 삭제한 키가 여전히 표시될 수 있습니다.

결과 로그나 플레이어 피드백처럼 계속 늘어나기만 하는 데이터에는, 하나의 키를 다시 쓰는 대신 이 패턴을 사용하세요:

1. 각 스냅샷을 다음에 쓰세요: **새 키** 시간순으로 정렬되고 충돌하지 않는, 예를 들어 `results-1-20261027-130512-k3fz`: 접두사, 파트 번호, UTC 시간, 그리고 몇 개의 무작위 문자입니다.
2. 읽으려면 다음을 사용해 해당 파트의 모든 키를 나열하세요: `Storage.getValues({ prefix, offset })`, 페이지별로 계속해서 다음을 얻을 때까지: `pagination.total` 개의 항목을 가져온 뒤, 행 ID별로 병합하세요.
3. 병합된 행을 새 키로 쓰세요. 그 키를 다시 읽어와서 일치할 때만 병합한 키들을 삭제하세요. 다른 작성자가 당신의 읽기 결과를 바탕으로 아직 변경할 수 있는 키는 절대 삭제하지 마세요.

그렇게 하면 경쟁 상태가 추가 키를 남기고, 다음 쓰기에서 그것을 병합하므로 행이 사라지지 않습니다. 데이터를 읽는 사람은 누구든, 당신이 다음에서 다운로드할 때를 포함해 [storage UI](#access-stored-data), 해당 파트의 모든 키를 읽고 중복 ID를 제거해야 합니다.

```typescript
import { Storage } from "@dcl/sdk/server"

type Row = { id: string; text: string }
const PREFIX = "results-1-"

// 접두사로 시작하는 모든 키, 오래된 것부터
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
  // 행 ID로 병합, 먼저 본 것이 우선
  const rows = new Map<string, Row>()
  for (const { value } of versions) for (const row of value as Row[]) if (!rows.has(row.id)) rows.set(row.id, row)
  for (const row of newRows) if (!rows.has(row.id)) rows.set(row.id, row)
  const merged = [...rows.values()]

  // 매번 새 키: UTC 시간 + 무작위 문자
  // "2026-10-27T13:05:12.123Z"는 "20261027-130512"가 됩니다
  const stamp = new Date().toISOString().slice(0, 19).replace(/[-:]/g, "").replace("T", "-")
  const key = `${PREFIX}${stamp}-${Math.random().toString(36).slice(2, 6)}`
  if (!(await Storage.set(key, merged))) return false // 저장되지 않았으므로 나중에 다시 시도

  // 새 키가 다시 읽힌 후에만 병합된 키를 삭제
  const check = await Storage.get<Row[]>(key, { fresh: true })
  if (JSON.stringify(check) !== JSON.stringify(merged)) return true
  for (const version of versions) {
    try {
      await Storage.delete(version.key)
    } catch {
      // 그대로 남겨 둠: 다음 쓰기에서 다시 병합됩니다
    }
  }
  return true
}
```

한 값에는 최대 512 KB만 저장할 수 있으므로, 새 파트(`results-2-`)을 파트가 그 한계에 가까워지기 전에 시작하세요.  [Scene Feedback 예제](https://github.com/decentraland/sdk7-test-scenes/tree/main/scenes/0%2C-1-scene-feedback) 는 CSV 파일에 이 패턴을 사용하며, 400 KB에서 새 파트로 넘깁니다.

### 저장된 데이터에 접근

다음 링크를 입력하여 storage UI에서 서버의 실시간 저장 데이터를 보고 편집할 수 있습니다:

[decentraland.org/storage](https://decentraland.org/storage)

Creator Hub에서도 이 페이지에 접근할 수 있습니다. 다음을 여세요: **관리** 탭에서 콘텐츠를 게시한 장소 옆의 세 점을 클릭한 다음, 다음을 선택하세요: **Storage 보기**.

여기에서 씬을 게시할 수 있는 모든 월드와 랜드 목록을 볼 수 있습니다.

씬을 연 다음 **씬** 또는 **플레이어** 탭을 여세요.

다음 **씬** 탭에서는 저장된 모든 변수가 표시됩니다. 여기서 연필 또는 휴지통 아이콘을 클릭해 이러한 변수를 편집하거나 제거할 수 있습니다.

![Activate stream](/docs/_remote/76bca9abaef9d73bc207163d.webp)

다음 **플레이어** 탭에서는 서버에 데이터가 저장된 모든 플레이어 목록이 표시됩니다. 주소나 이름으로 검색한 다음, 관련된 모든 데이터를 볼 수 있습니다. 연필 또는 휴지통 아이콘을 클릭해 이 데이터도 편집하거나 제거할 수 있습니다.

### 데이터 구조 변경

프로덕션의 저장된 데이터는 **씬의 새 버전을 게시할 때 지워지지 않습니다**. 이는 리더보드, 플레이어 진행 상황, 그리고 작은 씬 업데이트를 넘어 지속되기를 플레이어가 기대하는 지속적인 환경 변경에 아주 좋습니다.

반면 storage에 있는 데이터는 이전 버전의 코드가 작성한 것입니다. 새 코드가 다른 형태를 기대한다면, 그 오래된 데이터를 파싱하거나 읽는 과정이 미묘하게 실패할 수 있습니다. 이름을 바꾼 필드는 누락됩니다. 예전에는 문자열이었고 지금은 객체인 필드는 그 속성에 접근하려고 하면 예외를 던집니다. 몇 달 동안 로그인하지 않은 플레이어는 코드가 더 이상 처리 방법을 모르는 구조보다 더 오래된 데이터를 불러올 수 있습니다.


> [!WARNING]
> **📔 참고**: 스키마 변경은 배포 후 첫 번째 읽기에만 영향을 주는 것이 아닙니다. 저장된 데이터는 덮어쓰거나 삭제될 때까지 남아 있으므로, 오래된 형식의 값은 언제든지 나타날 수 있으며, 종종 잊고 있던 복귀 플레이어에게서 나타납니다.


#### 권장 사항

* *항상 방어적으로 읽으세요*. 저장소에서 나오는 모든 것은, 직접 저장했더라도 신뢰할 수 없는 입력으로 취급하세요. 다음에 전달하는 타입은 `get<T>()` 검증되지 않으므로, 읽기 전에 필드가 존재하고 올바른 타입인지 확인하며, 그렇지 않을 때 쓸 적절한 기본값을 준비하세요:

  ```typescript
  import { Storage } from "@dcl/sdk/server"

  const saved = await Storage.player.get<any>(playerAddress, "progress")
  const progress = {
    level: typeof saved?.level === "number" ? saved.level : 1,
    coins: typeof saved?.coins === "number" ? saved.coins : 0,
  }
  ```
* *필드를 추가하세요. 이름을 바꾸거나 제거하지 마세요*. 가장 안전한 스키마 변경은 추가형입니다: 기본값이 있는 새 필드를 도입하고 기존 필드는 그대로 두세요. 오래된 데이터에는 단순히 새 필드가 없을 뿐이며, 방어적 파싱이 이미 이를 처리합니다. 필드 이름을 바꾸면 모든 오래된 레코드가 깨집니다.
* *저장된 객체에 버전을 부여하세요*. 다음을 포함하세요: `version` 필드를 처음부터 넣으세요. 데이터를 읽을 때는 버전에 따라 분기하고, 사용하기 전에 이전 형태를 현재 형태로 마이그레이션하세요. 이렇게 하면 나머지 코드가 하나의 현재 형태로 계속 동작합니다:

  ```typescript
  type ProgressV2 = { version: 2; level: number; coins: number; xp: number }

  function migrate(raw: any): ProgressV2 {
    const version = raw?.version ?? 1
    if (version === 1) {
      // v1에는 xp 필드가 없었습니다 — 기본값을 넣습니다
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
* *마이그레이션된 값을 다시 쓰세요*. 메모리에서 레코드를 업그레이드했다면 다시 저장해 다음 읽기에서는 이미 새 형식이 되도록 하세요. 이렇게 하면 일회성 마이그레이션 스크립트 없이도 오래된 형태의 레코드가 점차 사라집니다.
* *호환성 깨짐 변경에는 새 키를 사용하세요*. 새 구조가 정말로 호환되지 않고 마이그레이션할 가치가 없다면, 새 storage 키(예: `progress_v2`)을 사용하고 이전 것은 무시하세요. 이전 키는 storage에 무해하게 남아 있고, 이를 해석해야 하는 읽기 경로를 피할 수 있습니다. 나중에 다음을 통해 이전 키를 정리할 수 있습니다: [storage UI](https://decentraland.org/storage) 또는 다음: `npx sdk-commands storage` 명령입니다.
* *실제 프로덕션 데이터로 테스트하세요*. 구조 변경을 배포하기 전에 저장소 UI에서 실제 레코드 몇 개를 가져와 새 파싱 코드를 그 데이터에 대해 실행해 보세요. 문제를 일으키는 모서리 사례는 보통 존재하는지 몰랐던 레코드입니다.
* *탈출구를 남겨 두세요*. 저장소 UI에서 또는 `npx sdk-commands storage`. 스키마 변경 후 한 명의 플레이어가 나쁜 상태에 갇히더라도, 재배포 없이 해당 레코드를 직접 수정할 수 있습니다.

## 환경 변수

값을 코드에 하드코딩하지 않고 장면을 구성하세요. 환경 변수는 민감한 데이터뿐 아니라, 장면을 다시 게시하지 않아도 쉽게 변경할 수 있는 기능 플래그나 매개변수에도 유용합니다.

환경 변수는 **서버 전용**. 다음으로 보호하세요 `isServer()`. 서버는 환경 변수를 읽을 수는 있지만, 값을 변경할 수는 없습니다.

`EnvVar.get()` 는 `Promise<string>` 를 반환하며, 변수가 설정되지 않은 경우 빈 문자열로 해결되므로, 누락된 값에는 항상 대체값을 제공하세요:

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

### 민감한 데이터

환경 변수는 공개 장면의 컴파일된 코드에 노출되면 위험할 수 있는 개인 키, 보상 청구 코드, 기타 민감한 데이터를 저장하는 데 특히 유용합니다.

개인 키를 서버의 저장소에 보관하고, 서버만 이를 다음으로 읽도록 하려면 `isServer()`. 이렇게 하면 민감한 데이터가 플레이어의 기기를 절대 거치지 않습니다.

### 로컬 개발

프로젝트를 로컬에서 실행하는 동안 환경 변수를 사용하려면, 프로젝트 루트에 `.env` 파일을 만드세요:

```
MAX_PLAYERS=8
GAME_DURATION=300
DEBUG=true
```

중요: `.env` 를 `.gitignore`에 추가하세요. 그러면 이러한 잠재적으로 민감한 값이 공개 콘텐츠 서버에 절대 업로드되지 않습니다.

### 환경 변수 변경하기

환경 변수 값을 변경하는 가장 쉬운 방법은 저장소 UI를 이용하는 것입니다.

다음 링크를 입력하여 장면의 저장소가 보관한 데이터에 접근할 수 있습니다:

[decentraland.org/storage](https://decentraland.org/storage)

Creator Hub에서도 이 페이지에 접근할 수 있습니다. 다음을 여세요: **관리** 탭에서 콘텐츠를 게시한 장소 옆의 세 점을 클릭한 다음, 다음을 선택하세요: **Storage 보기**.

여기에서 씬을 게시할 수 있는 모든 월드와 랜드 목록을 볼 수 있습니다.

씬을 연 다음 **환경** 탭. 프로젝트의 모든 환경 변수가 보여야 합니다.

![Activate stream](/docs/_remote/13189fe9bb9d6d0cc3454293.webp)

이러한 환경 변수의 값은 읽을 수 없다는 점에 유의하세요(민감한 데이터를 보호하기 위한 것입니다). 하지만 각각은 삭제하거나 덮어쓸 수 있습니다. 연필 아이콘이나 휴지통 아이콘을 클릭하면 됩니다.

명령줄을 사용해서도 환경 변수를 관리할 수 있습니다. 다음을 사용하세요 `npx sdk-commands storage env`:

```bash
# 변수 설정
npx sdk-commands storage env set MAX_PLAYERS --value 8

# 변수 삭제
npx sdk-commands storage env delete OLD_VAR

# 모든 환경 변수 삭제
npx sdk-commands storage env clear --confirm
```

다음으로 특정 환경을 대상으로 지정할 수도 있습니다 `--target` 플래그:

```bash
# 스테이징에 배포
npx sdk-commands storage env set MY_KEY --value my_value --target https://storage.decentraland.zone

# 로컬 개발 서버에 배포
npx sdk-commands storage env set MY_KEY --value my_value --target http://localhost:8000
```

배포된 환경 변수는 `.env` 값보다 우선합니다.

## 권장 프로젝트 구조

서버, 클라이언트, 공유 코드를 분리하면 코드베이스가 커져도 읽기 쉽게 유지됩니다:

```
src/
├── index.ts              # 진입점 — isServer() 분기
├── client/
│   ├── setup.ts          # 입력 핸들러, 메시지 송신자
│   └── ui.tsx            # React ECS UI(동기화된 상태 읽기)
├── server/
│   ├── server.ts         # 게임 루프, 메시지 핸들러, 상태 변경
│   └── gameState.ts      # 서버 상태용 헬퍼 함수
└── shared/
    ├── schemas.ts        # 컴포넌트 정의 + validateBeforeChange
    └── messages.ts       # registerMessages() — 양쪽에서 import됨
```


> [!NOTE]
> **💡 팁**: 모든 `registerMessages()` 호출과 사용자 정의 컴포넌트 정의를 `shared/`에 모두 두세요. 서버와 클라이언트는 모두 거기에서 import하므로 메시지 스키마에 항상 합의하게 됩니다.


## 성능 모범 사례

모든 컴포넌트 변경은 *전체* 컴포넌트 데이터를 네트워크로 전송합니다. 이는 diff만 전송하는 Colyseus와는 다릅니다. 사용자 정의 컴포넌트를 설계할 때 이 점을 염두에 두세요. 가장 적절한 해법은 변경 빈도에 따라 데이터를 별도 컴포넌트에 저장하는 것일 수 있습니다.

### ❌ 거대한 단일 컴포넌트는 피하세요

```typescript
import { engine, Schemas } from "@dcl/sdk/ecs"

// 나쁨 — 점수를 바꾸면 positions 배열도 전송됨
const GameState = engine.defineComponent("GameState", {
  playerAScore: Schemas.Int,
  playerBScore: Schemas.Int,
  timer: Schemas.Int,
  playerPositions: Schemas.Array(Schemas.Vector3), // 큰 페이로드
})
```

### ✅ 원자적 컴포넌트를 선호하세요

```typescript
import { engine, Schemas } from "@dcl/sdk/ecs"

// 좋음 — 각 업데이트가 작고 독립적임
const PlayerScore = engine.defineComponent("PlayerScore", {
  playerA: Schemas.Int,
  playerB: Schemas.Int,
})

const GameTimer = engine.defineComponent("GameTimer", {
  secondsLeft: Schemas.Int,
})
```

*경험칙*: 함께 그리고 비슷한 빈도로 바뀌는 필드를 묶으세요. 빠르게 바뀌는 데이터(타이머, 위치)는 느리게 바뀌는 데이터(점수, 설정)와 분리하세요.

### 빈번한 메시지 제한하기

매 프레임마다 메시지를 보내지 마세요. 가능하면 일괄 처리하거나 제한하세요:

```typescript
import { engine } from "@dcl/sdk/ecs"
import { room } from "./shared/messages"

let lastSend = 0
engine.addSystem((dt) => {
  lastSend += dt
  if (lastSend > 0.1) {
    // 100ms마다
    room.send("position", transform.position)
    lastSend = 0
  }
})
```

예를 들어 서버가 카운트다운 타이머를 제어한다면, 매초 모든 플레이어에게 업데이트를 보낼 필요는 없습니다. 각 클라이언트가 스스로 시간의 흐름을 계산하게 하고, 서버는 약 30초마다 현재 상태를 브로드캐스트하여 일관성을 보장하는 것이 가장 좋습니다.

## 서버 리소스 한계

멀티플레이어 서버는 각 장면을 샌드박스 처리된 isolate에서 실행하며, 엄격한 리소스 상한이 있습니다. 이 한계에 도달하면 데이터가 조용히 유실되거나 장면의 모든 사용자에 대해 서버가 종료될 수 있으므로, 장면이 이 범위 안에 충분히 머물도록 설계하세요.

### 메모리

이 isolate의 **256 MB** 메모리 상한이 있습니다. 초과되면 isolate가 폐기되고 연결된 모든 플레이어에 대해 서버가 종료됩니다. 작업 상태는 가볍게 유지하고, 플레이어가 떠날 때는 플레이어별 데이터를 정리하세요.

### CPU

각 실행 턴에는 실시간 기준 예산이 있습니다:

* **동기 실행**: 턴당 10초. 이를 초과하는 무한 루프는 isolate를 종료시킵니다.
* **비동기 턴 완료**: 60초. 만약 `Promise` 체인이나 `await` 이 시간보다 오래 걸려 해결되면 isolate가 종료됩니다.

다음으로 무거운 작업을 여러 틱에 나눠서 처리하세요. `dt` 누산기를 `engine.addSystem()`에 사용하세요. 서버에서 무한 동기 루프를 절대 실행하지 마세요.

### 들어오는 메시지 속도

연결된 각 피어는 대략 **1,000ms당 300개 메시지**까지 보낼 수 있습니다. 초과 데이터 프레임은 대기열에 쌓이지 않고 버려집니다. 클라이언트에서 매 프레임마다 메시지를 보내지 마세요. 제한 패턴은 [성능 모범 사례](#performance-best-practices) 를 참조하세요.

### 메시지 크기

* 들어오는 패킷은 패킷당 **128 KB** 로 제한됩니다. 너무 큰 패킷은 완전히 버려집니다.
* 장면-통신 메시지는 대략 **30 KB**로 제한됩니다. 실용적인 전송 계층에서는 동기화된 메시지를 **13 KB** 보다 훨씬 작게 유지하세요(섹션을 [메시지](#messages) 참조하세요).

### 외부 fetch

동시 `signedFetch` 호출은 **32** 진행 중으로 제한됩니다. 추가 fetch는 슬롯이 열릴 때까지 대기열에 들어갑니다. 각 fetch 시도에는 **15초** 타임아웃이 있습니다. fetch 응답은 **10 MB**로 제한됩니다. WebSocket 연결은 **32** 동시 소켓 **까지 허용되며, 메시지당 최대 크기는** 1 MB

### 입니다.

에서 설명한 40회 호출 제한은 [데이터 저장](#data-storage) Storage를 포함한 모든 호스트 호출에 대해 isolate 전체에 적용되며, `signedFetch`및 기타 런타임 API에도 적용됩니다. 초과 호출은 즉시 거부되며 대기열에 들어가지 않습니다.

## 흔한 함정

### 서버 전용 상태에 대한 검증을 잊는 것

없이 `validateBeforeChange`가 없으면 클라이언트가 어떤 컴포넌트든 쓸 수 있습니다:

```typescript
import { engine, Schemas } from "@dcl/sdk/ecs"
import { isServer } from "@dcl/sdk/network"
import { AUTH_SERVER_PEER_ID } from "@dcl/sdk/network/message-bus-sync"

// ❌ 나쁨 — 클라이언트가 속일 수 있음
const Score = engine.defineComponent("Score", { value: Schemas.Int })

// ✅ 좋음 — 서버 전용
if (isServer()) {
  Score.validateBeforeChange((v) => v.senderAddress === AUTH_SERVER_PEER_ID)
}
```

### 클라이언트가 제공한 값 믿기

체력, 점수, 위치 같은 중요한 데이터에 대해 클라이언트가 자기 자신의 값을 정하게 해서는 안 됩니다:

```typescript
import { room } from "./shared/messages"

// ❌ 나쁨
room.onMessage("setHealth", (data) => {
  player.health = data.health // 값은 클라이언트가 제어합니다!
})

// ✅ 좋음 — 서버가 결과를 계산함
room.onMessage("takeDamage", (data) => {
  const damage = calculateDamage(data.source)
  player.health = Math.max(0, player.health - damage)
})
```

### 메시지 속 금액과 주소를 믿는 것

클라이언트의 메시지는 사실이 아니라 요청입니다. 사용하기 전에 모든 필드를 확인하세요:

* **금액**: `Schemas.Int` 는 값을 32비트 정수로 디코딩하지만, 여전히 0이거나 음수일 수 있습니다. 0 이하의 금액은 거부하세요. 그렇지 않으면 플레이어가 "보내고" `-500` 코인을 받고 오히려 가져갈 수 있습니다. 발신자가 충분한지도 확인하세요.
* **주소**: 클라이언트가 보내는 지갑 주소는 반드시 `0x` 뒤에 40개의 16진수 문자가 와야 합니다. 맵이나 Storage에 도달하기 전에 그 외의 모든 것은 거부하세요.

```typescript
import { room } from "./shared/messages"

const ADDRESS = /^0x[0-9a-f]{40}$/
const balances: Record<string, number> = {}

room.onMessage("giveCoins", (data, context) => {
  if (!context) return
  const from = context.from.toLowerCase()
  const to = String(data.to).toLowerCase()
  if (!ADDRESS.test(to) || to === from) return
  if (!Number.isInteger(data.amount) || data.amount <= 0) return // 음수와 0을 거부
  if ((balances[from] ?? 0) < data.amount) return // 코인이 부족함
  balances[from] -= data.amount
  balances[to] = (balances[to] ?? 0) + data.amount
})
```

### 상태 동기화 전에 메시지 보내기

클라이언트는 상호작용하기 전에 상태가 동기화될 때까지 기다려야 합니다:

```typescript
import { engine } from "@dcl/sdk/ecs"
import { isStateSyncronized } from "@dcl/sdk/network"

engine.addSystem(() => {
  if (!isStateSyncronized()) return
  // 메시지를 보내도 안전
})
```

### 서버가 시작되기를 기다리기

서버는 씬에 최소 한 명의 플레이어가 있을 때만 활성화됩니다. 마지막 플레이어가 나가면 서버는 대략 2분간 유지된 뒤 종료됩니다. 다음 방문에서는 새 인스턴스가 콜드 스타트되며, 이는 대략 **프로덕션에서 15초**가 걸립니다. 로컬 미리보기는 서버를 즉시 시작하므로, 콜드 스타트 문제는 거의 항상 게시한 뒤에야 드러납니다.

씬의 코드는 서버가 온라인이 될 때까지 기다릴 준비가 되어 있어야 합니다. 서버가 시작을 마치기 전에 보낸 메시지는 조용히 사라집니다. 기다리는 동안 플레이어에게 "서버가 깨어나는 중" 메시지를 보여 주고, 초기 서버 요청에는 재시도 로직을 추가하세요.


> [!NOTE]
> **팁:** 서버 준비 상태를 감지하는 신뢰할 만한 방법은 하트비트입니다. 서버가 2초마다 동기화된 컴포넌트 필드에 `Date.now()` 를 쓰고, 클라이언트가 마지막으로 값이 바뀐 시점을 추적하게 하세요. 6초 안에 변화가 없으면 서버를 오프라인으로 간주하세요. 이는 `isStateSyncronized()`보다 더 견고합니다. 후자는 전송이 연결되었을 뿐 서버가 실행 중인지까지는 확인하지 않습니다.


## 완전한 예제

최소한의 멀티플레이어 카운터: 버튼을 클릭하면 서버가 동기화된 점수를 증가시킵니다. 서버는 카운터를 `Storage` 에 저장하여 값이 서버 재시작 후에도 유지되도록 합니다. 서버는 씬에 플레이어가 없으면 종료된다는 점을 기억하세요. 따라서 저장소가 없으면 씬이 플레이어 없이 비게 될 때마다 카운트가 0으로 초기화됩니다.

```typescript
import { engine, Schemas } from "@dcl/sdk/ecs"
import { registerMessages, isServer, syncEntity } from "@dcl/sdk/network"
import { AUTH_SERVER_PEER_ID } from "@dcl/sdk/network/message-bus-sync"
import { pointerEventsSystem } from "@dcl/sdk/ecs"
import { Storage } from "@dcl/sdk/server"

// 1. 메시지 정의(공유)
const Messages = {
  increment: Schemas.Map({}),
  stateUpdate: Schemas.Map({
    count: Schemas.Int,
    lastPlayer: Schemas.String,
  }),
}

// 2. 서버 전용 컴포넌트 정의(공유)
const Counter = engine.defineComponent("Counter", {
  value: Schemas.Int,
  lastPlayer: Schemas.String,
})

// 3. 룸 생성
const room = registerMessages(Messages)

export async function main() {
  if (isServer()) {
    // === 서버 ===

    // 이 컴포넌트는 서버만 수정할 수 있습니다
    Counter.validateBeforeChange(
      (v) => v.senderAddress === AUTH_SERVER_PEER_ID
    )

    // 서버가 재시작한 경우를 대비해 카운터를 저장소에서 복원
    let initialCount = 0
    let initialPlayer = "none"
    let loaded = false
    try {
      initialCount = (await Storage.get<number>("counter")) ?? 0
      initialPlayer = (await Storage.get<string>("lastPlayer")) ?? "none"
      loaded = true
    } catch (error) {
      console.log("[서버] 카운터를 불러올 수 없습니다:", error)
    }

    const counterEntity = engine.addEntity()
    syncEntity(counterEntity, [Counter.componentId], 1)
    Counter.create(counterEntity, {
      value: initialCount,
      lastPlayer: initialPlayer,
    })

    room.onMessage("increment", async (_data, context) => {
      if (!context) return
      if (!loaded) return // 로드되지 않은 카운터 위에 절대 저장하지 마세요
      const from = context.from.toLowerCase()
      const counter = Counter.getMutable(counterEntity)
      counter.value += 1
      counter.lastPlayer = from

      // 서버 재시작 후에도 값이 유지되도록 저장소에 저장
      const saved =
        (await Storage.set("counter", counter.value)) &&
        (await Storage.set("lastPlayer", from))
      if (!saved) console.log("[서버] 카운터가 저장되지 않았습니다. 다음 클릭에서 다시 저장됩니다")

      room.send("stateUpdate", {
        count: counter.value,
        lastPlayer: from,
      })
    })
  } else {
    // === 클라이언트 ===
    const button = engine.addEntity()
    // ... Transform, MeshRenderer 등을 추가

    pointerEventsSystem.onPointerDown(button, () => {
      room.send("increment", {})
    })

    room.onMessage("stateUpdate", (data) => {
      console.log(`카운트: ${data.count} (마지막 클릭자: ${data.lastPlayer})`)
    })
  }
}
```

## 로컬에서 테스트하기

표준 미리보기가 모든 것을 처리합니다. SDK의 auth-server 분기를 사용할 때 로컬 서버는 클라이언트 미리보기와 함께 백그라운드에서 자동으로 시작됩니다.

로컬에서 멀티플레이어 상호작용을 테스트하려면, 두 개의 별도 창에서 미리보기를 여세요. 각 창은 별도의 플레이어로 취급됩니다. 각 창을 서로 다른 주소로 연결하세요. 두 클라이언트는 같은 로컬 서버 인스턴스에 연결됩니다.

Creator Hub를 사용해 **다중 인스턴스 미리보기** 를 **데스크톱 클라이언트** 의 **플레이 옵션**하위 메뉴에서 켠 다음, **Play** 버튼을 한 번 더 클릭하여 두 번째 Decentraland 익스플로러 창을 여세요. 두 창 모두 서로 다른 주소로 연결해야 합니다. 장면이 다시 로드되는 동안 같은 세션은 계속 열려 있습니다.

대안으로, 다음을 브라우저 URL에 입력하여 두 번째 Decentraland 익스플로러 창을 열 수 있습니다:

> `decentraland://realm=http://127.0.0.1:8000&local-scene=true&debug=true&multi-instance=true`

### 디버깅 팁

* *로그 앞에 접두어를 붙이세요* 다음으로 `[서버]` 또는 `[클라이언트]` 그래서 터미널에서 구분할 수 있습니다:

  ```typescript
  import { isServer } from "@dcl/sdk/network"

  if (isServer()) {
    console.log("[서버] 시작 중...")
  } else {
    console.log("[클라이언트] 시작 중...")
  }
  ```
* *컴포넌트 동기화 확인* 클라이언트에서 엔티티 수를 로그로 출력해 확인하세요:

  ```typescript
  import { engine } from "@dcl/sdk/ecs"

  engine.addSystem(() => {
    const entities = Array.from(engine.getEntitiesWith(MyComponent))
    console.log("[클라이언트] 동기화된 엔티티:", entities.length)
  })
  ```

## 프로덕션에서 디버그하기

다음을 사용하세요 [Scene Insights](/docs/creator/content-creator-ko/sdk7/debugging/scene-insights/) 게시된 서버 로그를 검색하고, 실시간으로 따라가며, CPU, 메모리 및 기타 메트릭을 확인하려면 **7일**동안 보관됩니다. 재시작과 재배포를 거쳐서도 유지됩니다.

최신 `auth-server` SDK로 업데이트한 다음, 씬 프로젝트에서 다음 명령을 실행하세요:

```bash
npx sdk-commands scene-insights login
npx sdk-commands scene-insights tail
```

브라우저에서 권한이 있는 지갑으로 로그인하세요. 읽기 전용 세션은 이후 명령에 최대 12시간 동안 재사용됩니다. 누르세요 **Ctrl+C** 로그 추적을 중지하려면.

다음 [Scene Insights 가이드](/docs/creator/content-creator-ko/sdk7/debugging/scene-insights/) 는 과거 검색, 장면 선택, 접근 권한, 메트릭, AI 지원 디버깅, 수집 한도를 다룹니다.

### 저장소 데이터 보기

다음 링크를 입력하여 장면의 저장소가 보관한 데이터에 접근할 수 있습니다:

[decentraland.org/storage](https://decentraland.org/storage)

Creator Hub에서도 이 페이지에 접근할 수 있습니다. 다음을 여세요: **관리** 탭에서 콘텐츠를 게시한 장소 옆의 세 점을 클릭한 다음, 다음을 선택하세요: **Storage 보기**.

여기에서 씬을 게시할 수 있는 모든 월드와 랜드 목록을 볼 수 있습니다.

각각에 저장된 정보를 보려면 월드 또는 플레이어 데이터를 여세요.

예를 들어 특정 플레이어가 당신의 장면을 플레이할 때 문제가 생긴다면, 주소로 이 플레이어를 찾아 그에게 저장된 데이터를 확인해 상황을 파악할 수 있습니다. 어쩌면 서로 충돌하는 데이터 상태에 빠지는 모서리 사례를 만났을 수도 있습니다. 이 페이지에서 그 플레이어의 데이터를 지우거나 편집해 안정된 상태로 되돌릴 수도 있습니다.

## 버전 관리

장면의 게시된 모든 버전에는 고유한 해시 ID가 부여되며, 각 해시는 전용 서버 인스턴스와 짝지어집니다. 즉 클라이언트 코드와 서버 코드는 항상 함께 움직이며, 구버전 로직을 실행하는 클라이언트가 새 로직을 실행하는 서버와 통신하는(또는 그 반대의) 시간 창이 없습니다.

업데이트를 게시하면:

* *이미 씬에 있는 플레이어들* 그들은 떠났다가 다시 돌아올 때까지 계속 이전 버전의 씬을 보게 됩니다. 해당 클라이언트는 이전 해시에 해당하는 서버 인스턴스에 계속 연결된 상태로 유지됩니다.
* *새로 들어오는 플레이어* 업데이트 후에는 새 씬 버전을 불러오고 새 서버 인스턴스에 연결합니다.

이렇게 하면 스키마 변경이나 구성 요소 이름 변경 때문에 클라이언트와 서버 상태가 절대 동기화에서 벗어나지 않습니다. 업데이트가 이미 씬에 있는 플레이어의 세션을 절대 중단시키지 않습니다.

트레이드오프는 배포 직후 짧은 시간 동안 플레이어가 두 개의 서로 다른 서버 인스턴스로 나뉠 수 있다는 점입니다. 이미 그곳에 있던 플레이어와 방금 도착한 플레이어는 이전 플레이어들이 떠나 다시 합류할 때까지 서로를 볼 수 없거나 씬을 통해 상호작용할 수 없을 수 있습니다.


> [!NOTE]
> **💡 팁**: 다음을 통해 저장된 데이터 [Storage](#data-storage) 서비스(예: 순위표, 플레이어 진행 상황, 또는 지속되는 환경 변경)는 *저장되지* 버전 간에 초기화됩니다. 저장소는 위치 수준에서 유지되며 동일한 씬을 가리키는 모든 서버 인스턴스 간에 공유되므로 새 버전은 이전 버전이 중단된 지점부터 그대로 이어집니다.


## Colyseus에서 마이그레이션

기존에 Colyseus로 구축한 씬이 있다면, 아래 표에서 일반적인 Colyseus 패턴과 SDK7의 대응 항목을 확인할 수 있습니다:

| Colyseus                   | SDK7 멀티플레이어 서버                       |
| -------------------------- | ------------------------------------ |
| `room.send(type, data)`    | `room.send(type, data)` — 동일한 API    |
| `room.onMessage(type, cb)` | `room.onMessage(type, cb)` — 동일한 API |
| `room.state.players` (스키마) | `syncEntity` + 사용자 정의 컴포넌트           |
| JSON 직렬화                   | 바이너리 직렬화(다음을 통해 자동으로 `Schemas`)      |
| 별도의 서버 애플리케이션              | 동일한 코드베이스 — `isServer()` 브랜칭         |
| 사용자 지정 서버 호스팅              | 내장: 미리보기에서 서버가 자동으로 실행됨              |

기억해 둘 주요 차이점:

* *직렬화*: Colyseus는 JSON diff를 전송하고, SDK는 변경될 때마다 전체 컴포넌트를 전송합니다. 컴포넌트는 작게 유지하세요(참조 [성능 모범 사례](#performance-best-practices)).
* *상태 모델*: Colyseus는 자동 diffing이 가능한 변경 가능한 상태 트리를 사용합니다. SDK는 다음을 통해 동기화되는 ECS 컴포넌트를 사용합니다. `syncEntity` 그리고 다음으로 보호됩니다 `validateBeforeChange`.
* *호스팅*: 별도의 서버 배포가 없습니다. 멀티플레이어 서버는 씬과 함께 자동으로 배포됩니다.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/networking/authoritative-servers.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
