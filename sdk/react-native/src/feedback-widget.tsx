// v0.9.1 #9 — Feedback Widget.
//
// `<FeedbackButton trigger="shake|manual|fab" />` — drop into the
// app root, opens a modal prompt that submits via the existing
// `sentori.sendUserFeedback` API. Shake detection is opt-in and
// requires `expo-sensors` (Accelerometer) — falls back to manual
// trigger if not installed. The button can also be controlled
// programmatically via a ref: `feedbackRef.current.open()`.
//
// Aesthetic: very plain Modal + Text + TextInput so it adopts the
// host app's color scheme without forcing a sentori theme on it.

import React, {
  forwardRef,
  useCallback,
  useEffect,
  useImperativeHandle,
  useRef,
  useState,
} from 'react';
import {
  Modal,
  Pressable,
  StyleSheet,
  Text,
  TextInput,
  View,
} from 'react-native';

import { sendUserFeedback } from './capture';
import { isAnyNativeModuleLinked } from './native-loader';

type Trigger = 'fab' | 'manual' | 'shake';

export type FeedbackButtonHandle = {
  /** Open the feedback modal. Returns immediately. */
  open: (defaults?: { body?: string; eventId?: string; title?: string }) => void;
  /** Close the modal if open. */
  close: () => void;
};

export type FeedbackButtonProps = {
  /** When to surface the prompt. Default `'fab'`. */
  trigger?: Trigger;
  /** Pass the eventId from the last `captureException` to tie the
   *  report to that crash. Optional. */
  eventId?: string;
  /** Localized strings. Defaults are English. */
  labels?: {
    title?: string;
    bodyPlaceholder?: string;
    emailPlaceholder?: string;
    submit?: string;
    cancel?: string;
    sent?: string;
  };
  /** Shake sensitivity in m/s² above gravity. Default 18 (≈ a normal
   *  intentional shake; lower triggers more easily). Only used when
   *  `trigger="shake"`. */
  shakeThreshold?: number;
};

export const FeedbackButton = forwardRef<FeedbackButtonHandle, FeedbackButtonProps>(
  function FeedbackButton(props, ref) {
    const trigger: Trigger = props.trigger ?? 'fab';
    const [open, setOpen] = useState(false);
    const [title, setTitle] = useState('');
    const [body, setBody] = useState('');
    const [email, setEmail] = useState('');
    const [submitting, setSubmitting] = useState(false);
    const [sent, setSent] = useState(false);
    const eventIdRef = useRef<string | undefined>(props.eventId);

    useImperativeHandle(
      ref,
      () => ({
        open: (defaults) => {
          setTitle(defaults?.title ?? '');
          setBody(defaults?.body ?? '');
          eventIdRef.current = defaults?.eventId ?? props.eventId;
          setSent(false);
          setOpen(true);
        },
        close: () => setOpen(false),
      }),
      [props.eventId],
    );

    // Shake detection — opt-in. We load expo-sensors lazily so apps
    // that don't install it never pay the bundle cost. Same native
    // module guard as netinfo: if the JS package is in node_modules
    // but native bridge isn't linked we'd otherwise crash inside the
    // sensor emitter.
    useEffect(() => {
      if (trigger !== 'shake') return;
      if (!isAnyNativeModuleLinked(['ExponentAccelerometer', 'EXAccelerometer', 'ExpoAccelerometer'])) {
        return;
      }
      let sub: { remove: () => void } | null = null;
      try {
        // eslint-disable-next-line @typescript-eslint/no-require-imports
        const mod = require('expo-sensors') as {
          Accelerometer?: {
            addListener: (cb: (d: { x: number; y: number; z: number }) => void) => {
              remove: () => void;
            };
            setUpdateInterval: (ms: number) => void;
          };
        };
        const A = mod.Accelerometer;
        if (!A) return;
        A.setUpdateInterval(100);
        const thr = props.shakeThreshold ?? 18;
        let lastTriggerAt = 0;
        sub = A.addListener(({ x, y, z }) => {
          const mag = Math.sqrt(x * x + y * y + z * z) * 9.81; // g → m/s²
          const now = Date.now();
          if (mag > thr && now - lastTriggerAt > 1500) {
            lastTriggerAt = now;
            setSent(false);
            setOpen(true);
          }
        });
      } catch {
        // expo-sensors not installed → silently fall back to manual
      }
      return () => {
        if (sub) sub.remove();
      };
    }, [trigger, props.shakeThreshold]);

    const onSubmit = useCallback(async () => {
      if (!title.trim() || !body.trim()) return;
      setSubmitting(true);
      try {
        await sendUserFeedback({
          title: title.trim().slice(0, 200),
          body: body.trim().slice(0, 8000),
          email: email.trim() || undefined,
          eventId: eventIdRef.current,
        });
        setSent(true);
        setTimeout(() => setOpen(false), 1200);
      } finally {
        setSubmitting(false);
      }
    }, [title, body, email]);

    const L = {
      bodyPlaceholder: 'What happened?',
      cancel: 'Cancel',
      emailPlaceholder: 'email (optional)',
      sent: 'Thanks — report sent.',
      submit: 'Send',
      title: 'Report a problem',
      ...props.labels,
    };

    return (
      <>
        {trigger === 'fab' && (
          <Pressable
            accessibilityLabel="Open feedback"
            onPress={() => {
              setSent(false);
              setOpen(true);
            }}
            style={styles.fab}
          >
            <Text style={styles.fabText}>?</Text>
          </Pressable>
        )}
        <Modal animationType="fade" transparent visible={open} onRequestClose={() => setOpen(false)}>
          <View style={styles.backdrop}>
            <View style={styles.card}>
              {sent ? (
                <Text style={styles.sentMessage}>{L.sent}</Text>
              ) : (
                <>
                  <Text style={styles.heading}>{L.title}</Text>
                  <TextInput
                    autoCapitalize="sentences"
                    onChangeText={setTitle}
                    placeholder="Subject"
                    placeholderTextColor="#888"
                    style={styles.titleInput}
                    value={title}
                  />
                  <TextInput
                    multiline
                    onChangeText={setBody}
                    placeholder={L.bodyPlaceholder}
                    placeholderTextColor="#888"
                    style={styles.bodyInput}
                    textAlignVertical="top"
                    value={body}
                  />
                  <TextInput
                    autoCapitalize="none"
                    keyboardType="email-address"
                    onChangeText={setEmail}
                    placeholder={L.emailPlaceholder}
                    placeholderTextColor="#888"
                    style={styles.emailInput}
                    value={email}
                  />
                  <View style={styles.actions}>
                    <Pressable onPress={() => setOpen(false)} style={styles.cancelBtn}>
                      <Text style={styles.cancelText}>{L.cancel}</Text>
                    </Pressable>
                    <Pressable
                      disabled={submitting || !title.trim() || !body.trim()}
                      onPress={onSubmit}
                      style={[styles.submitBtn, submitting && styles.submitBtnDisabled]}
                    >
                      <Text style={styles.submitText}>
                        {submitting ? '…' : L.submit}
                      </Text>
                    </Pressable>
                  </View>
                </>
              )}
            </View>
          </View>
        </Modal>
      </>
    );
  },
);

const styles = StyleSheet.create({
  actions: {
    flexDirection: 'row',
    gap: 8,
    justifyContent: 'flex-end',
  },
  backdrop: {
    alignItems: 'center',
    backgroundColor: 'rgba(0,0,0,0.5)',
    flex: 1,
    justifyContent: 'center',
    paddingHorizontal: 24,
  },
  bodyInput: {
    backgroundColor: '#f7f7f8',
    borderColor: '#e5e5e8',
    borderRadius: 6,
    borderWidth: 1,
    color: '#111',
    fontSize: 14,
    marginBottom: 8,
    minHeight: 100,
    paddingHorizontal: 12,
    paddingVertical: 10,
  },
  cancelBtn: {
    paddingHorizontal: 14,
    paddingVertical: 8,
  },
  cancelText: { color: '#666', fontSize: 14 },
  card: {
    backgroundColor: '#fff',
    borderRadius: 10,
    padding: 16,
    width: '100%',
  },
  emailInput: {
    backgroundColor: '#f7f7f8',
    borderColor: '#e5e5e8',
    borderRadius: 6,
    borderWidth: 1,
    color: '#111',
    fontSize: 14,
    marginBottom: 14,
    paddingHorizontal: 12,
    paddingVertical: 8,
  },
  fab: {
    alignItems: 'center',
    backgroundColor: '#111',
    borderRadius: 24,
    bottom: 28,
    elevation: 4,
    height: 48,
    justifyContent: 'center',
    position: 'absolute',
    right: 18,
    shadowColor: '#000',
    shadowOffset: { height: 2, width: 0 },
    shadowOpacity: 0.18,
    shadowRadius: 4,
    width: 48,
  },
  fabText: { color: '#fff', fontSize: 22, fontWeight: '500' },
  heading: { color: '#111', fontSize: 18, fontWeight: '500', marginBottom: 12 },
  sentMessage: { color: '#111', fontSize: 14, paddingVertical: 18, textAlign: 'center' },
  submitBtn: {
    backgroundColor: '#111',
    borderRadius: 6,
    paddingHorizontal: 16,
    paddingVertical: 8,
  },
  submitBtnDisabled: { opacity: 0.5 },
  submitText: { color: '#fff', fontSize: 14, fontWeight: '500' },
  titleInput: {
    backgroundColor: '#f7f7f8',
    borderColor: '#e5e5e8',
    borderRadius: 6,
    borderWidth: 1,
    color: '#111',
    fontSize: 14,
    marginBottom: 8,
    paddingHorizontal: 12,
    paddingVertical: 8,
  },
});
