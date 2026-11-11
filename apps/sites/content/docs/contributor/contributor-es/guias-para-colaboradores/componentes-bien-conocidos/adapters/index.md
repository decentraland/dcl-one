# Adaptadores

Los adaptadores son componentes responsables de gestionar **procesos de E/S** e integraciones externas. Actúan como el puente entre tu lógica de negocio y sistemas externos como bases de datos, APIs, sistemas de archivos o almacenes de caché.

## Propósito

Los adaptadores abstraen los detalles de implementación de las dependencias externas, permitiendo:

* **Intercambiabilidad** - Cambiar fácilmente las implementaciones (por ejemplo, PostgreSQL → MongoDB)
* **Testabilidad** - Simular dependencias externas en las pruebas
* **Aislamiento** - Mantener la lógica de negocio independiente de las preocupaciones de infraestructura

## Ubicación

Todos los componentes de adaptador DEBEN colocarse bajo el `src/adapters` directorio en el proyecto.

```
src/
└── adapters/
    ├── database/
    ├── storage/
    ├── cache/
    └── external-api/
```

## Interfaces compartidas

Los adaptadores DEBERÍAN construirse pensando en la intercambiabilidad. Define interfaces compartidas en `src/types.ts` para que varios adaptadores puedan implementar el mismo contrato.

### Ejemplo: interfaz de almacenamiento

```tsx
// src/types.ts
export interface IStorage {
  set: (key: string, value: any) => Promise<void>
  get: (key: string) => Promise<any>
  delete: (key: string) => Promise<void>
}
```

## Adaptador de almacenamiento en memoria

Aquí tienes una implementación sencilla de un adaptador de almacenamiento en memoria, útil para pruebas o desarrollo.

### Estructura de directorios

```
src/adapters/memory-storage/
├── component.ts
├── types.ts
├── errors.ts
└── index.ts
```

### Implementación: `component.ts`

```tsx
import { IStorage } from '../../types'

export function createMemoryStorageAdapter(): IStorage {
  // Estado interno: limitado a esta instancia del componente
  const memory: Record<string, any> = {}

  async function set(key: string, value: any): Promise<void> {
    memory[key] = value
  }

  async function get(key: string): Promise<any> {
    return memory[key]
  }

  async function delete(key: string): Promise<void> {
    delete memory[key]
  }

  return {
    set,
    get,
    delete
  }
}
```

## Adaptador de almacenamiento Redis

Aquí tienes un adaptador Redis listo para producción que demuestra la gestión del ciclo de vida.

### Implementación: `component.ts`

```tsx
import { createClient, RedisClientType } from 'redis'
import { IStorage } from '../../types'
import { RedisConnectionError } from './errors'

export function createRedisStorageAdapter(
  components: Pick<AppComponents, 'config' | 'logs'>
): IStorage {
  const { config, logs } = components
  const logger = logs.getLogger('redis-storage')
  
  const hostUrl = config.requireString('REDIS_URL')
  const client: RedisClientType = createClient({ url: hostUrl })

  // Método de ciclo de vida: se llama cuando el componente inicia
  async function start(): Promise<void> {
    try {
      await client.connect()
      logger.info('El cliente Redis se conectó correctamente')
    } catch (error) {
      logger.error('No se pudo conectar a Redis', error)
      throw new RedisConnectionError('No se pudo conectar a Redis')
    }
  }

  // Método de ciclo de vida: se llama cuando el componente se detiene
  async function stop(): Promise<void> {
    try {
      await client.quit()
      logger.info('El cliente Redis se desconectó')
    } catch (error) {
      logger.error('Error al desconectar el cliente Redis', error)
    }
  }

  async function set(key: string, value: any): Promise<void> {
    const serialized = JSON.stringify(value)
    await client.set(key, serialized)
    logger.debug(`Clave establecida: ${key}`)
  }

  async function get(key: string): Promise<any> {
    const value = await client.get(key)
    logger.debug(`Clave obtenida: ${key}`)
    return value ? JSON.parse(value) : null
  }

  async function delete(key: string): Promise<void> {
    await client.del(key)
    logger.debug(`Clave eliminada: ${key}`)
  }

  return {
    [Lifecycle.ComponentStarted]: start,
    [Lifecycle.ComponentStopped]: stop,
    set,
    get,
    delete
  }
}
```

### Gestión de errores: `errors.ts`

```tsx
export class RedisConnectionError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'RedisConnectionError'
  }
}
```

## Ejemplo de adaptador de base de datos

Aquí tienes un ejemplo de un adaptador de base de datos PostgreSQL.

```tsx
import { Pool, QueryResult } from 'pg'
import { IDatabase } from '../../types'

export function createPostgresAdapter(
  components: Pick<AppComponents, 'config' | 'logs'>
): IDatabase {
  const { config, logs } = components
  const logger = logs.getLogger('postgres-adapter')

  const pool = new Pool({
    connectionString: config.requireString('DATABASE_URL'),
    max: config.getNumber('DATABASE_POOL_SIZE', 20)
  })

  async function start(): Promise<void> {
    // Probar la conexión
    const client = await pool.connect()
    logger.info('Pool de conexión a la base de datos inicializado')
    client.release()
  }

  async function stop(): Promise<void> {
    await pool.end()
    logger.info('Pool de conexión a la base de datos cerrado')
  }

  async function query<T = any>(
    sql: string,
    params?: any[]
  ): Promise<QueryResult<T>> {
    logger.debug('Ejecutando consulta', { sql, params })
    return pool.query<T>(sql, params)
  }

  return {
    [Lifecycle.ComponentStarted]: start,
    [Lifecycle.ComponentStopped]: stop,
    query
  }
}
```

## Métodos de ciclo de vida

WKC proporciona métodos especiales de ciclo de vida que se llaman automáticamente:

* `[Lifecycle.ComponentStarted]` o `[START_COMPONENT]` - Se llama cuando el servicio inicia
* `[Lifecycle.ComponentStopped]` o `[STOP_COMPONENT]` - Se llama cuando el servicio se apaga

### Cuándo usar los métodos de ciclo de vida

Usa los métodos de ciclo de vida cuando tu adaptador necesite:

* Establecer conexiones (base de datos, caché, cola de mensajes)
* Inicializar pools o clientes
* Realizar comprobaciones de estado
* Limpiar recursos al apagarse
* Cerrar conexiones de forma ordenada

## Buenas prácticas

### 1. Gestión de errores

Maneja siempre los errores de conexión y lanza errores personalizados con significado:

```tsx
try {
  await client.connect()
} catch (error) {
  logger.error('La conexión falló', error)
  throw new ConnectionError('No se pudo conectar al servicio externo')
}
```

### 2. Configuración

Usa el componente config para gestionar los ajustes del adaptador:

```tsx
const {
  host: config.requireString('DB_HOST'),
  port: config.getNumber('DB_PORT', 5432),
  timeout: config.getNumber('DB_TIMEOUT', 30000)
}
```

### 3. Registro

Registra las operaciones importantes para depuración y supervisión:

```tsx
logger.info('Operación completada', { recordId, duration })
logger.warn('Se detectó una consulta lenta', { query, duration })
logger.error('La operación falló', { error, context })
```

### 4. Seguridad de tipos

Tipa siempre correctamente los valores de retorno:

```tsx
async function getUser(id: string): Promise<User | null> {
  const result = await query<User>('SELECT * FROM users WHERE id = $1', [id])
  return result.rows[0] || null
}
```

### 5. Gestión de recursos

Limpia siempre los recursos en el método de ciclo de vida stop:

```tsx
async function stop(): Promise<void> {
  await pool.end()
  await client.disconnect()
  logger.info('Recursos limpiados')
}
```

## Probando adaptadores

Consulta la [documentación de Testing Services (WKC)](/docs/contributor/contributor-es/guias-para-colaboradores/testing-standards/testing-services-wkc/) para obtener una guía detallada sobre cómo probar adaptadores.

### Ejemplo rápido

```tsx
describe('cuando se crea un adaptador de almacenamiento en memoria', () => {
  let storage: IStorage

  beforeEach(() => {
    storage = createMemoryStorageAdapter()
  })

  describe('cuando se establece un valor', () => {
    it('debería almacenar y recuperar el valor', async () => {
      await storage.set('key', 'value')
      const result = await storage.get('key')
      expect(result).toBe('value')
    })
  })
})
```


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/guias-para-colaboradores/componentes-bien-conocidos/adapters.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
