// Phase 21: wire-format types moved to @goliapkg/sentori-core. Re-
// exported here so existing relative imports keep working.

export type {
  App,
  AttachmentKind,
  AttachmentMeta,
  AttachmentSource,
  Breadcrumb,
  BreadcrumbType,
  CaptureExtras,
  Device,
  DeviceOS,
  Event,
  EventKind,
  Frame,
  Platform,
  SentoriError,
  Tags,
  User,
} from '@goliapkg/sentori-core'
