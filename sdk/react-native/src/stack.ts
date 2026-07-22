// Phase 21: regex + parse loop moved to @goliapkg/sentori-core.
// RN keeps the long path (no shortFilenames option) because Hermes
// paths are already short and native symbolication needs the
// absolute form.
import { parseStack as parseStackCore } from '@goliapkg/sentori-core'

import type { Frame } from './types'

export const parseStack = (stack: string | undefined): Frame[] =>
  parseStackCore(stack)
