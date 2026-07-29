#!/usr/bin/env node

import { createHash } from 'node:crypto'
import { createReadStream, writeFileSync } from 'node:fs'
import { basename, resolve } from 'node:path'

function fail(message) {
  console.error(`generate-client-hash-manifest: ${message}`)
  process.exit(1)
}

const options = new Map()
for (let index = 2; index < process.argv.length; index += 2) {
  const key = process.argv[index]
  const value = process.argv[index + 1]
  if (!key?.startsWith('--') || value === undefined) {
    fail('expected --executable, --output, --version, --target and --git-commit')
  }
  options.set(key.slice(2), value)
}

for (const required of ['executable', 'output', 'version', 'target', 'git-commit']) {
  if (!options.has(required)) fail(`missing --${required}`)
}

const executable = resolve(options.get('executable'))
const hash = createHash('sha256')
const input = createReadStream(executable)
input.on('error', (error) => fail(error.message))
input.on('data', (chunk) => hash.update(chunk))
input.on('end', () => {
  const manifest = {
    schema_version: 1,
    client: 'bmz-player',
    version: options.get('version'),
    git_commit: options.get('git-commit'),
    target: options.get('target'),
    executable: basename(executable),
    client_hash: hash.digest('hex'),
  }
  writeFileSync(resolve(options.get('output')), `${JSON.stringify(manifest, null, 2)}\n`)
})
