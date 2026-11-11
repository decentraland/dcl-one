> 아바타 리그의 기본 사항.

# 아바타 리그

리그는 모델이 움직일 수 있게 해주는 가상 골격이다. 실제 골격처럼 개별 뼈의 계층 구조로 이루어져 있으며, 부모/자식 관계로 작동한다. 이 문서에서는 뼈의 위치, 뼈의 방향, 변형 뼈와 비변형 뼈, IK와 FK의 차이와 그 목적 등 기본적인 리깅 개념을 다룬다. 아바타 리그의 구조, 사용자 정의 속성, 그리고 애니메이션을 위한 설정은 [리그 기능](/docs/creator/content-creator-ko/wearables-and-emotes/emotes/rig-features/).


> [!NOTE]
> **💡 팁**: 다음을 설치하세요 [Decentraland Tools Blender 플러그인](https://extensions.blender.org/add-ons/decentraland-tools/). 여기에는 3D 모델, 웨어러블, 이모트를 편집하고 내보내는 데 도움이 되는 여러 유용한 기능이 포함되어 있습니다.


## **기본 사항**

#### 뼈 위치 또는 피벗 포인트

리그가 인체 골격을 정확히 그대로 복제한 것은 아니지만, 디지털 뼈를 배치할 때는 실제 뼈의 위치를 따르는 것이 좋다. 이는 자연스럽고 부드러운 변형을 보장하기 위해서다. 뼈의 위치는 움직임이 시작되는 피벗 포인트를 정하기 때문에 중요하다. 잘못 배치된 뼈는 메시의 변형을 좋지 않게 만든다. 아래 이미지들은 뼈 위치의 차이를 보여준다.

![head\_pivot\_rig\_1.0.gif](/docs/_remote/19b528a3a24a1ffcdeaf8d5f.webp)

![head\_pivot\_rig\_2.0.gif](/docs/_remote/95fc62ca479c3f614187167d.webp)

#### 뼈 방향

뼈의 방향은 양의 X, 음의 Z 등 어느 방향으로 회전할지를 정의한다. 리그 전체에서 일관성만 유지된다면 여러 가지 방식으로 설정할 수 있다. 예를 들어 척추의 앞으로 굽히는 동작에 X축을 사용한다면, 다리에도 같은 설정을 사용하는 것이 타당하다. 방향은 미러링 동작에도 영향을 주기 때문에 중요하다. 즉 보통 오른쪽의 방향은 왼쪽의 미러 버전이고 그 반대도 마찬가지다.

![](/docs/_remote/4d39fc26ef0dd267cbd54630.webp)

*뼈 방향을 표시하려면 축을 켜세요.*

![](/docs/_remote/8278804deb8abeeac7cba8ee.webp)

*축은 뼈가 회전하는 방향이다.*

#### 변형 뼈와 비변형 뼈

변형 뼈는 메시를 변형시키는 뼈로, 모델이 움직이는 방식을 담당한다. 이것이 기본 골격이며 **절대 편집하면 안 된다**. 어떤 방식으로든 변경하면 리그가 손상될 수 있고, 내보낼 때 애니메이션이 작동하지 않을 것이다. 이런 이유로 이 기본 리그는 자체 레이어(맨 아래의 마지막 레이어)로 옮겨졌다. 이 뼈들은 메시와 함께 내보내지는 아마추어이다.

비변형 뼈는 메시를 변형시키지 않는 뼈이지만, 견고한 리그에서는 여전히 필요하며 설정에 사용된다 [IK, FK](#whats-fk-and-ik-in-a-rig) 및 기타 사용자 정의 속성에 사용된다. 비변형 뼈의 예로는 컨트롤, IK 및 FK 뼈, 발 설정 뼈가 있다. 이것들은 내보내면 안 되며, 애니메이션 용도로만 사용된다.

![](/docs/_remote/4587f7ead5c1973d96247db4.webp)

*변형 뼈.*

![](/docs/_remote/8c4d5b63d6775310aef4709b.webp)

*비변형 뼈.*


> [!WARNING]
> ⚠️ **주의!** **기본 골격은 절대 편집하지 마세요!**


![](/docs/_remote/7856fc2ef29a54224be410db.webp)

*기본 골격.*

#### 컨트롤

좋은 관행으로, 리그는 변형 뼈를 조작하여 애니메이션하지 않아야 한다. 리그가 손상될 수 있기 때문이다. 대신, **애니메이션은 컨트롤을 조작하여 수행해야 한다.**

컨트롤은 기본적으로 비변형 뼈로, 메시에 영향을 주지 않기 때문에 조작하고 애니메이션하기에 완전히 안전하다. 이들의 기능은 직접 건드리지 않고 제약과 드라이버를 통해 기본 골격을 제어하는 것이다. 보통 기능과 목적을 시각적으로 구분할 수 있도록 서로 다른 모양과 색상을 가지며, 애니메이터가 뼈를 구별하기 쉽게 해준다. 또한 IK/FK 전환 같은 사용자 정의 속성을 추가할 수 있기 때문에 위치, 회전, 스케일 변환 외의 속성도 가질 수 있다.

또한 원래 만든 소프트웨어와 다른 소프트웨어에서 컨트롤 설정을 사용하는 것은 불가능하다는 점도 중요하다. 각 소프트웨어는 고유한 로직을 가지고 있으며, 제약은 내보낼 수 없다.

![](/docs/_remote/66b97ad8bec99d92c8bba7a2.webp) \_컨트롤과 각각의 다른 모양과 색상.\_


> [!WARNING]
> ⚠️ **경고**: 리그는 반드시 생성된 동일한 소프트웨어에서 애니메이션해야 한다. 예를 들어 Blender 설정을 Maya에서 사용하거나 그 반대로 사용하는 것은 불가능하다.


#### 리그에서 FK와 IK는 무엇인가?

리그는 움직임에 영향을 주는 두 가지 서로 다른 설정을 사용할 수 있다: FK와 IK.

#### FK - 순방향 운동학

순방향 운동학(FK)에서는 계층 구조의 부모가 그 아래의 모든 자식 뼈를 움직인다. 팔을 예로 들면, 어깨가 회전하면 팔 전체도 함께 회전하고, 팔이 회전하면 아래팔도 그 움직임을 따라간다. FK로 애니메이션할 때는 각 뼈를 개별적으로 회전해야 한다. 이 설정은 움직임을 세밀하게 제어할 수 있으며, 유려한 애니메이션에 필수적인 아크 동작에 매우 적합하다.

*계층 구조에서의 움직임 방향.*

![](/docs/_remote/c8dc3dc16dcc58581a238041.webp)

*FK에서는 각 뼈를 개별적으로 회전해야 한다.*

#### IK - 역운동학

역운동학(IK)에서는 계층 구조의 자식이 부모의 움직임에 영향을 줄 수 있다. 이번에도 팔을 예로 들면, 손을 움직이면 팔의 나머지 부분이 그 움직임을 따라간다. 또한 어깨가 어떻게 움직이든 손의 위치는 유지된다는 뜻이기도 하다. 이 설정에서는 폴 벡터/폴 타깃이 뼈가 어느 방향으로 굽을지를 제어한다. 다리는 보통 IK로 설정되며, 애니메이션 중 발이 지면에 붙어 있도록 하는 데 필수적이다.

![](/docs/_remote/2137414a0b2253ea260c8f89.webp)

*계층 구조에서의 움직임 방향.*

![](/docs/_remote/9de7bae6abe4d41fe6467d0e.webp)

*IK에서는 손이 팔 전체를 움직이면서도 제 위치를 유지한다. 폴 타깃은 팔꿈치가 굽는 방향을 제어한다.*


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/wearables-and-emotes/emotes/avatar-rig.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
