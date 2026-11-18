// The in-browser backend: the shared Decentraland rule book (@dcl-regenesislabs/wearable-validator),
// restricted to the model and emote groups plus the thumbnail rule, the parts the editor can act on.
import { BodyShape } from '@dcl/schemas'
import { englishMessage } from '~/intl'
import { fetchContent } from '../builder'
import { captureError } from '../monitoring'
import { THUMBNAIL_PATH } from '../itemFiles'
import { ItemType } from '../items'
import { AUDIO_LIKE, distinctModels, itemModels, type DistinctModel } from './models'
import { limited } from './queue'
import { type RuleBookOutput } from './ruleBook'
import { runOffMainThread } from './runner'
import {
  ValidationSeverity,
  type ItemValidator,
  type ThumbnailSource,
  type ValidateOptions,
  type ValidationContext,
  type ValidationIssue,
  type ValidationResult,
  type ValidationSource
} from './types'

const BOTH_SHAPES = [BodyShape.MALE, BodyShape.FEMALE]

type Loaded = { mainFile: string; bodyShapes: string[]; files: Map<string, Uint8Array> }

async function loadStored(path: string, hash: string | undefined, signal?: AbortSignal): Promise<Uint8Array> {
  if (!hash) throw new Error(`"${path}" has no stored content to validate`)
  return new Uint8Array(await (await fetchContent(hash, signal)).arrayBuffer())
}

/** The models to check: one per distinct main file, texture-only ones left out. */
function modelsOf(source: ValidationSource): DistinctModel[] {
  if (source.kind === 'item') {
    if (source.item.data.representations.length === 0)
      throw new Error(`Item "${source.item.id}" has no representation to validate`)
    return itemModels(source.item)
  }
  const allAudio = Object.keys(source.contents).filter(path => AUDIO_LIKE.test(path))
  const representations = source.representations ?? [{ mainFile: source.mainFile, bodyShapes: BOTH_SHAPES }]
  return distinctModels(
    representations.map(model => ({ ...model, contents: model.contents ?? allAudio })),
    path => source.contents[path]
  )
}

async function load(source: ValidationSource, model: DistinctModel, signal?: AbortSignal): Promise<Loaded> {
  const paths = [model.mainFile, ...model.audio]
  const entries = await Promise.all(
    paths.map(async path => {
      const bytes =
        source.kind === 'blob'
          ? new Uint8Array(await source.contents[path].arrayBuffer())
          : await loadStored(path, source.item.contents[path], signal)
      return [path, bytes] as const
    })
  )
  return { mainFile: model.mainFile, bodyShapes: model.bodyShapes, files: new Map(entries) }
}

/**
 * The slice of entity metadata the model/emote checks read. The explicit representation names the model, so
 * the validator never has to guess it from the file extension.
 */
function toMetadata(ctx: ValidationContext, { mainFile, bodyShapes, files }: Loaded) {
  const representations = [{ bodyShapes, mainFile, contents: [...files.keys()] }]
  return ctx.type === ItemType.EMOTE
    ? { emoteDataADR74: { loop: ctx.loop, representations } }
    : { data: { category: ctx.category, hides: ctx.hides ?? [], representations } }
}

function toIssues({ findings, checks, titles }: RuleBookOutput): ValidationIssue[] {
  // Without a category its limits are unknown, which is not a problem with the model.
  const issues: ValidationIssue[] = findings
    .filter(finding => finding.reason !== 'category-unknown')
    .map(finding => ({
      code: finding.check,
      severity: finding.severity === 'error' ? ValidationSeverity.ERROR : ValidationSeverity.WARNING,
      message: finding.message,
      title: titles[finding.check],
      where: finding.where
    }))
  // A crashed check asserts nothing, so it always shows. An unparseable model (a damaged GLB, a .gltf)
  // skips every check: shown only when nothing else was found, so the run never reads as a pass.
  const errored = checks.filter(check => check.status === 'errored' && check.skipReason)
  const skipped = checks.find(check => check.status === 'skipped' && check.skipReason)
  const incomplete = errored.length > 0 ? errored : issues.length === 0 && skipped ? [skipped] : []
  for (const check of incomplete) {
    // A crash's reason is a raw exception message: report it, show friendly copy instead.
    if (check.status === 'errored')
      captureError(new Error(check.skipReason), { flow: 'validation', check: check.check })
    // A crash says nothing about the model, so it never blocks publishing; an unparseable model does.
    issues.push({
      code: 'file-format',
      severity: check.status === 'errored' ? ValidationSeverity.WARNING : ValidationSeverity.ERROR,
      message:
        check.status === 'errored' ? englishMessage('item_editor.validation.check_crashed') : (check.skipReason ?? ''),
      title: titles[check.status === 'errored' ? check.check : 'file-format']
    })
  }
  return issues
}

async function validateModel(
  source: ValidationSource,
  ctx: ValidationContext,
  model: DistinctModel,
  signal?: AbortSignal
): Promise<ValidationIssue[]> {
  return limited(async () => {
    const loaded = await load(source, model, signal)
    signal?.throwIfAborted()
    const output = await runOffMainThread(
      { kind: 'model', files: loaded.files, metadata: toMetadata(ctx, loaded), category: ctx.category },
      signal
    )
    return toIssues(output)
  }, signal)
}

async function validate(
  source: ValidationSource,
  ctx: ValidationContext,
  opts: ValidateOptions = {}
): Promise<ValidationResult> {
  opts.signal?.throwIfAborted()
  const models = modelsOf(source)
  const perModel = await Promise.all(models.map(model => validateModel(source, ctx, model, opts.signal)))
  if (models.length < 2) return { issues: perModel[0] ?? [] }
  // A separate model per body shape: say which one each issue was found in.
  return {
    issues: perModel.flatMap((issues, index) =>
      issues.map(issue => ({ ...issue, bodyShapes: models[index].bodyShapes }))
    )
  }
}

async function validateThumbnail(source: ThumbnailSource, opts: ValidateOptions = {}): Promise<ValidationResult> {
  opts.signal?.throwIfAborted()
  return limited(async () => {
    const bytes =
      source.kind === 'blob'
        ? new Uint8Array(await source.blob.arrayBuffer())
        : await loadStored(source.item.thumbnail, source.item.contents[source.item.thumbnail], opts.signal)
    opts.signal?.throwIfAborted()
    const output = await runOffMainThread({ kind: 'thumbnail', path: THUMBNAIL_PATH, bytes }, opts.signal)
    return { issues: toIssues(output) }
  }, opts.signal)
}

export const localValidator: ItemValidator = { validate, validateThumbnail }
