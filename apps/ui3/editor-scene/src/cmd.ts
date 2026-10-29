import { BevyApi } from './bevy-api'
import { makeCommands } from './commands'

export const cmd = makeCommands((c, args) => BevyApi.consoleCommand(c, args))
