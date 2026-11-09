type GeneratedFile = { path: string; content: string }

type ImageFills = 'assets' | 'placeholder'

// An omitted imageFills means 'assets'. Keyless: this tree's generators take no file key.
type GeneratorContext = { imageFills?: ImageFills }

export type { GeneratedFile, GeneratorContext, ImageFills }
