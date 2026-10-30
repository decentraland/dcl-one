import { Material, MeshRenderer, Transform, engine } from '@dcl/sdk/ecs'
import { Color4, Vector3 } from '@dcl/sdk/math'
import ReactEcs, {
  Button,
  Dropdown,
  Input,
  Label,
  ReactEcsRenderer,
  UiEntity,
} from '@dcl/sdk/react-ecs'
import { setupVisualTest } from './visual-test-setup'

/**
 * ui — react-ecs screen-space UI surface.
 *
 * Visual contract: regressions in yoga layout, font hinting, UI text
 * rasterization, UiBackground color/texture/uv-sampling, 9-slice scaling,
 * button/input/dropdown chrome, border rendering, opacity stacking, and
 * alpha blending on the UI canvas show up here. The world is intentionally
 * kept to a uniform backdrop so UI changes pop in the diff.
 */

const PIZZA_TEX = 'assets/Images/pizza.png'
const NUMBER_SHEET_TEX = 'assets/Images/number_sheet.png'
const DARK_UI_BG_TEX = 'assets/Images/dark_ui_bg.png'
const RAYS_TEX = 'assets/Images/rays.png'

// UV order for UiBackground stretch mode is BL, TL, TR, BR (clockwise from
// bottom-left). v=0 is the bottom edge of the texture, v=1 is the top.
//
// To pick cell (col, row) from a (cols x rows) sheet where row 0 is the
// VISUALLY top row of the source image, we have to flip the v coordinate
// because image-space rows count from the top but uv-space counts from
// the bottom.
function spriteUvs(col: number, row: number, cols: number, rows: number): number[] {
  const u0 = col / cols
  const u1 = (col + 1) / cols
  // image-row r occupies the v-band [1 - (r+1)/rows, 1 - r/rows]
  const vBottom = 1 - (row + 1) / rows
  const vTop = 1 - row / rows
  // BL,             TL,             TR,             BR
  return [u0, vBottom, u0, vTop, u1, vTop, u1, vBottom]
}

// pizza.png is a 4x4 atlas — show the visually top-left slice as a static frame.
const PIZZA_FRAME_UVS = spriteUvs(0, 0, 4, 4)
// number_sheet.png is a 4x4 atlas (last row is empty) — show "5" (col 1, row 1).
const DIGIT_FIVE_UVS = spriteUvs(1, 1, 4, 4)
// digit "0" — col 0 row 0 — used as a second easy-to-eyeball frame.
const DIGIT_ZERO_UVS = spriteUvs(0, 0, 4, 4)

const WHITE = Color4.create(1, 1, 1, 1)
const GREY = Color4.create(0.85, 0.85, 0.85, 1)
const CAPTION = Color4.create(0.65, 0.78, 1, 1)
const PANEL_BG = Color4.create(0.08, 0.08, 0.1, 1)

export function main() {
  const backdrop = engine.addEntity()
  Transform.create(backdrop, {
    position: Vector3.create(8, 0, 8),
    scale: Vector3.create(16, 0.05, 16),
  })
  MeshRenderer.setBox(backdrop)
  Material.setPbrMaterial(backdrop, { albedoColor: Color4.create(0.1, 0.1, 0.12, 1) })

  ReactEcsRenderer.setUiRenderer(uiRoot)
  setupVisualTest()
}

const Caption = ({ text }: { text: string }) => (
  <Label
    value={text}
    fontSize={14}
    color={CAPTION}
    textAlign="bottom-center"
    uiTransform={{
      positionType: 'absolute',
      position: { bottom: 4, left: 0, right: 0 },
      width: '100%',
      height: 18,
    }}
  />
)

const uiRoot = () => (
  <UiEntity
    uiTransform={{
      width: '100%',
      height: '100%',
      flexDirection: 'column',
      alignItems: 'stretch',
      justifyContent: 'space-between',
      padding: 24,
    }}
    uiBackground={{ color: Color4.create(0, 0, 0, 0.55) }}
  >
    <Label
      value="Visual Regression — UI Surface"
      fontSize={36}
      color={WHITE}
      textAlign="middle-center"
      uiTransform={{ width: '100%', height: 50 }}
    />

    {/* ROW 1 — solid + translucent panels, buttons, input, dropdown */}
    <UiEntity
      uiTransform={{
        flexDirection: 'row',
        width: '100%',
        height: '14%',
        alignItems: 'stretch',
      }}
    >
      <UiEntity
        uiTransform={{ width: '18%', padding: 16, margin: { right: 12 } }}
        uiBackground={{ color: Color4.create(0.85, 0.2, 0.3, 1) }}
      >
        <Label value={'Solid panel\nLine two'} fontSize={20} color={WHITE} />
      </UiEntity>

      <UiEntity
        uiTransform={{
          width: '20%',
          padding: 16,
          margin: { right: 12 },
          flexDirection: 'column',
        }}
        uiBackground={{ color: Color4.create(0.2, 0.5, 0.85, 0.6) }}
      >
        <Label
          value={'Translucent panel'}
          fontSize={18}
          color={WHITE}
          uiTransform={{ margin: { bottom: 6 }, height: 24 }}
        />
        <Label
          value={'Stacked alpha blending\nover the world background.'}
          fontSize={14}
          color={GREY}
        />
      </UiEntity>

      <UiEntity
        uiTransform={{
          flexDirection: 'column',
          flexGrow: 1,
          justifyContent: 'space-between',
          alignItems: 'stretch',
        }}
      >
        <UiEntity uiTransform={{ flexDirection: 'row', height: 48 }}>
          <Button
            value="Primary"
            fontSize={18}
            variant="primary"
            uiTransform={{ width: '32%', height: 48, margin: { right: 12 } }}
          />
          <Button
            value="Secondary"
            fontSize={18}
            variant="secondary"
            uiTransform={{ width: '32%', height: 48, margin: { right: 12 } }}
          />
          <Button
            value="Disabled"
            fontSize={18}
            variant="primary"
            disabled={true}
            uiTransform={{ width: '32%', height: 48 }}
          />
        </UiEntity>

        <Input
          placeholder="Type something…"
          fontSize={16}
          uiTransform={{ width: '100%', height: 44 }}
        />

        <Dropdown
          options={['Option A', 'Option B', 'Option C']}
          selectedIndex={1}
          fontSize={16}
          color={WHITE}
          uiTransform={{ width: '100%', height: 44 }}
        />
      </UiEntity>
    </UiEntity>

    {/* ROW 2 — texture tests: spritesheet single frames, full atlas, center, repeat */}
    <UiEntity
      uiTransform={{
        flexDirection: 'row',
        width: '100%',
        height: '20%',
        alignItems: 'stretch',
      }}
    >
      {/* Single pizza slice from 4x4 atlas */}
      <UiEntity
        uiTransform={{
          flexGrow: 1,
          margin: { right: 12 },
          padding: 12,
          alignItems: 'center',
          justifyContent: 'center',
          positionType: 'relative',
        }}
        uiBackground={{ color: PANEL_BG }}
      >
        <UiEntity
          uiTransform={{ width: '80%', height: '80%' }}
          uiBackground={{
            texture: { src: PIZZA_TEX },
            textureMode: 'stretch',
            uvs: PIZZA_FRAME_UVS,
          }}
        />
        <Caption text="sprite frame (4x4 → 0,0)" />
      </UiEntity>

      {/* Single digit "5" from 4x3 atlas */}
      <UiEntity
        uiTransform={{
          flexGrow: 1,
          margin: { right: 12 },
          padding: 12,
          alignItems: 'center',
          justifyContent: 'center',
          positionType: 'relative',
        }}
        uiBackground={{ color: PANEL_BG }}
      >
        <UiEntity
          uiTransform={{ width: '70%', height: '80%' }}
          uiBackground={{
            texture: { src: NUMBER_SHEET_TEX },
            textureMode: 'stretch',
            uvs: DIGIT_FIVE_UVS,
          }}
        />
        <Caption text='digit "5" (4x3 → 1,1)' />
      </UiEntity>

      {/* Single digit "0" from 4x3 atlas — second eyeball-check */}
      <UiEntity
        uiTransform={{
          flexGrow: 1,
          margin: { right: 12 },
          padding: 12,
          alignItems: 'center',
          justifyContent: 'center',
          positionType: 'relative',
        }}
        uiBackground={{ color: PANEL_BG }}
      >
        <UiEntity
          uiTransform={{ width: '70%', height: '80%' }}
          uiBackground={{
            texture: { src: NUMBER_SHEET_TEX },
            textureMode: 'stretch',
            uvs: DIGIT_ZERO_UVS,
          }}
        />
        <Caption text='digit "0" (4x3 → 0,0)' />
      </UiEntity>

      {/* Color tint multiplied with texture */}
      <UiEntity
        uiTransform={{
          flexGrow: 1,
          margin: { right: 12 },
          padding: 12,
          alignItems: 'center',
          justifyContent: 'center',
          positionType: 'relative',
        }}
        uiBackground={{
          color: Color4.create(0.4, 0.85, 1, 1),
          texture: { src: NUMBER_SHEET_TEX },
          textureMode: 'stretch',
          uvs: spriteUvs(2, 0, 4, 4),
        }}
      >
        <Caption text="color × texture (cyan × 2)" />
      </UiEntity>

      {/* Full atlas stretched */}
      <UiEntity
        uiTransform={{
          flexGrow: 1.5,
          alignItems: 'flex-start',
          justifyContent: 'flex-start',
          positionType: 'relative',
        }}
        uiBackground={{
          texture: { src: PIZZA_TEX },
          textureMode: 'stretch',
        }}
      >
        <Caption text="stretch (full atlas)" />
      </UiEntity>
    </UiEntity>

    {/* ROW 3 — 9-slice scaling at three sizes (proves slices stay constant) */}
    <UiEntity
      uiTransform={{
        flexDirection: 'row',
        width: '100%',
        height: '14%',
        alignItems: 'center',
      }}
    >
      <UiEntity
        uiTransform={{
          width: '15%',
          height: '60%',
          margin: { right: 16 },
          padding: 12,
          alignItems: 'center',
          justifyContent: 'center',
        }}
        uiBackground={{
          texture: { src: DARK_UI_BG_TEX },
          textureMode: 'nine-slices',
          textureSlices: { top: 0.4, right: 0.4, bottom: 0.4, left: 0.4 },
        }}
      >
        <Label value="9-slice S" fontSize={16} color={WHITE} />
      </UiEntity>

      <UiEntity
        uiTransform={{
          width: '28%',
          height: '85%',
          margin: { right: 16 },
          padding: 16,
          alignItems: 'center',
          justifyContent: 'center',
        }}
        uiBackground={{
          texture: { src: DARK_UI_BG_TEX },
          textureMode: 'nine-slices',
          textureSlices: { top: 0.4, right: 0.4, bottom: 0.4, left: 0.4 },
        }}
      >
        <Label value="9-slice M" fontSize={20} color={WHITE} />
      </UiEntity>

      <UiEntity
        uiTransform={{
          flexGrow: 1,
          height: '100%',
          padding: 24,
          alignItems: 'center',
          justifyContent: 'center',
        }}
        uiBackground={{
          texture: { src: DARK_UI_BG_TEX },
          textureMode: 'nine-slices',
          textureSlices: { top: 0.4, right: 0.4, bottom: 0.4, left: 0.4 },
        }}
      >
        <Label
          value="9-slice L  —  corners stay crisp at every size"
          fontSize={22}
          color={WHITE}
        />
      </UiEntity>
    </UiEntity>

    {/* ROW 4 — borders / radius / opacity / z-index overlap / center mode / repeat */}
    <UiEntity
      uiTransform={{
        flexDirection: 'row',
        width: '100%',
        height: '17%',
        alignItems: 'stretch',
      }}
    >
      <UiEntity
        uiTransform={{
          flexGrow: 1,
          margin: { right: 12 },
          padding: 12,
          borderColor: Color4.create(1, 0.7, 0.2, 1),
          borderWidth: 4,
          borderRadius: 16,
          alignItems: 'center',
          justifyContent: 'center',
        }}
        uiBackground={{ color: Color4.create(0.15, 0.15, 0.2, 1) }}
      >
        <Label value="border + radius" fontSize={18} color={WHITE} />
      </UiEntity>

      <UiEntity
        uiTransform={{
          flexGrow: 1,
          margin: { right: 12 },
          padding: 12,
          borderColor: {
            top: Color4.create(0.95, 0.3, 0.3, 1),
            right: Color4.create(0.3, 0.85, 0.3, 1),
            bottom: Color4.create(0.3, 0.5, 0.95, 1),
            left: Color4.create(0.95, 0.85, 0.3, 1),
          },
          borderWidth: { top: 2, right: 4, bottom: 6, left: 8 },
          borderRadius: { topLeft: 0, topRight: 28, bottomRight: 0, bottomLeft: 28 },
          alignItems: 'center',
          justifyContent: 'center',
        }}
        uiBackground={{ color: Color4.create(0.18, 0.12, 0.22, 1) }}
      >
        <Label value="asymmetric" fontSize={18} color={WHITE} />
      </UiEntity>

      <UiEntity
        uiTransform={{
          flexGrow: 1,
          margin: { right: 12 },
          padding: 12,
          opacity: 0.4,
          alignItems: 'center',
          justifyContent: 'center',
        }}
        uiBackground={{ color: Color4.create(0.9, 0.4, 0.2, 1) }}
      >
        <Label value="opacity 0.4" fontSize={18} color={WHITE} />
      </UiEntity>

      {/* Centered texture (smaller than container) */}
      <UiEntity
        uiTransform={{
          flexGrow: 1,
          margin: { right: 12 },
          positionType: 'relative',
        }}
        uiBackground={{
          color: Color4.create(0.05, 0.05, 0.08, 1),
          texture: { src: DARK_UI_BG_TEX },
          textureMode: 'center',
        }}
      >
        <Caption text="textureMode: center" />
      </UiEntity>

      {/* Z-index overlap: middle tile (z=3) wins */}
      <UiEntity uiTransform={{ flexGrow: 1.4, positionType: 'relative' }}>
        <UiEntity
          uiTransform={{
            width: '50%',
            height: '75%',
            positionType: 'absolute',
            position: { top: '10%', left: '5%' },
            zIndex: 1,
            padding: 8,
          }}
          uiBackground={{ color: Color4.create(0.2, 0.5, 0.85, 0.95) }}
        >
          <Label value="z=1" fontSize={16} color={WHITE} />
        </UiEntity>
        <UiEntity
          uiTransform={{
            width: '50%',
            height: '75%',
            positionType: 'absolute',
            position: { top: '15%', left: '25%' },
            zIndex: 3,
            padding: 8,
          }}
          uiBackground={{ color: Color4.create(0.85, 0.2, 0.3, 0.95) }}
        >
          <Label value="z=3 (top)" fontSize={16} color={WHITE} />
        </UiEntity>
        <UiEntity
          uiTransform={{
            width: '50%',
            height: '75%',
            positionType: 'absolute',
            position: { top: '0%', left: '45%' },
            zIndex: 2,
            padding: 8,
          }}
          uiBackground={{ color: Color4.create(0.3, 0.8, 0.4, 0.95) }}
        >
          <Label value="z=2" fontSize={16} color={WHITE} />
        </UiEntity>
      </UiEntity>
    </UiEntity>

    {/* ROW 5 — fonts + text alignment matrix */}
    <UiEntity
      uiTransform={{
        flexDirection: 'row',
        width: '100%',
        height: '17%',
        alignItems: 'stretch',
      }}
    >
      <UiEntity
        uiTransform={{ flexGrow: 1, padding: 14, margin: { right: 12 } }}
        uiBackground={{ color: PANEL_BG }}
      >
        <Label
          value={'sans-serif\n0123 The quick fox\nABCDEFG abcdefg'}
          fontSize={18}
          font="sans-serif"
          color={WHITE}
          textAlign="top-left"
        />
      </UiEntity>
      <UiEntity
        uiTransform={{ flexGrow: 1, padding: 14, margin: { right: 12 } }}
        uiBackground={{ color: PANEL_BG }}
      >
        <Label
          value={'serif\n0123 The quick fox\nABCDEFG abcdefg'}
          fontSize={18}
          font="serif"
          color={WHITE}
          textAlign="top-left"
        />
      </UiEntity>
      <UiEntity
        uiTransform={{ flexGrow: 1, padding: 14, margin: { right: 12 } }}
        uiBackground={{ color: PANEL_BG }}
      >
        <Label
          value={'monospace\n0123 The quick fox\nABCDEFG abcdefg'}
          fontSize={18}
          font="monospace"
          color={WHITE}
          textAlign="top-left"
        />
      </UiEntity>

      {/* Text alignment matrix in a fixed-aspect box */}
      <UiEntity
        uiTransform={{
          flexGrow: 1.3,
          flexDirection: 'column',
          padding: 6,
        }}
        uiBackground={{ color: PANEL_BG }}
      >
        {(['top', 'middle', 'bottom'] as const).map((vert) => (
          <UiEntity
            key={vert}
            uiTransform={{ flexDirection: 'row', flexGrow: 1, width: '100%' }}
          >
            {(['left', 'center', 'right'] as const).map((horiz) => (
              <Label
                key={horiz}
                value={`${vert}-${horiz}`}
                fontSize={14}
                color={WHITE}
                textAlign={`${vert}-${horiz}` as const}
                uiTransform={{ flexGrow: 1, height: '100%' }}
              />
            ))}
          </UiEntity>
        ))}
      </UiEntity>
    </UiEntity>

    {/* Footer — multi-line label catches font-metrics drift */}
    <Label
      value={
        'Multi-line label exercises kerning and line-break behavior.\n' +
        'Three lines of varying widths catch font-metrics drift.\n' +
        'The quick brown fox jumps over the lazy dog. 0123456789'
      }
      fontSize={16}
      color={GREY}
      textAlign="middle-center"
      uiTransform={{ width: '100%', height: '8%' }}
    />
  </UiEntity>
)
