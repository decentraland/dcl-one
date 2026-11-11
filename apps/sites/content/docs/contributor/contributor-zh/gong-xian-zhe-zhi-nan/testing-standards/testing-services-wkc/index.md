# 测试服务（WKC）

众所周知的组件服务是使用以下方式构建的服务器 [众所周知的组件](https://well-known-components.github.io/documentation/) 架构，其中服务的每个部分都被模块化并封装在一个组件中。这些组件以及易于互换的能力，还具备易于进行单元测试的能力，因为它们的所有依赖项都会在创建时被注入。

测试这些服务应使用两种不同类型的测试， **单元** 和 **集成**。包含逻辑的组件必须使用单元测试进行测试，而主要与外部服务交互的组件应使用集成测试进行测试。也就是说，一个不包含任何逻辑、且主要包含数据库查询的数据库适配器组件必须通过集成测试进行测试。

## 众所周知的组件的单元测试

所有组件都必须进行单元测试，并且必须按照此处定义的关于如何编写测试的部分来编写。

为了展示如何测试这些组件，这里有一个只有一个方法和两个依赖的简单组件， **settings** 和 **friends**:

```tsx
// 语音组件

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

为了测试这些服务，必须构建合适的 mock，也就是说，这些 mock 必须具有与被测试组件交互所需的正确类型，并且必须足够灵活，以便我们创建所有类型的测试。这些 mock 也应当在不同测试之间保持不可变，从而体现每个测试都在其自己的上下文中执行，并与其他测试隔离。

这些 mock 应该使用组件 mock 创建函数来创建，并且应该命名为 create**组件名称**MockedComponent。以上面创建的组件为例，以下是它们应该如何显示的示例：

```tsx
// 放在 /test/mocks/settings.ts 文件中
export function createSettingsMockedComponent(overrides?: Partial<jest.Mocked<ISettingsComponent>>): jest.Mocked<ISettingsComponent> {
  return {
    getPrivacySettings: overrides?.getPrivacySettings ?? jest.fn()
  }
}

// 放在 test/mocks/friends.ts 文件中
export function createFriendsMockedComponent(overrides?: Partial<jest.Mocked<IFriendsComponent>>): jest.Mocked<IFriendsComponent> {
	return {
	  // 我们可以灵活地为组件导出的任意方法定义行为。
	  areUsersFriends: overrides?.areUsersFriends ?? jest.fn(),
	  // 未使用的方法可以在 overrides 参数中省略，使其成为一个简单的函数 mock。
	  hasBlockedUser: overrides?.hasBlockedUser ?? jest.fn()
	}
}
```

通过将这些 mock 谨慎地放在一个 `beforeEach`中使用，我们可以确保这些 mock 在不同执行之间被正确隔离。下面是这些 mock 的使用示例：

```tsx
// 在上下文中定义 mock 函数，以便我们可以修改其 mock。
let getPrivacySettingsMock: jest.MockedFn<ISettingsComponent['getPrivacySettings']>
let areUsersFriendsMock: jest.MockedFn<IFriendsComponent['areUsersFriends']>
let voice: IVoiceComponent

beforeEach({
  getPrivacySettingsMock = jest.fn()
  areUsersFriendsMock = jest.fn()
  // 构建 mock
  const settings = createSettingsMockedComponent({ getPrivacySettings: getPrivacySettingsMock })
  // 仅使用测试中将会用到的方法来初始化组件 mock
  const friends = createFriendsMockedComponent({ areUsersFriends: areUsersFriendsMock })
  // 使用这些 mock 创建待测试的组件
  voice = createVoiceComponent({ settings, friends })
})
```

这种定义 mock 的方式不仅允许我们在不同执行之间正确地隔离测试上下文，而且也为正确的上下文初始化打下了基础，因为 mock 函数已经定义好，所以构建每个上下文会非常容易：

```tsx
// 定义在前面的代码块之后。

describe('当检查两个用户是否可以互相通话时', () => {
	const calleeAddress = '0xd7D746d39D142b6bE752efd7626cE28F245a25D1'
	const callerAddress = '0x2e8b4De1230f827082202aa53d489A26163aace0'

  describe('且被叫方只接受来自朋友的通话时', () => {
    beforeEach(() => {
      // 为上下文定义 mock，使被叫方 
      getPrivacySettingsMock.mockImplementation((address: string) => {
	      switch(address) {
	        case caleeAddress:
		        return Settings.ONLY_FRIENDS
		      case callerAddress:
			      return Settings.ALL
			    default:
				    throw new Error("错误的 mock")
	      }
      })
    })
    
    describe('且被叫方和呼叫方是朋友时', () => {
	    beforeEach(() => {
		    areUsersFriendsMock.mockResolvedValueOnce(true)
	    })
	    
	    it('应解析为 true', () => {
		    return expect(voice.canCallEachOther(callerAddress, calleeAddress)).resolves.toBe(true)
	    })
    })
    
    describe('且被叫方和呼叫方不是朋友时', () =>{
		  beforeEach(() => {
		    areUsersFriendsMock.mockResolvedValueOnce(false)
	    })
	    
	    it('应解析为 false', () => {
		    return expect(voice.canCallEachOther(callerAddress, calleeAddress)).resolves.toBe(false)
	    })
    })
  })
  
  //……其他上下文
})
```

## 集成测试

所有端点、WS RPC 调用、作业或任务都必须进行集成测试，并且必须按照此处定义的关于如何编写测试的部分来编写。集成测试必须放在 `test/integration` 目录下，并且必须根据我们要测试的入口点命名。

集成测试应仅限于测试那些无法通过单元测试来测试的特定集成条件（SQL 查询、Redis 操作等），以及我们的逻辑组件与协议（HTTP、WS）之间的集成。

为了展示如何编写集成测试，我们将测试一个获取用户朋友列表的简单 HTTP 请求。

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
			throw InvalidRequest(isErrorWithMessage(error) ? error.message : '未知错误')
	  }

    return {
      status: 500,
      body: {
        消息
      }
    }
  }
}
```

还有一个 friends 组件如下：

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
GET https://docs.decentraland.org/contributor/contributor-zh/gong-xian-zhe-zhi-nan/testing-standards/testing-services-wkc.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
