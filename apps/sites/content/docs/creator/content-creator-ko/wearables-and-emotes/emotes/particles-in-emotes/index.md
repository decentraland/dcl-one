> Armature를 사용해 이모트에서 파티클을 내보내는 방법

# 이모트의 파티클

이모트 2.0에는 사운드와 소품을 넣을 수 있지만, 파티클도 사용할 수 있다는 사실을 알고 계셨나요? 파티클은 정말 재미있을 수 있고, 여기에 한층 더 특별한 시각적 효과를 더해 줍니다! 게다가 애니메이션 제작 과정을 더 빠르게 해 주기도 합니다.


> [!NOTE]
> **💡 팁**: 다음을 설치하세요 [Decentraland Tools Blender 플러그인](https://extensions.blender.org/add-ons/decentraland-tools/). 여기에는 3D 모델, 웨어러블, 이모트를 편집하고 내보내는 데 도움이 되는 여러 유용한 기능이 포함되어 있습니다.


하지만 오브젝트 애니메이션과 파티클 시스템은 이모트 2.0에서 지원되지 않기 때문에, 이모트에 추가하려면 이 파티클들을 아마추어(지원되는 형식)로 변환해야 합니다. 이 과정에서는 재단의 콘텐츠 팀이 개발한 Blender 애드온을 사용할 텐데, 아래 링크에서 다운로드할 수 있습니다.

다음을 사용하세요 [Blender용 Particles to Bones 애드온](https://github.com/decentraland/docs/blob/main/creator/images/emotes/particle_system_to_bone_animation.py) 부터 시작하세요!

![](/docs/_remote/8a0401cd85e912317d2b0e9e.webp) ![](/docs/_remote/86f127e163391ec5c9cc99a0.webp)

이모트에 파티클을 추가하는 과정은 두 단계로 이루어지며, 아래 영상과 함께 가이드를 따라 하시면 됩니다:

* **파티클을 아마추어로 변환하기**

[![Video Preview](/docs/_remote/26ae9b1db5276e225497fc37.webp)](https://vz-8a0704eb-552.b-cdn.net/bb51a195-4a80-48d3-badd-0bfe6cc9e3b3/playlist.m3u8)

* **파티클 아마추어를 소품 아마추어에 병합하기**

[![Video Preview](/docs/_remote/5a12aec98b02dff46c6ea7a5.webp)](https://vz-8a0704eb-552.b-cdn.net/9f512c34-ae83-42fe-a858-165ccea6d0c5/playlist.m3u8)


> [!NOTE]
> **주의!**
> 
> ⚠️ 파티클 시스템을 만드는 방법은 이 문서에서 다루지 않습니다. 목적은 이를 여러분의 이모트에 구현하는 데 도움을 주는 것입니다.


## 파티클을 아마추어로 변환하기

앞서 언급했듯이, 이모트 2.0은 오브젝트 애니메이션이나 어떤 형태의 파티클 시스템도 지원하지 않습니다. 그래서 파티클을 아마추어로 변환해야 합니다.

시작하려면 작동하는 파티클 시스템과 애드온이 필요합니다. 아래에 표시된 파티클 시스템은 아주 단순한 예시일 뿐이며, 애드온이 어떻게 작동하는지 보여 주기 위한 것입니다. 더 나은 결과를 위해 파티클 시스템이 여러분의 이모트 애니메이션과 일치하는지 확인하세요.

![](/docs/_remote/221d5be3906962eace9e413c.webp)

*단순한 파티클 시스템.*

다음으로 애드온을 다운로드하고 설치합니다. 그러려면 Edit > Preferences > Add-ons로 이동한 뒤 Install을 클릭하여 컴퓨터에서 파일을 찾아보세요.

![](/docs/_remote/7494e551d315ebd81e317174.webp)

*Edit > Preferences를 클릭하세요.*

![](/docs/_remote/455940b9766bf96f12df39e3.webp)

*오른쪽에서 Add-ons를 선택하고 Install을 클릭하세요.*

완료되면 체크박스를 클릭하여 활성화하세요. Viewport 오른쪽에 Converter라는 이름의 탭이 나타납니다.

![](/docs/_remote/4a04786944a55dc6ea9f1b36.webp)

*체크박스를 클릭하여 애드온을 활성화하세요.*

![](/docs/_remote/c89bb19680957280f6fbecbf.webp)

*Viewport 오른쪽에서 Converter 탭을 찾을 수 있습니다.*

다음으로 오브젝트를 선택하고 Converter 탭으로 이동한 뒤, 원하는 대로 Output Collection의 이름을 바꾸고, Start 및 End Frames를 설정한 다음 아래와 같이 Particle System to Armature Converter를 클릭하세요.

![](/docs/_remote/e7e1f6344eb6ec89ec8c469f.webp)

Outliner에 파티클 아마추어가 포함된 컬렉션이 생성된 것을 확인할 수 있습니다. 또한 각 오브젝트의 모든 움직임을 베이킹하여 애니메이션 클립도 생성되었습니다.

![](/docs/_remote/d4067e87e6fe33342f19de70.webp)

*애드온으로 생성된 아마추어.*

![](/docs/_remote/b2b2e68b9a2d4d5dc47d2c10.webp)

*애드온으로 생성된 애니메이션 클립.*

이것으로 파티클을 아마추어로 변환하는 과정은 끝입니다! 파일은 다른 이름으로 저장하는 것을 잊지 마세요(원본은 백업으로 보관). 다음 단계를 위해 필요합니다.


> [!NOTE]
> **주의!**
> 
> ⚠️ 파티클은 이모트에 추가하는 마지막 요소여야 합니다. 이모트가 준비되면 애니메이션의 정확한 길이를 알 수 있으므로, 파티클을 수행 동작과 더 쉽게 동기화할 수 있습니다.


## 파티클 아마추어를 소품 아마추어에 병합하기

이모트 2.0은 파일당 아마추어 2개와 애니메이션 클립 2개만 허용합니다. 파티클을 그대로 가져오면 아마추어 3개와 애니메이션 클립 3개가 생기게 됩니다. 이를 해결하는 방법은 파티클 아마추어를 소품 아마추어에 병합하는 것입니다. avatar 아마추어가 아니라 prop에 병합하는 이유는 아주 간단합니다. 아바타 리그는 뼈를 추가하거나 삭제하는 방식으로는 어떤 식으로도 수정할 수 없기 때문입니다. 따라서 소품 아마추어만이 유일한 선택지입니다.

## 파티클 아마추어를 소품 아마추어에 병합하기

이모트 2.0은 파일당 아마추어 2개와 애니메이션 클립 2개만 허용합니다. 파티클을 그대로 가져오면 아마추어 3개와 애니메이션 클립 3개가 생기게 됩니다. 이를 해결하는 방법은 파티클 아마추어를 소품 아마추어에 병합하는 것입니다. avatar 아마추어가 아니라 prop에 병합하는 이유는 아주 간단합니다. 아바타 리그는 뼈를 추가하거나 삭제하는 방식으로는 어떤 식으로도 수정할 수 없기 때문입니다. 따라서 소품 아마추어만이 유일한 선택지입니다.

먼저 해야 할 일은 애드온으로 생성된 컬렉션을 가져오거나 추가(Append)하는 것입니다. 언제든지 glb 파일로 내보낸 뒤 이모트 파일로 가져올 수도 있습니다. 또는 더 쉽고 빠른 방법으로 컬렉션을 추가할 수도 있는데, 다음으로 이동하면 됩니다 ***File*** > ***Append***&#xB97C; 선택한 뒤 컴퓨터에서 파티클 파일을 찾아 ***Collection*** 폴더를 선택하고 원하는 컬렉션을 선택하세요.

![](/docs/_remote/9ca230b3c18bf89586ff9347.webp)

*컬렉션을 추가하려면 File > Append로 이동하세요.*

![](/docs/_remote/1f0417578e1b9a2dcae76f3e.webp)

*파티클 파일을 찾아본 뒤 Collection 폴더를 선택하세요.*

![](/docs/_remote/539369b8db786f772b98368b.webp)

*애드온으로 생성된 컬렉션(원하면 이름을 바꿀 수 있었던 것)을 선택하세요. 이 예시에서는 Particles\_Out으로 이름을 바꾼 것입니다.*

다음으로 아마추어를 병합합니다. Object Mode에서 먼저 파티클 아마추어를 선택한 다음 Shift를 누른 채 소품 아마추어를 선택하고, 아래와 같이 Ctrl+J를 눌러 둘을 결합/병합하세요.

![](/docs/_remote/c6adaaf3d6e33639b4ce3f4e.webp)

그렇게 하면 파티클이 이상하게 보일 수 있지만, 걱정하지 마세요. 이는 과정의 일부입니다. 이를 수정하려면 Outliner에서 각 파티클 오브젝트를 하나씩 선택하고 Object Properties(주황색 사각형 아이콘)로 이동한 다음 Relations 아래에서 parent가 Armature\_Prop인지 확인하세요. 모든 파티클 오브젝트에 대해 이 작업을 수행하세요. 즉, 모든 오브젝트가 Armature\_Prop의 자식이 되어야 합니다.

![](/docs/_remote/bed704b9a320a1befd208977.webp)

*모든 파티클 오브젝트의 부모가 Armature\_Pop인지 확인하세요.*

모든 파티클 오브젝트에 대해 이 작업을 한 뒤, Outliner에서 첫 번째 파티클 오브젝트를 선택하고 Modifiers(파란색 렌치 아이콘)로 이동하면 Object 필드가 비어 있을 것입니다. 이 탭이 Modifiers 탭이므로, 여기에서 어떤 아마추어가 해당 오브젝트를 제어할지 선택하게 됩니다. 따라서 Object를 클릭하고 Armature\_Prop를 선택한 다음, 이 작업을 모든 파티클 오브젝트에 대해 반복하세요. 이렇게 하면 파티클 오브젝트가 원래 크기로 돌아가며, 아마추어를 병합하면서 생긴 모든 문제가 해결되는 것을 볼 수 있습니다.

![](/docs/_remote/39489288235ee5b1b5748410.webp)

*오브젝트를 제어할 올바른 아마추어를 선택하세요.*

이제 아마추어가 올바르게 병합되고 설정되었으니, 작동시키기 위한 마지막 단계가 남았습니다. 애니메이션을 재생해 보면 파티클이 움직이지 않는 것을 알 수 있습니다. 그 이유는 이제 파티클이 Armature\_Prop의 일부가 되었고, 이 아마추어가 재생하는 애니메이션은 소품용이기 때문입니다. 파티클에는 자체 애니메이션 클립이 있었다는 것을 기억하세요. 이제 해야 할 일은 파티클 애니메이션 클립의 키프레임을 복사해서 소품 애니메이션 클립에 붙여 넣는 것입니다.

이를 위해 ***Pose Mode*** 로 이동한 뒤 Outliner에서 ***Armature\_Prop*** 를 확장하고 ***Pose***. 파티클에 속한 모든 뼈를 선택하세요.

![](/docs/_remote/10184a7f4c04c033592867a1.webp)

*모든 파티클 뼈를 선택하는 중.*

뼈를 선택한 상태에서 Action Editor에서 모든 키프레임이 들어 있는 파티클의 원본 애니메이션 클립을 선택하세요. A를 눌러 모든 키프레임을 선택한 다음 마우스 오른쪽 버튼을 클릭해 키프레임을 복사합니다. 그런 다음 소품 애니메이션을 선택하고 첫 프레임에 키프레임을 설정한 뒤 파티클 애니메이션 클립의 키프레임을 붙여 넣으세요.

![](/docs/_remote/b16528d9ab25ee97d3d824e0.webp)

*파티클 애니메이션에서 소품 애니메이션으로 프레임을 복사하는 중.*

이제 끝입니다! 파일을 저장하고, 원본 파티클 애니메이션 클립을 삭제한 뒤 평소처럼 내보내세요.

더 자세한 방법이 필요하면 ***내보내기*** 섹션을 [이모트에 소품과 사운드 추가하기](/docs/creator/wearables-and-emotes/emotes/props-and-sounds/) 문서에서 확인하세요.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/wearables-and-emotes/emotes/particles-in-emotes.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
