import type { MessageKey } from '../i18n/en';
type FieldSpec = {
  hint: MessageKey;
  key: string;
  label: MessageKey;
  multiline?: boolean;
  options?: { label: MessageKey; value: string }[];
  placeholder?: MessageKey;
  required?: boolean;
};

export const PROVIDER_FIELDS: Record<string, FieldSpec[]> = {
  apns: [
    {
      key: 'keyId',
      label: 'push.field.keyId',
      hint: 'push.hint.apns.keyId',
      placeholder: 'push.placeholder.apns.keyId',
      required: true,
    },
    {
      key: 'teamId',
      label: 'push.field.teamId',
      hint: 'push.hint.apns.teamId',
      placeholder: 'push.placeholder.apns.teamId',
      required: true,
    },
    {
      // Sent as `topic`, because that is the name the worker reads.
      // The label says bundle id, which is what Apple calls it — and
      // the old placeholder said `bundleId`, which is what nothing
      // reads.
      key: 'topic',
      label: 'push.field.topic',
      hint: 'push.hint.apns.topic',
      placeholder: 'push.placeholder.apns.topic',
      required: true,
    },
    {
      key: 'production',
      label: 'push.field.production',
      hint: 'push.hint.apns.production',
      options: [
        { label: 'push.option.production', value: 'production' },
        { label: 'push.option.sandbox', value: 'sandbox' },
      ],
    },
    {
      key: 'secretKey',
      label: 'push.field.secretKey',
      hint: 'push.hint.apns.secretKey',
      placeholder: 'push.placeholder.apns.secretKey',
      multiline: true,
      required: true,
    },
  ],
  fcm: [
    {
      // Everything else is read out of the file itself.
      key: 'secretKey',
      label: 'push.field.secretKey',
      hint: 'push.hint.fcm.secretKey',
      placeholder: 'push.placeholder.fcm.secretKey',
      multiline: true,
      required: true,
    },
  ],
  webpush: [
    {
      key: 'subject',
      label: 'push.field.subject',
      hint: 'push.hint.webpush.subject',
      placeholder: 'push.placeholder.webpush.subject',
    },
    {
      key: 'vapidPublicKey',
      label: 'push.field.vapidPublicKey',
      hint: 'push.hint.webpush.vapidPublicKey',
      placeholder: 'push.placeholder.webpush.vapidPublicKey',
      required: true,
    },
    {
      key: 'secretKey',
      label: 'push.field.secretKey',
      hint: 'push.hint.webpush.secretKey',
      placeholder: 'push.placeholder.webpush.secretKey',
      multiline: true,
      required: true,
    },
  ],
};
