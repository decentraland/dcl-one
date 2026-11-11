> 이모트에 소품과 사운드를 추가하는 가이드라인.

# 소품 및 사운드

Decentraland 이모트를 한 단계 끌어올리려면 프롭(3D 지오메트리) 및/또는 사운드를 추가할 수 있으며, 이를 통해 이모트를 훨씬 더 재미있고 몰입감 있게 만들 수 있습니다! 이 가이드에서는 올바르게 내보내기 위해 알아야 할 모든 내용을 확인할 수 있습니다!


> [!NOTE]
> **💡 팁**: 다음을 설치하세요 [Decentraland Tools Blender 플러그인](https://extensions.blender.org/add-ons/decentraland-tools/). 여기에는 3D 모델, 웨어러블 및 이모트를 편집하고 내보내는 데 유용한 여러 기능이 포함되어 있습니다.


## **기본 사항과 제한 사항**

이모트에 프롭을 추가하려면 먼저 [Decentraland 템플릿 파일](https://raw.githubusercontent.com/decentraland/documentation-creators/main/images/emotes/BaseMale_Rig_1.0.blend) 을 사용하는 것이 중요합니다. 여기에는 아바타의 리그와 함께 작업을 허용된 공간 경계 안에 유지하기 위한 Ground Reference가 포함되어 있습니다.

**현재 프롭 애니메이션은 Armature/Bones 애니메이션에서만 작동하므로 \_transform 애니메이션**\_\*\*는 허용되지 않습니다.\*\*

이모트와 그 프롭은 모두 하나의 단일 *.glb* 파일(Avatar\_Armature + 애니메이션이 포함된 Props\_Armature)로 함께 내보내야 합니다.

* 총 3MB를 초과할 수 없습니다.
* 프롭의 총 트라이 수는 3천 개를 초과할 수 없습니다.
* 프롭에 사용할 재질은 2개, 텍스처는 2개를 초과할 수 없습니다.
* 프롭 아마추어의 본은 62개를 초과할 수 없습니다.
* 이모트에는 아바타용 애니메이션 하나와 프롭용 애니메이션 하나가 있어야 합니다. *현재는 여러 애니메이션이 허용되지 않습니다.*
* 두 애니메이션(아바타와 프롭)은 동일한 키프레임 길이를 가져야 합니다.
* 애니메이션은 300프레임 또는 10초를 초과할 수 없습니다.
* 공간 경계는 4제곱미터입니다. 프롭과 파티클은 Avatar File에서 제공되는 기준 큐브 안에 있어야 합니다. 아바타 움직임에 대해서는 [지면 기준 및 애니메이션 영역](/docs/creator/content-creator-ko/wearables-and-emotes/emotes/creating-emotes/#ground-reference-and-animation-area). 섹션을 확인하세요.

## **명명 규칙:**

이모트가 작동하려면 명명 규칙을 엄격히 따라야 합니다! 그렇지 않으면 빌더에서도 월드에서도 올바르게 재생되지 않습니다.

#### 아마추어 이름 규칙:

**아바타의 경우:**

`아마추어`

**프롭의 경우:**

`Armature_Prop`

#### 애니메이션 이름 규칙:

**아바타의 경우:**

`AnimationName_Avatar`

* 예: `TennisServe_Avatar`, `GunShoot_Avatar`

**프롭의 경우:**

`AnimationName_Prop`

* 예: `TennisServe_Prop`, `GunShoot_Prop`

## **시작하기**

애니메이션을 시작하기 전에 프롭용 리그를 만들어야 합니다. 절차에 익숙하지 않다면 [리그 만들기](/docs/creator/content-creator-ko/3d/create-a-rig/) 자세한 방법은 여기에서 확인하세요.

프롭 오브젝트와 아마추어의 원점이 Blender 내 0,0 위치에 있도록 하세요. 또한 프롭 오브젝트와 아마추어에 변환을 적용하여 스케일이 1,1,1로 고정되도록 하세요. 이는 월드에서 사용되거나 애니메이션 중에 프롭의 동작에 잠재적인 문제가 생기는 것을 방지하는 데 매우 중요합니다.

#### 프롭을 아바타 리그에 따라 움직이게 만들기

일부 프롭은 테니스 라켓처럼 손과 같은 특정 신체 부위에 부착해야 할 수 있습니다. 이는 단순히 제약을 추가하면 됩니다. 그렇게 하려면 ***포즈 모드***, 프롭 본(예: 테니스 라켓 본)을 선택하고 ***CTRL + Shift + C*** 를 키보드에서 누른 다음 ***자식 대상*** 또는 그냥 ***본 제약 속성*** 탭을 클릭하고 드롭다운 메뉴에서 ***자식 대상***.

*제약을 추가하려면 `Ctrl + Shift + C` 를 키보드에서 누르세요.*

그런 다음, ***타깃***, 아바타 아마추어를 선택하고 ***본*** 프롭이 따라가게 할 본을 선택하세요. 프롭의 원래 위치를 유지하려면 ***Set Inverse*** 를 제약을 추가한 후 한 번 클릭하세요. 영향도가 1이면 프롭이 선택한 본을 완전히 따라가고, 0이면 제약이 비활성화됩니다. 애니메이션 내내 켜고 끄려면 영향도에 키프레임을 설정할 수 있습니다. 그렇게 하려면 커서가 ***Influence***.

***자식 대상** 제약 메뉴에서. 영향도에 키프레임을 설정해 켜고 끄세요.*


> [!NOTE]
> **💡애니메이션 팁!** 슬라이더로 Influence를 끄면 프롭이 이전 위치를 유지하지 못해 애니메이션을 부드럽게 유지하기 어렵습니다. 위치를 수동으로 고치지 않으려면 슬라이더 대신 Influence 옆의 X를 클릭하고, 그 항목에 키프레임을 설정한 다음 모든 변환 속성에도 하나씩 키프레임을 설정하세요. 이렇게 하면 프롭이 Influence가 켜져 있을 때와 같은 위치를 유지합니다!



> [!NOTE]
> **💡애니메이션 팁**
> 
> 프롭을 처음부터 보이게 두지 마세요! 다음에 일어날 일을 미리 보여주거나 갑작스러운 전환이 생기는 것을 피하려면 애니메이션을 프롭 스케일을 0.001로 줄인 상태로 시작하고, 나타나게 하고 싶을 때만 1로 바꾸세요. 작업이 끝날 때는 다시 0으로 줄이는 것을 잊지 마세요. 이렇게 하면 전환이 훨씬 더 부드럽고 멋져집니다!


### 애니메이션 슬롯

Blender 4.4는 새로운 기능인 애니메이션 슬롯을 도입했습니다. Blender 문서에 따르면 “슬롯의 목적은 액션이 여러 데이터 블록에 대한 서로 다른 애니메이션 데이터를 저장할 수 있게 하는 것입니다”. 요약하면, 슬롯을 사용하면 같은 액션 안에 여러 항목의 애니메이션을 저장할 수 있습니다. 이것이 Emotes 2.0에 어떤 영향을 줄까요?

Blender 4.4의 새로운 기능: 애니메이션 슬롯.

아바타와 프롭이 같은 액션 클립을 공유하게 하는 것은 가능하지만, Emotes 2.0의 명명 규칙과 관련된 애니메이션 클립 수 때문에 작동하지 않습니다. 따라서 이 작업 흐름은 다음과 같습니다:

1. 아바타용 애니메이션 클립을 만들거나 제공된 것을 이름 변경하세요 (***Starting\_Pose***). 이미 애니메이션 슬롯이 있지만, 자유롭게 사용해도 됩니다 (***Avatar\_Animation***) 또는 새로 만들 수 있습니다.
2. 애니메이션 클립의 이름을 변경하세요 ***AnimationName\_Avatar***
3. 프롭용 애니메이션 클립을 만들고 이름을 변경하세요 ***AnimationName\_Prop***
4. 다음을 클릭하세요 ***New*** 버튼을 눌러 해당 애니메이션 슬롯을 만드세요(자동 이름이 부여됩니다: ***Armature\_Prop***)
5. 이전 Blender 버전에서 하듯이 애니메이션을 만드세요.

프롭 애니메이션용 액션 클립과 슬롯 만들기.

## **NLA 트랙**

모든 애니메이션이 내보내지려면 클립을 NLA 트랙에 추가해야 합니다. 아바타용 애니메이션 클립 하나와 프롭용 애니메이션 클립 하나만 있도록 하세요, **그리고 두 클립은 정확히 같은 프레임 수를 가져야 합니다.**

에서 ***오브젝트 모드***, 아바타 아마추어를 선택하고 ***포즈 모드***, Browse Action 메뉴에서 해당 애니메이션 클립을 선택한 다음 ***액션*** 을 클릭하고 그다음 ***아래로 내리기*** 옵션.

을 클릭하세요. ***오브젝트 모드***&#xADF8;런 다음 다시 ***, 프롭 아마추어를 선택하고 포즈 모드로 이동하세요.***, Browse Action 메뉴에서 해당 애니메이션 클립을 선택한 다음 ***액션*** 을 클릭하고 그다음 ***아래로 내리기*** 옵션.

액션을 NLA 트랙으로 내리기.


> [!WARNING]
> ⚠️ 액션을 내릴 때 주의하세요. 원하는 애니메이션이 적용된 원하는 아마추어를 선택했는지 확인하세요. 다른 아마추어를 선택하기 전에 단순히 애니메이션을 바꾸고 내리면 한 아마추어에는 두 개의 액션이 할당되고 다른 하나에는 아무것도 할당되지 않게 됩니다.


NLA 트랙은 이렇게 보여야 합니다: 아마추어마다 하나의 애니메이션.


> [!NOTE]
> **🔥 최적화 팁**
> 
> **이 단계 전에 프로젝트를 백업해 두세요.**
> 
> 프롭에 서로 다른 오브젝트가 있다면 하나의 메쉬로 합칠 수 있습니다. 오브젝트를 선택하고 단축키 ctrl + J를 누르면 됩니다.
> 
> 이렇게 하면 게임 내 드로우 콜 수를 줄여 이모트 성능이 더 좋아집니다.
> 
> 다만 파티클에는 적용되지 않습니다.
> 
> *오브젝트를 선택하고 `Ctrl+J` 를 눌러 하나로 합치세요.*


## **내보내기**

Emotes 2.0은 일반 이모트와 같은 방식으로 내보냅니다. 아바타 아마추어, 프롭 아마추어, 프롭 메쉬만 보이도록 하고 나머지는 모두 숨기세요.

내보내기에는 아바타 아마추어, 프롭 아마추어, 프롭 메쉬만 보이게 하세요.

내보내려면 File > Export > glTF2.0 (.glb, .gltf)로 이동하세요

내보내기 설정에서 Include를 확장하고 Limit to에서 Visible Objects를 활성화하세요. 그런 다음 Data 탭을 확장하고 Armature를 확장한 뒤 Export Deformation Bones Only를 활성화하세요.

|   |   |
| - | - |

Export를 누르면 완료됩니다!

## **이모트에 오디오 추가하기**

### 오디오 클립의 형식과 제한

* 이모트용 사운드를 내보내는 올바른 형식은 `.mp3` 및 `.ogg`.
* 오디오 클립은 이모트와 같은 재생 시간을 가져야 합니다.
* 오디오에는 크기 제한이 없지만, 프롭과 사운드가 포함된 이모트는 3mb를 초과할 수 없습니다.


> [!NOTE]
> **📔 참고**: 이모트에 사운드(mp3 또는 ogg)가 있으면 .glb와 함께 zip으로 묶어야 합니다. 그다음 .zip을 빌더에 드래그 앤 드롭하기만 하면 됩니다. 자세한 내용은 여기에서 확인할 수 있습니다: [사운드가 포함된 이모트 업로드](/docs/creator/content-creator-ko/wearables-and-emotes/manage-collections/uploading-emotes/#uploading-emotes-using-a-zip-file)



> [!NOTE]
> **💡 주의!** 이모트에 사용되는 오디오 클립은 원본 IP(Intellectual Property)여야 하며, 복제 권리를 보유하고 다음을 준수해야 한다는 점을 고려하세요 [콘텐츠 정책](https://decentraland.org/content/)기준.


### 사운드 편집

이모트에 사운드를 추가하는 방법은 여러 가지가 있습니다:

1. **Blender에서 사운드를 직접 편집하기**

이모트에 사운드를 추가하는 한 가지 방법은 Blender가 제공하는 비디오 시퀀서 편집기를 사용하는 것입니다.

사운드를 추가하려면 먼저 *편집기 유형 > 비디오 시퀀서.*

사운드를 채널 인터페이스로 드래그 앤 드롭하세요.

단축키 `N` 을 눌러 파형 표시, 사운드를 모노로 만들기, 볼륨 변경 등 사운드를 다루기 위한 더 많은 옵션을 확인하세요.


> [!NOTE]
> 페이드 인/아웃을 원하면 볼륨 속성에 0에서 1로, 또는 그 반대로 키프레임을 추가하면 됩니다.


사운드 편집을 마쳤다면 *렌더 > 오디오 렌더*. 내보내기 옵션에서 `.mp3` 또는 `.ogg` 형식을 *Container* 섹션에서 선택한 다음 *믹스다운*. **프레임 범위 내의 오디오만 내보내집니다.**

2. **애니메이션을 렌더링하고 사운드 편집 소프트웨어로 사운드 추가하기**

Blender에서 사운드를 직접 편집하는 것은 편리할 수 있지만, 이 소프트웨어는 주로 사운드 편집에 초점을 맞추고 있지 않기 때문에 그다지 유연하지 않습니다. 사용할 수 있는 도구도 매우 기본적입니다. 사운드에 더 전문적인 느낌을 더하고 싶다면, 원하는 전용 사운드 편집 소프트웨어를 사용하는 것을 추천합니다.

사용할 수 있는 여러 소프트웨어가 있으며, 예를 들면 [Audacity](https://www.audacityteam.org/) (무료 및 오픈 소스), Adobe Audition, Ableton Live, 또는 ProTools가 있습니다. 전용 사운드 편집 소프트웨어를 사용하면 더 넓은 범위의 도구, 기능, 사운드 효과를 사용할 수 있어 사운드를 향상시키고 더 전문적인 느낌을 줄 수 있습니다.

이모트를 렌더링하려면 Blender 씬에 카메라를 추가하고, 모든 요소가 최대한 명확하게 보이도록 위치를 잡아 나중에 사운드를 추가할 때 좋은 참고 자료가 되게 하면 됩니다.

이모트를 렌더링할 때는 이모트의 프레임 범위만 포함하고 그 이상은 포함하지 않는 것이 중요합니다. 필요에 맞는 종횡비를 선택하고, 비디오나 이미지 시퀀스를 저장할 출력 폴더를 선택하세요.


> [!NOTE]
> **힌트!**
> 
> *렌더링하기 전에 렌더 시간을 절약할 수 있도록 저샘플링 렌더링을 하세요!*


이 단계가 완료되면, 선호하는 사운드 편집 소프트웨어를 사용해 해당 사운드를 만들기 위한 참고 자료로 비디오를 사용하세요. **비디오 시퀀스가 애니메이션의 초당 30프레임(fps) 프레임레이트와 일치하는지 확인하세요**


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/wearables-and-emotes/emotes/props-and-sounds.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
