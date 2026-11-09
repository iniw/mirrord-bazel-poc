import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import fs from 'fs'
import path from 'path'

// The bundle is embedded into the CLI binary built from the same checkout, so the workspace
// version is exactly the mirrord version this UI ships in. Bazel passes it in `MIRRORD_VERSION`,
// so that the bundle doesn't depend on the rest of `Cargo.toml`, and other builds read it from
// `Cargo.toml` themselves.
function cargoVersion(): string | undefined {
  const cargoToml = fs.readFileSync(path.resolve(__dirname, '../../Cargo.toml'), 'utf8')
  return /^version = "([^"]+)"/m.exec(
    cargoToml.slice(cargoToml.indexOf('[workspace.package]')),
  )?.[1]
}
const mirrordVersion = process.env.MIRRORD_VERSION || cargoVersion()
if (!mirrordVersion) {
  throw new Error('failed to read the workspace version from Cargo.toml')
}

// `mirrord-ui` is the single built site. It composes the feature packages, like the config wizard
// (`packages/wizard`), which are compiled straight from source via the aliases below.
export default defineConfig({
  plugins: [react()],
  define: {
    __MIRRORD_VERSION__: JSON.stringify(mirrordVersion),
  },
  resolve: {
    alias: {
      '@mirrord/wizard': path.resolve(__dirname, '../wizard/src/index.tsx'),
    },
  },
})
