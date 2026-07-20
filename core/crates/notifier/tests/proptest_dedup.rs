//! Property tests for `dedup_key` handling.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use proptest::prelude::*;
use sentori_notifier::{Channel, Notification};
use sentori_workspace_identity::WorkspaceId;

proptest! {
    #[test]
    fn builder_helpers_idempotent(
        recipient in "[a-z]{3,12}@[a-z]{3,8}\\.[a-z]{2,4}",
        subject in "[a-zA-Z ]{1,40}",
        body in "[a-zA-Z \\n]{1,200}",
        dedup in "[a-z0-9_]{4,32}",
    ) {
        let n = Notification::new(WorkspaceId::new(), Channel::Email, &recipient, &subject, &body)
            .with_dedup_key(&dedup);
        prop_assert_eq!(n.recipient, recipient);
        prop_assert_eq!(n.subject, subject);
        prop_assert_eq!(n.body, body);
        prop_assert_eq!(n.dedup_key, Some(dedup));
    }

    #[test]
    fn channel_round_trip(
        chan in prop_oneof![
            Just(Channel::Email),
            Just(Channel::Webhook),
            Just(Channel::Mock),
        ],
    ) {
        let n = Notification::new(WorkspaceId::new(), chan, "x@y", "s", "b");
        prop_assert_eq!(n.channel, chan);
    }
}
