import { resolve } from 'path'
import { itExecutes } from '../../scripts/helpers'
import { TSC } from '../../scripts/common'

describe('should compile all snippets files', () => {
  const snippetsPath = resolve(process.cwd(), 'test', 'ecs', 'snippets')
  itExecutes(`${TSC} -p tsconfig.json`, snippetsPath)
})
