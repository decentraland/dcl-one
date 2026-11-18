// Offscreen posed thumbnail render (legacy builder's lib/getScreenshot, ported from Babylon to three):
// the wearable alone, front-facing orthographic camera, transparent background, avatar-reference
// materials hidden. Used for categories with a pose; everything else screenshots the preview iframe.
import type { AnimationClip, Material, Object3D } from 'three'
import { loadGltf } from './models'

const THUMBNAIL_SIZE = 1024
const HIDDEN_MATERIALS = ['hair_mat', 'avatarskin_mat']
const POSE_PROPERTIES = new Set(['position', 'quaternion', 'scale'])

/** Blender-style duplicate suffixes (`.001`, sanitized by three to `001`) so pose bones match model bones. */
function baseName(name: string): string {
  return name.replace(/\.?\d{3}$/, '')
}

/**
 * Writes the first keyframe of every pose track straight into the matching bones of `target`,
 * so the rig holds a static pose without running the animation system.
 */
export function applyPose(target: Object3D, animations: AnimationClip[]): void {
  const nodes: Object3D[] = []
  target.traverse(node => nodes.push(node))

  for (const clip of animations) {
    for (const track of clip.tracks) {
      const separator = track.name.lastIndexOf('.')
      const nodeName = baseName(track.name.slice(0, separator))
      const property = track.name.slice(separator + 1)
      if (!POSE_PROPERTIES.has(property)) continue

      const values = Array.from(track.values.slice(0, track.getValueSize()))
      for (const node of nodes) {
        if (baseName(node.name) !== nodeName) continue
        if (property === 'quaternion') node.quaternion.fromArray(values)
        else if (property === 'position') node.position.fromArray(values)
        else node.scale.fromArray(values)
      }
    }
  }
  target.updateMatrixWorld(true)
}

function toObjectURLs(contents: Record<string, Blob>): Record<string, string> {
  return Object.fromEntries(Object.entries(contents).map(([path, blob]) => [path, URL.createObjectURL(blob)]))
}

/** Renders the model in the given pose to a 1024x1024 PNG data URL. */
export async function renderPosedThumbnail(
  contents: Record<string, Blob>,
  model: string,
  poseUrl: string
): Promise<string> {
  const Three = await import('three')
  const mappings = toObjectURLs(contents)
  try {
    const [gltf, pose] = await Promise.all([loadGltf(mappings[model], mappings), loadGltf(poseUrl, {})])
    const root = gltf.scene

    root.traverse(node => {
      if (!(node instanceof Three.Mesh)) return
      const material = node.material as Material | Material[]
      const materials = Array.isArray(material) ? material : [material]
      const name = materials.map(material => material.name.toLowerCase()).join(' ')
      if (HIDDEN_MATERIALS.some(hidden => name.includes(hidden))) node.visible = false
    })

    applyPose(root, pose.animations)

    // Bounds follow the posed skin so the framing matches what is drawn.
    const bounds = new Three.Box3()
    root.traverse(node => {
      if (!(node instanceof Three.Mesh) || !node.visible) return
      if (node instanceof Three.SkinnedMesh) {
        node.skeleton.update()
        node.computeBoundingBox()
        bounds.union(node.boundingBox.clone().applyMatrix4(node.matrixWorld))
      } else {
        bounds.expandByObject(node)
      }
    })
    const size = bounds.getSize(new Three.Vector3()).length() / 2 || 1
    const center = bounds.getCenter(new Three.Vector3())
    const parent = new Three.Group()
    parent.add(root)
    parent.scale.setScalar(1 / size)
    parent.position.copy(center.multiplyScalar(-1 / size))

    const scene = new Three.Scene()
    scene.add(parent)
    scene.add(new Three.HemisphereLight(0xffffff, 0x888888, 2))
    const key = new Three.DirectionalLight(0xffffff, 2)
    key.position.set(0, 0, 2)
    scene.add(key)

    const camera = new Three.OrthographicCamera(-1, 1, 1, -1, 0.1, 10)
    camera.position.set(0, 0, 2)
    camera.lookAt(0, 0, 0)

    const renderer = new Three.WebGLRenderer({ alpha: true, antialias: true, preserveDrawingBuffer: true })
    try {
      renderer.setPixelRatio(1)
      renderer.setSize(THUMBNAIL_SIZE, THUMBNAIL_SIZE)
      renderer.setClearColor(0x000000, 0)
      renderer.render(scene, camera)
      return renderer.domElement.toDataURL('image/png')
    } finally {
      // Browsers cap live WebGL contexts; release this one before the next draft renders.
      renderer.dispose()
      renderer.forceContextLoss()
    }
  } finally {
    for (const url of Object.values(mappings)) URL.revokeObjectURL(url)
  }
}
