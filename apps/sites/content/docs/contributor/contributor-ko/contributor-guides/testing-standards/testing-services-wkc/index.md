# 서비스 테스트 (WKC)

잘 알려진 컴포넌트 서비스는 다음으로 구축된 서버입니다 [잘 알려진 컴포넌트](https://well-known-components.github.io/documentation/) 아키텍처로, 서비스의 각 부분이 모듈화되어 컴포넌트에 캡슐화되어 있습니다. 이러한 컴포넌트는 쉽게 교체할 수 있다는 점과 함께, 생성될 때 모든 의존성이 주입되므로 단위 테스트도 쉽게 할 수 있습니다.

이 서비스들은 두 가지 서로 다른 유형의 테스트를 사용해 테스트해야 하며, **단위** 및 **통합**. 로직을 포함하는 컴포넌트는 단위 테스트로 테스트해야 하며, 주로 외부 서비스와 상호작용하는 컴포넌트는 통합 테스트로 테스트해야 합니다. 즉, 어떤 로직도 포함하지 않고 주로 데이터베이스 쿼리만 포함하는 데이터베이스 어댑터 컴포넌트는 반드시 통합적으로 테스트해야 합니다.

## 잘 알려진 컴포넌트 컴포넌트의 단위 테스트

모든 컴포넌트는 단위 테스트를 거쳐야 하며, 테스트 작성 방법에 대해 여기에서 정의된 섹션을 따라 작성되어야 합니다.

이러한 컴포넌트가 어떻게 테스트되는지 보여주기 위해, 메서드 하나와 의존성 두 개만 가진 간단한 컴포넌트를 보겠습니다, **settings** 및 **friends**:

```tsx
// 음성 컴포넌트

export function createVoiceComponent(dependencies: Pick<AppComponents, 'settings' | 'friends'>) {
	const { settings, friends } = dependencies

  async function canCallEachOther(caller: string, callee: string): Promise<boolean> {
	  const [callerSettings, calleeSetting] = Promise.all([settings.getPrivacySetting(caller), settings.getPrivacySetting(callee)])
	  const areFriends = await friends.areUsersFriends(caller, callee)
		return areFriends || (callerSettings !== Settings.ONLY_FRIENDS && calleeSettings !== Settings.ONLY_FRIENDS)
  }
  
  return {
	  createVoiceChat
  }
}
```

이러한 컴포넌트를 테스트하려면 올바른 모의 객체를 만들어야 합니다. 즉, 테스트할 컴포넌트와 인터페이스를 맞출 수 있는 올바른 타입을 가진 모의 객체여야 하며, 모든 종류의 테스트를 만들 수 있도록 유연해야 합니다. 또한 이러한 모의 객체는 테스트 간에 변경되지 않아야 하며, 각 테스트가 다른 테스트와 격리된 자신만의 컨텍스트에서 실행된다는 아이디어를 유지해야 합니다.

이러한 모의 객체는 컴포넌트 모의 생성 함수를 사용해 만들어야 하며 이름은 create**ComponentName**MockedComponent로 해야 합니다. 위에서 만든 컴포넌트를 예로 들면, 다음과 같이 되어야 합니다:

```tsx
// /test/mocks/settings.ts 파일에 배치
export function createSettingsMockedComponent(overrides?: Partial<jest.Mocked<ISettingsComponent>>): jest.Mocked<ISettingsComponent> {
  return {
    getPrivacySettings: overrides?.getPrivacySettings ?? jest.fn()
  }
}

// test/mocks/friends.ts 파일에 배치
export function createFriendsMockedComponent(overrides?: Partial<jest.Mocked<IFriendsComponent>>): jest.Mocked<IFriendsComponent> {
	return {
	  // 컴포넌트가 내보내는 어떤 메서드에 대해서도 동작을 정의할 수 있는 유연성이 있습니다.
	  areUsersFriends: overrides?.areUsersFriends ?? jest.fn(),
	  // 사용하지 않는 메서드는 overrides 매개변수에서 제외할 수 있으며, 그렇게 하면 단순한 함수 모의가 됩니다.
	  hasBlockedUser: overrides?.hasBlockedUser ?? jest.fn()
	}
}
```

이러한 모의 객체를 `beforeEach` 안에 신중하게 배치해 사용하면, 서로 다른 실행에서 모의 객체가 올바르게 격리되어 정의된다는 것을 확신할 수 있습니다. 다음은 이러한 모의 객체를 사용하는 예시입니다:

```tsx
// 모의 함수를 컨텍스트에서 정의하여 나중에 그 모의를 변경할 수 있게 합니다.
let getPrivacySettingsMock: jest.MockedFn<ISettingsComponent['getPrivacySettings']>
let areUsersFriendsMock: jest.MockedFn<IFriendsComponent['areUsersFriends']>
let voice: IVoiceComponent

beforeEach({
  getPrivacySettingsMock = jest.fn()
  areUsersFriendsMock = jest.fn()
  // 모의를 만듭니다
  const settings = createSettingsMockedComponent({ getPrivacySettings: getPrivacySettingsMock })
  // 테스트에서 사용될 메서드만 포함해 컴포넌트 모의를 초기화합니다
  const friends = createFriendsMockedComponent({ areUsersFriends: areUsersFriendsMock })
  // 모의를 사용해 테스트할 컴포넌트를 생성합니다
  voice = createVoiceComponent({ settings, friends })
})
```

모의를 이렇게 정의하는 방식은 실행 간 테스트 컨텍스트를 올바르게 격리할 수 있게 해줄 뿐만 아니라, 모의 함수가 이미 정의되어 있으므로 각 컨텍스트를 쉽게 구성할 수 있게 하여 적절한 컨텍스트 초기화의 기반도 마련해 줍니다:

```tsx
// 이전 코드 블록 뒤에 정의됩니다.

describe('두 사용자가 서로 통화할 수 있는지 확인할 때', () => {
	const calleeAddress = '0xd7D746d39D142b6bE752efd7626cE28F245a25D1'
	const callerAddress = '0x2e8b4De1230f827082202aa53d489A26163aace0'

  describe('수신자가 친구에게서만 통화를 허용하는 경우', () => {
    beforeEach(() => {
      // 컨텍스트에 대한 모의를 정의하여 수신자가 
      getPrivacySettingsMock.mockImplementation((address: string) => {
	      switch(address) {
	        case caleeAddress:
		        return Settings.ONLY_FRIENDS
		      case callerAddress:
			      return Settings.ALL
			    default:
				    throw new Error("잘못된 모의 객체")
	      }
      })
    })
    
    describe('수신자와 발신자가 친구인 경우', () => {
	    beforeEach(() => {
		    areUsersFriendsMock.mockResolvedValueOnce(true)
	    })
	    
	    it('true로 해결되어야 합니다', () => {
		    return expect(voice.canCallEachOther(callerAddress, calleeAddress)).resolves.toBe(true)
	    })
    })
    
    describe('수신자와 발신자가 친구가 아닌 경우', () => {
		  beforeEach(() => {
		    areUsersFriendsMock.mockResolvedValueOnce(false)
	    })
	    
	    it('false로 해결되어야 합니다', () => {
		    return expect(voice.canCallEachOther(callerAddress, calleeAddress)).resolves.toBe(false)
	    })
    })
  })
  
  // ... 다른 컨텍스트들
})
```

## 통합 테스트

모든 엔드포인트, WS RPC 호출, 잡 또는 태스크는 통합적으로 테스트되어야 하며, 테스트 작성 방법에 대해 여기에서 정의된 섹션을 따라 작성되어야 합니다. 통합 테스트는 `test/integration` 디렉터리 아래에 배치되어야 하며, 테스트하려는 엔트리 포인트에 맞게 이름을 지어야 합니다.

통합 테스트는 단위 테스트로는 테스트할 수 없는 특정 통합 조건(SQL 쿼리, Redis 작업 등)이나, 우리 로직 컴포넌트와 프로토콜(HTTP, WS) 간의 통합을 테스트하는 데만 제한되어야 합니다.

통합 테스트를 어떻게 작성해야 하는지 예시를 보여주기 위해, 사용자의 친구를 가져오는 간단한 HTTP 요청을 테스트해 보겠습니다.

```tsx
export async function getFriendHandler(
  context: Pick<
    HandlerContextWithPath<'logs' | 'communities', '/v1/users/:user/friends'>,
    'url' | 'components' | 'params' | 'verification'
  >
): Promise<HTTPResponse<AggregatedCommunityWithMemberData>> {
  const {
    components: { friends },
    params: { id },
    verification
  } = context

  try {
    const userAddress = verification!.auth.toLowerCase()

    return {
      status: 200,
      body: {
        data: await friends.getFriends(userAddress)
      }
    }
  } catch (error) {
  
	  if (error instanceof InvalidFriendshipRequest) {
			throw InvalidRequest(isErrorWithMessage(error) ? error.message : '알 수 없는 오류')
	  }

    return {
      status: 500,
      body: {
        message
      }
    }
  }
}
```

그리고 friends 컴포넌트는 다음을 가집니다:

```tsx
async function getFriends(addres: string) {
	friendsDb.getFriends()
}
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-ko/contributor-guides/testing-standards/testing-services-wkc.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
