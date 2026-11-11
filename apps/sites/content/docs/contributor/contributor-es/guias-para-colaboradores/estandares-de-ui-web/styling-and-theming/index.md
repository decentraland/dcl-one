# Estilos y temas

Esta página cubre estándares completos de estilo para las UIs web de Decentraland usando styled-components con la solución de estilo de Material UI.


> [!NOTE]
> Todos los ejemplos de código de este documento son ilustrativos. No representan componentes de producción, sino que demuestran patrones y estándares.


## Principios fundamentales

1. **Solo sintaxis de objeto** - Usa notación de objeto para un sólido soporte de TypeScript
2. **Primero el theme** - Todos los valores provienen del theme de UI2
3. **Sin estilos en línea** - Usa componentes styled para todo el estilo
4. **Todo tipado** - Aprovecha TypeScript para props y theme
5. **Responsive por defecto** - Usa breakpoints del theme

***

## Estándar de sintaxis de objeto

Componentes de UI2 **DEBEN** usar la sintaxis de objeto. Esto garantiza un sólido soporte de TypeScript, validación de csstype e integración directa con el theme.


> [!WARNING]
> La sintaxis de literal de plantilla **no está permitida**. Usa siempre notación de objeto.


### Ejemplo básico

```tsx
// ✅ Bien: sintaxis de objeto
const Button = styled('button')({
  color: 'turquoise',
  padding: '8px 16px',
});

// ❌ Mal: sintaxis de literal de plantilla
const Button = styled.button`
  color: turquoise;
  padding: 8px 16px;
`;
```

### Con Theme y props

```tsx
interface ButtonProps {
  primary?: boolean;
  size?: 'small' | 'medium' | 'large';
}

// ✅ Bien: sintaxis de objeto con theme y props
const Button = styled('button')<ButtonProps>(({ theme, primary, size = 'medium' }) => ({
  color: primary ? theme.palette.primary.main : theme.palette.text.primary,
  backgroundColor: primary ? theme.palette.primary.main : 'transparent',
  borderRadius: theme.shape.borderRadius,
  padding: {
    small: theme.spacing(0.5, 1),
    medium: theme.spacing(1, 2),
    large: theme.spacing(1.5, 3),
  }[size],
}));
```


> [!CAUTION]
> **Regla**: Los valores SIEMPRE deben provenir del theme de UI2. No se permiten códigos hexadecimales arbitrarios ni valores de píxeles.


***

## Sintaxis de elemento

Al dar estilo a elementos HTML nativos, usa siempre la forma de llamada de función `styled('tag')`.

### Sintaxis correcta

```tsx
// ✅ Bien: forma de llamada de función
const Container = styled('div')({
  display: 'flex',
  flexDirection: 'column',
});

const Action = styled('button')(({ theme }) => ({
  color: theme.palette.primary.main,
  padding: theme.spacing(1, 2),
}));

const Label = styled('label')(({ theme }) => ({
  color: theme.palette.text.secondary,
  fontSize: theme.typography.caption.fontSize,
}));
```

### Sintaxis incorrecta

```tsx
// ❌ Mal: forma de propiedad (sintaxis heredada)
const Container = styled.div`
  display: flex;
  flex-direction: column;
`;

const Action = styled.button`
  color: ${props => props.theme.palette.primary.main};
`;
```

***

## Sin estilos en línea

Los estilos en línea (`style={...}`) **NO DEBEN** usarse en componentes de UI2.

### ¿Por qué no usar estilos en línea?

* Eluden la tipificación del theme
* Más difíciles de mantener
* Impiden la reutilización
* No se pueden optimizar
* Sin validación de TypeScript

### La forma correcta

```tsx
// ❌ Mal: estilos en línea
<Card 
  key={id} 
  style={{ backgroundColor: color } as React.CSSProperties}
>
  <CardHeader>
    <Title style={{ fontSize: 20 }}>{title}</Title>
  </CardHeader>
</Card>

// ✅ Bien: componentes styled con props
interface CardProps {
  backgroundColor: string;
}

interface TitleProps {
  size: number;
}

const StyledCard = styled('div')<CardProps>(({ backgroundColor, theme }) => ({
  backgroundColor,
  borderRadius: theme.shape.borderRadius,
  padding: theme.spacing(2),
}));

const Title = styled('h2')<TitleProps>(({ size, theme }) => ({
  fontSize: theme.typography.pxToRem(size),
  color: theme.palette.text.primary,
}));

<StyledCard key={id} backgroundColor={color}>
  <CardHeader>
    <Title size={20}>{title}</Title>
  </CardHeader>
</StyledCard>
```

***

## Puntos de interrupción

Usa `theme.breakpoints` helpers en lugar de valores de píxeles codificados.

### Helpers de breakpoints

| Ayudante              | Uso                                     | Descripción             |
| --------------------- | --------------------------------------- | ----------------------- |
| `up(key)`             | `theme.breakpoints.up('md')`            | Ancho mínimo y superior |
| `down(key)`           | `theme.breakpoints.down('md')`          | Ancho máximo e inferior |
| `between(start, end)` | `theme.breakpoints.between('sm', 'lg')` | Entre dos breakpoints   |
| `only(key)`           | `theme.breakpoints.only('md')`          | Solo en este breakpoint |

### Ejemplos

```tsx
// ❌ Mal: breakpoints codificados
const Panel = styled('div')`
  @media (max-width: 768px) {
    width: 100%;
  }
`;

// ✅ Bien: breakpoints del theme
interface PanelProps {
  expanded: boolean;
}

const Panel = styled('div')<PanelProps>(({ theme, expanded }) => ({
  width: expanded ? '400px' : '0',
  transition: theme.transitions.create('width'),
  
  [theme.breakpoints.down('sm')]: {
    width: expanded ? '100%' : '0',
  },
}));
```

### Múltiples breakpoints

```tsx
const Layout = styled('div')(({ theme }) => ({
  display: 'grid',
  gridTemplateColumns: '1fr 320px',
  gap: theme.spacing(2),
  
  // Móvil: una sola columna
  [theme.breakpoints.down('md')]: {
    gridTemplateColumns: '1fr',
  },
  
  // Escritorio grande: barra lateral más ancha
  [theme.breakpoints.up('xl')]: {
    gridTemplateColumns: '1fr 400px',
  },
  
  // Rango de tablet: gap diferente
  [theme.breakpoints.between('sm', 'lg')]: {
    gap: theme.spacing(3),
  },
  
  // Solo tablet
  [theme.breakpoints.only('md')]: {
    padding: theme.spacing(2),
  },
}));
```

***

## Escala de espaciado

Usa `theme.spacing` exclusivamente para márgenes, rellenos y separaciones.

### Convención de espaciado

* `theme.spacing(n)` donde `n` es un número
* La unidad base suele ser 8px
* `spacing(1)` = 8px, `spacing(2)` = 16px, etc.
* Se permiten decimales: `spacing(1.5)` = 12px

```tsx
// ✅ Bien: espaciado del theme
const Box = styled('div')(({ theme }) => ({
  padding: theme.spacing(2),           // 16px
  margin: theme.spacing(1, 0),         // 8px vertical, 0 horizontal
  gap: theme.spacing(1.5),             // 12px
  paddingInline: theme.spacing(3),     // 24px izquierda/derecha
}));

// ❌ Mal: valores de píxeles en bruto
const Box = styled('div')({
  padding: '16px',
  margin: '8px 0',
  gap: '12px',
});
```

### Patrones comunes de espaciado

```tsx
const Card = styled('div')(({ theme }) => ({
  // Espaciado uniforme en todos los lados
  padding: theme.spacing(3),
  
  // Diferente en vertical/horizontal
  padding: theme.spacing(2, 3),  // 16px vertical, 24px horizontal
  
  // Todos los lados diferentes
  padding: theme.spacing(1, 2, 3, 2),  // superior, derecha, inferior, izquierda
  
  // Propiedades lógicas (preferidas para soporte RTL)
  paddingBlock: theme.spacing(2),      // superior e inferior
  paddingInline: theme.spacing(3),     // izquierda y derecha
  marginBlockStart: theme.spacing(1),  // margin-top
}));
```

***

## Z-Index y apilamiento

Sigue la escala de z-index del theme. Nunca uses valores arbitrarios de z-index.

### Valores de z-index del theme

```tsx
theme.zIndex.mobileStepper  // 1000
theme.zIndex.fab            // 1050
theme.zIndex.speedDial      // 1050
theme.zIndex.appBar         // 1100
theme.zIndex.drawer         // 1200
theme.zIndex.modal          // 1300
theme.zIndex.snackbar       // 1400
theme.zIndex.tooltip        // 1500
```

### Uso correcto

```tsx
// ✅ Bien: z-index del theme
const StickyBar = styled('div')(({ theme }) => ({
  position: 'sticky',
  top: 0,
  zIndex: theme.zIndex.appBar,
  backgroundColor: theme.palette.background.paper,
}));

const Overlay = styled('div')(({ theme }) => ({
  position: 'fixed',
  inset: 0,
  zIndex: theme.zIndex.modal,
  backgroundColor: 'rgba(0, 0, 0, 0.5)',
}));

// ❌ Mal: z-index arbitrario
const StickyBar = styled('div')({
  position: 'sticky',
  top: 0,
  zIndex: 999,  // No hagas esto
});
```

### Mejores prácticas del contexto de apilamiento

* Sé consciente al crear nuevos contextos de apilamiento
* Evita usar `position: relative` innecesariamente en los padres
* Documenta por qué se necesita un z-index
* Si necesitas una nueva capa, añádela primero al theme

***

## Tokens de color

Usa siempre los colores de `theme.palette` y `dclColors`. Sin valores hexadecimales ad hoc.

### Estructura de la paleta

```tsx
// Colores de texto
theme.palette.text.primary
theme.palette.text.secondary
theme.palette.text.disabled

// Colores de fondo
theme.palette.background.default
theme.palette.background.paper

// Primario/Secundario/Error
theme.palette.primary.main
theme.palette.primary.light
theme.palette.primary.dark
theme.palette.primary.contrastText

// Colores de acción
theme.palette.action.active
theme.palette.action.hover
theme.palette.action.selected
theme.palette.action.disabled
theme.palette.action.disabledBackground

// Divisores
theme.palette.divider
```

### Colores de Decentraland

```tsx
import { dclColors } from 'decentraland-ui2';

// Colores de rareza
dclColors.rarity.unique
dclColors.rarity.mythic
dclColors.rarity.legendary
dclColors.rarity.epic
dclColors.rarity.rare
dclColors.rarity.uncommon
dclColors.rarity.common
```

### Ejemplos

```tsx
// ✅ Bien: colores del theme
const Chip = styled('span')(({ theme }) => ({
  color: theme.palette.text.secondary,
  backgroundColor: theme.palette.background.paper,
  borderColor: theme.palette.divider,
  
  '&:hover': {
    backgroundColor: theme.palette.action.hover,
  },
}));

const RarityBadge = styled('span')<{ rarity: string }>(({ rarity }) => ({
  backgroundColor: dclColors.rarity[rarity],
  color: '#FFFFFF', // Color de contraste: aceptable si está documentado
  padding: '4px 8px',
  borderRadius: '4px',
}));

// ❌ Mal: valores hexadecimales arbitrarios
const Chip = styled('span')({
  color: '#666666',
  backgroundColor: '#FFFFFF',
  borderColor: '#E0E0E0',
});
```

***

## Estados interactivos

Todos los controles interactivos DEBEN mostrar estados visibles para hover, focus, active y disabled.

### Componente interactivo completo

```tsx
const InteractiveButton = styled('button')(({ theme }) => ({
  // Estado base
  padding: theme.spacing(1, 2),
  backgroundColor: theme.palette.primary.main,
  color: theme.palette.primary.contrastText,
  border: 'none',
  borderRadius: theme.shape.borderRadius,
  cursor: 'pointer',
  outline: 'none',
  transition: theme.transitions.create([
    'background-color',
    'transform',
    'box-shadow',
  ]),
  
  // Estado hover (mouse)
  '&:hover': {
    backgroundColor: theme.palette.primary.dark,
  },
  
  // Estado focus (navegación con teclado)
  '&:focus-visible': {
    outline: `2px solid ${theme.palette.primary.main}`,
    outlineOffset: 2,
    boxShadow: theme.shadows[2],
  },
  
  // Estado active/pressed
  '&:active': {
    backgroundColor: theme.palette.primary.dark,
    transform: 'scale(0.98)',
  },
  
  // Estado disabled
  '&:disabled': {
    backgroundColor: theme.palette.action.disabledBackground,
    color: theme.palette.action.disabled,
    cursor: 'not-allowed',
    transform: 'none',
  },
}));
```

### Patrón focus-visible

Usa siempre `:focus-visible` en lugar de `:focus` para evitar mostrar anillos de foco en clics del mouse:

```tsx
// ✅ Bueno: Solo muestra el anillo de foco para el teclado
'&:focus-visible': {
  outline: `2px solid ${theme.palette.primary.main}`,
  outlineOffset: 2,
}

// ❌ Malo: Muestra el anillo de foco en cada clic
'&:focus': {
  outline: `2px solid ${theme.palette.primary.main}`,
}
```

***

## Tipografía

Usa las variantes tipográficas del theme en lugar de propiedades de fuente personalizadas.

```tsx
// ✅ Bueno: Variantes tipográficas
const Heading = styled('h1')(({ theme }) => ({
  ...theme.typography.h1,
  marginBottom: theme.spacing(2),
}));

const Body = styled('p')(({ theme}) => ({
  ...theme.typography.body1,
  color: theme.palette.text.secondary,
}));

// Con el componente Typography de MUI (preferido)
<Typography variant="h1">Heading</Typography>
<Typography variant="body1">Texto del cuerpo</Typography>

// ❌ Malo: Tipografía personalizada
const Heading = styled('h1')({
  fontSize: '32px',
  fontWeight: 700,
  lineHeight: 1.2,
});
```

***

## Optimización del rendimiento

### No crees Styled Components en render

```tsx
// ❌ Malo: Crea un nuevo componente en cada render
function Component({ color }) {
  const Box = styled('div')({
    backgroundColor: color,
  });
  return <Box />;
}

// ✅ Bueno: Créalo una vez, pasa props
const Box = styled('div')<{ color: string }>(({ color }) => ({
  backgroundColor: color,
}));

function Component({ color }) {
  return <Box color={color} />;
}
```

### Memoiza las props derivadas

```tsx
// ❌ Malo: Crea un nuevo objeto en cada render
<StyledComponent style={{ color: isDark ? 'white' : 'black' }} />

// ✅ Bueno: Pasa props primitivas
const StyledComponent = styled('div')<{ isDark: boolean }>(({ isDark, theme }) => ({
  color: isDark ? theme.palette.common.white : theme.palette.common.black,
}));

<StyledComponent isDark={isDark} />
```

***

## Convenciones de nomenclatura

### Nombres de componentes

* **PascalCase** para componentes
* Nombres descriptivos

```tsx
// ✅ Nombres correctos
const UserCard = styled('div')({...});
const PrimaryButton = styled('button')({...});
const NavigationList = styled('ul')({...});

// ❌ Nombres incorrectos
const card = styled('div')({...});
const btn = styled('button')({...});
const list1 = styled('ul')({...});
```

### Estructura de archivos

```
Component.tsx        # Componente principal
Component.styles.ts  # Componentes con estilos
Component.test.tsx   # Pruebas
Component.stories.tsx # Storybook
```

***

## Próximos pasos

* Revisa [Componentes personalizados](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui-web/custom-components/) para las directrices de creación de componentes
* Consulta [Guía de migración](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui-web/migration/) para migraciones de UI1 a UI2
* Revisa [Resumen del proceso](/docs/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui-web/process-overview/) para el flujo de trabajo completo


---

# Agent Instructions
This documentation is published with GitBook. GitBook is the documentation platform designed so that both humans and AI agents can read, navigate, and reason over technical content effectively. Learn more at gitbook.com.

## Querying This Documentation
If you need additional information that is not directly available in this page, you can query the documentation by asking a question.

Perform an HTTP GET request on the following URL with the `ask` and `goal` query parameters:

```
GET https://docs.decentraland.org/contributor/contributor-es/guias-para-colaboradores/estandares-de-ui-web/styling-and-theming.md?ask=<question>&goal=<user_goal>
```

`ask` is the immediate question: it should be specific, self-contained, and written in natural language.
`goal` is what the user is ultimately trying to achieve, the reason they need the answer. Sharing it helps GitBook give you a better, more relevant answer. A goal is most helpful when it describes the outcome the user wants rather than restating the question. For example, with `ask=how do I create an API token`, a goal like `automate deployments from our CI pipeline` lets GitBook tailor the answer to that use case.

The response will contain a direct answer to the question and relevant excerpts and sources from the documentation.

Use this mechanism when the answer is not explicitly present in the current page, you need clarification or additional context, or you want to retrieve related documentation sections.
