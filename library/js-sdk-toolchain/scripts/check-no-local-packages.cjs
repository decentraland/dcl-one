const fs = require('fs')
const path = require('path')
const npa = require('npm-package-arg')

// Function to recursively find all package.json files
function findPackageJsonFiles(dir, fileList = []) {
  const files = fs.readdirSync(dir)

  for (const file of files) {
    const filePath = path.join(dir, file)
    const stat = fs.statSync(filePath)

    if (stat.isDirectory() && !filePath.includes('node_modules')) {
      findPackageJsonFiles(filePath, fileList)
    } else if (file === 'package.json') {
      fileList.push(filePath)
    }
  }

  return fileList
}

const DEPENDENCY_FIELDS = ['dependencies', 'devDependencies', 'peerDependencies', 'optionalDependencies', 'overrides']

// Only what the registry resolves: a version, a range or a dist-tag, directly or through an `npm:`
// alias to one (a rescoped sibling). Anything else (a path, a tarball, a link, a workspace, a URL, a
// git or `user/repo` source) names something unpublished or not under its own name.
const PUBLISHED_TYPES = ['version', 'range', 'tag']

function unpublishedReason(name, spec) {
  if (typeof spec !== 'string') return `is not a version string: ${JSON.stringify(spec)}`
  // an override's `$name` reuses that direct dependency's spec, which is checked on its own
  if (spec.startsWith('$')) return null
  try {
    const parsed = npa.resolve(name, spec)
    const type = parsed.type === 'alias' ? parsed.subSpec.type : parsed.type
    return PUBLISHED_TYPES.includes(type) ? null : `is a ${type} dependency, not a published version: ${spec}`
  } catch (error) {
    return `is not a valid dependency spec (${error.message}): ${spec}`
  }
}

// `overrides` nests: a key maps to a spec or to an object of further overrides ("." is the key's own spec)
function* dependencyEntries(field, deps, prefix = '') {
  for (const [key, value] of Object.entries(deps)) {
    if (field === 'overrides' && value && typeof value === 'object') {
      yield* dependencyEntries(field, value, `${prefix}${key} > `)
    } else {
      const name = key === '.' ? prefix.slice(0, -3) : key
      yield [`${prefix}${key}`, name.replace(/@[^@/]+$/, ''), value]
    }
  }
}

// Check for local package dependencies
function checkNoLocalPackages(packageJsonPath) {
  try {
    const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf-8'))
    const errors = []

    for (const field of DEPENDENCY_FIELDS) {
      const deps = packageJson[field]
      if (!deps) continue

      for (const [label, name, spec] of dependencyEntries(field, deps)) {
        const reason = unpublishedReason(name, spec)
        if (reason) errors.push(`${field}: ${label} ${reason}`)
      }
    }

    // bundled packages ship inside the tarball instead of coming from the registry
    const bundled = packageJson.bundleDependencies || packageJson.bundledDependencies
    if (bundled === true || (Array.isArray(bundled) && bundled.length)) {
      errors.push(`bundleDependencies: ships packages inside the tarball: ${JSON.stringify(bundled)}`)
    }

    if (errors.length) {
      console.error(`\n\nErrors in ${packageJsonPath}:`)
      errors.forEach((error) => console.error(`- ${error}`))
      return false
    }

    return true
  } catch (error) {
    console.error(`Error processing ${packageJsonPath}: ${error.message}`)
    return false
  }
}

// Main function
function main() {
  // Check if a directory was passed as an argument
  const targetDir = process.argv[2] || '.'

  console.log(`Checking for local package dependencies in ${targetDir}...`)
  const packageJsonFiles = findPackageJsonFiles(targetDir)
  console.log(`Found ${packageJsonFiles.length} package.json files to check.`)

  let hasErrors = false

  for (const packageJsonPath of packageJsonFiles) {
    console.log(`Checking ${packageJsonPath}...`)
    if (!checkNoLocalPackages(packageJsonPath)) {
      hasErrors = true
    }
  }

  if (hasErrors) {
    console.error(
      `\n\nFound packages with local dependencies in ${targetDir}. Please publish these dependencies instead of using local references.`
    )
    process.exit(1)
  } else {
    console.log(`\n\nNo local dependencies found in ${targetDir}. All good!`)
  }
}

main()
