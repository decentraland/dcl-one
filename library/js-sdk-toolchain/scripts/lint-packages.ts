#!/usr/bin/env node

import { execSync } from 'child_process'
import fs from 'fs'

// Parse command line arguments
const args = process.argv.slice(2)
const shouldFix = args.includes('--fix')

// Directories that should be linted
const packages = [
  'packages/@dcl/ecs',
  'packages/@dcl/sdk',
  'packages/@dcl/sdk-commands',
  'packages/@dcl/react-ecs',
  // 'packages/@dcl/js-runtime' - This package is ignored by ESLint
  'test',
  'scripts'
]

console.log(`🔍 Linting packages individually${shouldFix ? ' with auto-fix' : ''}...\n`)

let hasErrors = false

// Lint each package individually
for (const pkg of packages) {
  if (fs.existsSync(pkg)) {
    console.log(`📦 Linting ${pkg}...`)
    try {
      // Use the root ESLint to lint this specific package
      const fixFlag = shouldFix ? '--fix' : ''
      execSync(`NODE_OPTIONS="--max-old-space-size=4096" eslint ${pkg} --ext .ts,.tsx ${fixFlag}`, {
        stdio: 'inherit',
        cwd: process.cwd()
      })
      console.log(`✅ ${pkg} - No issues found\n`)
    } catch (error: any) {
      // ESLint exits 0 when it finds only warnings, 1 when a rule reports an error and 2 when it cannot run.
      console.log(`❌ ${pkg} - ${error.status === 1 ? 'Lint errors found' : 'Linting failed'}\n`)
      hasErrors = true
    }
  } else {
    console.log(`⚠️  ${pkg} - Package not found, skipping\n`)
  }
}

if (hasErrors) {
  console.log('❌ Some packages had linting errors')
  process.exit(1)
} else {
  console.log('✅ All packages linted successfully!')
}
