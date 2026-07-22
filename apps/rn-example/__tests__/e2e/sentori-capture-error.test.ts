/**
 * 极简 sentori e2e 范本 — 拷过去改 BUNDLE / DEEPLINK / TAP_TEXT / RESULT_TEXT
 * 即用。
 *
 * 运行：
 *   simx run __tests__/e2e/sentori-capture-error.test.ts \
 *     --udid <sim-sentori-udid> --json
 *
 * `simx run` 默认走 `--runner=auto`：检 `--port`（默认 22087）是否有 listening
 * → 有则 attach，否则自启 xcodebuild runner test_runForever 并等 /health 200；
 * 跑完按 RunnerSession[Symbol.asyncDispose] graceful stop。**作者无须手动启
 * runner**，一行 simx run 即可。
 *
 * 期望：单行 JSON 末行 `.passed==1 && .failed==0`，exit code 0。
 *
 * 形态要点（sentori 测试作者拷过去保留，简化 / 删 / 重命名都可）：
 *   - 仅 import simx 公共 entry（`'simx'`），零 simx 内部 `src/...` 偷依
 *   - 仅用 §9.3 语义 selector（`{ text }` / `{ id }` / `{ role, name }`），
 *     绝不用 x/y/xpath/coordinate
 *   - launch + deeplink 一步走 = `app.launch(bundleId, { url })`（v1.4 ③-C3
 *     新能力；等价于 `simctl launch <bundle>` 后 `simctl openurl <url>`）
 *   - 系统弹窗（SpringBoard "在 X 中打开" / RN dev-menu / Downloading…）
 *     用 `app.system.drainPopups()` 一行处置；默认 locale-invariant decide
 *     policy 只读 role + outcomeHint + SF Symbol id（零 locale 文案硬编码），
 *     必要时传 `{ customDecide }` 覆盖
 *   - RN dev-mode 主 content tap 推荐显式 `{ via: 'runner' }` escape hatch：
 *     走 runner 内 firstSeeThroughMatch + XCUIElement atomic .tap()，规避
 *     SDK 默认 host-HID 在 dev-mode 下偶发的 race / setState 不 propagate
 *     （v1.1 C3 default 仍是 host-HID 不变；这里只是 quickstart 显式选 path）
 */
import { test, expect } from 'simx'

test('sentori capture-error: deeplink + manual button + assert visible', async ({ app }) => {
  // 4 常量字面 — 改这 4 行即可换到你自己的 sentori app 场景。
  // BUNDLE / DEEPLINK / TAP_TEXT / RESULT_TEXT 字面与 ③-C2 e2e 同款
  // （v1.4 stage ③ 真证已封；见 docs/plan-history/v1.4-stage3-c2-hot.md）。
  const BUNDLE = 'com.goliapanda.sentori-example'
  const METRO_URL = 'http://127.0.0.1:9090'
  // Metro `/message` 是 WebSocket 端点 (RCT_PACKAGER_CLIENT_PROTOCOL_VERSION
  // 2); SDK reloadViaMetro 自动取 ws:// 副本.
  const METRO_WS = 'ws://127.0.0.1:9090'
  const DEEPLINK = `com.goliapanda.sentori-example://expo-development-client/?url=${METRO_URL}`
  const TAP_TEXT = 'Manual sentori.captureException()'
  const RESULT_TEXT = 'captureException manual…'

  // 1) 启动 sentori app + 经 deeplink 引 expo-dev-client 加载 Metro bundle 一步走。
  //    sentori dev 流约定 Metro 端口 9090（见 sentori apps/rn-example/metro.config.js）。
  await app.launch(BUNDLE, { url: DEEPLINK })

  // 2) **先** Metro `/message` reload — RN dev-mode 下 deeplink 偶发 expo-dev-
  //    menu native overlay 滞留 (其 30+ button native subtree 一次性 /tree
  //    列举 ~120s; drainPopups 若先碰它会超时). reload 触发 expo-dev-menu
  //    DevMenuPackagerConnectionHandler.swift case "reload" → reloadAppAsync
  //    = 全量 RN 重载, dev-menu dismiss, RN content 稳态. version:2 在 SDK
  //    内固定写死. 等价于 c1/c2 e2e shell helper 的 broadcast_reload().
  await app.system.reloadViaMetro(METRO_WS)

  // 3) 处置系统弹窗 + RN dev-menu 覆盖层. include='all-windows' 让 drainPopups
  //    看到 bound-app 非主 window (dev-menu); swift /system-popups 内 button +
  //    staticText 列举有 consumeMaxButtons=40 / 5s budget cap, 重 native overlay
  //    (~12 button) 不再卡 ~120s. 默认 decide policy:
  //    - SpringBoard alert outcomeHint='scheme-confirm-affirm' ⇒ tap affirmative
  //    - non-SpringBoard window 内 SF Symbol id='xmark' ⇒ tap dismiss (locale-
  //      invariant; RN expo-dev-menu close button 即 xmark identifier)
  //    dev-menu dismiss 后 RN content 可视, 后续 tap 真触发 onPress (iOS hit-
  //    testing 不再被 dev-menu 截走).
  await app.system.drainPopups({ include: 'all-windows' })

  // 4) 等 RN 主屏渲染出按钮 (reloadViaMetro 触发的 RN 重载在 sim 上耗
  //    10-25s settle, 超 default 5s timeout). 30s 给足 reload + RN
  //    re-render 余量. reloadViaMetro 已 dismiss dev-menu, RN content 直接
  //    在主 window — 走默认 (无 see-through) 路径 = XCUIElement.tap()
  //    真触发 RN Pressable onPress (与 c1 e2e Phase 9.3 同款决策).
  await app.waitFor({ text: TAP_TEXT }, { timeoutMs: 30_000 })
  await app.tap({ text: TAP_TEXT }, { via: 'runner' })

  // 5) 断言 captureException 真触发: append('captureException manual…') 出现在屏上.
  await expect(app.element({ text: RESULT_TEXT })).toBeVisible()
})
