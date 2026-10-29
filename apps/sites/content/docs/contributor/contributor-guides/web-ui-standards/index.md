# Web UI Standards

This section explains how Decentraland builds web UI for dApps using our component library and design system. These standards ensure consistency, accessibility, and maintainability across all web applications.

## Overview

Our web UI architecture is built on three core pillars:

* **UI2 (`decentraland-ui2`)** - Our primary component library and design system
* **Material UI (MUI)** - The foundation wrapped by UI2 with Decentraland theming
* **Design System** - Unified colors, typography, spacing, and components

### UI2: The Single Source of Truth

**UI2** is our current component library that wraps Material UI components with Decentraland's theme and provides custom components like `Navbar` that don't exist in MUI.

* **Live Demo**: [ui2.decentraland.org](http://ui2.decentraland.org)
* **GitHub**: [github.com/decentraland/ui2](https://github.com/decentraland/ui2)
* **Figma Library**: [Decentraland UI2 on Figma](https://www.figma.com/design/tsyaDSedcsVZ8iM9N0McT2/DCL-UI2)

### UI1: Legacy Library

**UI1** (`decentraland-ui`) is our previous component library, still maintained but being gradually migrated to UI2. See the [Migration Guide](/docs/contributor/contributor-guides/web-ui-standards/migration/) for details on transitioning components.

## Documentation Sections

* [**Process Overview**](/docs/contributor/contributor-guides/web-ui-standards/process-overview/) - UX discovery, Figma design, and implementation workflow
* [**Custom Components**](/docs/contributor/contributor-guides/web-ui-standards/custom-components/) - Guidelines for project-specific and reusable components
* [**Styling & Theming**](/docs/contributor/contributor-guides/web-ui-standards/styling-and-theming/) - Comprehensive styling standards using styled-components
* [**Migration Guide**](/docs/contributor/contributor-guides/web-ui-standards/migration/) - Migrating components from UI1 to UI2

## Key Principles

### 1. Design-Implementation Alignment

What's designed in Figma is what gets built in code. Our process ensures:

* UX defines goals, flows, and states
* Designers work from our Figma library (mirrors MUI + Decentraland theme)
* Developers implement using UI2 components

### 2. Theme-First Approach

All visual properties MUST come from the UI2 theme:

* **Colors**: `theme.palette` and `dclColors`
* **Typography**: Material Design type scale
* **Spacing**: `theme.spacing(n)`
* **Breakpoints**: Predefined responsive breakpoints
* **No arbitrary values**: No ad-hoc hex codes or pixel values

### 3. Accessibility by Default

Every component MUST:

* Support keyboard navigation
* Provide visible focus states
* Include ARIA labels where appropriate
* Meet color contrast requirements
* Handle all states (loading, error, disabled)

### 4. Strong Typing

Use TypeScript throughout:

* Typed theme with autocomplete
* Typed component props
* Typed styled-components
* No `any` types

## Component Hierarchy

```
UI2 (decentraland-ui2)
├── MUI Components (themed)
│   ├── Button, Card, TextField, etc.
│   └── Decentraland theme applied
├── Custom Components
│   ├── Navbar
│   ├── UserMenu
│   └── Other Decentraland-specific components
└── Theme System
    ├── Colors (dclColors)
    ├── Typography
    ├── Spacing
    └── Breakpoints
```

## Quick Start Example

```tsx
import { ThemeProvider } from '@mui/material/styles';
import { Button, Card } from '@mui/material';
import { theme, dclColors } from 'decentraland-ui2';
import { styled } from '@mui/material/styles';

// Use UI2 theme provider
function App() {
  return (
    <ThemeProvider theme={theme}>
      <MyComponent />
    </ThemeProvider>
  );
}

// Use MUI components directly
function MyComponent() {
  return (
    <Card>
      <Button variant="contained" color="primary">
        Click Me
      </Button>
    </Card>
  );
}

// Create styled components using theme
const CustomCard = styled('div')(({ theme }) => ({
  padding: theme.spacing(2),
  borderRadius: theme.shape.borderRadius,
  backgroundColor: theme.palette.background.paper,
  color: theme.palette.text.primary,
}));
```

## Theme Reference

### Colors

```tsx
import { dclColors } from 'decentraland-ui2';

// Usage
dclColors.rarity.unique
dclColors.rarity.mythic
theme.palette.primary.main
theme.palette.text.primary
```

**Source**: [colors.ts](https://github.com/decentraland/ui2/blob/master/src/theme/colors.ts)

### Typography

```tsx
// Available variants
<Typography variant="h1">Heading 1</Typography>
<Typography variant="body1">Body text</Typography>
<Typography variant="caption">Caption</Typography>
```

**Source**: [typography.ts](https://github.com/decentraland/ui2/blob/master/src/theme/typography.ts)

### Breakpoints

```tsx
xs: 768px   // Extra small (mobile)
sm: 991px   // Small (tablet)
md: 1024px  // Medium (small desktop)
lg: 1280px  // Large (desktop)
xl: 1500px  // Extra large (large desktop)
```

**Source**: [index.ts](https://github.com/decentraland/ui2/blob/master/src/theme/index.ts)

## Standards Compliance

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY", and "OPTIONAL" in this documentation are to be interpreted as described in [RFC 2119](https://datatracker.ietf.org/doc/html/rfc2119).

## Getting Started

1. Review the [Process Overview](/docs/contributor/contributor-guides/web-ui-standards/process-overview/) to understand the workflow
2. Learn about [Custom Components](/docs/contributor/contributor-guides/web-ui-standards/custom-components/) for when to create new components
3. Study [Styling & Theming](/docs/contributor/contributor-guides/web-ui-standards/styling-and-theming/) for implementation details
4. If migrating from UI1, see the [Migration Guide](/docs/contributor/contributor-guides/web-ui-standards/migration/)

## Resources

* **Storybook**: [ui2.decentraland.org](http://ui2.decentraland.org)
* **Figma Library**: [DCL UI2 Design System](https://www.figma.com/design/tsyaDSedcsVZ8iM9N0McT2/DCL-UI2)
* **Material UI Docs**: [mui.com](https://mui.com/material-ui/)
* **Material Design**: [material.io](https://m2.material.io/)
* **GitHub Repository**: [decentraland/ui2](https://github.com/decentraland/ui2)

## Related Standards

* [Dependency Management](/docs/contributor/contributor-guides/dependency-management/) - Managing npm dependencies and peerDependencies
* [UI Standards](/docs/contributor/contributor-guides/ui-standards/) - Redux and RTK Query patterns
* [Testing Standards](/docs/contributor/contributor-guides/testing-standards/) - Testing patterns for UIs
