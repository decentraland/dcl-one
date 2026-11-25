import { useState, type ReactNode } from 'react'
import {
  Checkbox,
  CheckboxLabel,
  Dropdown,
  GeneralSearch,
  InputExplorerNames,
  InputWebCreatorTools,
  SearchBar,
  Switch,
  WebSelect,
  WebTextarea,
  type InputWebCreatorToolsProps,
  CharCounter,
  CodeInput,
  FieldLabel,
  SubmitField,
  Slider
} from '@dcl/one-ui'
import type { GalleryEntry } from '../registry'
import layout from './layout.module.css'

function SearchBarDemo({ initial }: { initial: string }) {
  const [value, setValue] = useState(initial)
  return (
    <div className={layout.field}>
      <SearchBar
        label="Search places"
        placeholder="Search places"
        value={value}
        onValueChange={setValue}
        onClear={() => setValue('')}
        clearLabel="Clear search"
      />
    </div>
  )
}

function HudSearchBarDemo({
  variant,
  fieldSize,
  initial
}: {
  variant: 'dark' | 'light'
  fieldSize: 'md' | 'sm'
  initial: string
}) {
  const [value, setValue] = useState(initial)
  return (
    <SearchBar
      appearance="hud"
      variant={variant}
      fieldSize={fieldSize}
      label="Search places"
      placeholder="Search places"
      value={value}
      onValueChange={setValue}
      onClear={() => setValue('')}
      clearIcon={
        <svg viewBox="0 0 10 10" width="10" height="10">
          <path
            d="M1.5 1.5l7 7M8.5 1.5l-7 7"
            stroke="currentColor"
            strokeWidth="1.6"
            strokeLinecap="round"
          />
        </svg>
      }
      leading={
        <svg viewBox="0 0 16 16" width="15" height="15">
          <circle cx="7" cy="7" r="5" fill="none" stroke="currentColor" strokeWidth="1.6" />
          <path d="M11 11l3.5 3.5" stroke="currentColor" strokeWidth="1.6" strokeLinecap="round" />
        </svg>
      }
    />
  )
}

const networks = [
  { value: 'ethereum', label: 'Ethereum' },
  { value: 'polygon', label: 'Polygon' },
  { value: 'testnet', label: 'Testnet', disabled: true }
] as const
type Network = (typeof networks)[number]['value']

function SingleDropdownDemo({ initial }: { initial: Network | null }) {
  const [value, setValue] = useState<Network | null>(initial)
  return (
    <div className={layout.field}>
      <Dropdown
        label="Network"
        placeholder="Choose a network"
        options={networks}
        value={value}
        onChange={setValue}
        help="Where the purchase is paid."
      />
    </div>
  )
}

function HudDropdownDemo({ variant }: { variant: 'dark' | 'light' }) {
  const [value, setValue] = useState<string>('ethereum')
  return (
    <Dropdown
      appearance="hud"
      variant={variant}
      label="Network"
      value={value}
      options={networks}
      onChange={setValue}
    />
  )
}

function MultipleDropdownDemo() {
  const [value, setValue] = useState<Network[]>(['polygon'])
  return (
    <div className={layout.field}>
      <Dropdown
        label="Networks"
        multiple
        options={networks}
        value={value}
        onChange={setValue}
        valueText={(chosen) => `${chosen.length} selected`}
      />
    </div>
  )
}

type InputComponent = (props: InputWebCreatorToolsProps) => ReactNode

// Both input surfaces take the same props; only their look differs.
function inputExamples(Input: InputComponent): GalleryEntry['examples'] {
  return [
    {
      id: 'states',
      title: 'Empty, filled and disabled',
      render: () => (
        <div className={layout.stack}>
          <div className={layout.field}>
            <Input label="Scene name" placeholder="My scene" />
          </div>
          <div className={layout.field}>
            <Input label="Scene name" defaultValue="Genesis Plaza" />
          </div>
          <div className={layout.field}>
            <Input label="Scene name" defaultValue="Read only" disabled />
          </div>
        </div>
      )
    },
    {
      id: 'details',
      title: 'Hint, description, counter and suffix',
      render: () => (
        <div className={layout.stack}>
          <div className={layout.field}>
            <Input
              label="Display name"
              defaultValue="Ada"
              hint="Optional"
              description="Shown to other players."
              counter="3/15"
            />
          </div>
          <div className={layout.field}>
            <Input label="Price" defaultValue="12" suffix="MANA" />
          </div>
        </div>
      )
    },
    {
      id: 'error',
      title: 'Error, icons and password reveal',
      render: () => (
        <div className={layout.stack}>
          <div className={layout.field}>
            <Input label="Email" defaultValue="ada@" error="Enter a valid email address." />
          </div>
          <div className={layout.field}>
            <Input
              label="Search"
              placeholder="Search"
              leadingIcon={<GeneralSearch size={20} aria-hidden="true" />}
            />
          </div>
          <div className={layout.field}>
            <Input
              label="Password"
              type="password"
              defaultValue="not-a-secret"
              allowPasswordReveal
            />
          </div>
        </div>
      )
    }
  ]
}

export const formEntries: GalleryEntry[] = [
  {
    name: 'Checkbox',
    group: 'Forms',
    summary: 'The bare checkbox control; it needs an accessible name.',
    components: ['Checkbox'],
    examples: [
      {
        id: 'sizes',
        title: 'Sizes and variants',
        render: () => (
          <div className={layout.stack}>
            {(['outline', 'filled'] as const).map((variant) => (
              <div key={variant} className={layout.row}>
                {(['sm', 'md', 'lg'] as const).map((size) => (
                  <Checkbox
                    key={size}
                    aria-label={`${variant} ${size}`}
                    variant={variant}
                    size={size}
                    defaultChecked={size !== 'sm'}
                  />
                ))}
              </div>
            ))}
            <div className={layout.row}>
              <Checkbox aria-label="hud" appearance="hud" defaultChecked />
              <Checkbox aria-label="hud off" appearance="hud" />
            </div>
          </div>
        )
      },
      {
        id: 'states',
        title: 'Unchecked, checked, mixed and disabled',
        render: () => (
          <div className={layout.row}>
            <Checkbox aria-label="Unchecked" />
            <Checkbox aria-label="Checked" defaultChecked />
            <Checkbox aria-label="Mixed" indeterminate />
            <Checkbox aria-label="Disabled" disabled />
            <Checkbox aria-label="Disabled checked" disabled defaultChecked />
          </div>
        )
      }
    ]
  },
  {
    name: 'CheckboxLabel',
    group: 'Forms',
    summary: 'A checkbox with its visible label and optional help.',
    components: ['CheckboxLabel'],
    examples: [
      {
        id: 'positions',
        title: 'Label after and before the box',
        render: () => (
          <div className={layout.stack}>
            <CheckboxLabel label="Remember me" defaultChecked />
            <CheckboxLabel label="Label first" labelPosition="start" />
            <CheckboxLabel label="Disabled" disabled />
          </div>
        )
      },
      {
        id: 'help',
        title: 'With help text',
        render: () => (
          <CheckboxLabel
            label="Share my location"
            help="Friends see the parcel you are in."
            helpLabel="About location sharing"
          />
        )
      }
    ]
  },
  {
    name: 'Switch',
    group: 'Forms',
    summary: 'An on/off switch with its label.',
    components: ['Switch'],
    examples: [
      {
        id: 'states',
        title: 'Off, on, disabled and hidden label',
        render: () => (
          <div className={layout.stack}>
            <Switch label="Notifications" />
            <Switch label="Background music" defaultChecked />
            <Switch label="Unavailable" disabled />
            <Switch label="Voice chat" labelHidden defaultChecked />
          </div>
        )
      },
      {
        id: 'hud',
        title: 'Explorer HUD appearance: default, small and small on a light panel',
        render: () => (
          <div className={layout.row}>
            <Switch appearance="hud" label="Fullscreen" labelHidden defaultChecked />
            <Switch appearance="hud" label="Fullscreen off" labelHidden />
            <Switch appearance="hud" size="sm" label="Hear others" labelHidden defaultChecked />
            <Switch appearance="hud" size="sm" label="Hear others off" labelHidden />
            <span style={{ padding: 8, borderRadius: 8, background: '#fcfcfc' }}>
              <Switch appearance="hud" size="sm" surface="light" label="Map layer" labelHidden />
            </span>
            <Switch appearance="hud" label="Unavailable" labelHidden disabled />
          </div>
        )
      }
    ]
  },
  {
    name: 'InputExplorerNames',
    group: 'Forms',
    summary: 'The text field on the Explorer surface.',
    components: ['InputExplorerNames'],
    examples: [
      ...inputExamples(InputExplorerNames),
      {
        id: 'hud',
        title:
          'Explorer HUD: the dark field, the light field, an invalid value, an action, the large field and a multi-line one',
        render: () => (
          <div className={`${layout.stack} ${layout.field}`}>
            <InputExplorerNames appearance="hud" label="Name" defaultValue="robtfm" />
            <InputExplorerNames
              appearance="hud"
              variant="light"
              label="Name"
              placeholder="Your name"
            />
            <InputExplorerNames appearance="hud" invalid label="Link" defaultValue="not-a-url" />
            <InputExplorerNames
              appearance="hud"
              label="Name"
              defaultValue="robtfm"
              fieldAction={{
                icon: <GeneralSearch size={18} aria-hidden="true" />,
                label: 'Search',
                onClick: () => {}
              }}
            />
            <InputExplorerNames
              appearance="hud"
              fieldSize="lg"
              label="Name"
              defaultValue="robtfm"
            />
            <InputExplorerNames
              appearance="hud"
              multiline
              label="About me"
              placeholder="Tell people about yourself"
            />
          </div>
        )
      }
    ]
  },
  {
    name: 'InputWebCreatorTools',
    group: 'Forms',
    summary: 'The text field on the web surface.',
    components: ['InputWebCreatorTools'],
    examples: inputExamples(InputWebCreatorTools)
  },
  {
    name: 'WebSelect',
    group: 'Forms',
    summary: 'The native select styled for the web surface.',
    components: ['WebSelect'],
    examples: [
      {
        id: 'states',
        title: 'Enabled and disabled',
        render: () => (
          <div className={layout.stack}>
            <WebSelect aria-label="Network" defaultValue="polygon">
              <option value="ethereum">Ethereum</option>
              <option value="polygon">Polygon</option>
            </WebSelect>
            <WebSelect aria-label="Network (disabled)" disabled defaultValue="ethereum">
              <option value="ethereum">Ethereum</option>
            </WebSelect>
          </div>
        )
      }
    ]
  },
  {
    name: 'WebTextarea',
    group: 'Forms',
    summary: 'The native textarea styled for the web surface.',
    components: ['WebTextarea'],
    examples: [
      {
        id: 'states',
        title: 'Empty, filled and disabled',
        render: () => (
          <div className={layout.stack}>
            <WebTextarea aria-label="Description" placeholder="Describe your scene" rows={3} />
            <WebTextarea
              aria-label="Description (filled)"
              defaultValue="A plaza with a stage and a gallery."
              rows={3}
            />
            <WebTextarea aria-label="Description (disabled)" disabled rows={2} />
          </div>
        )
      }
    ]
  },
  {
    name: 'Dropdown',
    group: 'Forms',
    summary: 'A single or multiple choice list with its label and optional help.',
    components: ['Dropdown'],
    minHeight: 420,
    examples: [
      {
        id: 'empty',
        title: 'Single, nothing chosen',
        render: () => <SingleDropdownDemo initial={null} />
      },
      {
        id: 'chosen',
        title: 'Single, chosen',
        render: () => <SingleDropdownDemo initial="ethereum" />
      },
      { id: 'multiple', title: 'Multiple', render: () => <MultipleDropdownDemo /> },
      {
        id: 'disabled',
        title: 'Disabled',
        render: () => (
          <div className={layout.field}>
            <Dropdown
              label="Network"
              options={networks}
              value={null}
              onChange={() => {}}
              disabled
            />
          </div>
        )
      },
      {
        id: 'hud',
        title: 'Explorer HUD: the dark field, the light field, a disabled one and the sort list',
        render: () => (
          <div className={`${layout.row} ${layout.selectRoom}`}>
            <HudDropdownDemo variant="dark" />
            <HudDropdownDemo variant="light" />
            <Dropdown
              appearance="hud"
              label="Network"
              value="ethereum"
              options={networks}
              onChange={() => {}}
              disabled
            />
            <div className={layout.narrow}>
              <Dropdown
                appearance="hud"
                kind="sort"
                options={['Recently listed', 'Cheapest', 'Newest']}
              />
            </div>
          </div>
        )
      }
    ]
  },
  {
    name: 'SearchBar',
    group: 'Forms',
    summary: 'A controlled search field with an optional clear button.',
    components: ['SearchBar'],
    examples: [
      { id: 'empty', title: 'Empty', render: () => <SearchBarDemo initial="" /> },
      {
        id: 'value',
        title: 'With a value and the clear button',
        render: () => <SearchBarDemo initial="Genesis" />
      },
      {
        id: 'hud',
        title: 'Explorer HUD: the dark pill, the light field and the small field',
        render: () => (
          <div className={`${layout.stack} ${layout.field}`}>
            <HudSearchBarDemo variant="dark" fieldSize="md" initial="Genesis" />
            <HudSearchBarDemo variant="light" fieldSize="md" initial="Genesis" />
            <HudSearchBarDemo variant="dark" fieldSize="sm" initial="" />
          </div>
        )
      }
    ]
  },
  {
    name: 'FieldLabel',
    group: 'Forms',
    summary: 'A field label with an optional notice and helper line.',
    components: ['FieldLabel'],
    examples: [
      {
        id: 'forms',
        title: 'Plain, and with a notice and a helper line',
        render: () => (
          <div className={layout.stack}>
            <FieldLabel>Name</FieldLabel>
            <FieldLabel notice="Optional" sublabel="Shown on your passport">
              Country
            </FieldLabel>
          </div>
        )
      }
    ]
  },
  {
    name: 'CharCounter',
    group: 'Forms',
    summary: 'The current/max readout under a text field.',
    components: ['CharCounter'],
    examples: [
      {
        id: 'formats',
        title: 'The three formats, and a count above its maximum',
        render: () => (
          <div className={layout.narrow}>
            <CharCounter current={3} max={10} />
            <CharCounter current={3} max={10} format="spaced" />
            <CharCounter current={3} max={10} format="words" />
            <CharCounter current={12} max={10} />
          </div>
        )
      }
    ]
  },
  {
    name: 'CodeInput',
    group: 'Forms',
    summary: 'A one-time code typed into a row of slots.',
    components: ['CodeInput'],
    examples: [
      {
        id: 'states',
        title: 'Typing, accepted and refused',
        render: () => <CodeInputExample />
      }
    ]
  },
  {
    name: 'Slider',
    group: 'Forms',
    summary: 'A range input with a ruby fill, in four tracks, with optional steppers.',
    components: ['Slider'],
    examples: [
      {
        id: 'variants',
        title: 'Thin, thick with steppers, pin with steppers, rail, and disabled',
        render: () => <SliderExample />
      }
    ]
  },
  {
    name: 'SubmitField',
    group: 'Forms',
    summary: 'A light text field with its submit button inside it.',
    components: ['SubmitField'],
    examples: [
      {
        id: 'states',
        title: 'Ready, held, busy and with an error',
        render: () => <SubmitFieldExample />
      }
    ]
  }
]

function CodeInputExample() {
  const [code, setCode] = useState('12')
  return (
    <div className={layout.strip}>
      <div className={layout.stack}>
        <CodeInput value={code} onChange={setCode} />
        <CodeInput
          value="123456"
          onChange={() => {}}
          state="success"
          disabled
          aria-label="Accepted code"
        />
        <CodeInput
          value="123456"
          onChange={() => {}}
          state="error"
          disabled
          aria-label="Refused code"
        />
      </div>
    </div>
  )
}

function SubmitFieldExample() {
  const [email, setEmail] = useState('ada@example.com')
  const noop = () => {}
  return (
    <div className={layout.narrow}>
      <div className={layout.stack}>
        <SubmitField
          aria-label="Email"
          type="email"
          placeholder="your@email.com"
          value={email}
          onChange={setEmail}
          onSubmit={noop}
          submitLabel="NEXT"
          canSubmit
        />
        <SubmitField
          aria-label="Email, held"
          value=""
          placeholder="your@email.com"
          onChange={noop}
          onSubmit={noop}
          submitLabel="NEXT"
          canSubmit={false}
        />
        <SubmitField
          aria-label="Email, busy"
          value="ada@example.com"
          onChange={noop}
          onSubmit={noop}
          submitLabel="NEXT"
          canSubmit
          busy
        />
        <SubmitField
          aria-label="Email, invalid"
          value="ada@"
          onChange={noop}
          onSubmit={noop}
          submitLabel="NEXT"
          canSubmit={false}
          error="Invalid email"
        />
      </div>
    </div>
  )
}

function SliderExample() {
  const [value, setValue] = useState(40)
  return (
    <div className={layout.narrow}>
      <div className={layout.stack}>
        <Slider value={value} onChange={setValue} aria-label="Thin" />
        <Slider variant="thick" arrows value={value} onChange={setValue} aria-label="Thick" />
        <Slider variant="pin" arrows value={value} onChange={setValue} aria-label="Pin" />
        <Slider variant="rail" value={value} onChange={setValue} aria-label="Rail" />
        <Slider value={value} disabled onChange={setValue} aria-label="Disabled" />
      </div>
    </div>
  )
}
