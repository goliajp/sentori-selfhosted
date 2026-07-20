import { init } from './init';
import { addBreadcrumb } from './breadcrumbs';
import {
  captureError,
  captureException,
  captureMessage,
  captureStep,
  getUser,
  sendUserFeedback,
  setTag,
  setTags,
  setUser,
} from './capture';
import { ErrorBoundary } from './error-boundary';
import { FeedbackButton, type FeedbackButtonHandle, type FeedbackButtonProps } from './feedback-widget';
import {
  clearAllFeatureFlags,
  clearFeatureFlag,
  getFeatureFlags,
  setFeatureFlag,
} from './feature-flags';
import { clearMaskQuery, registerMaskQuery } from './mask';
import { measureFn } from './measure';
import {
  getColdStartMs,
  markTimeToFullDisplay,
  type TimeToFullDisplayHandle,
} from './mobile-vitals';
import { bindState, recordState, unbindState } from './state-snapshots';
import {
  startMoment,
  startSpan,
  startTrace,
  withScopedSpan,
} from '@goliapkg/sentori-core';
import { getInstallId } from './install-id';
import { flushMetrics, recordMetric } from './metrics';
import { close, flush } from './lifecycle';
import { linkFederatedIdentity, reportPinMismatch, reportSecurity } from './report-security';
import { flushTrack, track } from './track';
import { queryTrustScore } from './trust-score';
import { RageTapCapture } from './rage-tap';
import {
  endSession,
  markSessionCrashed,
  startSession,
} from './session-tracker';

export const sentori = {
  init,
  addBreadcrumb,
  setUser,
  getUser,
  setTag,
  setTags,
  captureError,
  captureException,
  captureMessage,
  captureStep,
  sendUserFeedback,
  recordMetric,
  flushMetrics,
  track,
  flushTrack,
  getInstallId,
  reportSecurity,
  reportPinMismatch,
  queryTrustScore,
  linkFederatedIdentity,
  measureFn,
  startMoment,
  startSpan,
  startTrace,
  withScopedSpan,
  bindState,
  recordState,
  unbindState,
  markTimeToFullDisplay,
  getColdStartMs,
  setFeatureFlag,
  clearFeatureFlag,
  clearAllFeatureFlags,
  getFeatureFlags,
  ErrorBoundary,
  FeedbackButton,
  RageTapCapture,
  registerMaskQuery,
  clearMaskQuery,
  startSession,
  endSession,
  markSessionCrashed,
  flush,
  close,
};

export default sentori;

export { init, init as initSentori } from './init';
export { addBreadcrumb } from './breadcrumbs';
export {
  captureError,
  captureException,
  captureMessage,
  captureStep,
  getUser,
  sendUserFeedback,
  setTag,
  setTags,
  setUser,
} from './capture';
export {
  startMoment,
  startSpan,
  startTrace,
  withScopedSpan,
  type SpanContextLike,
  type StartSpanOptions,
} from '@goliapkg/sentori-core';
export { close, flush } from './lifecycle';
export type {
  CaptureMessageOptions,
  MessageLevel,
} from '@goliapkg/sentori-core';
/** v2.3 — logger surface re-exported from core so hosts can
 *  `import { setLogLevel } from '@goliapkg/sentori-react-native'`
 *  per design §3 ("Production override"). `setLogTransport` lets
 *  hosts route Sentori-internal lines into their own log
 *  aggregator (Datadog, etc.). */
export {
  getLogLevel,
  type LogLevel,
  logger,
  type LogTransport,
  setLogLevel,
  setLogTransport,
} from '@goliapkg/sentori-core';
export type { ReadyInfo } from './config';
export { ErrorBoundary } from './error-boundary';
export { FeedbackButton, type FeedbackButtonHandle, type FeedbackButtonProps } from './feedback-widget';
export {
  clearAllFeatureFlags,
  clearFeatureFlag,
  getFeatureFlags,
  setFeatureFlag,
} from './feature-flags';
export { clearMaskQuery, registerMaskQuery } from './mask';
export { flushMetrics, recordMetric } from './metrics';
export { flushTrack, track, type TrackEvent, type TrackProps } from './track';
export { getInstallId, peekInstallId } from './install-id';
export {
  linkFederatedIdentity,
  reportPinMismatch,
  reportSecurity,
  type SecurityReportData,
} from './report-security';
export {
  queryTrustScore,
  type TrustScore,
  type TrustSignal,
} from './trust-score';
export { measureFn } from './measure';
export {
  getColdStartMs,
  markTimeToFullDisplay,
  type TimeToFullDisplayHandle,
} from './mobile-vitals';
export { MomentHandle, type MomentProperties } from '@goliapkg/sentori-core';
export {
  bindState,
  recordState,
  type StateSnapshot,
  unbindState,
} from './state-snapshots';
export { RageTapCapture } from './rage-tap';
export {
  probeNativeScreenshot,
  probeNativeWireframe,
  startAnrWatchdog,
  stopAnrWatchdog,
  triggerNativeCrash,
} from './native';
export { drainReplay, startReplay, stopReplay } from './replay';
export {
  endSession,
  markSessionCrashed,
  startSession,
} from './session-tracker';
export { type NavigationRefLike, useTraceNavigation } from './navigation';

// v2.9 — Push notifications (iOS this release; v2.10 lights Android).
// Surfaced as a `sentori.push` sub-namespace from the default barrel.
// Opt-in: `sentori.push.register({...})` triggers the OS permission
// prompt. Sentori never prompts on its own.
import * as _push from './push';
export const push = {
  register: _push.register,
  unregister: _push.unregister,
  getCachedIpt: _push.getCachedIpt,
  getStatus: _push.getStatus,
  requestPermission: _push.requestPermission,
  // v2.26 — let the host stamp the current session id on outgoing
  // ack POSTs so v2.27 push-correlation BI can JOIN on session_id.
  setSessionContext: _push.setSessionContext,
};
export type {
  PushRegisterOptions,
  PushRegisterResult,
  PushNotificationPayload,
} from './push';
export type {
  PushMessage,
  PushOptions,
  PushPriority,
  PushReceipt,
  PushTicket,
  PushTicketStatus,
} from '@goliapkg/sentori-core';

export type {
  Event,
  SentoriError,
  Frame,
  Breadcrumb,
  BreadcrumbType,
  Device,
  DeviceOS,
  App,
  User,
  Tags,
  EventKind,
  Platform,
} from './types';
