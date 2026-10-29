# Controllers

Controllers are responsible for handling HTTP and WebSocket requests in your application. They serve as the entry point for external requests and act as the bridge between the transport layer and your business logic.

## Purpose

Controllers MUST only perform these specific tasks:

1. **Validate transport layer input** - Verify authentication, authorization, and input format
2. **Call logic components** - Delegate business logic to appropriate logic components
3. **Transform input** - Convert request data into parameters for logic component calls
4. **Handle errors** - Catch and properly respond to errors from logic components


> [!WARNING]
> Controllers should NOT contain business logic. All business rules MUST be implemented in logic components.


## Location

All controllers MUST be placed under the `/src/controllers` directory and be named in reference to the action they perform.

```
src/
└── controllers/
    ├── users/
    │   ├── get-user.ts
    │   ├── create-user.ts
    │   └── update-user.ts
    ├── content/
    │   ├── publish-content.ts
    │   └── delete-content.ts
    └── health.ts
```

## Controller Structure

### Basic HTTP Handler

```tsx
import { HandlerContextWithPath } from '@dcl/platform-server-commons'

export async function getUserHandler(
  context: HandlerContextWithPath<'userLogic' | 'logs', '/users/:id'>
) {
  const { 
    components: { userLogic, logs },
    params: { id },
    verification 
  } = context

  const logger = logs.getLogger('get-user-handler')

  try {
    // 1. Validate authentication
    if (!verification?.auth) {
      return {
        status: 401,
        body: {
          error: 'Unauthorized',
          message: 'Authentication required'
        }
      }
    }

    // 2. Validate input
    if (!id || typeof id !== 'string') {
      return {
        status: 400,
        body: {
          error: 'Bad Request',
          message: 'Invalid user ID'
        }
      }
    }

    // 3. Call logic component
    const user = await userLogic.getUser(id)

    // 4. Return success response
    return {
      status: 200,
      body: {
        ok: true,
        data: user
      }
    }
  } catch (error) {
    // 5. Handle errors
    logger.error('Error fetching user', error)

    if (error instanceof UserNotFoundError) {
      return {
        status: 404,
        body: {
          error: 'Not Found',
          message: error.message
        }
      }
    }

    return {
      status: 500,
      body: {
        error: 'Internal Server Error',
        message: 'An unexpected error occurred'
      }
    }
  }
}
```

### POST Request Handler

```tsx
export async function createUserHandler(
  context: HandlerContextWithPath<'userLogic' | 'logs', '/users'>
) {
  const {
    components: { userLogic, logs },
    request,
    verification
  } = context

  const logger = logs.getLogger('create-user-handler')

  try {
    // 1. Validate authentication
    if (!verification?.auth) {
      return {
        status: 401,
        body: {
          error: 'Unauthorized',
          message: 'Authentication required'
        }
      }
    }

    // 2. Parse and validate request body
    const body = await request.json()
    
    if (!body.username || !body.email) {
      return {
        status: 400,
        body: {
          error: 'Bad Request',
          message: 'Username and email are required'
        }
      }
    }

    // 3. Transform input for logic component
    const userData = {
      username: body.username.toLowerCase(),
      email: body.email.toLowerCase(),
      display_name: body.display_name || body.username
    }

    // 4. Call logic component
    const user = await userLogic.createUser(userData)

    // 5. Return success response
    return {
      status: 201,
      body: {
        ok: true,
        data: user
      }
    }
  } catch (error) {
    logger.error('Error creating user', error)

    if (error instanceof UserAlreadyExistsError) {
      return {
        status: 409,
        body: {
          error: 'Conflict',
          message: error.message
        }
      }
    }

    if (error instanceof InvalidEmailError) {
      return {
        status: 400,
        body: {
          error: 'Bad Request',
          message: error.message
        }
      }
    }

    return {
      status: 500,
      body: {
        error: 'Internal Server Error',
        message: 'An unexpected error occurred'
      }
    }
  }
}
```

### Query Parameters Handler

```tsx
export async function searchUsersHandler(
  context: HandlerContextWithPath<'userLogic' | 'logs', '/users/search'>
) {
  const {
    components: { userLogic, logs },
    url,
    verification
  } = context

  const logger = logs.getLogger('search-users-handler')

  try {
    // 1. Validate authentication
    if (!verification?.auth) {
      return {
        status: 401,
        body: {
          error: 'Unauthorized',
          message: 'Authentication required'
        }
      }
    }

    // 2. Extract and validate query parameters (lowercase!)
    const searchParams = new URLSearchParams(url.search)
    const query = searchParams.get('query')?.toLowerCase()
    const limit = parseInt(searchParams.get('limit') || '10', 10)
    const offset = parseInt(searchParams.get('offset') || '0', 10)

    if (!query) {
      return {
        status: 400,
        body: {
          error: 'Bad Request',
          message: 'Query parameter is required'
        }
      }
    }

    // 3. Call logic component
    const results = await userLogic.searchUsers({
      query,
      limit,
      offset
    })

    // 4. Return success response
    return {
      status: 200,
      body: {
        ok: true,
        data: results
      }
    }
  } catch (error) {
    logger.error('Error searching users', error)

    return {
      status: 500,
      body: {
        error: 'Internal Server Error',
        message: 'An unexpected error occurred'
      }
    }
  }
}
```

## Input Requirements

All input keys received through controllers (JSON body, query parameters, URL parameters, headers, etc.) **MUST be in lowercase** to prevent casing issues when processing data.

### Multi-word Parameters

Multi-word parameters MUST be defined using **snake_case**.


> [!CAUTION]
> **Incorrect naming:**
> 
> * `minPrice`
> * `Car`
> * `somethingURL`
> * `userId`



> [!TIP]
> **Correct naming:**
> 
> * `min_price`
> * `car`
> * `something_url`
> * `user_id`


### Example

```tsx
// Request body
{
  "user_name": "john_doe",        // ✅ Correct
  "email_address": "john@example.com",  // ✅ Correct
  "display_name": "John Doe",     // ✅ Correct
  "max_items": 100                // ✅ Correct
}

// Query parameters
?search_query=test&max_results=50&sort_by=created_at  // ✅ Correct

// URL parameters
/users/:user_id/posts/:post_id   // ✅ Correct
```

## Error Handling

### Standard Error Responses

Controllers should return consistent error response formats:

```tsx
// 400 Bad Request
{
  "error": "Bad Request",
  "message": "Detailed error message",
  "details": {} // Optional additional details
}

// 401 Unauthorized
{
  "error": "Unauthorized",
  "message": "Authentication required"
}

// 403 Forbidden
{
  "error": "Forbidden",
  "message": "You don't have permission to access this resource"
}

// 404 Not Found
{
  "error": "Not Found",
  "message": "Resource not found"
}

// 409 Conflict
{
  "error": "Conflict",
  "message": "Resource already exists"
}

// 500 Internal Server Error
{
  "error": "Internal Server Error",
  "message": "An unexpected error occurred"
}
```

### Error Mapping

Map domain errors to appropriate HTTP status codes:

```tsx
function mapErrorToResponse(error: Error, logger: ILogger) {
  logger.error('Handler error', error)

  // Domain-specific errors
  if (error instanceof UserNotFoundError) {
    return { status: 404, body: { error: 'Not Found', message: error.message } }
  }
  
  if (error instanceof UserAlreadyExistsError) {
    return { status: 409, body: { error: 'Conflict', message: error.message } }
  }
  
  if (error instanceof UnauthorizedError) {
    return { status: 403, body: { error: 'Forbidden', message: error.message } }
  }
  
  if (error instanceof InvalidInputError) {
    return { status: 400, body: { error: 'Bad Request', message: error.message } }
  }

  // Generic error
  return {
    status: 500,
    body: {
      error: 'Internal Server Error',
      message: 'An unexpected error occurred'
    }
  }
}
```

## Authentication and Authorization

### Authentication Check

```tsx
// Require authentication
if (!verification?.auth) {
  return {
    status: 401,
    body: {
      error: 'Unauthorized',
      message: 'Authentication required'
    }
  }
}

const userAddress = verification.auth.toLowerCase()
```

### Authorization Check

```tsx
// Check permissions through logic component
const canAccess = await permissionsLogic.canUserAccessResource(
  userAddress,
  resourceId
)

if (!canAccess) {
  return {
    status: 403,
    body: {
      error: 'Forbidden',
      message: 'You don\'t have permission to access this resource'
    }
  }
}
```

## Best Practices

### 1. Keep Controllers Thin

Controllers should be thin wrappers around logic components:

```tsx
// ✅ Good: Thin controller
export async function handler(context) {
  const { userLogic } = context.components
  const user = await userLogic.getUser(context.params.id)
  return { status: 200, body: { data: user } }
}

// ❌ Bad: Business logic in controller
export async function handler(context) {
  const { database } = context.components
  const user = await database.query('SELECT * FROM users...')
  const processed = processUser(user) // Business logic!
  const validated = validateUser(processed) // Business logic!
  return { status: 200, body: { data: validated } }
}
```

### 2. Validate Early

Validate and fail fast:

```tsx
// Validate authentication first
if (!verification?.auth) {
  return { status: 401, body: { error: 'Unauthorized' } }
}

// Then validate input
if (!params.id) {
  return { status: 400, body: { error: 'Bad Request' } }
}

// Then proceed with business logic
const result = await logic.doSomething(params.id)
```

### 3. Use Type Safety

Leverage TypeScript for type safety:

```tsx
interface CreateUserRequest {
  user_name: string
  email_address: string
  display_name?: string
}

const body: CreateUserRequest = await request.json()
```

### 4. Log Appropriately

Log important events and errors:

```tsx
logger.info('User created', { userId: user.id, userAddress })
logger.warn('Rate limit approaching', { userAddress, requests })
logger.error('Failed to create user', { error, userAddress })
```

## Testing Controllers

See the [Testing Services (WKC)](/docs/contributor/contributor-guides/testing-standards/testing-services-wkc/) documentation for integration testing guidance.
