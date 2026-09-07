# sentori quickstart — 用 simx 跑 sentori 的 iOS GUI e2e

这是一份 **可拷可改** 的最小套件，目标读者是 sentori 团队的测试作者。
拷到 `apps/rn-example/__tests__/e2e/` 下（RN/Jest 习惯目录，monorepo
内 RN app workspace），改 4 个常量，就能用 `simx run` 一行命令
驱动 sentori 真 app + tap + assert + 干净 teardown。

不需要懂 simx 仓内部架构，不需要懂 XCUITest，不需要写 cliclick / osascript /
useEffect 自动按钮——这些 simx 全包了。

## Install

在 sentori monorepo 顶层装 simx（dev dependency，bun workspaces 会
hoist 到顶层 `node_modules/`，`apps/rn-example/` 可直接拿到）：

```bash
bun add -d simx
```

或者把 simx 仓 clone 到本机后做 file: link（推荐 sentori 仓 dev 阶段，
不必发包到 registry 也能用）：

```bash
# sentori monorepo 顶层视角；假设 simx 仓与 sentori 仓同级
bun add -d file:../simx
```

装完验环境：

```bash
bunx simx doctor       # 探 Xcode / iOS runtime / claude / bun
bunx simx list         # 列可用模拟器（看 sim-sentori 是否 Booted）
```

`simx doctor` 应该报 Xcode 26+ / iOS 26.x runtime / claude CLI ok。如果某项
缺，按它的提示装。

## Setup once

simx 把它管的 sim 设备登记在 `.simx/sims.json`（gitignored）。第一次跑前
注册 sim-sentori（或你本机 booted 的 iOS 模拟器）：

```bash
bunx simx sim add --udid <sim-sentori-udid>
```

`<sim-sentori-udid>` 替成你本机 `xcrun simctl list devices booted` 找到的
sim-sentori UDID。这是**一次性**前置——`simx run` 经 `acquireCell` 校验
sim 必在 registry 内（v1.3 引入的成员检查），未注册的 sim 拒绝驱动以避免
误碰用户日常设备。

如果不想污染仓内 `.simx/sims.json`（CI / 临时跑），可经 env 隔离：

```bash
export SIMX_REGISTRY_ROOT=$(mktemp -d)
bunx simx sim add --udid <sim-sentori-udid>
# 接下来所有 simx 命令读这个 scratch registry，不动仓内的
```

## Run locally

确保 sentori 已经按 `apps/rn-example/` 的 README 把 Metro 跑起来
（默认端口 9090），且 sim-sentori 模拟器 Booted、sentori app 已 install。
然后 cwd 切到 RN app workspace 内跑：

```bash
cd apps/rn-example
bunx simx run __tests__/e2e/sentori-capture-error.test.ts \
  --udid <sim-sentori-udid> \
  --runner=auto \
  --target-bundle com.goliapanda.sentori-example \
  --json
```

期望（机器可判）：
- exit code = 0
- stdout 末行是单行 JSON：`{"exitCode":0,"passed":1,"failed":0,...}`

`<sim-sentori-udid>` 替成你 Setup once 段注册的同一个 UDID。

参数含义：

| flag | 含义 | 默认 |
|---|---|---|
| `--udid` | 目标 sim UDID | 必填 |
| `--runner=auto` | runner 编排模式：`auto` 探 22087 port → 在则 attach、不在则 spawn xcodebuild runner 自动起；`attach` 仅 attach、`spawn` 强制 spawn 不探 | `auto`（推荐） |
| `--port` | runner HTTP 端口 | `22087` |
| `--target-bundle` | XCUITest runner 绑的目标 app bundle id；不传则绑 `com.apple.Preferences`（默认），sentori app 必须显式指定 | `com.apple.Preferences` |
| `--json` | 输出单行 JSON 给 CI / agent 解析；不带则人类可读 | off |

把 `--json` 去掉就出人类可读输出；带 `--json` 是给 CI / agent 解析用。

## CI

`bun run e2e:simx` 在任何 CI 上调即可——没有 simx-specific yaml /
plugin / runner action。把 `package-json-snippet.json` 的 `scripts.e2e:simx`
合并进 `apps/rn-example/package.json`（RN app workspace），CI job 调它
就够了。

最小前置（任何 CI 框架都通用，写法按你们的 CI 风格）：

```bash
# 1. boot 一个 iOS 模拟器
xcrun simctl boot <udid> && xcrun simctl bootstatus <udid> -b

# 2. 一次性注册（用 RUNNER_TEMP / scratch 目录避免污染 checkout）
export SIMX_REGISTRY_ROOT="$RUNNER_TEMP/simx-registry"  # 或 mktemp -d
bunx simx sim add --udid <udid>

# 3. 起 Metro + install sentori app（走 sentori 既有 build 链路）
( cd apps/rn-example && bun start & )
( cd apps/rn-example && bun run ios --no-launch )

# 4. 跑测试（在 RN app workspace 内）
( cd apps/rn-example && bun run e2e:simx )
```

simx 不替你选 CI 框架（GH Actions / CircleCI / self-hosted Jenkins
都 OK）也不替你决定何时开真触发——`bun run e2e:simx` 是你跟 simx 唯一
的 CI 接面。

## 玻璃瓶旁证读法

`simx run --json` 末行 JSON 除了 `.passed` / `.failed`，simx stage ③ 还
透传了一组 **玻璃瓶旁证** 字段，含义：

| 字段 | 含义 | 健康值 |
|---|---|---|
| `isolated_build` | sentori app 是否经隔离副本可复现 build 出 | `"ok"` |
| `sim_matrix` | 多 runtime / locale / appearance 矩阵闸是否同绿 | `"ok"` |
| `driver_ready` | rn-expo driver 把 expo-dev-client 驱到 content-ready | `true` |
| `deeplink` | deeplink 路径是否成功打开 Metro bundle | `"ok"` |
| `tree_sentori_id` | a11y 树根 .identifier | `"com.goliapanda.sentori-example"` |
| `tree_sentori` | a11y 树节点数（合理值 ~200） | 数字 |
| `assert_visible` | 你写的 `expect(...).toBeVisible()` 真过 | `true` |
| `teardown` | 收尾路径是 graceful 还是被 SIGKILL | `"graceful"` |
| `ips_before` / `ips_after` | 跑前 / 跑后 `~/Library/Logs/DiagnosticReports/*.ips` 数 | 应**相等** |
| `sim_never_frontmost` | 整个 e2e 期间 sim 窗口未抢焦点（用户可继续打字） | `true` |
| `insight_alive` | sim-insight 模拟器（用户常驻）在 e2e 期间一直 Booted | `true` |
| `user_8081_alive` | 用户:8081 上的服务（Metro / dev server）在 e2e 期间不被打扰 | `true` |
| `sentori_tracked_clean` | sentori 仓 tracked 文件在 e2e 期间字面不变 | `true` |
| `sim_sentori_restored` | e2e 结束时 sim-sentori 恢复到 e2e 前 Booted 状态 | `true` |
| `popups_sensed` | 旅程中感知到的系统弹窗数 | 数字（通常 ≥1） |
| `popup_decided` | 弹窗决策结果（仅当 `popup_tap=="ok"` 时存在） | `"affirmative-by-hint"` 等 |
| `popup_tap` | 弹窗按钮 tap 结果 | `"ok"` 或 `"no-popup"` |

> **`true` / 数字相等 = 玻璃瓶 invariance 持守**——你跑这条 e2e 没影响
> 用户的另一个 sim、没改你不知道的文件、没让系统崩溃报告框抢焦点。
>
> **任何字段红** → 抓 simx 团队回 issue（带上 `simx run --json` 末行原文 +
> simx 仓 commit hash），不是你写测试写错的事。

---

简而言之：拷过去，改 4 个常量，跑 `simx run`，看 `.passed==1`。
不要在测试里写 sim 启动 / app 安装 / deeplink 触发 / cliclick 自动 tap
——这些 simx 全已封过。你只管写 **业务断言**。
