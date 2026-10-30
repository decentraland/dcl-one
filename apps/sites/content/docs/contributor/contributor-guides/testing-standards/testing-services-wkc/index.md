# Testing Services (Well-Known-Component)

Well known component services are servers built with the [well known component](https://well-known-components.github.io/documentation/) architecture, where each part of the service is modularized and encapsulated in a component. These components, alongside the ability of being easy to be interchanged, have the ability of being easy to unit test, as all of their dependencies are injected when created.

Testing these services SHOULD be done using two different types of test, **unit** and **integration**. Components that contain logic MUST be tested using unit tests and components that have mostly interactions with external services SHOULD be tested using integrations tests. That is, a database adapter component which does not contain any logic and contains mostly database queries MUST be tested integratedly.

## Unit testing well known component components

All components MUST be unit tested and MUST be written following the section defined here on how to write tests.

To show how these component are tested, here's a simple component with only one method and two dependencies, **settings** and **friends**:

```tsx
// Voice component

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

In order to test this component, proper mocks must be built, that is, mocks that have the correct types to interface with the component to be tested and that are flexible to allow us to create all types of tests. These mocks should also be immutable between tests, preserving the idea that each test is executed in its own context, isolated from the others.

These mocks SHOULD be created using component mock creator functions and SHOULD be named as create**ComponentName**MockedComponent. Taking as an example the component created above, here's an example on how they SHOULD look like:

```tsx
// Placed in the /test/mocks/settings.ts file
export function createSettingsMockedComponent(overrides?: Partial<jest.Mocked<ISettingsComponent>>): jest.Mocked<ISettingsComponent> {
  return {
    getPrivacySettings: overrides?.getPrivacySettings ?? jest.fn()
  }
}

// Places in the test/mocks/friends.ts file
export function createFriendsMockedComponent(overrides?: Partial<jest.Mocked<IFriendsComponent>>): jest.Mocked<IFriendsComponent> {
	return {
	  // We have the flexibility to define behavior upon any of the methods exported by the component.
	  areUsersFriends: overrides?.areUsersFriends ?? jest.fn(),
	  // Methods that are not used can be avoided in the overrides parameter, making them a simple function mock.
	  hasBlockedUser: overrides?.hasBlockedUser ?? jest.fn()
	}
}
```

By using these mocks carefully placed inside a `beforeEach`, we can be sure that the mocks are defined correctly isolated from different executions. Here's an example on how these mocks can be used:

```tsx
// Defines the mocked functions in the context so we can change its mock.
let getPrivacySettingsMock: jest.MockedFn<ISettingsComponent['getPrivacySettings']>
let areUsersFriendsMock: jest.MockedFn<IFriendsComponent['areUsersFriends']>
let voice: IVoiceComponent

beforeEach({
  getPrivacySettingsMock = jest.fn()
  areUsersFriendsMock = jest.fn()
  // Builds the mock
  const settings = createSettingsMockedComponent({ getPrivacySettings: getPrivacySettingsMock })
  // Initializes the component mock with only the methods that are going to be used in the tests
  const friends = createFriendsMockedComponent({ areUsersFriends: areUsersFriendsMock })
  // Creates the component to be tested using the mocks
  voice = createVoiceComponent({ settings, friends })
})
```

This way of defining the mocks not only allows us to correctly isolate the contexts of the tests between executions, but also builds the grounds for a proper context initialization, as by having the mocked functions already defined, it's quite easy to build each context:

```tsx
// Defined after the previous code block.

describe('when checking if two users can call each other', () => {
	const calleeAddress = '0xd7D746d39D142b6bE752efd7626cE28F245a25D1'
	const callerAddress = '0x2e8b4De1230f827082202aa53d489A26163aace0'

  describe('and the callee only accepts calls from friends', () => {
    beforeEach(() => {
      // Defines the mock for the context, making the callee 
      getPrivacySettingsMock.mockImplementation((address: string) => {
	      switch(address) {
	        case caleeAddress:
		        return Settings.ONLY_FRIENDS
		      case callerAddress:
			      return Settings.ALL
			    default:
				    throw new Error("Wrong mock")
	      }
      })
    })
    
    describe('and the calle and the caller are friends', () => {
	    beforeEach(() => {
		    areUsersFriendsMock.mockResolvedValueOnce(true)
	    })
	    
	    it('should resolve to true', () => {
		    return expect(voice.canCallEachOther(callerAddress, calleeAddress)).resolves.toBe(true)
	    })
    })
    
    describe('and the callee and the caller are not friends', () =>{
		  beforeEach(() => {
		    areUsersFriendsMock.mockResolvedValueOnce(false)
	    })
	    
	    it('should resolve to false', () => {
		    return expect(voice.canCallEachOther(callerAddress, calleeAddress)).resolves.toBe(false)
	    })
    })
  })
  
  //... other contexts
})
```

## Integration testing

All endpoints, WS RPC calls, jobs or task MUST be integrally tested and MUST be written following the section defined here on how to write tests. Integration tests must be placed under the `test/integration` directory and must be named in accordance with the entry point we want to test.

Integration tests should be limited to testing either specific integration conditions that can't be tested with unit tests (SQL queries, Redis operations, etc) and the integration between our logic components and the protocol (HTTP, WS).

To show an example on how integration tests must be made, we'll be testing a simple HTTP request that retrieves a user's friends.

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
			throw InvalidRequest(isErrorWithMessage(error) ? error.message : 'Uknown error')
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

And a friends component having the following:

```tsx
async function getFriends(addres: string) {
	friendsDb.getFriends()
}
```
