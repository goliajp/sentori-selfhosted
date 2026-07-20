// v2.0 W3 — `feedback` subpath. The feedback widget pulls in a
// component tree (react + tsx) and an internal viewer route, so
// importing it eagerly from the SDK's top-level barrel pays a
// bundle cost every consumer pays — even ones that never render
// the widget.
//
// Subpath import `@goliapkg/sentori-react-native/feedback`
// resolves to just the widget surface. Hosts that don't render
// the feedback button pay zero bundle delta; hosts that do reach
// for it import via this module and get exactly what they need.
//
// Top-level (`index.ts`) still re-exports `FeedbackButton` for one
// more release cycle so v1 callsites don't break on upgrade. v3
// will drop the top-level re-export — see
// `docs/recipes/v1-to-v2-migration.md` for the deprecation timeline.

export {
  FeedbackButton,
  type FeedbackButtonHandle,
  type FeedbackButtonProps,
} from './feedback-widget'
