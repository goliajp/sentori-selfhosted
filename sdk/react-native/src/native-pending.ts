// Native pending-crash drain: the crash handlers (Swift/Kotlin)
// write one JSON file per crash in the legacy event shape; this
// module converts them to the v1 wire and enqueues.
//
// The converter lives on the JS side so the native writers stay
// untouched — native code doesn't compile in preflight/CI, and a
// silent .kt/.swift break ships to npm (the v3.0.0 K2 lesson).

import { uuidV7 } from '@goliapkg/sentori-core';
import type { SentoriError, WireEvent } from '@goliapkg/sentori-core';

import { drainNativePending } from './native';
import { enqueue, flush, uploadAttachment } from './transport';

type LegacyNativeEvent = {
  id?: string;
  timestamp?: string;
  platform?: string;
  release?: string;
  environment?: string;
  device?: Record<string, unknown>;
  app?: Record<string, unknown>;
  error?: {
    type?: string;
    message?: string;
    stack?: Array<Record<string, unknown>>;
  };
  _pendingAttachments?: Array<{
    kind?: string;
    base64?: string;
    mediaType?: string;
    source?: string;
  }>;
};

const toWire = (raw: LegacyNativeEvent): WireEvent => {
  const platform = raw.platform === 'android' ? 'android' : 'ios';
  const error: SentoriError = {
    type: raw.error?.type ?? 'NativeCrash',
    message: raw.error?.message ?? '',
    stack: (raw.error?.stack ?? []).map((f) => ({
      file: typeof f.file === 'string' ? f.file : undefined,
      function: typeof f.function === 'string' ? f.function : undefined,
      line: typeof f.line === 'number' ? f.line : undefined,
      inApp: typeof f.inApp === 'boolean' ? f.inApp : undefined,
    })),
    cause: null,
  };
  return {
    id: raw.id ?? uuidV7(),
    kind: 'error',
    occurredAt: raw.timestamp ?? new Date().toISOString(),
    platform,
    release: raw.release ?? '',
    environment: raw.environment ?? '',
    payload: {
      error,
      device: raw.device as WireEvent['payload']['device'],
      app: raw.app as WireEvent['payload']['app'],
      nativeCrash: true,
    },
  };
};

/** Drain, convert, enqueue, and ship pre-death attachments. */
export const shipNativePending = async (): Promise<void> => {
  const files = await drainNativePending();
  for (const text of files) {
    try {
      const raw = JSON.parse(text) as LegacyNativeEvent;
      const pending = raw._pendingAttachments;
      delete raw._pendingAttachments;
      const wire = toWire(raw);
      enqueue(wire);
      if (pending && wire.id) {
        for (const att of pending) {
          if (att.base64 && att.kind) {
            void uploadAttachment(
              wire.id,
              att.kind as import('@goliapkg/sentori-core').AttachmentKind,
              { base64: att.base64, mediaType: att.mediaType ?? 'application/octet-stream' },
              { source: (att.source as 'android' | 'ios' | 'js') ?? 'ios' },
            );
          }
        }
      }
    } catch {
      // One corrupt crash file must not block the rest.
    }
  }
  if (files.length > 0) await flush();
};
