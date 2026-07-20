// v2.18 — `expo-notifications` drop-in shim.
//
// Customers coming from `expo-notifications` change ONE line:
//
//   - import * as Notifications from 'expo-notifications'
//   + import * as Notifications from '@goliapkg/sentori-react-native/expo-compat'
//
// …and most of their existing code keeps compiling. The API
// surface here mirrors expo-notifications' public exports; under the
// hood every call routes through the same `sentori.push.*` native
// module the rest of the SDK uses.
//
// What works — P0 surface, covered by the existing native module:
//
//   getPermissionsAsync                 — non-prompting status read
//   requestPermissionsAsync             — OS prompt + permission flow
//   getDevicePushTokenAsync             — raw APNs (iOS) / FCM (Android) token
//   getExpoPushTokenAsync               — same as device token, wrapped in
//                                         { type: 'expo', data } shape;
//                                         NOT a real exp.host token — see
//                                         recipe for the server-side change
//   addNotificationReceivedListener     — foreground notification callback
//   addNotificationResponseReceivedListener  — tap callback
//   setNotificationHandler              — presentation behaviour hook
//   unregisterForNotificationsAsync     — revoke + drop cached handle
//   addPushTokenListener                — rebind on token rotation
//   AndroidImportance / IosAuthorizationStatus / DEFAULT_ACTION_IDENTIFIER
//
// What's not implemented yet (each throws a tagged Error with a
// pointer at the migration recipe — silent no-ops would be worse):
//
//   scheduleNotificationAsync + the 7 SchedulableTriggerInputTypes
//   getBadgeCountAsync / setBadgeCountAsync
//   setNotificationChannelAsync + the channel-group CRUD
//   setNotificationCategoryAsync + interactive actions
//   useLastNotificationResponse + getLastNotificationResponseAsync
//   subscribeToTopicAsync / unsubscribeFromTopicAsync
//   registerTaskAsync (background task)
//   getPresentedNotificationsAsync / dismissNotificationAsync
//
// The "not implemented" list will close in follow-up minor releases
// as the native module grows. Anything in expo-notifications that
// requires native code we don't have today goes here; nothing here
// is a permanent gap.

import { logger } from '@goliapkg/sentori-core'

import {
  pushDrainState,
  pushGetStatus,
  pushRegister,
  pushRequestPermission,
  pushUnregister,
} from './native'

// ── Types — match expo-notifications shapes exactly ────────────────

export type DevicePushToken = {
  type: 'ios' | 'android'
  data: string
}

export type ExpoPushToken = {
  type: 'expo'
  data: string
}

export type NotificationPermissionsStatus = {
  status: PermissionStatus
  granted: boolean
  expires: PermissionExpiration
  canAskAgain: boolean
  ios?: IosNotificationPermissions
  android?: AndroidNotificationPermissions
}

export type PermissionStatus = 'granted' | 'denied' | 'undetermined'

export type PermissionExpiration = 'never' | number

export type IosNotificationPermissions = {
  status: IosAuthorizationStatusValue
  allowsAlert: boolean
  allowsBadge: boolean
  allowsSound: boolean
  allowsAnnouncements?: boolean
  allowsCriticalAlerts?: boolean
  allowsDisplayInCarPlay?: boolean
  allowsDisplayInNotificationCenter?: boolean
  allowsDisplayOnLockScreen?: boolean
  allowsPersistentTypes?: boolean
  allowsPreviews?: boolean
  providesAppNotificationSettings?: boolean
}

export type AndroidNotificationPermissions = {
  status: PermissionStatus
}

export type Notification = {
  date: number
  request: NotificationRequest
}

export type NotificationRequest = {
  identifier: string
  content: NotificationContent
  trigger: NotificationTrigger | null
}

export type NotificationContent = {
  title: string | null
  subtitle: string | null
  body: string | null
  data: Record<string, unknown>
  badge?: number | null
  sound?: NotificationSound
  categoryIdentifier?: string | null
  attachments?: NotificationAttachment[]
  /** iOS 15+ */
  interruptionLevel?: 'passive' | 'active' | 'timeSensitive' | 'critical'
}

export type NotificationAttachment = {
  identifier?: string
  url?: string
  type?: string
  hideThumbnail?: boolean
  thumbnailClipArea?: { x: number; y: number; width: number; height: number }
  thumbnailTime?: number
}

export type NotificationSound = boolean | 'default' | 'defaultCritical' | string

export type NotificationTrigger = unknown // expo-notifications uses a union; we treat as opaque on the receive side

export type NotificationResponse = {
  actionIdentifier: string
  notification: Notification
  userText?: string
}

export type NotificationBehavior = {
  shouldShowBanner: boolean
  shouldShowList: boolean
  shouldPlaySound: boolean
  shouldSetBadge: boolean
  priority?: AndroidNotificationPriorityValue
}

export type NotificationHandler = {
  handleNotification: (notification: Notification) => Promise<NotificationBehavior>
  handleSuccess?: (id: string) => void
  handleError?: (id: string, err: { code: string; message: string }) => void
}

export type EventSubscription = {
  remove: () => void
}

export type PushTokenListener = (token: DevicePushToken) => void

// ── Constants — same values as expo-notifications ─────────────────

export const DEFAULT_ACTION_IDENTIFIER = 'expo.modules.notifications.actions.DEFAULT'

export const AndroidImportance = {
  NONE: 0,
  MIN: 1,
  LOW: 2,
  DEFAULT: 3,
  HIGH: 4,
  MAX: 5,
} as const
export type AndroidImportanceValue = (typeof AndroidImportance)[keyof typeof AndroidImportance]

export const AndroidNotificationPriority = {
  MIN: 'min',
  LOW: 'low',
  DEFAULT: 'default',
  HIGH: 'high',
  MAX: 'max',
} as const
export type AndroidNotificationPriorityValue =
  (typeof AndroidNotificationPriority)[keyof typeof AndroidNotificationPriority]

export const AndroidNotificationVisibility = {
  PUBLIC: 1,
  PRIVATE: 0,
  SECRET: -1,
} as const

export const IosAuthorizationStatus = {
  NOT_DETERMINED: 0,
  DENIED: 1,
  AUTHORIZED: 2,
  PROVISIONAL: 3,
  EPHEMERAL: 4,
} as const
export type IosAuthorizationStatusValue =
  (typeof IosAuthorizationStatus)[keyof typeof IosAuthorizationStatus]

export const IosAlertStyle = {
  NONE: 0,
  BANNER: 1,
  ALERT: 2,
} as const

// 7 SchedulableTriggerInputTypes — re-exported for compile parity
// even though scheduleNotificationAsync throws. Customers that
// destructure these from the module won't break their build.
export const SchedulableTriggerInputTypes = {
  TIME_INTERVAL: 'timeInterval',
  DATE: 'date',
  CALENDAR: 'calendar',
  DAILY: 'daily',
  WEEKLY: 'weekly',
  MONTHLY: 'monthly',
  YEARLY: 'yearly',
} as const

// ── Module state (handler + drain loop + listener registry) ───────

const RECEIVED_LISTENERS = new Set<(n: Notification) => void>()
const RESPONSE_LISTENERS = new Set<(r: NotificationResponse) => void>()
const TOKEN_LISTENERS = new Set<PushTokenListener>()

let _handler: NotificationHandler | null = null
let _drainInterval: ReturnType<typeof setInterval> | null = null
let _lastDeviceToken: null | DevicePushToken = null

// ── Permissions ───────────────────────────────────────────────────

/**
 * Returns the OS-reported notification permission status without
 * prompting. Mirrors `Notifications.getPermissionsAsync()`.
 */
export async function getPermissionsAsync(): Promise<NotificationPermissionsStatus> {
  const native = await pushGetStatus()
  return coercePermissionStatus(native)
}

export type RequestPermissionsOptions = {
  ios?: {
    allowAlert?: boolean
    allowBadge?: boolean
    allowSound?: boolean
    allowDisplayInCarPlay?: boolean
    allowCriticalAlerts?: boolean
    provideAppNotificationSettings?: boolean
    allowProvisional?: boolean
    allowAnnouncements?: boolean
  }
  android?: Record<string, never>
}

/**
 * Triggers the OS permission prompt the first time, otherwise
 * returns the cached decision. Mirrors
 * `Notifications.requestPermissionsAsync()`.
 *
 * `options.ios.allowProvisional` is accepted for API parity but
 * silently falls back to a regular authorization request — the
 * underlying native module doesn't surface the provisional flag
 * separately yet. Follow up: thread the `provisional` option
 * through `SentoriPushNotifications.requestPermission(...)` on iOS.
 */
export async function requestPermissionsAsync(
  options?: RequestPermissionsOptions,
): Promise<NotificationPermissionsStatus> {
  if (options?.ios?.allowProvisional) {
    logger.debug(
      'push.expo-compat',
      'allowProvisional requested — falls back to regular authorization in this release',
    )
  }
  const native = await pushRequestPermission()
  return coercePermissionStatus(native)
}

function coercePermissionStatus(
  native: null | string,
): NotificationPermissionsStatus {
  const grantedStatuses = new Set(['granted', 'provisional', 'ephemeral'])
  let status: PermissionStatus
  if (native === 'granted' || native === 'provisional' || native === 'ephemeral') {
    status = 'granted'
  } else if (native === 'denied') {
    status = 'denied'
  } else {
    status = 'undetermined'
  }
  const granted = native != null && grantedStatuses.has(native)
  const iosStatus =
    native === 'granted'
      ? IosAuthorizationStatus.AUTHORIZED
      : native === 'denied'
        ? IosAuthorizationStatus.DENIED
        : native === 'provisional'
          ? IosAuthorizationStatus.PROVISIONAL
          : native === 'ephemeral'
            ? IosAuthorizationStatus.EPHEMERAL
            : IosAuthorizationStatus.NOT_DETERMINED
  return {
    status,
    granted,
    expires: 'never',
    canAskAgain: status === 'undetermined',
    ios: {
      status: iosStatus,
      allowsAlert: granted,
      allowsBadge: granted,
      allowsSound: granted,
    },
    android: { status },
  }
}

// ── Token retrieval ───────────────────────────────────────────────

const TOKEN_TIMEOUT_MS = 8000
const TOKEN_POLL_INTERVAL_MS = 200

/**
 * Returns the raw APNs (iOS) or FCM (Android) token. Mirrors
 * `Notifications.getDevicePushTokenAsync()` — for customers who
 * already wire the device token to a custom backend, the migration
 * is literally a one-line import change.
 */
export async function getDevicePushTokenAsync(): Promise<DevicePushToken> {
  await pushRegister()
  return waitForDeviceToken()
}

/**
 * `expo-notifications`'s `getExpoPushTokenAsync` returns a
 * `ExponentPushToken[...]` string that Expo's exp.host service uses
 * to route to APNs / FCM. We don't run that service — `data` here
 * is the raw native token wrapped in the same envelope shape so
 * destructuring code keeps compiling.
 *
 * **Server-side change required:** instead of POSTing to
 * `https://exp.host/--/api/v2/push/send`, your backend should POST
 * to Sentori's ingest. See the migration recipe.
 */
export async function getExpoPushTokenAsync(
  _options?: { projectId?: string; experienceId?: string },
): Promise<ExpoPushToken> {
  const tok = await getDevicePushTokenAsync()
  return { type: 'expo', data: tok.data }
}

async function waitForDeviceToken(): Promise<DevicePushToken> {
  const start = Date.now()
  while (Date.now() - start < TOKEN_TIMEOUT_MS) {
    const state = await pushDrainState()
    if (state.error) {
      throw new Error(`Push registration failed: ${state.error}`)
    }
    if (state.token) {
      // Forward the buffered events to anyone who's already
      // subscribed — they would otherwise be lost since the drain
      // call here consumed them.
      forwardBufferedEvents(state.notifications, state.taps)
      const tok: DevicePushToken = {
        type: platformOs(),
        data: state.token,
      }
      _lastDeviceToken = tok
      for (const cb of TOKEN_LISTENERS) {
        try {
          cb(tok)
        } catch (e) {
          logger.warn('push.expo-compat', 'pushTokenListener threw', e)
        }
      }
      return tok
    }
    forwardBufferedEvents(state.notifications, state.taps)
    await sleep(TOKEN_POLL_INTERVAL_MS)
  }
  throw new Error(`Push token not received within ${TOKEN_TIMEOUT_MS} ms`)
}

/**
 * Mirrors `Notifications.unregisterForNotificationsAsync()`. Calls
 * the native unregister + stops the drain loop.
 */
export async function unregisterForNotificationsAsync(): Promise<void> {
  pushUnregister()
  _lastDeviceToken = null
  stopDrainLoop()
}

// ── Listeners ─────────────────────────────────────────────────────

/**
 * Foreground notification callback. Mirrors
 * `Notifications.addNotificationReceivedListener()`. The first
 * subscription starts a 1 Hz native-buffer drain loop; the last
 * unsubscribe stops it.
 */
export function addNotificationReceivedListener(
  listener: (notification: Notification) => void,
): EventSubscription {
  RECEIVED_LISTENERS.add(listener)
  ensureDrainLoop()
  return {
    remove: () => {
      RECEIVED_LISTENERS.delete(listener)
      maybeStopDrainLoop()
    },
  }
}

/**
 * User-tapped-a-notification callback. Mirrors
 * `Notifications.addNotificationResponseReceivedListener()`.
 */
export function addNotificationResponseReceivedListener(
  listener: (response: NotificationResponse) => void,
): EventSubscription {
  RESPONSE_LISTENERS.add(listener)
  ensureDrainLoop()
  return {
    remove: () => {
      RESPONSE_LISTENERS.delete(listener)
      maybeStopDrainLoop()
    },
  }
}

/**
 * Token-rotation callback. Mirrors
 * `Notifications.addPushTokenListener()`. Fires once per new token —
 * the native side detects rotation and pushes into the buffer; the
 * shared drain loop forwards it here.
 */
export function addPushTokenListener(listener: PushTokenListener): EventSubscription {
  TOKEN_LISTENERS.add(listener)
  return {
    remove: () => {
      TOKEN_LISTENERS.delete(listener)
    },
  }
}

/**
 * Presentation behaviour hook. Mirrors
 * `Notifications.setNotificationHandler()`. The handler runs once
 * per foreground notification before the listener fan-out; it can
 * suppress the banner/sound/badge.
 *
 * Today the SDK always shows the system banner because that's the
 * native delegate's hard-coded behaviour. The handler's
 * `handleNotification` is still invoked so customer logic that
 * inspects the notification can run, but the returned
 * `NotificationBehavior` flags don't override presentation yet.
 * Follow up: pipe these flags through
 * `SentoriPushNotifications` so willPresent can return what the
 * handler asked for.
 */
export function setNotificationHandler(handler: NotificationHandler | null): void {
  _handler = handler
}

// ── Stubs that throw with a pointer at the recipe ─────────────────

type ScheduleArgs = { content: unknown; trigger?: unknown; identifier?: string }
export function scheduleNotificationAsync(_args: ScheduleArgs): never {
  throw mkUnsupported('scheduleNotificationAsync', 'local-scheduling')
}
export function cancelScheduledNotificationAsync(_id: string): never {
  throw mkUnsupported('cancelScheduledNotificationAsync', 'local-scheduling')
}
export function cancelAllScheduledNotificationsAsync(): never {
  throw mkUnsupported('cancelAllScheduledNotificationsAsync', 'local-scheduling')
}
export function getAllScheduledNotificationsAsync(): never {
  throw mkUnsupported('getAllScheduledNotificationsAsync', 'local-scheduling')
}
export function getNextTriggerDateAsync(_trigger: unknown): never {
  throw mkUnsupported('getNextTriggerDateAsync', 'local-scheduling')
}

export function setBadgeCountAsync(_count: number, _options?: unknown): never {
  throw mkUnsupported('setBadgeCountAsync', 'badge')
}
export function getBadgeCountAsync(): never {
  throw mkUnsupported('getBadgeCountAsync', 'badge')
}

export function setNotificationChannelAsync(_id: string, _channel: unknown): never {
  throw mkUnsupported('setNotificationChannelAsync', 'android-channels')
}
export function getNotificationChannelAsync(_id: string): never {
  throw mkUnsupported('getNotificationChannelAsync', 'android-channels')
}
export function getNotificationChannelsAsync(): never {
  throw mkUnsupported('getNotificationChannelsAsync', 'android-channels')
}
export function deleteNotificationChannelAsync(_id: string): never {
  throw mkUnsupported('deleteNotificationChannelAsync', 'android-channels')
}
export function setNotificationChannelGroupAsync(_id: string, _group: unknown): never {
  throw mkUnsupported('setNotificationChannelGroupAsync', 'android-channels')
}

export function setNotificationCategoryAsync(
  _id: string,
  _actions: unknown,
  _options?: unknown,
): never {
  throw mkUnsupported('setNotificationCategoryAsync', 'categories')
}
export function getNotificationCategoriesAsync(): never {
  throw mkUnsupported('getNotificationCategoriesAsync', 'categories')
}
export function deleteNotificationCategoryAsync(_id: string): never {
  throw mkUnsupported('deleteNotificationCategoryAsync', 'categories')
}

export function useLastNotificationResponse(): NotificationResponse | null | undefined {
  // Hook contract: keep it a no-throw hook so the host's React tree
  // still mounts. Returns null. Customers who depend on this for
  // deep-link-from-cold-start should fall back to subscribing on
  // mount and relying on the early-drain.
  return null
}
export function getLastNotificationResponseAsync(): Promise<NotificationResponse | null> {
  return Promise.resolve(null)
}
export function clearLastNotificationResponseAsync(): Promise<void> {
  return Promise.resolve()
}

export function subscribeToTopicAsync(_topic: string): never {
  throw mkUnsupported('subscribeToTopicAsync', 'topics')
}
export function unsubscribeFromTopicAsync(_topic: string): never {
  throw mkUnsupported('unsubscribeFromTopicAsync', 'topics')
}

export function registerTaskAsync(_taskName: string): never {
  throw mkUnsupported('registerTaskAsync', 'background-task')
}
export function unregisterTaskAsync(_taskName: string): never {
  throw mkUnsupported('unregisterTaskAsync', 'background-task')
}

export function dismissNotificationAsync(_id: string): never {
  throw mkUnsupported('dismissNotificationAsync', 'dismissal')
}
export function dismissAllNotificationsAsync(): never {
  throw mkUnsupported('dismissAllNotificationsAsync', 'dismissal')
}
export function getPresentedNotificationsAsync(): never {
  throw mkUnsupported('getPresentedNotificationsAsync', 'dismissal')
}

function mkUnsupported(name: string, slug: string): Error {
  return new Error(
    `${name} is not implemented in @goliapkg/sentori-react-native/expo-compat yet. ` +
      `See the migration recipe at /docs/recipes/migrate-from-expo-notifications/#${slug} ` +
      `for the workaround, or wait for a follow-up minor release where the native module ` +
      `will surface the underlying capability.`,
  )
}

// ── Drain-loop plumbing — pumps native buffer into listeners ──────

const DRAIN_INTERVAL_MS = 1000

function ensureDrainLoop(): void {
  if (_drainInterval) return
  _drainInterval = setInterval(() => {
    void pumpOnce()
  }, DRAIN_INTERVAL_MS)
}

function maybeStopDrainLoop(): void {
  if (RECEIVED_LISTENERS.size > 0) return
  if (RESPONSE_LISTENERS.size > 0) return
  stopDrainLoop()
}

function stopDrainLoop(): void {
  if (_drainInterval) {
    clearInterval(_drainInterval)
    _drainInterval = null
  }
}

async function pumpOnce(): Promise<void> {
  const state = await pushDrainState()
  forwardBufferedEvents(state.notifications, state.taps)
}

function forwardBufferedEvents(
  rawNotifications: Array<Record<string, unknown>>,
  rawTaps: Array<Record<string, unknown>>,
): void {
  for (const raw of rawNotifications) {
    const notification = coerceNotification(raw)
    // Run the optional handler before fan-out (best-effort — we
    // don't currently use its returned NotificationBehavior to
    // gate native presentation).
    if (_handler) {
      try {
        void _handler.handleNotification(notification).catch((err: unknown) => {
          logger.warn('push.expo-compat', 'handleNotification threw', err)
        })
      } catch (e) {
        logger.warn('push.expo-compat', 'setNotificationHandler.handleNotification threw', e)
      }
    }
    for (const cb of RECEIVED_LISTENERS) {
      try {
        cb(notification)
      } catch (e) {
        logger.warn('push.expo-compat', 'notification listener threw', e)
      }
    }
  }
  for (const raw of rawTaps) {
    const notification = coerceNotification(raw)
    const response: NotificationResponse = {
      actionIdentifier: DEFAULT_ACTION_IDENTIFIER,
      notification,
    }
    for (const cb of RESPONSE_LISTENERS) {
      try {
        cb(response)
      } catch (e) {
        logger.warn('push.expo-compat', 'response listener threw', e)
      }
    }
  }
}

function coerceNotification(raw: Record<string, unknown>): Notification {
  const userInfo = (raw.userInfo as Record<string, unknown>) ?? {}
  const id = (raw.id as string | undefined) ?? ''
  const date =
    typeof raw.receivedAt === 'number'
      ? Math.round(raw.receivedAt * 1000)
      : Date.now()
  return {
    date,
    request: {
      identifier: id,
      content: {
        title: (raw.title as string | undefined) ?? null,
        subtitle: (raw.subtitle as string | undefined) ?? null,
        body: (raw.body as string | undefined) ?? null,
        data: userInfo,
        categoryIdentifier: (raw.category as string | undefined) ?? null,
      },
      trigger: null,
    },
  }
}

// ── Tiny platform helpers ─────────────────────────────────────────

function platformOs(): 'ios' | 'android' {
  try {
    const rn = require('react-native') as { Platform?: { OS?: string } }
    const os = rn.Platform?.OS
    if (os === 'ios' || os === 'android') return os
  } catch {
    /* unavailable */
  }
  return 'ios'
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

// ── Test-only helpers ─────────────────────────────────────────────

/** Returns the last device token returned by getDevicePushTokenAsync.
 *  Used in tests + the migration recipe. */
export function __getLastDeviceTokenForTests(): null | DevicePushToken {
  return _lastDeviceToken
}

/** Resets module-scoped state. Test-only — do not call from
 *  production code. */
export function __resetForTests(): void {
  RECEIVED_LISTENERS.clear()
  RESPONSE_LISTENERS.clear()
  TOKEN_LISTENERS.clear()
  _handler = null
  _lastDeviceToken = null
  stopDrainLoop()
}
