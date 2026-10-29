# Logic Components


> [!NOTE]
> This section is currently being developed. Check back soon for comprehensive documentation on logic components.


Logic components are pieces of software that contain the **business logic** of your application. They are organized semantically by domain or functionality and serve as the orchestration layer between controllers and adapters.

## Purpose

Logic components:

* Implement business rules and domain logic
* Orchestrate operations across multiple adapters
* Encapsulate complex workflows
* Remain independent of transport layer (HTTP, WebSocket) concerns
* Can be thoroughly unit tested without I/O dependencies

## Location

Logic components SHOULD be placed under the `src/logic` or `src/components` directory, organized by domain:

```
src/
└── logic/
    ├── users/
    ├── content/
    ├── permissions/
    └── notifications/
```

## Characteristics

### 1. Adapter Consumption

Logic components are the primary consumers of adapters. They use adapters to interact with external resources while focusing on business rules.

```tsx
export function createUserLogic(
  components: Pick<AppComponents, 'database' | 'cache' | 'logs'>
): IUserLogic {
  const { database, cache, logs } = components
  const logger = logs.getLogger('user-logic')

  async function getUserProfile(userId: string): Promise<UserProfile> {
    // Check cache first
    const cached = await cache.get(`user:${userId}`)
    if (cached) {
      return cached
    }

    // Fetch from database
    const user = await database.query('SELECT * FROM users WHERE id = $1', [userId])
    
    // Apply business logic
    const profile = transformUserToProfile(user)
    
    // Cache the result
    await cache.set(`user:${userId}`, profile, { ttl: 3600 })
    
    return profile
  }

  return {
    getUserProfile
  }
}
```

### 2. Business Rules

Logic components enforce business rules and validation:

```tsx
async function createUser(userData: CreateUserInput): Promise<User> {
  // Business rule: username must be unique
  const existing = await database.query(
    'SELECT id FROM users WHERE username = $1',
    [userData.username]
  )
  
  if (existing.rows.length > 0) {
    throw new UserAlreadyExistsError(userData.username)
  }

  // Business rule: validate user data
  if (!isValidEmail(userData.email)) {
    throw new InvalidEmailError(userData.email)
  }

  // Create the user
  const user = await database.query(
    'INSERT INTO users (username, email) VALUES ($1, $2) RETURNING *',
    [userData.username, userData.email]
  )

  logger.info('User created', { userId: user.id })
  
  return user
}
```

### 3. Workflow Orchestration

Logic components coordinate complex multi-step operations:

```tsx
async function publishContent(
  userId: string,
  content: ContentInput
): Promise<PublishedContent> {
  // Step 1: Validate permissions
  const canPublish = await permissions.canUserPublish(userId)
  if (!canPublish) {
    throw new UnauthorizedError('User cannot publish content')
  }

  // Step 2: Process content
  const processed = await processContent(content)

  // Step 3: Store content
  const stored = await storage.save(processed)

  // Step 4: Update indexes
  await searchIndex.index(stored)

  // Step 5: Notify subscribers
  await notifications.notifySubscribers(userId, stored)

  logger.info('Content published', { contentId: stored.id, userId })

  return stored
}
```

## Best Practices

### 1. Single Responsibility

Each logic component should focus on one domain or bounded context:

```tsx
// Good: Focused on user domain
createUserLogic()
createContentLogic()
createPermissionsLogic()

// Avoid: Mixed responsibilities
createUserAndContentLogic()
```

### 2. Dependency Injection

Always inject dependencies through the components parameter:

```tsx
export function createOrderLogic(
  components: Pick<AppComponents, 'database' | 'payments' | 'inventory' | 'logs'>
): IOrderLogic {
  // Use injected dependencies
}
```

### 3. Error Handling

Throw meaningful domain errors that controllers can catch and handle:

```tsx
// Define domain-specific errors
export class InsufficientInventoryError extends Error {
  constructor(public readonly productId: string, public readonly requested: number) {
    super(`Insufficient inventory for product ${productId}. Requested: ${requested}`)
    this.name = 'InsufficientInventoryError'
  }
}
```

### 4. Pure Business Logic

Keep logic components free from transport layer concerns:

```tsx
// Good: Pure business logic
async function calculateOrderTotal(items: OrderItem[]): Promise<number> {
  return items.reduce((total, item) => total + item.price * item.quantity, 0)
}

// Avoid: HTTP/transport layer concerns
async function calculateOrderTotal(req: Request, res: Response): Promise<void> {
  // Don't do this in logic components
}
```

## Testing Logic Components

Logic components should be thoroughly unit tested. See the [Testing Services (WKC)](/docs/contributor/contributor-guides/testing-standards/testing-services-wkc/) documentation for details.

```tsx
describe('when creating a user', () => {
  let userLogic: IUserLogic
  let mockDatabase: jest.Mocked<IDatabase>

  beforeEach(() => {
    mockDatabase = createMockDatabase()
    userLogic = createUserLogic({ database: mockDatabase, logs: mockLogs })
  })

  describe('and the username already exists', () => {
    beforeEach(() => {
      mockDatabase.query.mockResolvedValueOnce({ rows: [{ id: '123' }] })
    })

    it('should throw UserAlreadyExistsError', async () => {
      await expect(
        userLogic.createUser({ username: 'existing', email: 'test@test.com' })
      ).rejects.toThrow(UserAlreadyExistsError)
    })
  })
})
```

## Coming Soon

This section will be expanded with:

* Detailed examples of domain-driven logic components
* Patterns for handling complex workflows
* Guidelines for organizing large domain logic
* Integration with event systems
* Caching strategies
* Transaction management patterns
