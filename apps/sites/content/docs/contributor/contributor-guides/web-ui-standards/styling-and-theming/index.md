# Styling & Theming

This page covers comprehensive styling standards for Decentraland web UIs using styled-components with Material UI's styling solution.


> [!NOTE]
> All code examples in this document are illustrative. They do not represent production components but demonstrate patterns and standards.


## Core Principles

1. **Object syntax only** - Use object notation for strong TypeScript support
2. **Theme-first** - All values come from the UI2 theme
3. **No inline styles** - Use styled components for all styling
4. **Typed everything** - Leverage TypeScript for props and theme
5. **Responsive by default** - Use theme breakpoints

***

## Object Syntax Standard

UI2 components **MUST** use the object syntax. This ensures strong TypeScript support, csstype validation, and direct theme integration.


> [!WARNING]
> Template literal syntax is **not allowed**. Always use object notation.


### Basic Example

```tsx
// ✅ Good: Object syntax
const Button = styled('button')({
  color: 'turquoise',
  padding: '8px 16px',
});

// ❌ Bad: Template literal syntax
const Button = styled.button`
  color: turquoise;
  padding: 8px 16px;
`;
```

### With Theme and Props

```tsx
interface ButtonProps {
  primary?: boolean;
  size?: 'small' | 'medium' | 'large';
}

// ✅ Good: Object syntax with theme and props
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
> **Rule**: Values MUST always come from the UI2 theme. Arbitrary hex codes or pixel values are not allowed.


***

## Element Syntax

When styling native HTML elements, always use the function call form `styled('tag')`.

### Correct Syntax

```tsx
// ✅ Good: Function call form
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

### Incorrect Syntax

```tsx
// ❌ Bad: Property form (legacy syntax)
const Container = styled.div`
  display: flex;
  flex-direction: column;
`;

const Action = styled.button`
  color: ${props => props.theme.palette.primary.main};
`;
```

***

## No Inline Styles

Inline styles (`style={...}`) **MUST NOT** be used in UI2 components.

### Why Not Inline Styles?

* Bypass theme typing
* Harder to maintain
* Prevent reusability
* Can't be optimized
* No TypeScript validation

### The Right Way

```tsx
// ❌ Bad: Inline styles
<Card 
  key={id} 
  style={{ backgroundColor: color } as React.CSSProperties}
>
  <CardHeader>
    <Title style={{ fontSize: 20 }}>{title}</Title>
  </CardHeader>
</Card>

// ✅ Good: Styled components with props
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

## Breakpoints

Use `theme.breakpoints` helpers instead of hardcoded pixel values.

### Breakpoint Helpers

| Helper                | Usage                                   | Description             |
| --------------------- | --------------------------------------- | ----------------------- |
| `up(key)`             | `theme.breakpoints.up('md')`            | Min-width and up        |
| `down(key)`           | `theme.breakpoints.down('md')`          | Max-width and down      |
| `between(start, end)` | `theme.breakpoints.between('sm', 'lg')` | Between two breakpoints |
| `only(key)`           | `theme.breakpoints.only('md')`          | Only at this breakpoint |

### Examples

```tsx
// ❌ Bad: Hardcoded breakpoints
const Panel = styled('div')`
  @media (max-width: 768px) {
    width: 100%;
  }
`;

// ✅ Good: Theme breakpoints
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

### Multiple Breakpoints

```tsx
const Layout = styled('div')(({ theme }) => ({
  display: 'grid',
  gridTemplateColumns: '1fr 320px',
  gap: theme.spacing(2),
  
  // Mobile: single column
  [theme.breakpoints.down('md')]: {
    gridTemplateColumns: '1fr',
  },
  
  // Large desktop: wider sidebar
  [theme.breakpoints.up('xl')]: {
    gridTemplateColumns: '1fr 400px',
  },
  
  // Tablet range: different gap
  [theme.breakpoints.between('sm', 'lg')]: {
    gap: theme.spacing(3),
  },
  
  // Only tablet
  [theme.breakpoints.only('md')]: {
    padding: theme.spacing(2),
  },
}));
```

***

## Spacing Scale

Use `theme.spacing` exclusively for margins, paddings, and gaps.

### Spacing Convention

* `theme.spacing(n)` where `n` is a number
* Base unit is typically 8px
* `spacing(1)` = 8px, `spacing(2)` = 16px, etc.
* Decimals allowed: `spacing(1.5)` = 12px

```tsx
// ✅ Good: Theme spacing
const Box = styled('div')(({ theme }) => ({
  padding: theme.spacing(2),           // 16px
  margin: theme.spacing(1, 0),         // 8px vertical, 0 horizontal
  gap: theme.spacing(1.5),             // 12px
  paddingInline: theme.spacing(3),     // 24px left/right
}));

// ❌ Bad: Raw pixel values
const Box = styled('div')({
  padding: '16px',
  margin: '8px 0',
  gap: '12px',
});
```

### Common Spacing Patterns

```tsx
const Card = styled('div')(({ theme }) => ({
  // Even spacing all around
  padding: theme.spacing(3),
  
  // Different vertical/horizontal
  padding: theme.spacing(2, 3),  // 16px vertical, 24px horizontal
  
  // All sides different
  padding: theme.spacing(1, 2, 3, 2),  // top, right, bottom, left
  
  // Logical properties (preferred for RTL support)
  paddingBlock: theme.spacing(2),      // top and bottom
  paddingInline: theme.spacing(3),     // left and right
  marginBlockStart: theme.spacing(1),  // margin-top
}));
```

***

## Z-Index and Stacking

Follow the theme's z-index scale. Never use arbitrary z-index values.

### Theme Z-Index Values

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

### Correct Usage

```tsx
// ✅ Good: Theme z-index
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

// ❌ Bad: Arbitrary z-index
const StickyBar = styled('div')({
  position: 'sticky',
  top: 0,
  zIndex: 999,  // Don't do this
});
```

### Stacking Context Best Practices

* Be conscious of creating new stacking contexts
* Avoid unnecessary `position: relative` on parents
* Document why a z-index is needed
* If you need a new layer, add it to the theme first

***

## Color Tokens

Always consume colors from `theme.palette` and `dclColors`. No ad-hoc hex values.

### Palette Structure

```tsx
// Text colors
theme.palette.text.primary
theme.palette.text.secondary
theme.palette.text.disabled

// Background colors
theme.palette.background.default
theme.palette.background.paper

// Primary/Secondary/Error
theme.palette.primary.main
theme.palette.primary.light
theme.palette.primary.dark
theme.palette.primary.contrastText

// Action colors
theme.palette.action.active
theme.palette.action.hover
theme.palette.action.selected
theme.palette.action.disabled
theme.palette.action.disabledBackground

// Dividers
theme.palette.divider
```

### Decentraland Colors

```tsx
import { dclColors } from 'decentraland-ui2';

// Rarity colors
dclColors.rarity.unique
dclColors.rarity.mythic
dclColors.rarity.legendary
dclColors.rarity.epic
dclColors.rarity.rare
dclColors.rarity.uncommon
dclColors.rarity.common
```

### Examples

```tsx
// ✅ Good: Theme colors
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
  color: '#FFFFFF', // Contrast color - acceptable if documented
  padding: '4px 8px',
  borderRadius: '4px',
}));

// ❌ Bad: Arbitrary hex values
const Chip = styled('span')({
  color: '#666666',
  backgroundColor: '#FFFFFF',
  borderColor: '#E0E0E0',
});
```

***

## Interactive States

All interactive controls MUST show visible states for hover, focus, active, and disabled.

### Complete Interactive Component

```tsx
const InteractiveButton = styled('button')(({ theme }) => ({
  // Base state
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
  
  // Hover state (mouse)
  '&:hover': {
    backgroundColor: theme.palette.primary.dark,
  },
  
  // Focus state (keyboard navigation)
  '&:focus-visible': {
    outline: `2px solid ${theme.palette.primary.main}`,
    outlineOffset: 2,
    boxShadow: theme.shadows[2],
  },
  
  // Active/Pressed state
  '&:active': {
    backgroundColor: theme.palette.primary.dark,
    transform: 'scale(0.98)',
  },
  
  // Disabled state
  '&:disabled': {
    backgroundColor: theme.palette.action.disabledBackground,
    color: theme.palette.action.disabled,
    cursor: 'not-allowed',
    transform: 'none',
  },
}));
```

### Focus-Visible Pattern

Always use `:focus-visible` instead of `:focus` to avoid showing focus rings on mouse clicks:

```tsx
// ✅ Good: Only shows focus ring for keyboard
'&:focus-visible': {
  outline: `2px solid ${theme.palette.primary.main}`,
  outlineOffset: 2,
}

// ❌ Bad: Shows focus ring on every click
'&:focus': {
  outline: `2px solid ${theme.palette.primary.main}`,
}
```

***

## Typography

Use theme typography variants instead of custom font properties.

```tsx
// ✅ Good: Typography variants
const Heading = styled('h1')(({ theme }) => ({
  ...theme.typography.h1,
  marginBottom: theme.spacing(2),
}));

const Body = styled('p')(({ theme}) => ({
  ...theme.typography.body1,
  color: theme.palette.text.secondary,
}));

// With MUI Typography component (preferred)
<Typography variant="h1">Heading</Typography>
<Typography variant="body1">Body text</Typography>

// ❌ Bad: Custom typography
const Heading = styled('h1')({
  fontSize: '32px',
  fontWeight: 700,
  lineHeight: 1.2,
});
```

***

## Performance Optimization

### Don't Create Styled Components in Render

```tsx
// ❌ Bad: Creates new component on every render
function Component({ color }) {
  const Box = styled('div')({
    backgroundColor: color,
  });
  return <Box />;
}

// ✅ Good: Create once, pass props
const Box = styled('div')<{ color: string }>(({ color }) => ({
  backgroundColor: color,
}));

function Component({ color }) {
  return <Box color={color} />;
}
```

### Memoize Derived Props

```tsx
// ❌ Bad: Creates new object on every render
<StyledComponent style={{ color: isDark ? 'white' : 'black' }} />

// ✅ Good: Pass primitive props
const StyledComponent = styled('div')<{ isDark: boolean }>(({ isDark, theme }) => ({
  color: isDark ? theme.palette.common.white : theme.palette.common.black,
}));

<StyledComponent isDark={isDark} />
```

***

## Naming Conventions

### Component Names

* **PascalCase** for components
* Descriptive names

```tsx
// ✅ Good names
const UserCard = styled('div')({...});
const PrimaryButton = styled('button')({...});
const NavigationList = styled('ul')({...});

// ❌ Bad names
const card = styled('div')({...});
const btn = styled('button')({...});
const list1 = styled('ul')({...});
```

### File Structure

```
Component.tsx        # Main component
Component.styles.ts  # Styled components
Component.test.tsx   # Tests
Component.stories.tsx # Storybook
```

***

## Next Steps

* Review [Custom Components](/docs/contributor/contributor-guides/web-ui-standards/custom-components/) for component creation guidelines
* See [Migration Guide](/docs/contributor/contributor-guides/web-ui-standards/migration/) for UI1 to UI2 migrations
* Check [Process Overview](/docs/contributor/contributor-guides/web-ui-standards/process-overview/) for the complete workflow
