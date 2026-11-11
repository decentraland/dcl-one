# Componentes

Los componentes son piezas de software autónomas diseñadas como cajas negras que reciben otros componentes (o ninguno) para realizar sus tareas. El objetivo principal es desacoplar secciones de código y maximizar la capacidad de prueba.

## Estructura de directorios

Todos los componentes DEBEN definirse dentro de un directorio dedicado que contenga al menos estos cuatro archivos:

* `component.ts` - Contiene el código de implementación del componente
* `types.ts` - Contiene la interfaz del componente y los tipos expuestos
* `errors.ts` - Contiene todas las clases de error exportadas
* `index.ts` - Exporta la API pública del componente

### Estructura de ejemplo

```
src/
└── components/
    └── my-component/
        ├── component.ts
        ├── types.ts
        ├── errors.ts
        └── index.ts
```

## Archivo de tipos

Los componentes se ajustan a una interfaz que define su API pública. Esta interfaz puede compartirse entre componentes intercambiables o usarse para entender los métodos expuestos por el componente.

Todos los componentes que tienen su propia interfaz DEBEN definirla en un `types.ts` archivo dentro del directorio del componente.

Otros tipos expuestos por el componente TAMBIÉN DEBEN colocarse en este archivo.

### Ejemplo: `types.ts`

```tsx
export type Something = {
  aValue: boolean
  anotherValue: string
}

export interface IMyComponent {
  myFunction: () => boolean
  getSomething: () => Something
}
```

### Tipos compartidos

Los tipos compartidos entre varios componentes, como las interfaces comunes, DEBEN colocarse en un `types.ts` archivo en el directorio raíz del código fuente del proyecto (`/src/types.ts`).

## Archivo del componente

El archivo del componente contiene la función creadora del componente. Esta función sigue una convención específica de nomenclatura y estructura.

### Convención de nomenclatura

Las funciones creadoras de componentes DEBEN nombrarse como: `create` + `ComponentName` + `Componente`

**Ejemplo:** `createMyNewComponent`

### Firma de la función

La función creadora DEBE:

1. Recibir un objeto components como primer parámetro que contenga todas las dependencias
2. Recibir opcionalmente parámetros de configuración adicionales
3. Devolver un objeto que contenga los métodos expuestos (públicos)

### Estructura del componente

Al inicio de la función creadora del componente:

1. Extraer las dependencias del objeto components
2. Inicializar variables comunes (p. ej., loggers, configuración)
3. Definir funciones auxiliares internas
4. Definir métodos públicos
5. Devolver la API pública

### Ejemplo: `component.ts`

```tsx
import { IMyComponent, Something } from './types'
import { WrongStringError } from './errors'

export function createMyNewComponent(
  components: Pick<AppComponents, 'logs'>
): IMyComponent {
  const { logs } = components
  const logger = logs.getLogger('my-component')

  // Método interno - no expuesto
  function computeString(fstString: string, sndString: string): string {
    if (fstString.length === 0) {
      throw new WrongStringError()
    }
    
    return fstString + ' ' + sndString
  }

  // Método público - expuesto en el objeto de retorno
  function myFunction(): boolean {
    return true
  }

  // Método público - expuesto en el objeto de retorno
  function getSomething(): Something {
    logger.info('Obteniendo algo')
    
    return {
      aValue: true,
      anotherValue: computeString('aString', 'anotherString')
    }
  }

  // Devolver la API pública
  return {
    myFunction,
    getSomething
  }
}
```

## Archivo de errores

El archivo de errores contiene clases de error personalizadas que un componente puede lanzar. Estos errores pueden usarse en otros componentes o controladores para identificar y gestionar correctamente condiciones de error específicas.

### Ejemplo: `errors.ts`

```tsx
export class WrongStringError extends Error {
  constructor(message?: string) {
    super(message || 'Cadena incorrecta')
    this.name = 'WrongStringError'
  }
}

export class ComponentNotInitializedError extends Error {
  constructor() {
    super('El componente no se ha inicializado')
    this.name = 'ComponentNotInitializedError'
  }
}
```

### Buenas prácticas para el manejo de errores

* Crear clases de error específicas para diferentes condiciones de error
* Incluir mensajes de error significativos
* Establece el `name` propiedad para que coincida con el nombre de la clase y facilitar la depuración
* Documentar qué errores puede lanzar cada método público

## Archivo de índice

El archivo de índice actúa como punto de entrada público del componente y exporta solo lo que debe ser accesible para otras partes de la aplicación.

### Ejemplo: `index.ts`

```tsx
export { createMyNewComponent } from './component'
export type { IMyComponent, Something } from './types'
export { WrongStringError } from './errors'
```

## Buenas prácticas

1. **Responsabilidad única** - Cada componente debe tener un propósito claro
2. **Dependencias explícitas** - Todas las dependencias deben inyectarse a través del parámetro components
3. **Inmutabilidad** - Preferir estructuras de datos inmutables cuando sea posible
4. **Manejo de errores** - Usar clases de error personalizadas para diferentes condiciones de error
5. **Registro** - Usar el logger para depuración y supervisión
6. **Seguridad de tipos** - Aprovechar al máximo el sistema de tipos de TypeScript
7. **Documentación** - Documentar los métodos complejos y la lógica de negocio

## Ciclo de vida del componente

Es posible que algunos componentes necesiten realizar operaciones de configuración o limpieza. WKC proporciona métodos del ciclo de vida:

* `[START_COMPONENT]` - Se llama cuando el componente se inicia
* `[STOP_COMPONENT]` - Se llama cuando el componente se detiene

Consulta la [Adaptadores](/docs/contributor/contributor-es/guias-para-colaboradores/componentes-bien-conocidos/adapters/) sección para ejemplos de componentes que usan métodos del ciclo de vida.


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/guias-para-colaboradores/componentes-bien-conocidos/components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
