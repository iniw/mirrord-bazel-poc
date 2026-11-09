import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { Wizard } from '@mirrord/wizard'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <h1>mirrord {__MIRRORD_VERSION__}</h1>
    <Wizard />
  </StrictMode>,
)
