type CheckBuildSummary = {
  entry: string
  components: number
  component: string
  variant: number
  audit: { width: number; height: number }
  requests: number
}

export declare function checkBuild(options: {
  dist: string
  prefix?: string
  browser?: string
}): Promise<CheckBuildSummary>
export declare function main(argv: string[]): Promise<number>
