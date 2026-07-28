import assert from 'node:assert/strict'
import { execFileSync } from 'node:child_process'
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { resolve } from 'node:path'
import test from 'node:test'
import { fileURLToPath } from 'node:url'

const script = fileURLToPath(new URL('./combine-client-hash-manifests.mjs', import.meta.url))

function writeManifest(directory, target, overrides = {}) {
  writeFileSync(
    resolve(directory, `${target}-client-manifest.json`),
    JSON.stringify({
      schema_version: 1,
      client: 'bmz-player',
      version: '0.1.11',
      git_commit: 'a'.repeat(40),
      target,
      executable: target.startsWith('windows') ? 'bmz-player.exe' : 'bmz-player',
      client_hash: 'b'.repeat(64),
      ...overrides,
    }),
  )
}

test('combines schema v1 target manifests into deterministic schema v2', () => {
  const directory = mkdtempSync(resolve(tmpdir(), 'bmz-client-manifest-'))
  for (const target of ['windows-x64', 'macos-arm64', 'linux-x64-flatpak', 'macos-x64']) {
    writeManifest(directory, target)
  }
  const output = resolve(directory, 'client-manifest-bmz-player-v0.1.11.json')

  execFileSync(process.execPath, [
    script,
    '--input-dir',
    directory,
    '--output',
    output,
    '--expected-targets',
    'windows-x64,macos-arm64,macos-x64,linux-x64-flatpak',
  ])

  const combined = JSON.parse(readFileSync(output, 'utf8'))
  assert.equal(combined.schema_version, 2)
  assert.equal(combined.client, 'bmz-player')
  assert.deepEqual(
    combined.artifacts.map((artifact) => artifact.target),
    ['linux-x64-flatpak', 'macos-arm64', 'macos-x64', 'windows-x64'],
  )
})

test('rejects manifests from different commits', () => {
  const directory = mkdtempSync(resolve(tmpdir(), 'bmz-client-manifest-'))
  writeManifest(directory, 'windows-x64')
  writeManifest(directory, 'macos-arm64', { git_commit: 'c'.repeat(40) })

  assert.throws(() =>
    execFileSync(
      process.execPath,
      [script, '--input-dir', directory, '--output', resolve(directory, 'combined.json')],
      { stdio: 'pipe' },
    ),
  )
})
