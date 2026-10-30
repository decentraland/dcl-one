// explorer-automation `CameraTests` (3): photo mode (C or the sidebar's camera button) and the
// capture. The Bevy HUD has no camera mode: the rail has no camera button, no key opens a photo
// HUD, and the Gallery only views, copies, shares and deletes photos that already exist.
//
// | Unity test                    | Here                                        | Class |
// | TestToggleCameraWithShortcut  | no photo mode to toggle with a key          | Gap   |
// | TestOpenCameraFromSidebar     | no camera button on the rail                | Gap   |
// | TestTakePhoto                 | no capture control; the Gallery only views  | Gap   |
import { spec } from '../driver/index.ts'

const scenario = spec(import.meta.url)

scenario('C toggles photo mode', async () => {}, {
  todo: 'the Bevy HUD has no photo mode and binds no key to one; the Camera key bindings only move the view'
})

scenario('the rail opens photo mode', async () => {}, {
  todo: 'the Bevy HUD rail has no camera button'
})

scenario('Space in photo mode takes a photo', async () => {}, {
  todo: 'the Bevy HUD has no photo capture; the Gallery only views photos that already exist'
})
