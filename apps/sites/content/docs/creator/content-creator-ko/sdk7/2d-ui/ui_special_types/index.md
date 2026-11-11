> 드롭다운과 입력 상자를 포함한 UI의 특수 엔티티 유형.

# UI 특수 유형

특정 특별한 개체 유형은 일부 특별한 종류의 상호작용을 허용합니다.

## 드롭다운

만드세요 `드롭다운` 사용자가 목록에서 항목을 펼쳐 선택할 수 있게 하는 개체입니다.

하나는 `드롭다운` 개체에는 최소한 다음 속성이 있어야 합니다:

* `options`: 드롭다운이 확장될 때 표시할 값입니다. 각 옵션에 대한 문자열 값이 들어 있는 배열을 제공하세요. 배열의 첫 번째 값이 기본 옵션으로 표시됩니다.
* `onChange`: 드롭다운에서 값이 선택될 때마다 실행되는 함수입니다. 다음을 사용하여

다른 구성 요소도 구성할 수 있습니다. `드롭다운` 개체, 예를 들어 `uiTransform`, 다른 UI 개체에서처럼. 드롭다운 텍스트는 다음과 동일한 스타일 속성을 허용합니다 `Label`, 예를 들어 `색상`, `fontSize`, `글꼴` 및 `fontSrc` 사용자 지정 글꼴의 [사용자 지정 글꼴](/docs/creator/content-creator-ko/sdk7/2d-ui/ui_text/#custom-fonts).

***ui.tsx 파일:***

```tsx
import { UiEntity, Label, Dropdown, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'
import { Color4Type } from '@dcl/sdk/ecs'

function selectOption(index: number) {
  switch (index) {
    case 0:
      textColor = Color4.Red()
      break
    case 1:
      textColor = Color4.Blue()
      break
    case 2:
      textColor = Color4.Green()
      break
  }
}

let textColor: Color4Type = Color4.Red()

export const uiMenu = () => (
    <UiEntity
      uiTransform={{
        width: '200px',
        height: '100px',
        alignContent: 'auto',
        flexDirection: 'column',
        alignSelf: 'center',
      }}
    >
      <Label
        value="색상을 선택하세요"
        fontSize={18}
        color={textColor}
        uiTransform={{
          width: '140px',
          height: '40px',
        }}
      />
      <Dropdown
        options={[`빨강`, `파랑`, `초록`]}
        onChange={selectOption}
        uiTransform={{
          width: '100px',
          height: '40px',
        }}
      />
    </UiEntity>
)
```

***index.ts 파일:***

```ts
import { ReactEcsRenderer } from '@dcl/sdk/react-ecs'
import { uiMenu } from './ui'

export function main() {
    ReactEcsRenderer.setUiRenderer(uiMenu)
}
```


> [!WARNING]
> **📔 참고**: 이 페이지의 다음 스니펫들은 모두 다음이 있다고 가정합니다: `.ts` 위와 유사한 것이 실행 중이며, `ReactEcsRenderer.setUiRenderer()` 함수입니다.


## 입력 텍스트

생성하기 `입력` 사용자가 텍스트를 입력할 수 있게 하는 개체입니다. 플레이어는 먼저 이 상자를 클릭해야 그 안에 글을 입력할 수 있습니다.

의 동작은 `입력` 개체는 다음 속성으로 관리됩니다:

* `onSubmit`: 플레이어가 Return/Enter 키를 누를 때 제공된 텍스트를 입력값으로 사용하여 실행되는 함수입니다. 텍스트 필드는 **아니며** 이 경우 자동으로 지워지지 않으므로 직접 지워야 합니다(아래 예시 참조).
* `onChange`: 입력 텍스트의 값이 변경될 때마다 실행되는 함수입니다. 플레이어가 상자에 입력할 때 이 함수는 추가되거나 삭제된 각 문자마다 한 번씩 실행됩니다.
* `disabled`: 만약 *true*, 플레이어는 입력 개체와 상호작용할 수 없습니다.

다음 예시는 `onSubmit` 제공된 텍스트를 콘솔에 로그합니다.

```tsx
import { UiEntity, Input, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'


export const uiMenu = () => (
  <UiEntity
    uiTransform={{
      width: 400,
      height: 300,
      positionType: 'absolute',
      position: {
        left: '35%',
        top: '40%',
      },
    }}
    uiBackground={{
      color: Color4.Gray(),
    }}
  >
    <Input
      onSubmit={(value) => {
        console.log('제출된 값: ' + value)
      }}
      fontSize={35}
      placeholder={'무언가를 입력하세요'}
      placeholderColor={Color4.Black()}
      uiTransform={{
        width: '400px',
        height: '80px',
      }}
    ></Input>
  </UiEntity>
)
```

제출용 버튼을 제공하고 다음을 처리하는 것도 좋은 방법입니다: `onSubmit` 플레이어가 Enter/Return 키를 눌렀을 때의 이벤트입니다. 다음 예시는 이를 수행하는 방법을 보여줍니다.


> [!WARNING]
> **📔 참고**: 하지 **아니며** 다음을 직접 바인딩하지 `값` 컴포넌트의 `입력` 플레이어가 입력하는 내용에 직접 `onChange`. 이렇게 하면 씬과 Explorer 사이에 왕복이 생겨 빠르게 입력할 때 문자가 누락됩니다. 대신 `값` 를 빈 문자열로 유지하고 입력 필드를 지우고 싶을 때만 변경하세요.


입력 필드를 지우려면 `값` 을(를) `' '` (공백 한 칸)으로 설정하세요. Explorer는 이를 비어 있는 것으로 처리하고 플레이스홀더 텍스트를 다시 활성화합니다. UI 함수는 매 프레임 실행되므로, 한 프레임에만 지우기를 적용하려면 플래그를 사용하세요.

```tsx
import { UiEntity, Input, Button, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

let inputText = ''
let clearInput = false

export const uiMenu = () => {
  const inputValue = clearInput ? ' ' : ''
  if (clearInput) clearInput = false

  return (
    <UiEntity
      uiTransform={{
        width: 400,
        height: 300,
        positionType: 'absolute',
        position: {
          left: '35%',
          top: '40%',
        },
        flexDirection: 'column',
      }}
      uiBackground={{
        color: Color4.Gray(),
      }}
    >
      <Input
        onSubmit={() => {
          console.log('제출된 값: ' + inputText)
          inputText = ''
          clearInput = true
        }}
        fontSize={35}
        placeholder={'무언가를 입력하세요'}
        placeholderColor={Color4.Black()}
        value={inputValue}
        onChange={(value) => { inputText = value }}
        uiTransform={{
          height: '80px',
          margin: '15px',
        }}
      />
      <Button
        value="텍스트 제출"
        variant="primary"
        uiTransform={{ alignSelf: 'center', padding: '25px' }}
        onMouseDown={() => {
          console.log('제출된 값: ' + inputText)
          inputText = ''
          clearInput = true
        }}
      />
    </UiEntity>
  )
}
```

텍스트 필드의 모양을 사용자 지정하기 위해 다음 속성도 사용할 수 있으며, 대부분은 다음에 있는 속성과 비슷합니다: `Label` 개체:

* `placeholder`: 플레이어가 아무것도 입력하기 전에 표시할 문자열입니다. 이 텍스트를 작성해야 할 내용에 대한 힌트로 사용하는 것이 유용합니다.
* `placeholderColor`: 플레이스홀더 텍스트에 사용할 색상으로, [Color4](/docs/creator/content-creator-ko/sdk7/3d/color-types/).


> [!NOTE]
> **💡 팁**: 플레이어가 입력한 텍스트 색상보다 더 옅은 색조를 사용하세요.


* `fontSize`: 텍스트 크기를 숫자로 지정합니다.

  > 참고: `fontSize` 는 해당 개체나 부모 개체의 크기에 영향을 받지 않습니다.
* `색상`: 플레이어가 입력한 텍스트의 색상으로, [Color4](/docs/creator/content-creator-ko/sdk7/3d/color-types/).


> [!WARNING]
> **📔 참고**: 다음과는 다른 색상을 사용하세요. `placeholderColor`.


* `글꼴`: 사용할 글꼴입니다. 지원되는 값은 다음과 같습니다: - `'sans-serif'` *(기본값)* - `'serif'` - `'monospace'`
* `fontSrc`: 다음에 대한 경로 `.ttf` 글꼴 파일입니다. 씬에서 사용자 지정 글꼴로 텍스트를 렌더링하는 데 사용됩니다. `글꼴` 를 대체로 사용합니다. 다음을 참조하세요. [사용자 지정 글꼴](/docs/creator/content-creator-ko/sdk7/2d-ui/ui_text/#custom-fonts).
* `textAlign`: 텍스트가 부모와 어떻게 정렬될지입니다. \`TextAlignType\`의 값을 사용합니다. TextAlignType = 'top-left' | 'top-center' | 'top-right' | 'middle-left' | 'middle-center' | 'middle-right' | 'bottom-left' | 'bottom-center' | 'bottom-right';

다른 구성 요소도 구성할 수 있습니다. `입력` 개체, 예를 들어 `uiTransform`, `OnMouseDown` 다른 UI 개체에서처럼.

```tsx
import { UiEntity, Input, Label, ReactEcs } from '@dcl/sdk/react-ecs'
import { Color4 } from '@dcl/sdk/math'

var currentTextString = ''

export const uiMenu = () => (
    <UiEntity
      uiTransform={{
        width: '50%',
        height: 150,
        flexDirection: 'column',
        alignContent: 'flex-start',
        margin: { left: 20, top: 20 },
        padding: { left: 10, top: 10, right: 10 },
        alignSelf: 'center',
      }}
      uiBackground={{ color: Color4.Gray() }}
    >
      <Input
        onChange={(e) => {
          currentTextString = '입력한 내용: ' + e
        }}
        fontSize={35}
        placeholder={'무언가를 입력하세요'}
        placeholderColor={Color4.Gray()}
      />
      <Label value={currentTextString} fontSize={40} />
    </UiEntity>
)

```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/2d-ui/ui_special_types.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
