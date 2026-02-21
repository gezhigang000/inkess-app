#!/usr/bin/env node
/**
 * setup-python.mjs
 * Downloads python-build-standalone and installs scientific packages
 * into code/src-tauri/python-standalone/ for embedding in the app.
 */

import { execSync } from 'node:child_process'
import { createWriteStream, existsSync, mkdirSync, rmSync } from 'node:fs'
import { pipeline } from 'node:stream/promises'
import { join, dirname } from 'node:path'
import { fileURLToPath } from 'node:url'
import { createGunzip } from 'node:zlib'
import { Readable } from 'node:stream'

const __dirname = dirname(fileURLToPath(import.meta.url))
const TARGET_DIR = join(__dirname, '..', 'src-tauri', 'python-standalone')

// python-build-standalone release info
const PBS_VERSION = '20241219'
const PYTHON_VERSION = '3.12.8'
const BASE_URL = `https://github.com/indygreg/python-build-standalone/releases/download/${PBS_VERSION}`

function getPlatformInfo() {
  const platform = process.platform
  const arch = process.arch

  if (platform === 'darwin' && arch === 'arm64') {
    return {
      filename: `cpython-${PYTHON_VERSION}+${PBS_VERSION}-aarch64-apple-darwin-install_only.tar.gz`,
      pythonBin: 'bin/python3',
    }
  }
  if (platform === 'darwin' && arch === 'x64') {
    return {
      filename: `cpython-${PYTHON_VERSION}+${PBS_VERSION}-x86_64-apple-darwin-install_only.tar.gz`,
      pythonBin: 'bin/python3',
    }
  }
  if (platform === 'win32' && arch === 'x64') {
    return {
      filename: `cpython-${PYTHON_VERSION}+${PBS_VERSION}-x86_64-pc-windows-msvc-install_only.tar.gz`,
      pythonBin: 'python.exe',
    }
  }
  if (platform === 'linux' && arch === 'x64') {
    return {
      filename: `cpython-${PYTHON_VERSION}+${PBS_VERSION}-x86_64-unknown-linux-gnu-install_only.tar.gz`,
      pythonBin: 'bin/python3',
    }
  }

  console.error(`Unsupported platform: ${platform}-${arch}`)
  process.exit(1)
}

async function download(url, dest) {
  console.log(`Downloading: ${url}`)
  const resp = await fetch(url, { redirect: 'follow' })
  if (!resp.ok) throw new Error(`Download failed: ${resp.status} ${resp.statusText}`)
  const fileStream = createWriteStream(dest)
  await pipeline(Readable.fromWeb(resp.body), fileStream)
  console.log(`Saved to: ${dest}`)
}

async function main() {
  const info = getPlatformInfo()
  const url = `${BASE_URL}/${info.filename}`
  const tarball = join(__dirname, info.filename)

  // Clean previous installation
  if (existsSync(TARGET_DIR)) {
    console.log('Removing previous python-standalone...')
    rmSync(TARGET_DIR, { recursive: true, force: true })
  }

  // Download
  if (!existsSync(tarball)) {
    await download(url, tarball)
  } else {
    console.log(`Using cached tarball: ${tarball}`)
  }

  // Extract — tar.gz extracts to a "python/" directory
  console.log('Extracting...')
  const extractDir = join(__dirname, '..', 'src-tauri')
  execSync(`tar -xzf "${tarball}" -C "${extractDir}"`, { stdio: 'inherit' })

  // python-build-standalone extracts to "python/", rename to "python-standalone/"
  const extractedDir = join(extractDir, 'python')
  if (existsSync(extractedDir)) {
    execSync(`mv "${extractedDir}" "${TARGET_DIR}"`)
  } else if (!existsSync(TARGET_DIR)) {
    console.error('Extraction failed: python directory not found')
    process.exit(1)
  }

  // Install scientific packages
  const pythonBin = join(TARGET_DIR, info.pythonBin)
  console.log(`Python binary: ${pythonBin}`)

  const packages = [
    'numpy', 'matplotlib', 'pandas', 'scipy', 'sympy', 'Pillow', 'openpyxl',
  ]
  console.log(`Installing packages: ${packages.join(', ')}`)
  execSync(
    `"${pythonBin}" -m pip install --no-warn-script-location ${packages.join(' ')}`,
    { stdio: 'inherit' },
  )

  // Clean up tarball
  rmSync(tarball, { force: true })

  console.log('\nDone! python-standalone is ready at:')
  console.log(`  ${TARGET_DIR}`)
  console.log(`\nTest: "${pythonBin}" -c "import numpy; print(numpy.__version__)"`)
}

main().catch(err => {
  console.error(err)
  process.exit(1)
})
