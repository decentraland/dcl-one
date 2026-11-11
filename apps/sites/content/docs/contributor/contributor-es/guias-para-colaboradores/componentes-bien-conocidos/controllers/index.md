# Controladores

Los controladores son responsables de manejar solicitudes HTTP y WebSocket en tu aplicación. Sirven como punto de entrada para las solicitudes externas y actúan como puente entre la capa de transporte y tu lógica de negocio.

## Propósito

Los controladores DEBEN realizar solo estas tareas específicas:

1. **Validar la entrada de la capa de transporte** - Verificar autenticación, autorización y formato de entrada
2. **Llamar a los componentes de lógica** - Delegar la lógica de negocio a los componentes de lógica apropiados
3. **Transformar la entrada** - Convertir los datos de la solicitud en parámetros para las llamadas a los componentes de lógica
4. **Manejar errores** - Capturar y responder adecuadamente a los errores de los componentes de lógica


> [!WARNING]
> Los controladores no deben contener lógica de negocio. Todas las reglas de negocio DEBEN implementarse en los componentes de lógica.


## Ubicación

Todos los controladores DEBEN colocarse en el `/src/controllers` directorio y nombrarse en referencia a la acción que realizan.

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

## Estructura del controlador

### Manejador HTTP básico

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
    // 1. Validar autenticación
    if (!verification?.auth) {
      return {
        status: 401,
        body: {
          error: 'No autorizado',
          message: 'Se requiere autenticación'
        }
      }
    }

    // 2. Validar entrada
    if (!id || typeof id !== 'string') {
      return {
        status: 400,
        body: {
          error: 'Solicitud incorrecta',
          message: 'ID de usuario no válido'
        }
      }
    }

    // 3. Llamar al componente de lógica
    const user = await userLogic.getUser(id)

    // 4. Devolver respuesta exitosa
    return {
      status: 200,
      body: {
        ok: true,
        data: user
      }
    }
  } catch (error) {
    // 5. Manejar errores
    logger.error('Error al obtener el usuario', error)

    if (error instanceof UserNotFoundError) {
      return {
        status: 404,
        body: {
          error: 'No encontrado',
          message: error.message
        }
      }
    }

    return {
      status: 500,
      body: {
        error: 'Error interno del servidor',
        message: 'Ocurrió un error inesperado'
      }
    }
  }
}
```

### Manejador de solicitudes POST

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
    // 1. Validar autenticación
    if (!verification?.auth) {
      return {
        status: 401,
        body: {
          error: 'No autorizado',
          message: 'Se requiere autenticación'
        }
      }
    }

    // 2. Analizar y validar el cuerpo de la solicitud
    const body = await request.json()
    
    if (!body.username || !body.email) {
      return {
        status: 400,
        body: {
          error: 'Solicitud incorrecta',
          message: 'Se requieren nombre de usuario y correo electrónico'
        }
      }
    }

    // 3. Transformar la entrada para el componente de lógica
    const userData = {
      username: body.username.toLowerCase(),
      email: body.email.toLowerCase(),
      display_name: body.display_name || body.username
    }

    // 4. Llamar al componente de lógica
    const user = await userLogic.createUser(userData)

    // 5. Devolver respuesta exitosa
    return {
      status: 201,
      body: {
        ok: true,
        data: user
      }
    }
  } catch (error) {
    logger.error('Error al crear el usuario', error)

    if (error instanceof UserAlreadyExistsError) {
      return {
        status: 409,
        body: {
          error: 'Conflicto',
          message: error.message
        }
      }
    }

    if (error instanceof InvalidEmailError) {
      return {
        status: 400,
        body: {
          error: 'Solicitud incorrecta',
          message: error.message
        }
      }
    }

    return {
      status: 500,
      body: {
        error: 'Error interno del servidor',
        message: 'Ocurrió un error inesperado'
      }
    }
  }
}
```

### Manejador de parámetros de consulta

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
    // 1. Validar autenticación
    if (!verification?.auth) {
      return {
        status: 401,
        body: {
          error: 'No autorizado',
          message: 'Se requiere autenticación'
        }
      }
    }

    // 2. Extraer y validar los parámetros de consulta (¡en minúsculas!)
    const searchParams = new URLSearchParams(url.search)
    const query = searchParams.get('query')?.toLowerCase()
    const limit = parseInt(searchParams.get('limit') || '10', 10)
    const offset = parseInt(searchParams.get('offset') || '0', 10)

    if (!query) {
      return {
        status: 400,
        body: {
          error: 'Solicitud incorrecta',
          message: 'El parámetro de consulta es obligatorio'
        }
      }
    }

    // 3. Llamar al componente de lógica
    const results = await userLogic.searchUsers({
      query,
      limit,
      offset
    })

    // 4. Devolver respuesta exitosa
    return {
      status: 200,
      body: {
        ok: true,
        data: results
      }
    }
  } catch (error) {
    logger.error('Error al buscar usuarios', error)

    return {
      status: 500,
      body: {
        error: 'Error interno del servidor',
        message: 'Ocurrió un error inesperado'
      }
    }
  }
}
```

## Requisitos de entrada

Todas las claves de entrada recibidas a través de los controladores (cuerpo JSON, parámetros de consulta, parámetros de URL, encabezados, etc.) **DEBEN estar en minúsculas** para evitar problemas de mayúsculas/minúsculas al procesar datos.

### Parámetros de varias palabras

Los parámetros de varias palabras DEBEN definirse usando **snake\_case**.


> [!CAUTION]
> **Nomenclatura incorrecta:**
> 
> * `minPrice`
> * `Car`
> * `somethingURL`
> * `userId`



> [!TIP]
> **Nomenclatura correcta:**
> 
> * `min_price`
> * `car`
> * `something_url`
> * `user_id`


### Ejemplo

```tsx
// Cuerpo de la solicitud
{
  "user_name": "john_doe",        // ✅ Correcto
  "email_address": "john@example.com",  // ✅ Correcto
  "display_name": "John Doe",     // ✅ Correcto
  "max_items": 100                // ✅ Correcto
}

// Parámetros de consulta
?search_query=test&max_results=50&sort_by=created_at  // ✅ Correcto

// Parámetros de URL
/users/:user_id/posts/:post_id   // ✅ Correcto
```

## Manejo de errores

### Respuestas de error estándar

Los controladores deben devolver formatos de respuesta de error consistentes:

```tsx
// 400 Solicitud incorrecta
{
  "error": "Solicitud incorrecta",
  "message": "Mensaje de error detallado",
  "details": {} // Detalles adicionales opcionales
}

// 401 No autorizado
{
  "error": "No autorizado",
  "message": "Se requiere autenticación"
}

// 403 Prohibido
{
  "error": "Prohibido",
  "message": "No tienes permiso para acceder a este recurso"
}

// 404 No encontrado
{
  "error": "No encontrado",
  "message": "Recurso no encontrado"
}

// 409 Conflicto
{
  "error": "Conflicto",
  "message": "El recurso ya existe"
}

// 500 Error interno del servidor
{
  "error": "Error interno del servidor",
  "message": "Ocurrió un error inesperado"
}
```

### Mapeo de errores

Mapea los errores de dominio a los códigos de estado HTTP apropiados:

```tsx
function mapErrorToResponse(error: Error, logger: ILogger) {
  logger.error('Error del manejador', error)

  // Errores específicos del dominio
  if (error instanceof UserNotFoundError) {
    return { status: 404, body: { error: 'No encontrado', message: error.message } }
  }
  
  if (error instanceof UserAlreadyExistsError) {
    return { status: 409, body: { error: 'Conflicto', message: error.message } }
  }
  
  if (error instanceof UnauthorizedError) {
    return { status: 403, body: { error: 'Prohibido', message: error.message } }
  }
  
  if (error instanceof InvalidInputError) {
    return { status: 400, body: { error: 'Solicitud incorrecta', message: error.message } }
  }

  // Error genérico
  return {
    status: 500,
    body: {
      error: 'Error interno del servidor',
      message: 'Ocurrió un error inesperado'
    }
  }
}
```

## Autenticación y autorización

### Comprobación de autenticación

```tsx
// Requerir autenticación
if (!verification?.auth) {
  return {
    status: 401,
    body: {
      error: 'No autorizado',
      message: 'Se requiere autenticación'
    }
  }
}

const userAddress = verification.auth.toLowerCase()
```

### Comprobación de autorización

```tsx
// Comprobar permisos a través del componente de lógica
const canAccess = await permissionsLogic.canUserAccessResource(
  userAddress,
  resourceId
)

if (!canAccess) {
  return {
    status: 403,
    body: {
      error: 'Prohibido',
      message: 'No tienes permiso para acceder a este recurso'
    }
  }
}
```

## Buenas prácticas

### 1. Mantén los controladores ligeros

Los controladores deben ser envoltorios ligeros alrededor de los componentes de lógica:

```tsx
// ✅ Bien: Controlador ligero
export async function handler(context) {
  const { userLogic } = context.components
  const user = await userLogic.getUser(context.params.id)
  return { status: 200, body: { data: user } }
}

// ❌ Mal: Lógica de negocio en el controlador
export async function handler(context) {
  const { database } = context.components
  const user = await database.query('SELECT * FROM users...')
  const processed = processUser(user) // ¡Lógica de negocio!
  const validated = validateUser(processed) // ¡Lógica de negocio!
  return { status: 200, body: { data: validated } }
}
```

### 2. Valida pronto

Valida y falla rápido:

```tsx
// Valida primero la autenticación
if (!verification?.auth) {
  return { status: 401, body: { error: 'No autorizado' } }
}

// Luego valida la entrada
if (!params.id) {
  return { status: 400, body: { error: 'Solicitud incorrecta' } }
}

// Luego continúa con la lógica de negocio
const result = await logic.doSomething(params.id)
```

### 3. Usa seguridad de tipos

Aprovecha TypeScript para la seguridad de tipos:

```tsx
interface CreateUserRequest {
  user_name: string
  email_address: string
  display_name?: string
}

const body: CreateUserRequest = await request.json()
```

### 4. Registra adecuadamente

Registra eventos y errores importantes:

```tsx
logger.info('Usuario creado', { userId: user.id, userAddress })
logger.warn('El límite de tasa se acerca', { userAddress, requests })
logger.error('No se pudo crear el usuario', { error, userAddress })
```

## Pruebas de controladores

Consulta la [documentación de Testing Services (WKC)](/docs/contributor/contributor-es/guias-para-colaboradores/testing-standards/testing-services-wkc/) documentación para orientación sobre pruebas de integración.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/guias-para-colaboradores/componentes-bien-conocidos/controllers.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
