# Componentes personalizados

Distinguimos entre dos tipos de componentes personalizados, cada uno con procesos y expectativas diferentes.

## Tipos de componentes

### A) Componentes personalizados específicos del proyecto

Componentes creados para un proyecto o pantalla específica, no pensados para reutilizarse en otros proyectos.

**Ejemplos:**

* Un `Box` con un layout especial utilizado dentro de un proyecto
* Un `Card` variante con un layout a medida para las pantallas de un proyecto
* Visualizaciones de datos específicas del proyecto
* Componentes de layout de uso único

**Cuándo usarlo:**

* El componente resuelve un problema único de un proyecto
* Es poco probable que se necesite en otros proyectos
* Demasiado específico para generalizar

### B) Componentes candidatos de UI2

Componentes pensados para reutilizarse en múltiples proyectos y productos.

**Ejemplos:**

* `Navbar` - Navegación en todo el sitio
* `UserMenu` - Menú de cuenta del usuario
* Estandarizado `Modal` diálogos
* Componentes migrados desde UI1

**Cuándo usarlo:**

* El componente se utilizará en múltiples proyectos
* Representa un patrón común de Decentraland
* Reemplaza o amplía un componente de UI1

***

## Componentes específicos del proyecto

### Requisitos

#### Usa MUI como base

**DEBEN** extiende los componentes existentes de MUI siempre que sea posible:

```tsx
// ✅ Bien: Extiende MUI Card
import { Card as MuiCard } from '@mui/material';
import { styled } from '@mui/material/styles';

const ProjectCard = styled(MuiCard)(({ theme }) => ({
  padding: theme.spacing(3),
  display: 'flex',
  flexDirection: 'column',
  gap: theme.spacing(2),
}));

// ❌ Mal: Construye desde cero
const ProjectCard = styled('div')(({ theme }) => ({
  padding: theme.spacing(3),
  borderRadius: '4px',
  boxShadow: '0 2px 4px rgba(0,0,0,0.1)',
  // Duplicando la funcionalidad de Card
}));
```

**No hagas fork ni dupliques patrones que MUI ya cubre:**

* Usa `Card` en lugar de crear una caja personalizada con sombras
* Usa `Button` en lugar de crear un enlace estilizado
* Usa `TextField` en lugar de crear una entrada personalizada
* Extender `Dialog` en lugar de crear un modal personalizado

#### Solo valores del theme

**DEBEN** usa valores del theme de UI2:

```tsx
// ✅ Bien: Todos los valores del theme
const StyledBox = styled('div')(({ theme }) => ({
  color: theme.palette.text.primary,
  backgroundColor: theme.palette.background.paper,
  padding: theme.spacing(2),
  borderRadius: theme.shape.borderRadius,
  border: `1px solid ${theme.palette.divider}`,
}));

// ❌ Mal: Valores ad hoc
const StyledBox = styled('div')({
  color: '#333333',
  backgroundColor: '#FFFFFF',
  padding: '16px',
  borderRadius: '8px',
  border: '1px solid #E0E0E0',
});
```

**No se permiten valores arbitrarios:**

* Colores: Usa `theme.palette` o `dclColors`
* Espaciado: Usa `theme.spacing(n)`
* Radio de borde: Usa `theme.shape.borderRadius`
* Tipografía: Usa `theme.typography` variantes
* Puntos de quiebre: Usa `theme.breakpoints` helpers

#### Estados y accesibilidad

**DEBEN** define e implementa todos los estados interactivos:

```tsx
const ActionButton = styled('button')(({ theme }) => ({
  // Estado base/inactivo
  padding: theme.spacing(1, 2),
  backgroundColor: theme.palette.primary.main,
  color: theme.palette.primary.contrastText,
  border: 'none',
  borderRadius: theme.shape.borderRadius,
  cursor: 'pointer',
  transition: theme.transitions.create(['background-color', 'transform']),
  
  // Estado hover
  '&:hover': {
    backgroundColor: theme.palette.primary.dark,
  },
  
  // Estado focus (navegación con teclado)
  '&:focus-visible': {
    outline: `2px solid ${theme.palette.primary.main}`,
    outlineOffset: 2,
  },
  
  // Estado activo/presionado
  '&:active': {
    transform: 'scale(0.98)',
  },
  
  // Estado disabled
  '&:disabled': {
    backgroundColor: theme.palette.action.disabledBackground,
    color: theme.palette.action.disabled,
    cursor: 'not-allowed',
  },
}));
```

**DEBEN** implementa accesibilidad básica:\*\*

* **Navegación con teclado** - Se puede enfocar y operar con teclado
* **Indicadores de enfoque** - Estados de enfoque visibles
* **Etiquetas ARIA** - Cuando el texto no es visible
* **HTML semántico** - Usa elementos apropiados
* **Contraste de color** - Cumple con los estándares WCAG AA

### Ejemplo: componente específico del proyecto

```tsx
// src/components/LandCard/LandCard.tsx
import { Card, CardContent, CardActions, Typography, Button } from '@mui/material';
import { styled } from '@mui/material/styles';
import type { Parcel } from '@/types';

interface LandCardProps {
  parcel: Parcel;
  onTransfer: (id: string) => void;
  onView: (id: string) => void;
}

const StyledCard = styled(Card)(({ theme }) => ({
  display: 'flex',
  flexDirection: 'column',
  height: '100%',
  transition: theme.transitions.create('transform'),
  
  '&:hover': {
    transform: 'translateY(-4px)',
  },
}));

const CoordinatesText = styled(Typography)(({ theme }) => ({
  color: theme.palette.text.secondary,
  fontFamily: theme.typography.fontFamilyMono,
}));

export function LandCard({ parcel, onTransfer, onView }: LandCardProps) {
  return (
    <StyledCard>
      <CardContent>
        <Typography variant="h6" gutterBottom>
          {parcel.name || `Parcel ${parcel.x},${parcel.y}`}
        </Typography>
        <CoordinatesText variant="body2">
          ({parcel.x}, {parcel.y})
        </CoordinatesText>
        <Typography variant="body2" color="text.secondary">
          Propietario: {parcel.owner}
        </Typography>
      </CardContent>
      <CardActions>
        <Button size="small" onClick={() => onView(parcel.id)}>
          Ver
        </Button>
        <Button size="small" onClick={() => onTransfer(parcel.id)}>
          Transferir
        </Button>
      </CardActions>
    </StyledCard>
  );
}
```

***

## Componentes candidatos de UI2

Los componentes que se compartirán entre proyectos requieren estándares más altos y una documentación más exhaustiva.

### Requisitos

#### Alineación con el theme

**DEBEN** se basan exclusivamente en valores del theme de UI2:

```tsx
// ✅ Bien: Integración completa con el theme
const NavbarContainer = styled('nav')(({ theme }) => ({
  backgroundColor: theme.palette.background.paper,
  borderBottom: `1px solid ${theme.palette.divider}`,
  padding: theme.spacing(0, 2),
  height: 64,
  display: 'flex',
  alignItems: 'center',
  gap: theme.spacing(2),
  
  [theme.breakpoints.down('md')]: {
    padding: theme.spacing(0, 1),
  },
}));
```

#### Cobertura en Storybook

**DEBEN** añade historias completas en Storybook:

**Cobertura requerida:**

1. **Todas las props y variantes**
   * Cada combinación de props
   * Todas las variantes de tamaño
   * Todas las variantes de color
2. **Todos los estados**
   * Inactivo/por defecto
   * Cargando
   * Error
   * Deshabilitado
   * Hover (mediante el addon de pseudoestados)
   * Focus (mediante el addon de pseudoestados)
3. **Interacciones**
   * Manejadores de click
   * Envíos de formularios
   * Navegación con teclado
4. **Esquemas de color**
   * Modo claro
   * Modo oscuro
5. **Comportamiento responsive**
   * Puntos de quiebre clave (xs, md, lg)
   * Documenta el comportamiento en cada punto de quiebre

**Archivo de Storybook de ejemplo:**

```tsx
// Navbar.stories.tsx
import type { Meta, StoryObj } from '@storybook/react';
import { Navbar } from './Navbar';

const meta: Meta<typeof Navbar> = {
  title: 'Components/Navbar',
  component: Navbar,
  parameters: {
    layout: 'fullscreen',
  },
  argTypes: {
    variant: {
      control: 'select',
      options: ['default', 'compact'],
    },
    showUserMenu: {
      control: 'boolean',
    },
  },
};

export default meta;
type Story = StoryObj<typeof Navbar>;

export const Default: Story = {
  args: {
    variant: 'default',
    showUserMenu: true,
  },
};

export const Compact: Story = {
  args: {
    variant: 'compact',
    showUserMenu: true,
  },
};

export const WithoutUserMenu: Story = {
  args: {
    variant: 'default',
    showUserMenu: false,
  },
};

export const Loading: Story = {
  args: {
    variant: 'default',
    showUserMenu: true,
    isLoading: true,
  },
};

// Test different viewports
export const Mobile: Story = {
  args: {
    variant: 'compact',
    showUserMenu: true,
  },
  parameters: {
    viewport: {
      defaultViewport: 'mobile1',
    },
  },
};

export const Tablet: Story = {
  args: {
    variant: 'default',
    showUserMenu: true,
  },
  parameters: {
    viewport: {
      defaultViewport: 'tablet',
    },
  },
};

// Test color schemes
export const DarkMode: Story = {
  args: {
    variant: 'default',
    showUserMenu: true,
  },
  parameters: {
    backgrounds: {
      default: 'dark',
    },
  },
};
```

### Estructura del componente

Los componentes candidatos de UI2 DEBEN seguir esta estructura:

```
src/components/Navbar/
├── Navbar.tsx           # Componente principal
├── Navbar.styles.ts     # Componentes estilizados
├── Navbar.stories.tsx   # Historias de Storybook
├── Navbar.test.tsx      # Pruebas unitarias
├── types.ts             # Tipos de TypeScript
├── index.ts             # Exportaciones públicas
└── README.md            # Documentación del componente
```

### Requisitos de documentación

**DEBEN** incluye en el README del componente:

1. **Propósito** - ¿Qué problema resuelve?
2. **Uso** - Cómo usar el componente
3. **Props** - Todas las props con tipos y descripciones
4. **Ejemplos** - Casos de uso comunes
5. **Accesibilidad** - Soporte de teclado, etiquetas ARIA
6. **Tematización** - Qué valores del theme usa
7. **Notas de migración** - Si reemplaza un componente de UI1

**Ejemplo de README:**

````markdown
# Navbar

Componente de navegación en todo el sitio con menú de usuario y comportamiento responsive.

## Uso

\```tsx
import { Navbar } from 'decentraland-ui2';

function App() {
  return (
    <Navbar
      variant="default"
      showUserMenu={true}
      onLogoClick={() => navigate('/')}
      onLoginClick={handleLogin}
    />
  );
}
\```

## Props

| Prop         | Tipo                   | Predeterminado | Descripción                   |
| ------------ | ---------------------- | --------- | ----------------------------- |
| variant      | 'default' \| 'compact' | 'default' | Variante de navegación        |
| showUserMenu | boolean                | true      | Mostrar el menú de usuario cuando haya sesión iniciada |
| onLogoClick  | () => void             | -         | Controlador del click en el logo            |
| onLoginClick | () => void             | -         | Controlador del click en el botón de inicio de sesión    |

## Accesibilidad

* Navegación con teclado: Tabulador para recorrer los elementos del menú
* ARIA: Landmarks y etiquetas correctos
* Lector de pantalla: Anuncia el estado del menú

## Tematización

Usa estos valores del theme:

* `theme.palette.background.paper`
* `theme.palette.divider`
* `theme.spacing`
* `theme.breakpoints`

````

### Requisitos de pruebas

**DEBEN** incluye pruebas para:

* Renderizado de props
* Interacciones del usuario
* Funciones de accesibilidad
* Comportamiento responsive
* Estados de error

```tsx
// Navbar.test.tsx
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Navbar } from './Navbar';

describe('Navbar', () => {
  it('should render logo', () => {
    render(<Navbar />);
    expect(screen.getByRole('banner')).toBeInTheDocument();
  });

  it('should call onLogoClick when logo is clicked', async () => {
    const onLogoClick = jest.fn();
    render(<Navbar onLogoClick={onLogoClick} />);
    
    await userEvent.click(screen.getByRole('link', { name: /decentraland/i }));
    expect(onLogoClick).toHaveBeenCalled();
  });

  it('should be keyboard navigable', async () => {
    render(<Navbar />);
    const firstLink = screen.getAllByRole('link')[0];
    
    firstLink.focus();
    expect(firstLink).toHaveFocus();
  });
});
```

***

## Matriz de decisión

Usa esto para decidir qué tipo de componente crear:

| Pregunta                                       | Específico del proyecto | Candidato de UI2   |
| ---------------------------------------------- | ----------------------- | ------------------ |
| ¿Otros proyectos usarán esto?                  | No                      | Sí                 |
| ¿UI1 tiene un equivalente?                     | N/A                     | Probablemente      |
| ¿Necesita documentación en Storybook?          | No                      | **Sí**             |
| ¿Necesita pruebas exhaustivas?                 | Básicas                 | **Extensas**       |
| ¿Se requiere revisión de diseño?               | A nivel de proyecto     | **A nivel de UI2** |
| ¿Puede usar patrones específicos del proyecto? | Sí                      | **No**             |
| ¿Debe funcionar en todos los themes?           | No                      | **Sí**             |

***

## Proceso de aprobación

### Componentes específicos del proyecto

1. Revisión de código por el mantenedor del proyecto
2. Verificar el cumplimiento del theme
3. Probar en el contexto del proyecto
4. Fusionar cuando se apruebe

### Componentes candidatos de UI2

1. Revisión y aprobación de diseño
2. Revisión técnica del diseño
3. Implementación
4. Historias de Storybook
5. Pruebas exhaustivas
6. Revisión de accesibilidad
7. Revisión de código
8. PR al repositorio de UI2
9. Versionar y publicar
10. Actualizar los proyectos dependientes

***

## Buenas prácticas

### Composición sobre personalización

```tsx
// ✅ Bien: Compón componentes de MUI
function FeatureCard({ title, children }) {
  return (
    <Card>
      <CardContent>
        <Typography variant="h6">{title}</Typography>
        {children}
      </CardContent>
    </Card>
  );
}

// ❌ Mal: Re-implementa la funcionalidad de Card
function FeatureCard({ title, children }) {
  return (
    <div className="custom-card">
      <div className="custom-card-content">
        <h3>{title}</h3>
        {children}
      </div>
    </div>
  );
}
```

### Mejora progresiva

Empieza de forma simple y añade funciones según sea necesario:

1. Versión básica con funcionalidad principal
2. Añade comportamiento adaptable
3. Añade funciones de accesibilidad
4. Añade interacciones avanzadas
5. Optimiza el rendimiento

### La documentación primero

Antes de escribir código:

1. Escribe el README del component
2. Define la interfaz de props
3. Enumera los estados requeridos
4. Planifica las historias de Storybook
5. Luego implementa

***

## Próximos pasos

* Revisa [Estándares de estilo y tematización](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui-web/styling-and-theming/) para detalles de implementación
* Consulta [Guía de migración](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui-web/migration/) para migraciones de UI1 a UI2
* Revisa [Resumen del proceso](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui-web/process-overview/) para todo el flujo de trabajo


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui-web/custom-components.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
