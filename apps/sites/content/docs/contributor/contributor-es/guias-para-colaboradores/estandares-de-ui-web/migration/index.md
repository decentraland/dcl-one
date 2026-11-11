# Guía de migración

Esta guía cubre el proceso de migrar componentes de UI1 (`decentraland-ui`) a UI2 (`decentraland-ui2`).

## Cuándo migrar

Las migraciones pueden comenzar por tres razones válidas:

### 1. Mejora técnica

Quieres migrar un componente por:

* Mejor soporte de theming
* Tipos de TypeScript mejorados
* Consistencia con otros componentes de UI2
* Optimización del rendimiento
* Mejoras de accesibilidad

### 2. Actualización de un componente

* Se necesita actualizar un componente de UI1
* Aún no existe un equivalente en UI2
* **Requisito**: Créalo primero en UI2 y luego úsalo

### 3. Necesidades del proyecto

* El componente se usará en un proyecto nuevo o existente
* Hay tiempo disponible para migrarlo correctamente
* Los recursos del proyecto permiten una migración exhaustiva


> [!WARNING]
> **No** migres componentes "solo porque sí". Cada migración debe tener una justificación técnica o de negocio clara.


***

## Proceso de migración

### Paso 1: Planificación

Antes de iniciar la migración:

1. **Identificar dependencias**
   * ¿Qué otros componentes usa?
   * ¿Qué proyectos lo usan actualmente?
   * ¿Hay algún breaking change planificado?
2. **Revisar el uso actual**
   * ¿Cuántos proyectos usan este componente?
   * ¿Qué props se usan con más frecuencia?
   * ¿Hay algún problema conocido?
3. **Definir el alcance**
   * ¿Será una migración 1:1?
   * ¿Hay mejoras planificadas?
   * ¿Cuál es el cronograma?

### Paso 2: Crear el componente de UI2

Sigue la [Componentes personalizados](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui-web/custom-components/) guía para componentes candidatos de UI2.

**Requisitos:**

* Usa la sintaxis de objetos para styled-components
* Usa solo valores del theme (sin valores arbitrarios)
* Añade historias completas de Storybook
* Escribe pruebas completas
* Documenta todas las props y comportamientos

**Ejemplo de estructura:**

```
ui2/src/components/Button/
├── Button.tsx
├── Button.styles.ts
├── Button.stories.tsx
├── Button.test.tsx
├── types.ts
├── index.ts
└── README.md
```

### Paso 3: Mantener la compatibilidad

El componente de UI2 **DEBEN** debe exponer las mismas props y comportamientos que la versión de UI1.

#### Mismas props

```tsx
// Botón de UI1
interface ButtonProps {
  primary?: boolean;
  size?: 'small' | 'medium' | 'large';
  onClick?: () => void;
  disabled?: boolean;
  children: React.ReactNode;
}

// Botón de UI2 - DEBE admitir las mismas props
interface ButtonProps {
  primary?: boolean;
  size?: 'small' | 'medium' | 'large';
  onClick?: () => void;
  disabled?: boolean;
  children: React.ReactNode;
  // Se pueden añadir nuevas props opcionales
  variant?: 'text' | 'outlined' | 'contained';
}
```

#### Cambios compatibles hacia atrás

Si necesitas cambiar o añadir props:

1. **Primero**, añade las nuevas props a UI1 como opcionales
2. **Luego**, migra a los consumidores de UI1 para que usen las nuevas props
3. **Finalmente**, crea el componente de UI2 con la nueva API

```tsx
// Paso 1: Añadir prop opcional a UI1
interface ButtonProps {
  primary?: boolean;
  // Nueva prop opcional
  variant?: 'primary' | 'secondary';
}

// Paso 2: Actualizar la implementación de UI1
export function Button({ primary, variant = primary ? 'primary' : 'secondary' }: ButtonProps) {
  // Usar variant en lugar de primary internamente
}

// Paso 3: Crear UI2 con la nueva API
interface ButtonProps {
  // variant es ahora la prop principal
  variant?: 'primary' | 'secondary';
  // Mantener primary por compatibilidad, marcar como deprecated
  /** @deprecated Usa variant en su lugar */
  primary?: boolean;
}
```

### Paso 4: Marcar como deprecated el componente de UI1

Añade una advertencia de deprecación al componente de UI1:

````tsx
/**
 * @deprecated Este componente ha sido migrado a UI2.
 * Importa desde 'decentraland-ui2' en su lugar:
 * 
 * ```tsx
 * import { Button } from 'decentraland-ui2';
 * ```
 * 
 * Consulta la guía de migración: https://docs.decentraland.org/contributor-guides/web-ui-standards/migration
 */
export function Button(props: ButtonProps) {
  // ... implementación existente
}
````

### Paso 5: Adopción gradual

No fuerces la migración inmediata. Permite una adopción gradual:

1. **Publicar** componente de UI2
2. **Documentar** ruta de migración
3. **Actualizar** los nuevos proyectos para usar UI2
4. **Migrar** los proyectos existentes de forma oportunista
5. **Planificar** la eliminación eventual de UI1 (con aviso)

***

## Ejemplos de migración

### Ejemplo 1: Componente simple

Migrar una `Card` básica:

#### Versión de UI1

```tsx
// decentraland-ui/src/components/Card/Card.tsx
import React from 'react';
import './Card.css';

export interface CardProps {
  className?: string;
  children: React.ReactNode;
}

export function Card({ className, children }: CardProps) {
  return (
    <div className={`dcl-card ${className || ''}`}>
      {children}
    </div>
  );
}
```

#### Versión de UI2

```tsx
// decentraland-ui2/src/components/Card/Card.tsx
import { styled } from '@mui/material/styles';

export interface CardProps {
  className?: string;
  children: React.ReactNode;
}

const StyledCard = styled('div')(({ theme }) => ({
  backgroundColor: theme.palette.background.paper,
  borderRadius: theme.shape.borderRadius,
  padding: theme.spacing(2),
  boxShadow: theme.shadows[1],
  
  [theme.breakpoints.down('sm')]: {
    padding: theme.spacing(1),
  },
}));

export function Card({ className, children }: CardProps) {
  return (
    <StyledCard className={className}>
      {children}
    </StyledCard>
  );
}
```

### Ejemplo 2: Componente con variantes

Migrando un `Button` con variantes:

#### Versión de UI1

```tsx
// UI1
import './Button.css';

interface ButtonProps {
  primary?: boolean;
  secondary?: boolean;
  size?: 'small' | 'medium' | 'large';
}

export function Button({ primary, secondary, size = 'medium', ...props }: ButtonProps) {
  const classes = [
    'dcl-button',
    primary && 'primary',
    secondary && 'secondary',
    `size-${size}`,
  ] .filter(Boolean).join(' ');
  
  return <button className={classes} {...props} />;
}
```

#### Versión de UI2

```tsx
// UI2
import { styled } from '@mui/material/styles';

interface ButtonProps {
  /** @deprecated Usa variant="contained" en su lugar */
  primary?: boolean;
  /** @deprecated Usa variant="outlined" en su lugar */
  secondary?: boolean;
  variant?: 'text' | 'outlined' | 'contained';
  size?: 'small' | 'medium' | 'large';
}

const StyledButton = styled('button')<ButtonProps>(({ theme, variant = 'contained', size = 'medium' }) => {
  const sizes = {
    small: theme.spacing(0.5, 1),
    medium: theme.spacing(1, 2),
    large: theme.spacing(1.5, 3),
  };
  
  const variants = {
    text: {
      backgroundColor: 'transparent',
      color: theme.palette.primary.main,
    },
    outlined: {
      backgroundColor: 'transparent',
      color: theme.palette.primary.main,
      border: `1px solid ${theme.palette.primary.main}`,
    },
    contained: {
      backgroundColor: theme.palette.primary.main,
      color: theme.palette.primary.contrastText,
    },
  };
  
  return {
    padding: sizes[size],
    borderRadius: theme.shape.borderRadius,
    border: 'none',
    cursor: 'pointer',
    ...variants[variant],
    
    '&:hover': {
      opacity: 0.9,
    },
    
    '&:disabled': {
      opacity: 0.5,
      cursor: 'not-allowed',
    },
  };
});

export function Button({ 
  primary, 
  secondary, 
  variant, 
  ...props 
}: ButtonProps) {
  // Manejar props deprecated
  const actualVariant = variant || 
    (primary ? 'contained' : secondary ? 'outlined' : 'text');
  
  return <StyledButton variant={actualVariant} {...props} />;
}
```

***

## Lista de verificación de migración

Usa esta lista de verificación para cada migración de componente:

### Fase de planificación

* [ ] Identificar todos los proyectos que usan el componente
* [ ] Documentar las props y comportamientos actuales
* [ ] Definir el alcance y el cronograma de la migración
* [ ] Obtener la aprobación de las partes interesadas

### Fase de implementación

* [ ] Crear el componente de UI2 siguiendo los estándares
* [ ] Mantener la compatibilidad de props
* [ ] Usar la sintaxis de objetos para el styling
* [ ] Usar solo valores del theme
* [ ] Implementar todos los estados (idle, hover, focus, disabled, error)
* [ ] Añade historias completas de Storybook
* [ ] Escribir pruebas unitarias
* [ ] Documentar las funciones de accesibilidad

### Fase de deprecación

* [ ] Añadir una advertencia de deprecación al componente de UI1
* [ ] Actualizar la documentación de UI1
* [ ] Crear una guía de migración para los consumidores
* [ ] Publicar el componente de UI2

### Fase de adopción

* [ ] Actualizar los nuevos proyectos para usar UI2
* [ ] Crear PRs de migración para los proyectos existentes
* [ ] Monitorear problemas
* [ ] Recopilar feedback
* [ ] Planificar el cronograma de eliminación de UI1

***

## Patrones comunes de migración

### De CSS a Styled Components

```tsx
// UI1: archivo CSS
.dcl-card {
  background: #fff;
  padding: 16px;
  border-radius: 8px;
}

// UI2: componente estilizado
const Card = styled('div')(({ theme }) => ({
  backgroundColor: theme.palette.background.paper,
  padding: theme.spacing(2),
  borderRadius: theme.shape.borderRadius,
}));
```

### De nombres de clase a props

```tsx
// UI1: variantes basadas en clases
<Button className={primary ? 'primary' : 'secondary'} />

// UI2: variantes basadas en props
<Button variant={primary ? 'contained' : 'outlined'} />
```

### De valores fijos a theme

```tsx
// UI1: valores fijos
const styles = {
  color: '#333',
  fontSize: '14px',
  padding: '8px 16px',
};

// UI2: valores del theme
const Component = styled('div')(({ theme }) => ({
  color: theme.palette.text.primary,
  fontSize: theme.typography.body2.fontSize,
  padding: theme.spacing(1, 2),
}));
```

***

## Breaking changes

A veces los breaking changes son necesarios. Manéjalos con cuidado:

### Cuándo son aceptables los breaking changes

* Correcciones de seguridad
* Errores críticos
* Actualizaciones de versión mayor
* Eliminar funciones deprecated (con aviso)

### Cómo manejar los breaking changes

1. **Anunciar con anticipación** - Comunicar los cambios con suficiente antelación
2. **Proporcionar una ruta de migración** - Documentar cómo actualizar
3. **Aumentar la versión** - Seguir el versionado semántico
4. **Período de deprecación** - Dar tiempo para migrar
5. **Codemods** - Proporcionar herramientas automatizadas de migración si es posible

### Ejemplo: eliminar props deprecated

```tsx
// Versión 1.0: introducir la nueva API, marcar la antigua como deprecated
interface ButtonProps {
  /** @deprecated Usa variant="contained" en su lugar */
  primary?: boolean;
  variant?: 'text' | 'outlined' | 'contained';
}

// Versión 1.5: avisar sobre la eliminación
interface ButtonProps {
  /** @deprecated Se eliminará en 2.0. Usa variant en su lugar */
  primary?: boolean;
  variant?: 'text' | 'outlined' | 'contained';
}

// Versión 2.0: eliminar la prop deprecated
interface ButtonProps {
  variant?: 'text' | 'outlined' | 'contained';
}
```

***

## Pruebas de migración

Asegúrate de que los componentes migrados funcionen correctamente:

### Pruebas de regresión visual

Compara visualmente los componentes de UI1 y UI2:

```tsx
// Historia de Storybook para comparación
export const ComparisonStory: Story = {
  render: () => (
    <div style={{ display: 'flex', gap: '2rem' }}>
      <div>
        <h3>UI1</h3>
        <UI1Button primary>Haz clic aquí</UI1Button>
      </div>
      <div>
        <h3>UI2</h3>
        <UI2Button variant="contained">Haz clic aquí</UI2Button>
      </div>
    </div>
  ),
};
```

### Pruebas de comportamiento

Asegúrate de que las props funcionen igual:

```tsx
describe('Button migration', () => {
  it('should handle primary prop (deprecated) the same as variant="contained"', () => {
    const { container: ui1 } = render(<UI1Button primary>Test</UI1Button>);
    const { container: ui2 } = render(<UI2Button primary>Test</UI2Button>);
    
    // Comparar la salida renderizada
    expect(ui1.textContent).toBe(ui2.textContent);
  });
});
```

***

## Actualizaciones de documentación

Después de la migración, actualiza la documentación:

### Actualizar la documentación del componente de UI1

```markdown
# Button (UI1 - Deprecated)

> ⚠️ **Este componente ha sido migrado a UI2.**
> Consulta la [documentación de Button de UI2](../ui2/button) para la nueva versión.

Este componente está deprecated y se eliminará en una versión futura.
Migra a UI2, por favor.

## Guía de migración

Consulta [Guía de migración](./migration) para más detalles.
```

### Crear la documentación del componente de UI2

```markdown
# Button (UI2)

Componente moderno Button con soporte completo de theme.

## Migración desde UI1

Si estás migrando desde UI1:

- prop `primary` → `variant="contained"`
- prop `secondary` → `variant="outlined"`
- clases CSS → styled-components

Consulta la [Guía de migración completa](./migration) para más detalles.
```

***

## Próximos pasos

* Revisa [Componentes personalizados](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui-web/custom-components/) para crear componentes de UI2
* Consulta [Estilo y theming](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui-web/styling-and-theming/) para estándares de styling
* Revisa [Resumen del proceso](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui-web/process-overview/) para el flujo de trabajo completo


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui-web/migration.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
