// rn-example — the eight-verb walkthrough app.
//
// One button per verb (plus the crash lanes). Point it at a local
// stack (`cargo run` + postgres) and every tap should land as an
// event, group into an issue, and show up in `GET /api/issues`.

import { useState } from 'react';
import {
  Platform,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  View,
} from 'react-native';

import { RageTapCapture, registerMaskQuery, sentori } from '@goliapkg/sentori-react-native';

const INGEST_URL =
  Platform.OS === 'android' ? 'http://10.0.2.2:8080' : 'http://localhost:8080';

// Paste an `ingest`-scope token minted from the local dashboard/API.
const TOKEN = 'st_dev0000000000000000000000';

registerMaskQuery(() => ['mask-me']);
sentori.init({
  token: TOKEN,
  ingestUrl: INGEST_URL,
  release: 'sentori-example@1.0.0+1',
  environment: 'dev',
  detect: { rageTap: true, longFreeze: true, slowColdStart: true, slowApi: true },
  replaySeconds: 30,
  replayScreens: true,
});
sentori.user({ id: 'demo-user-1', email: 'demo@example.com' });
sentori.context({ variant: 'walkthrough', flag_new_checkout: true });

type LogLine = { id: number; text: string };

export default function App() {
  const [log, setLog] = useState<LogLine[]>([]);

  const append = (text: string) => {
    setLog((prev) =>
      [{ id: Date.now() + Math.random(), text }, ...prev].slice(0, 12),
    );
  };

  const buttons: { onPress: () => void; title: string }[] = [
    {
      title: 'throw TypeError (auto capture)',
      onPress: () => {
        append('throwing TypeError…');
        setTimeout(() => {
          const x = undefined as unknown as { foo: () => void };
          x.foo();
        }, 0);
      },
    },
    {
      title: 'sentori.error(manual)',
      onPress: () => {
        const id = sentori.error(new Error('manual error from walkthrough'), {
          lane: 'manual',
        });
        append(`error → ${id.slice(0, 8)}`);
      },
    },
    {
      title: 'sentori.warn(pay.gateway-retry)',
      onPress: () => {
        const id = sentori.warn('pay.gateway-retry', {
          attempt: 2,
          error: new TypeError('gateway 503'),
        });
        append(`warn → ${id.slice(0, 8)}`);
      },
    },
    {
      title: 'sentori.trace(checkout.start)',
      onPress: () => {
        const id = sentori.trace('checkout.start', { cartSize: 3 });
        append(`trace → ${id.slice(0, 8)}`);
      },
    },
    {
      title: 'sentori.assert pass ×5',
      onPress: () => {
        for (let i = 0; i < 5; i++) sentori.assert('total-positive', true);
        append('assert ×5 pass (aggregates, no events)');
      },
    },
    {
      title: 'sentori.assert FAIL',
      onPress: () => {
        const id = sentori.assert('total-positive', false, { total: -1 });
        append(`assert fail → ${id.slice(0, 8)}`);
      },
    },
    {
      title: 'sentori.probe(SENT-1)',
      onPress: () => {
        const id = sentori.probe('SENT-1', { cardToken: null });
        append(`probe → ${id.slice(0, 8)}`);
      },
    },
    {
      title: 'block JS thread 2.5 s (long_freeze)',
      onPress: () => {
        append('blocking…');
        setTimeout(() => {
          const until = Date.now() + 2_500;
          while (Date.now() < until) {
            // spin
          }
          append('unblocked — long_freeze should fire');
        }, 50);
      },
    },
  ];

  return (
    <RageTapCapture style={styles.root}>
      <View style={styles.container}>
        <Text style={styles.title}>Sentori · eight verbs</Text>
        <Text style={styles.subtitle}>{INGEST_URL}</Text>
        {/* masked in every captured frame — the black box in the
            replay proves masking end to end */}
        <Text nativeID="mask-me" testID="mask-me" style={styles.subtitle}>
          SECRET user@example.com · card 4242
        </Text>
        <ScrollView style={styles.buttons}>
          {buttons.map((b) => (
            <Pressable key={b.title} onPress={b.onPress} style={styles.button}>
              <Text style={styles.buttonText}>{b.title}</Text>
            </Pressable>
          ))}
        </ScrollView>
        <View style={styles.log}>
          {log.map((l) => (
            <Text key={l.id} style={styles.logLine}>
              {l.text}
            </Text>
          ))}
        </View>
      </View>
    </RageTapCapture>
  );
}

const styles = StyleSheet.create({
  root: { flex: 1 },
  container: { flex: 1, backgroundColor: '#101014', paddingTop: 64 },
  title: {
    color: '#f5f5f7',
    fontSize: 22,
    fontWeight: '700',
    paddingHorizontal: 20,
  },
  subtitle: { color: '#8e8e93', fontSize: 12, paddingHorizontal: 20, paddingBottom: 12 },
  buttons: { flexGrow: 0 },
  button: {
    backgroundColor: '#1c1c22',
    borderRadius: 10,
    marginHorizontal: 16,
    marginVertical: 4,
    padding: 14,
  },
  buttonText: { color: '#e5e5ea', fontSize: 15 },
  log: { flex: 1, padding: 16 },
  logLine: { color: '#30d158', fontFamily: 'Menlo', fontSize: 11, paddingVertical: 1 },
});
