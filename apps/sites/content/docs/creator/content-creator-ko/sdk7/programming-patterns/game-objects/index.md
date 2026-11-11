> 코드를 더 읽기 쉽고 확장하기 쉽게 만들기 위해 게임 오브젝트 패턴을 사용하세요.

# 게임 오브젝트

씬이 더 복잡해질수록, 로직의 일부를 별도의 게임 오브젝트 파일로 분리해 두는 것이 유용합니다. 이렇게 하면 씬의 मुख्य 코드를 깔끔하고 읽기 쉽게 유지하는 동시에, 씬 안의 여러 엔티티를 제어하는 재사용 가능한 부분을 캡슐화할 수 있습니다.

게임 오브젝트는 문이나 버튼, 몬스터처럼 씬에서 볼 수 있는 어떤 유형의 엔티티에 대한 모든 속성과 메서드를 담고 있습니다. 이러한 추상화 덕분에 씬의 मुख्य 로직이 담긴 파일에서는 단 한 줄의 코드만으로 완전한 게임 오브젝트를 인스턴스화할 수 있습니다. 또한 같은 방식으로 이러한 오브젝트의 메서드를 호출할 수도 있습니다.

게임 오브젝트 정의는 각각 별도의 파일에 두는 것을 권장합니다.

## 타입 오브젝트 패턴

씬에서 공통점이 많은 다양한 유형의 게임 오브젝트를 사용한다면, 이 리팩터링을 한 단계 더 나아가 *base* 이들 사이의 공통 요소를 모두 담은 게임 오브젝트 타입을 정의할 수 있습니다. 그런 다음 이 기본 클래스를 상속하는 하위 클래스를 만들 수 있습니다.

이 패턴에 대해 더 자세히 알아보려면 [게임 프로그래밍 패턴](http://www.gameprogrammingpatterns.com/type-object.html).


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/creator/content-creator-ko/sdk7/programming-patterns/game-objects.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
