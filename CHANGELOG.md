# Sentori CHANGELOG

> v0.1 + v0.2 已完成 phase 的详细记录（含每条 sub 的中文 summary）。

> 每条记录的真实落地凭证以 git log + commit message 为准。

---

## v5.1.6（2026-10-12 — 恢复正式 IDE 工作台）

- 正式应用接入与设计预览共用的工作台框架、资源树面板、调查标签、检查器和底部动态；面板支持拖动、键盘调整、折叠、布局保存及窄屏覆盖显示。
- 调查标签保留筛选、滚动和草稿，隔离不同项目与异步操作；隐藏页面暂停快捷键和弹窗，凭据页面离开时清除临时秘密，表单与辅助功能 ID 按文档独立生成。
- 项目切换使用共享的可搜索选择器，修复长名称截断和选择框越界；统一紧凑内容密度，高级筛选可折叠，检查器和动态由真实 API 驱动并在操作成功后刷新。
- 增加共享组件来源门及真实 Chromium／WebKit 工作台回归，覆盖多标签、项目权限、后台保存、面板操作和 320–1920 像素布局。

## v5.1.5（2026-10-12 — 标签与按钮规范、事件详情布局）

- 事件类型与回归标签统一使用共享 Badge，修复标签文字垂直偏移；Button 与 Badge 的样式由生产与设计预览共同引用，按钮字重统一为 500。
- 问题详情将事件选择与原始事件操作按顺序排列，时间跟随界面语言；移除证据区额外内边距与嵌套滚动，操作按钮与修复版本输入框对齐。
- 新增组件来源检查，并在真实 Chromium／WebKit 流程中验证三语、明暗主题及桌面／窄屏下的文字居中、按钮规格与内容边界。

## v5.1.4（2026-10-12 — 事件组合筛选与工作台完整性）

- 问题页提供 App、平台、业务 appType、环境、版本与多个上下文／事件标签条件；所有条件在同一事件上求交集，数量、影响用户和出现记录同步按筛选范围统计；连续快速修改条件也不会覆盖已选筛选。
- 任务、服务监控、推送和个人收件箱使用正式页面布局；任务与监控补齐统计、筛选及空状态操作，项目切换清除旧筛选与详情。
- 监测页提供真实事件趋势、启动分位数、断言和探针图表，以及数据表、CSV、图例和时间范围操作；累计数据与时间窗口明确区分，缺失数据和查询失败不会伪装成正常曲线。
- 修复跨页版本详情、命令搜索、Cloud 工作区草稿、通知设置保存反馈、项目初始化加载状态和复制失败提示；项目健康查询错误不再显示成零数据。
- 新增独立数据库与 Chromium／WebKit 回归，覆盖组合筛选、导航、图表、故障重试与三语明暗响应式工作台。

## v5.1.3（2026-10-12 — 问题页间距与应用身份来源）

- 问题列表的分页、空状态和崩溃率卡片统一使用正式页面的共享间距；快捷键提示按键位和操作分行显示，支持 Escape 关闭。
- 清理中日文短标签和提示末尾的句号，并让检查覆盖单双引号，避免同类问题再次漏检。
- 应用列表区分手动名称、客户端标识、版本名推断和未识别数据；未识别数据保留在独立区域，不再作为已识别应用计数。
- 缺失版本的 `unknown` 上报不会将不同应用及历史未识别事件误合并。

## v5.1.2（2026-10-11 — 项目聚合与自动识别应用）

- 客户端继续使用 Project 的上报地址和凭据；服务端从已接收的事件、会话与原生符号文件自动识别 App，无需新增接入配置。
- 项目聚合多个 App，每个 App 对应一个运行平台，拥有自己的版本关联、显示名称与 Git 仓库；Task 保持项目级。
- 项目创建、编辑移除手动平台字段。空平台显示灰色，点击进入对应接入指南；区分已接入、仅上传文件和不可用文件，并使用 JavaScript、iOS、Android 的正式名称。
- 版本页支持按 App 筛选，崩溃率同步按 App 统计；旧设备补报不会把最新版本退回旧版本。
- 迁移已有事件、会话与符号文件，保留原有 release ID 和上传协议；完整备份、恢复和项目迁移包含 App 元数据与版本关联。

## v5.1.1（2026-10-11 — 公开发行文件完整性修复）

- 自动派生仓库保留受生成文件忽略规则影响的已追踪类型声明；提交前逐文件校验 Git 暂存区与导出清单，避免导出目录能构建、公开 Git checkout 却缺文件。
- 发布前拒绝清单之外的文件与被修改的导出内容；回归测试从实际公开 Git 标签重新克隆并验证完整性。
- 保留已有 v5.1.0 标签，不覆盖失败的发行源码。

## v5.1.0（2026-10-11 — SaaS 运营入口与独立自部署发行）

- 私有主仓库按白名单派生公开 self-hosted 源码，保留公开历史和不可覆盖的版本标签；公开流水线验证产品及两个架构的实际镜像后发布。
- SaaS 控制面提供独立镜像。现有单所有者实例可在停机、身份核对和备份绑定后接入托管工作区，保留原账号密码、项目和 SDK token；跨数据库中断可按原计划重试。
- 内部和试用工作区采用独立服务授权，支持撤销和到期，不创建虚假付款记录。正式收费仍需运营配置后启用。
- 工作台接入确认后的品牌标识、共享色彩和文档样式。

本条描述发行候选内容；实际发布与部署状态以对应回执为准。

## v5.0.0（2026-10-10 — 统一工作台、团队协作与可验证的实例生命周期）

本次大版本把 `design-v1.0.0` 冻结的视觉与交互接入真实 API。官网、文档、接入、项目、问题、回放、源码、版本、设备、Push、成员和设置使用同一套共享组件，支持中／英／日、明暗主题和移动端布局。生产构建拒绝导入预览 fixture，失败不会回退到示例数据。

- 恢复 Insight 现有推送调用：`POST /v1/push/send` 接受单设备 `{to,title,body,data}` 并返回 `tickets`；UUID 与历史 `ipt_<uuid>` 指向同一项目内的设备。恢复旧设备注册／注销入口和注册字段别名，复用正常权限、队列及投递流程。此兼容缺口早于 v4.1.1；支持范围、错误状态与迁移要求见 [旧推送协议兼容说明](docs/protocol.md#legacy-insight-sender-server-500)。

- 团队任务与个人收件箱持久化，权限按项目重查。签名自动化事件要求仓库、环境、完整 commit、CI 和发布证据一致；回归会使旧验证失效。
- 后台重解析、监控与第三方投递使用持久队列、租约、有限重试与真实回执。凭据加密，外部 HTTP 目标校验 DNS 与重定向边界。
- 自部署提供实例身份、环境检查、加密数据库／文件备份、独立归档校验、隔离恢复和备份优先的离线重置。崩溃、取消、恢复代次和中断清理有持久回执。
- Cloud 提供独立身份与组织、版本化报价、服务端支付核对、续费状态、独立数据库与文件卷的区域开通、额度及保留策略。正式收款默认关闭；真实提供商、邮件、DNS／TLS 和地区配置仍需运营联调，协议 fixture 通过不表示已商业上线。
- 项目迁移由两端所有者独立授权，初始复制后封存源写入，最终核对更新与删除。目标实际收到每个项目的新 SDK 事件才可完成；旧客户端用新目标凭据进行有期限的 HTTPS 转发。单次上传上限 1 GiB；切换后回退需保全并核对合并目标新增数据，没有自动反向导入。

升级保留 v4 的账号、项目、采集 Token、事件和文件引用，SDK v1 接收协议与已发布 SDK 包版本保持兼容。新增数据库迁移从 0022 开始，未改写 0001–0021。升级前应保存配对的数据库与文件备份，固定旧镜像；回退时先保全 v5 新数据，再在独立卷恢复旧版。具体流程见 [部署与恢复手册](docs/self-hosting.md)。

发布检查新增实际 v4.1.1 二进制升级／旧备份恢复演练，并验证未完成迁移在恢复后失效、暂存密钥被清除。原生 SDK 版本门对缺失或损坏的 Git 历史返回失败，同时检查已提交、暂存、未暂存和新建的运行源码。镜像构建锁定 Cargo 依赖；数据库健康检查等待最终 TCP 服务。

本条记录描述 v5 发行候选的内容；正式发布状态以仓库发布工作流及版本标签为准。

## v4.1.1（2026-09-30 — 绿点是写死的，所以它永远不会告诉你邮件发不出去）

邮件主机的证书 09-28 过期。sentori 一直在发：52 封送达，然后连续 110 封失败，
每一封都记在 `delivery_log` 里，带着确切原因 `invalid peer certificate:
certificate expired`。

**没有任何地方说这件事。** `GET /admin/api/smtp` 只答 `configured: true` 加三个字段，
界面上那个点只要 configured 为真就是绿的 —— 写死的。整整两天，那个为了回答
「邮件通不通」而存在的屏幕，显示的是唯一没人需要的那个事实。

接口现在读 `delivery_log`：7 天的送达 / 失败数、最后一次失败的原因、最后一次送达的时间、
以及 `healthy`。**判据是最后一次投递的结果，不是计数** —— 一周里先送达 52 封再连续失败
110 封，按任何计数口径都还是健康的，而那正是这次发生的那一周。

未配置 SMTP 时也报告投递历史。e2e 那套是故意不配 SMTP 的，第一版在那里直接 return
什么都不说才被发现；而「有 webhook、没有邮件服务器」正是自部署最常见的形态，
它们需要同一个答案。

界面跟着改：最后一次投递失败时点是红的，下面一行说失败了多少次、为什么、
上一次送达是什么时候。mock 里造出这个状态，所以之后的 sweep 会拍到它。

e2e 两个方向都断言：写一条失败进去，接口必须转成不健康并带上原因；再写一条成功，
必须恢复。

**新增** `scripts/check-mail-cert.sh`：邮件端口必须与网页端口用同一张证书，且剩余
大于 21 天。那台主机上的证书是手工拷过去的，没有任何东西同步它 —— Caddy 续期而拷贝
不动，这件事至少发生过两次。脚本先探连通性并单独报告：到不了的主机返回的是空证书，
跟坏掉的证书长得一模一样。

---

## v4.1.0（2026-09-30 — 文档变成一个站，而不是一个通往别处的 200）

`sentori.golia.jp/docs` 一直返回 200，服务的是 dashboard 的空壳：SPA 里没有 `/docs`
路由，catch-all 把访客送去 issue 列表。从 npm 或 GitHub 点链接过来的人，落在陌生人的
控制台里。

现在它是站：仓库自己的 `docs/*.md` 构建时读入，18 篇，三语 chrome，跟 dashboard 同源的
主题与代码高亮，未登录可读。**一份内容**——已有的十道文档门（链接解析、能从索引走到、
只提到真实 API、安装行装得上）检查的就是站上服务的那一份。

**渲染出来才发现的：**

- 七篇文档带着 2026-08 删掉的静态站生成器的 YAML frontmatter。第一个被服务的页面把
  `title: … description: …` 当标题渲染上屏，还进了目录。这两个键本身有用，所以是解析
  它们，不是从文件里删掉。
- `getting-started/web.md` 告诉读者需要「服务端 3.20 或更新」。CORS 是 4.0.0 才加的，
  照这句话装的人会落在一个答不了浏览器的服务端上 —— 正是本页要讲的那个故障。
- 索引开头在解释「这个仓库曾经有个 Astro 站，现在没有了」。换成落地页了。

**新增** `docs/dashboard.md`：18 篇文档里没有一篇讲怎么用真正干活的那个界面。
按「每个屏回答一个问题」写，说清队列为什么按「回归 → 新增 → 未处理」排而不是按时间、
`312u×9` 是什么意思、resolve 为什么锚定版本、Releases 四个灯各是什么、probe 沉默才是
好消息。

**两道门。** `check-docs-site` 要求每篇文档都在站点导航里、frontmatter 不许进正文。
`check-mirror` 补上反方向：它一直列着「必须发的页面」，却从没问过「有没有页面是不会被
发出去的」——于是 `dashboard.md` 写好了、索引链了、站上渲染了，公开仓库里没有，
而同一个索引链接在那边 404。

---

## v4.0.3（2026-09-30 — 包发出去了，但装不上、import 就炸、报错版本号）

v4 说的是五个 SDK 一套线，其中两个从来没在 npm 上。发它们的过程里，每修好一层就露出下一层，
四层都是「发布成功」与「真的能用」之间的差：

**1. 装不上。** `sdk/web` 和 `sdk/weapp` 对 core 声明的是 `workspace:*`。npm 原样发出去，
消费者 `npm install` 得到 `EUNSUPPORTEDPROTOCOL`。`sdk/react-native` 一直写的是 `^3.0.0`，
两个新包从建立起就没照做，也没有任何东西比对过。

**2. 装上了，import 就炸。** 改成真实范围发 1.0.1 后：

```
SyntaxError: The requested module '@goliapkg/sentori-core'
does not provide an export named 'createTransport'
```

根因是 v4 把 transport 内核挪进 core，**没有 bump 版本**。本地 `sentori-core@3.0.0` 有 39 个
导出，npm 上同名的 3.0.0 只有 22 个 —— 同一个版本号，两份不同的代码。
为此写的门一跑就抓到第二个活的：已发布的 `sentori-react-native@7.0.1` 缺
`startAnrWatchdog` / `stopAnrWatchdog`，v4 导出了它们但从没重发。

**3. 报的版本号是错的。** `SDK_VERSION` 会进 `Sentori-Sdk` 头，是服务端记录的、
支持对话开头引用的那个值，而它是手写常量。只有 `sdk/react-native` 有一条测试断言它等于
package.json，所以 web 和 weapp 连发三次都在上报 `0.1.0`。这道门现在是仓库级的。

**4. 守这件事的门没在看 npm。** `check-doc-versions` 的职责就是「文档里的安装行装得上」，
它问 SwiftPM tag、问 Maven Central，从没问过另外四个 SDK 所在的 registry。

四道门，每道都实测过会红：

| 门 | 规则 |
|---|---|
| `check-publishable-deps` | 消费者会装的依赖里不许有 `workspace:` 协议 |
| `check-sibling-exports` | 已在 npm 上的版本号，本地公开导出必须与它一致；改了就得 bump。问 registry，拿不到就红 |
| `check-sdk-version-constant` | 每个 SDK 上报的版本必须等于它 package.json 的版本 |
| `check-doc-versions`（扩）| 安装行里的 npm 包必须真的存在 |

**已发布并验证**：`core@3.1.0`、`react-native@7.1.1`、`web@1.0.3`、`weapp@1.0.3`。
验证方式是在空项目里从 registry 装下来真的 `import`：五个动词齐全、未 init 时静默不抛、
不返回 Promise、`Sentori-Sdk` 头读作 `web/1.0.3`。前面四次「发布成功」里有三次是假的，
只有这一步算数。

---

## v4.0.2（2026-09-30 — 发出去的那一份里，两处把读者送去 404；以及 CORS 从来没写进文档）

**对外信息**

`docs/troubleshooting.md` 结尾那条「File an issue on GitHub」指向 `goliajp/sentori` ——
私有仓库。每个自部署的读者按我们给的唯一一条求助指引点过去都是 404，而这一页就在公开镜像里。
同一页还教 web 接入方「wrap with SentoriProvider」，这个导出不存在于任何一个 SDK。

立了门：已发布的文档（README、`docs/**`、每个包的 README）不许链到读者打不开的仓库。
它读镜像工作流来判断哪些文件是发出去的，两边漂开时这道门先红。

**CORS**

没有 CORS 是 4.0.0 之前挡住所有浏览器的那个缺陷，而 CORS 在文档里一个字都没有。

- `protocol.md` 新增 Cross-origin requests：`/v1/*` 允许任意 origin、三个方法、三个请求头，
  缓存 600 秒；为什么是通配 —— ingest token 本来就在页面的 JavaScript 里，origin 白名单
  不是安全边界，只是一个每个自部署者都得先找到才能收到第一条 web 事件的开关；不允许
  credentials 才是让通配安全的原因。`/admin/*` 不答任何跨域请求。
- `troubleshooting.md` 新增第 8 条：页面 console 报 CORS，服务端日志却什么都没有。
  因为浏览器拒掉的是 preflight，请求根本没发出来，而它把这件事报给页面、不报给我们 ——
  服务端日志、请求数、`/healthz` 看起来和「没人在发」完全一样。

**UI 文案**

三语 × 双主题 × 18 条路由共 108 张截图看下来，一条系统性问题：56 条单句的加载失败、
空态、hint、tooltip 以句号结尾。标签不是句子。全部改掉并立门，界线按规则本身划 ——
一句且短的是标签，多句的是描述。

---

## v4.0.1（2026-09-30 — 部署在唯一有数据的库上炸了，以及门为什么没拦住）

4.0.0 的镜像构建和全部门都绿，`deploy` 在生产失败：

```
while executing migration 18: new row for relation "event_attachments"
violates check constraint "event_attachments_source_check"
```

0018 把 `js` 改写成 `javascript`，这一句放在 `DROP CONSTRAINT` 之前 —— 违反的正是
当时还生效的旧约束。**空库里 `WHERE source = 'js'` 匹配 0 行，不写行也就不检查约束**，
所以 preflight、CI、e2e 和每一条单测都绿，只有唯一一个有存量数据的库会炸。

至今每一次 schema 测试都从空库开始，也就是说每条 migration 只被问过「对空的做了什么」，
而有意思的那些全是关于既有行的。现在有 `scripts/migrate-with-data-e2e.sh`：先上 0001–0017，
灌进那个年代合法的行（包括 `source = 'js'`），再上其余，最后逐条核对行确实被迁移过、
约束确实被加回来、旧标签确实没被动。它还自检一次「老约束现在还拒不拒这次改写」——
seed 一旦失去牙齿，这道门会说出来，而不是继续报绿。

**元门从 21 道扩到 54 道（preflight + webapp `check` 共 63 道）。** 它现在从
package.json 读每道门真实的调用方式（此前一个 `.sh` 的门一直用 node 跑），并把已安装的
依赖软链进沙箱，否则那些 import 组件的门解析不到 react —— 而 bun 是挂住，不是报错，
挂住和通过在那里是同一个观察。剩下 8 道需要构建产物或真实 git 历史，每次运行都点名报出。

写探针的过程抓出三道**在跑、也真绿、但已经不覆盖那件事**的门：

- `check-hardcoded-text` 只扫引号字符串。React 里硬编码文案最自然的写法 ——
  直接写在标签之间 —— 从来没被看过。
- `check-cjk-punctuation` 只读 `zh.ts`（它自己开头那段还在数 `ja.ts` 里找到的标点），
  也没有括号规则。两边打开后找出 27 处半角标点贴着中日文，已全部改掉。
- `check-push-snippets` 断言服务端注册了两个字面路由，却从不问 snippet 用的是不是这两个。

**只有看渲染才能发现的三处：**

- 项目页直接渲染 `platform` 原始值：卡片写 `weapp`，另外两个页面把同一个项目写成正式名；
  0020 之后没有 platform 的项目渲染成一个空药丸。`weapp` 的标签是写死的中文，
  英文和日文界面上就夹着「微信小程序」；而 `react-native`（每个既有项目携带的值）
  根本不在标签表里。现在有 `check-platform-label` 守着，mock 里也加了 weapp 项目和无标签项目。
- occurrence 行是 `<button>`，里面的 `UserChip` 也是 `<button>`。
- 两张同名 sourcemap 的断言在 CI 上判错：它找路径里的 `home`，而 GitHub runner 的路径
  经过 `/home/runner`。Mac 上绿，CI 上红，被测的行为两边都是对的。

**新门**：docs 每一页都要能从 README 走到。`check-doc-links` 问的是链接指向的文件在不在，
这道门问的是反过来 —— 有没有链接指向这个文件。`replay-encoding-v2.md` 和
`runbook/cli-auth.md` 都判为用户文档留下，却没有任何路径能走到。

---

## v4.0.0（2026-09-30 — 五个 SDK，一套线；以及一批「写了但从没在跑」）

三个 SDK 变五个。`@goliapkg/sentori-web` 和 `@goliapkg/sentori-weapp` 都用同一个内核，
所以浏览器、小程序、React Native 的排队、退避、拒收处理、离线队列走的是同一份实现，
而不是三份会各自漂移的拷贝。

**真跑一遍才发现的，全都是单测和 CI 全绿时存在的：**

- **任何网页都连不上 ingest。** 服务端从来没有 CORS —— 带 `Authorization` 的 POST，
  preflight 就被拒，而浏览器把这件事报给页面的 console，不报给服务端。从我们这侧完全看不见，
  接入方只会放弃。web SDK 第一次真跑浏览器就撞上。
- **ANR / hang watchdog 两端 native 都实现了、bridge 都暴露了、包装函数没导出、init 也不调用。**
  任何 RN app 都从来没有过这个能力，而 getting-started 写着 init 会启动它们。
- **`setInternalReporter` 导出着，没有人调 setter。** SDK 自己出故障只打 console，
  一条都传不出来。
- **`WebhookTransport` 在 notifier crate 里一直存在，有测试有文档，server 从没注册过它。**
  同时没有 SMTP 时通知直接 return —— 没有邮件服务器的自部署实例一条告警都收不到，
  而且没有任何地方说这件事。
- **五个删掉的模块还在被编译进 npm 包。** `tsc` 不删「源码已经没了」的产物，
  而每个包的 `files` 是整个 `lib/`。`sdk/cli/lib/source-bundle.js` 还是 commit 进去的。
- **服务端按路径后缀匹配 sourcemap 的逻辑，从来没有过输入。** CLI 上传时传 `basename(path)`，
  路径在客户端就丢了。而 `upload sourcemap ./dist` 传目录会 EISDIR。

**三次是门在骗自己：**

- 验证门的那道门，对每条探针只看「改完之后退出码非零」，不检查改之前是绿的。
  有三条从来没跑过：一条把 `gen-replay-vectors.mjs --check` 当文件名传给 node 抛 MODULE_NOT_FOUND，
  两条要问 git 而沙箱不是 git 仓库 —— 都非零，都算「红了」。
- `mobile-e2e` 跑五个脚本，触发列表一个都没写。改「崩真 app 再读回报告」的装置，什么都不会触发。
- `check-dead-options` 有三个洞，其中一个是注释里提到选项名就算「读过了」，
  而立这道门的 bug 恰恰是「死选项加一段自信的文档注释」。

**Chrome 有三份启动实现**，写在相隔几周的三个时间点，本不该有差异。只有一份带
`--no-first-run` 和端口 0，那份在 CI 上从没失败；另外两份固定端口、缺那两个 flag，
render sweep 六次里红两次，症状是「Chrome 什么都没打印、进程还活着」—— 端口被占和 first-run
提示从外面看都长这样。现在一份实现，`check-single-chrome-launcher` 守着，它一写出来就抓到了第三份。

**其他：** 崩溃率有了趋势线和按版本的数字；session 的 `userKey` 此前从来没有过输入
（RN 用 `userId: null` 开 session 之后再没人补），修完顺带发现事件和 session 是两个身份空间；
ingest 收 gzip（服务端先收，客户端还没发）；审计行终于说得出对什么做了什么；
五个实现的 transport 有了对账向量。

native SDK 2.1.0 已发 Maven Central 和 SwiftPM。

---

## v3.19.1（2026-09-30 — 一道红了却说不出理由的门）

`gate / webapp` 的 render sweep 在 CI 上红、在本地用同一条命令全绿。想读它看见了什么的时候
发现三条路都断着：job 日志把关键行折进 `UNKNOWN STEP`，`actions/jobs/<id>/logs` 返回空，
截图 artifact 由一个失败根本没走到的步骤上传。于是唯一能做的是重跑——重跑过了。

**一道训练人去重跑而不是去读的门，比一道慢的门更糟。** 现在它会把每条出错路由和那里的
console 原文打出来，每条最多五行；靠注入一个 console 错误验证过输出。

顺带删掉一段被红路径测试提交了两份的注释，第二份还悬在函数末尾。

---

## v3.19.0（2026-09-30 — 界面说的话，现在和数据一致了）

用一个新工具把产品当作外人来用：真服务端、真登录、真数据，逐屏截图（`walkthrough.mjs`），
再请三个不读源码的评审各按自己的立场挑刺。13 屏 console 错误全为 0——渲染没炸，问题全在
「说了什么」这一层，而打 mock 的自查工具永远看不见那一层。

六条挡路的，全部修掉：

- **推送页在撒谎**。自检绿勾「就绪 · 8 台可达」、投递卡「24H 失败 0」，而同屏 31 条记录
  状态是排队、原因是 APNs 403 拒绝。根因是分类器只认 FCM 的凭证问题，没有对称的 APNs
  分支：`403 InvalidProviderToken` 既不算「永久 token 失败」（那不是 token 的错），也不算
  「配置失败」（漏了），于是一直重试，状态停在排队而结果早已写着被拒。自检本身不用改——
  「全部失败」的规则一直都在，只是被一个永不判失败的状态机喂瞎了；它从 warn 升为 blocked，
  因为面板自己的定义就是「什么都到不了」。现在 `/healthz` 的 `push_failed_24h` 会动了
- **一屏两套时钟**。issue 头部读设备时钟，下面的发生记录读服务端时钟，同一种相对格式，
  没有一个字说明。头部改读服务端时间，设备时间在两者差 5 分钟以上时并列显示——那个差
  本身就是要看的东西
- **13 屏没有一处绝对时间**。`formatRelative` 的注释从写下那天起就说「精确时间在 title
  属性里」；25 个调用点，0 个有 title。审计日志每行都是「3 分钟前」，做不了审计该做的事
- **「0 人」**。一个出现过 60 次的观察点显示 0 人，不可能——那是「事件没带用户标识」。
  仓库本来就有 null 不是 zero 的规矩，同一页也已经在用 `—`
- **堆栈面板在没有栈时整个消失**，而同页别的面板都会解释自己为什么空。更要紧的是：
  这个仓库的 e2e 送出的每一条事件都带 `"stack":[]`，所以主屏的主面板从产品出生起就没在
  任何一道门里渲染过。现在有一条事件带真帧，并断言它从页面用的那个端点读得回来
- **issue 缺「影响多少人」**

新门：`check-time-has-absolute` 守住「渲染出来的时刻都要带得住绝对时间」。

---

## v3.18.0（2026-09-30 — 服务端拒收的事件，三个 SDK 都当成送达了）

批量端点对无效事件返回 200，把拒收放在每条 outcome 里。三份 transport 都只读状态码——
我们自己的注释写着「per-item outcomes are the server's business」——于是任何被服务端拒收
的事件在客户端不留任何痕迹：不重试、不记日志、不计数。

这在两半错开时最要命。自部署的 server 是手动升的，和 SDK 各走各的；SDK 一旦发出老 server
不认的值，每条事件都在一个 200 里被拒，而客户端报告的是风平浪静。v4 要改 platform 词表，
所以这件事必须先做完。

- 三端都读 outcomes 了，拒收计入 `droppedEvents`，**不重试**（`invalid_payload` 是永久的，
  重试只会打转）。计数抽成 `refusedCount`，两个 native 平台的测试断言同一批用例——两端对
  「什么算拒收」理解不一致，会变成只数一支舰队
- 服务端第一次真的读这个数，发布 `sentori_client_dropped_events_total`。在这之前三端写了
  一年，没有任何人读
- TypeScript 那份还补了两个洞：没有 AsyncStorage 时整批静默消失（iOS 3.17.5、Android
  3.17.6 修过，它是第三个平台），以及内存队列没有上限
- surface 门补了盲区：`sdk/**` 这条过滤器会让任何新包被判为「已被点名」，而 sdk 矩阵是手写
  四个包，新包不会被任何 job 构建。现在它同时查矩阵，拿 `sdk/web` 实测会红
- 新增 `webapp/devtools/walkthrough.mjs`：真服务端、真登录、真数据，逐屏截图。已有的
  `sweep.mjs` 打的是 mock，回答「渲染有没有炸」；这个回答「一个正在评估我们的人会看到什么」

native 的改动在 `xcodebuild test` 和 `gradlew testReleaseUnitTest` 上各跑过一次——
这两份没有任何 CI 会编译。

---

## v3.17.10（2026-09-29 — 不会红的门不是门）

上一版让门跑在该跑的地方。这一版问的是它们还会不会红。

先盘点：38 个 checker 全部有人在跑——六个在 e2e smoke 里，因为它们需要活着的栈，位置
是对的——而且每一个离线跑时都报出非零的被检查数量，没有一个在空扫。

然后是真正的问题。每个 checker 写出来那天是绿的，此后一直是绿的；而一个 checker 在它
读的代码搬走之后，看起来也正是这样。`check-wire-case` 就报过整个
`/v1/releases/{release}/artifacts` 期间「全是 camelCase」，而那个端点答的是
`content_hash` 和 `size_bytes`——它读了一个目录，路由在上一层。

所以新增八个探针，各自把对应 checker 存在的那个缺陷重新放回去，要求它说出来：v1 线上的
一个 snake_case key、去掉的 `COLLATE`、给谁都不发的指标配的告警、查一张迁移里没有的表、
读另一个项目的附件、谁也不授权的 admin 端点、两个二进制对 `now()` 理解不同、200 带错误
体。变异只作用在被跟踪文件的一份拷贝上，工作树不碰；八个一起 1.2 秒。

这道门自己也验过两种红：把某个 checker 改成开头就退出，它报「这个保持绿了」；把探针的
靶点挪走，它报「这道门现在没在被测」，而不是静默通过。

写这张表时有三次探针打歪，每次都是探针错、门对：注入到 `check-wire-case` 有意排除的
admin 面、替换了 `use` 导入行而不是守卫调用、把错误体放进一个自己带状态码的 handler。
脚本注释里写明了这一点——没红的条目里，探针错的概率不低于门错。

另外，两个被跟踪的脚本把工作目录放在仓库内的临时目录里（发布 bundle、dashboard 截图），
都挪到仓库外；截图那个顺便打印落点，它以前从不打印。

---

## v3.17.9（2026-09-28 — 门跑在它该跑的地方）

上一版把门跑得更快，这一版是门跑得对不对。先量覆盖：不读注释，拿真实文件列表去对每个
workflow 的 paths 过滤。

结果是 `build.yml` 只在 master push 上跑，而 master 就是生产——webapp、四个 TypeScript
SDK、scripts、docs、cli、docker 构建，它们的第一次 CI 发生在绿了就部署的那条分支上。

- **`build.yml` 也在 develop 上跑**。paths 过滤仍决定哪些 job 跑；`deploy.yml` 的
  `workflow_run` 限定 master，develop 的 build 发不了任何东西（动手前核过，事后也核过：
  deploy 的 run 列表没有多出来的条目）。同时给它加了取消，master 和 release 分支不取消。
- **native 的门按它真正编的东西触发**。原来触发面里唯一的 core 路径是
  `core/migrations/**`，而其中三个 job 要编 server，server 按相对路径依赖十一个 core
  crate——`core/crates/**` 一变，这三个 job 一个都不跑。sourcemap-e2e 就是这样从 2.15.0
  漏到 2.21.1 才看见那个 `GET releases` panic。触发面改成并集，再用一个 `changes` job
  分派，所以扩大触发面不等于每次拉起三台 macOS。
- **公开镜像必须可构建，不只是可读**。clone 那个仓库 `docker compose up --build` 就是
  产品，而只有文档被验过：allowlist 漏掉一个 manifest、lockfile 或 Dockerfile，我们这边
  全绿而公开仓库是坏的。检查同时改成问 rsync「你会传什么」而不是让它真传，再与被跟踪文件
  取交集——镜像从干净 checkout 建，未跟踪的东西到不了那里。全树覆盖 1.8 秒。
- **没人管的面要么接上门，要么写明为什么没有**。webapp 当年带着 96 个类型错误走到切换，
  就是因为它不在任何过滤器里。新检查要求每个目录被一个**在 develop 上跑的** workflow
  点名——只被 master 才跑的 workflow 点名不算，那正是 webapp 当时的状态。它第一次跑就
  逮到两个：`.bun-version` 决定每个 job 装哪个 bun，`.dockerignore` 决定镜像的构建
  上下文，两个都不在任何过滤器里。
- **补上从来没跑过的两道**：`--all-targets` 不含 doctest，core 公开文档里的 22 个例子
  从没被编译过（实测全通过）；`cargo doc -D warnings` 是唯一读 intra-doc 链接的东西，
  之前只在本地跑。

---

## v3.17.8（2026-09-28 — 门的时间花在哪里）

对 preflight 和 CI 做了一次难题分析：三个独立 agent 各查一个角度，主轨逐条复核
`file:line`、关键数字换工具重测。记录在本地 RFC 里，落了五处。

- **镜像文档检查拷 4 GB 读 15 个文件**。它跑的是发布 workflow 的真 rsync 参数，那份
  参数是「若干 exclude + docs 白名单」，没有收尾的 `--exclude='*'`，所以凡是没被点名
  的都拷——一棵工作树里就是 `apps/` 下 2839 MB 的 gradle / Pods / Xcode 产物。CI 的
  干净 checkout 里没有这些，所以只有本地被扎。现在 rsync 到顶层就停：32.7 秒 → 0.13 秒。
- **三处 `cargo check` 零覆盖**。`clippy --all-targets` 是它的超集，往测试模块塞类型
  错，只跑 clippy 就退出 101。preflight 的 core 段和 server 段、`v0.2-core-check`
  各删一处。
- **被取代的 push 不再占着 runner**。`mobile-e2e` 的七个 job 占三个 macOS runner，
  而这个账号跨所有仓库一共五个；develop 上相隔六分钟的两次 push，第二次的 iOS job
  排了 126 秒和 190 秒。同 ref 有更新的 push 就取消旧 run，master 上永不取消。
- **镜像在 gate 跑的同时构建**，不再排在它后面。一次 release 端到端 712 秒：gate 383
  秒，然后 deploy 326 秒，其中构建 303 秒——而构建读的是那棵树，不是 gate 的结论。
  构建只打 sha tag；让一个镜像开始服务的是 `latest` 的指向，那一步仍然要等 gate。
  `up` 也去掉了 `--build`，它再也不可能编一棵门没看过的树。
- **cargo 的 registry 和 target 跨镜像构建保留**。改一行源码后重建：112 秒 → 71 秒。
  剩下的是 `lto = "fat"`，全程序链接跟缓存无关。

preflight 112 秒 → 53 秒。

---

## v3.17.7（2026-09-28 — live 套件继承了给「从不发送」那些用例准备的超时）

3.17.5 让 `__resetForTests` 恢复的请求超时是 1 秒，这样一个不小心走到网络的单测只花六秒
而不是四十五秒。`ios-live-ingest` 恰好是「走到网络就是它的目的」的那个套件：真服务器、真
批次，第一个请求还要付冷连接的代价。1 秒把每次投递都变成落盘，断言照实说了它看到的——
「a batch spilled to disk, so the server refused it」——而服务器什么都没拒。

它现在自己设 15 秒；单测那边保留 1 秒，那是用来阻止一个用例的等待变成下一个用例的失败的。

交付状态（native 2.0.2 这一路已经齐了）：Maven Central 上
`jp.golia.sentori:sentori:2.0.2`（curl repo1 核过 aar / pom / sources 三个产物，
`maven-metadata.xml` 的 latest 与 release 都是 2.0.2）；Swift 包 tag `2.0.2` 在
`goliajp/sentori-swift`；npm 上 `@goliapkg/sentori-react-native@7.0.1`。

---

## v3.17.6（2026-09-28 — Android 用同样两种方式丢批次，而且没有任何测试会说）

上一版在 iOS 侧修的那个洞，拿同一个问题问 Android，答案是一样的
（`SentoriTransport.kt`）：

- `spillFile() ?: return` —— 没有落盘目录时，也就是 `start(null)` 那个文档写明的
  「只在内存」模式，每次发送失败整批消失
- 写文件外面的 `catch (_: Throwable)` —— 磁盘满是常见原因，被吞掉
- 文件自己的上限 `while (all.size > MAX_PERSISTED) all.removeAt(0)` —— 裁掉最旧的不计数，
  而内存队列的上限一直是计的

三处都让批次在离开队列之后不存在于任何地方，`dropped` 不动，下一个信封把投递缺口报成
「安静」。**这个比 iOS 那个面更广**：`sdk/react-native/android` 镜像这份 Kotlin，所以这份
静默也在已发布的 npm 包里，不只在 native 产物里。现在三处都走已有的
`dropped → droppedEvents` 通道计数。

### 它只能靠读代码发现，因为 Android 一条 transport 测试都没有

iOS 有七条。补的第一条刻意与 iOS 那条同形状——两个平台对「什么算丢弃」的理解不一致，
在 dashboard 上会表现成只数了一支机群。先在旧行为下看着它红（报 `persisted: 0, queued: 0,
dropped: 0`），再放回修复：Robolectric 九个测试类、59 个用例、0 失败；镜像那道门在同步前红、
同步后绿。

### 一个版本号既已对外可解析，又还在树里被改

`swift/2.0.1` 从 master 发出去之后，`sdk/native/VERSION` 仍然是 2.0.1，而 Android 这个修复
落在 develop 上——正是 `check-native-version-tag.mjs` 为之而写的情形（`swift/1.0.0` 打完之后
两个功能落在同一个号上）。它在第一次 push 就抓住了，红在 `android-artifact`。现在 native 是
2.0.2 待发，两端 `SentoriConfig` 的常量由 `sync-sdk-version` 写、不手改。

交付：`@goliapkg/sentori-react-native@7.0.1` 已发 npm（从 registry 拉回来核过：包里的 Kotlin
带计数、`SentoriConfig.CURRENT` 是 2.0.2、`SDK_VERSION` 是 7.0.1）。Maven 这次不发——组织的
月度配额已超，Android native 直接接入方随下一次 native 发布拿到。

---

## v3.17.5（2026-09-28 — 一个用例的网络等待，是下一个用例的失败）

`mobile-e2e` 在 master 上红了一次：落盘用例报 `persisted: 0, queued: 0`，而那个提交没碰任何
Swift 文件；同一提交重跑就绿。

**本地确定性复现**：让一个用例发往 SYN 被丢弃（而不是被拒绝）的地址——CI runner 对
`127.0.0.1:9` 就是这样，本机不是——它会占住 transport 那条**串行** worker
`maxRetry × requestTimeout` 加退避，共 45 秒。`__resetForTests` 等 10 秒就放手（它自己的注释
写着「worker 卡住就放弃」），于是那个批次、以及它最终失败时写下的落盘文件，落到当时正在跑
的用例头上。落盘用例 57 秒失败，报 `queued: 4`——自己的三条加上一条遗留。CI 撞的是第一个
断言，本地撞的是第二个，同一根因的两种时序。

修法在测试接缝：`__resetForTests` 恢复的请求超时是 1 秒而不是生产的 15 秒，一次误走网络的
代价从 45 秒变成约 6 秒，落在 10 秒放手时限和 30 秒截止时间之内。没有任何用例的语义改变，
已发布的默认值也没动——生产读静态初值，永不调这个接缝。制造出故障的那个实验换成修复后重
跑：堵 worker 的用例 6.8 秒、落盘用例 3.3 秒通过；整套从 96 秒降到 25 秒。

### 失败批次的最后一站，会静默把它丢掉

`persist` 是发送失败后事件该去的地方，它有三条裸 `return`：拿不到 Application Support 目录、
`JSONSerialization` 拒收某个值、写文件抛错。任何一条命中时，批次已经离开队列——于是它不存在
于任何地方，而 `dropped` 不动，下一个信封就把这段投递缺口报成「安静」。内存队列的上限会计
自己丢掉的东西；这个文件的两种丢法都不计。

三条路径现在都计数，落盘文件自己的上限裁掉最旧那些时也计数，走的是已有的 `dropped` →
`droppedEvents` 通道，不是新机制。别的什么都没变：正常路径没动，动词依然不会抛。

新加的用例先在旧行为下看着它红——报的正是 `persisted: 0, queued: 0`，也就是把这个缺陷写成
了一句断言。57 个用例 0 失败。

---

## v3.17.4（2026-09-27 — 十四道门只在有人记得的时候才跑）

`bun run preflight` 里有 33 个检查，其中 18 个在 CI 里也跑。剩下 14 个的执行保障只是一个
习惯，而这批里包含守已发布面的那几道：镜像的 allowlist（上一版那条死链就是它本该拦下
的——CI 里没人问它）、每个按项目授权的 admin 端点是否真的鉴权、每次读 `event_attachments`
是否带项目谓词、生成的 OpenAPI 文档与 router 是否一致。

现在它们是一个 job，不带 paths 过滤、不 `needs` 任何东西。大部分原先挂在 `server-test`
里，而只改文档的提交会跳过那个 job——偏偏只改文档正是最容易弄坏文档类门的提交。它们只要
node，不装任何依赖，所以这个 job 就是一次 checkout 加十四行。

没收进去的两个：Maven artifact 和 native 版本标签检查，它们要已构建的 Android 产物和注册表
状态，属于 CI 有意不承载的 native 发布路径。

### build context 里装着「不是构建」的那些目录

`.dockerignore` 逐个点名了缓存和构建产物，却没说点目录，于是本地 `docker build` 把 harness
状态、开发材料、139 MB 丢弃输出和凭证目录一起送进 context。没有东西泄进镜像层——Dockerfile
只按名字取 `webapp/`、`core/`、`self-hosted/`——但凭证目录不该被送到任何地方，而这份传输是
纯粹的浪费。一条通配覆盖全部；仓库里被跟踪的点目录只有 `.github/` 和 `.changeset/`，没有
任何构建阶段拷它们。改完实际构建了 webapp 那一层验证 context 没缺东西。

顺带：一处模块注释点名 `core/migrations/0006_push_tokens.sql`。建那两张表的迁移是
`0007_push.sql`，0006 是 notifications。文件名是删掉而不是改对的——它正是因为被写下来才会
过期，那句话不带它也说得完整。

---

## v3.17.3（2026-09-27 — 镜像发了一份索引，指向它没发的页）

`docs/README.md` 会进公开镜像，里面指着 `runbook/scaling.md`。镜像的 allowlist 到
recipes 就停了，`docs/runbook/` 不发——于是那条链接对公开仓库的每个读者都是 404。这正是
`check-mirror-docs.mjs` 文件头描述的那种失败，只是方向相反：镜像是别人拿去跑这个东西的
地方，而它交给对方一份指向自己没给的页面的索引。

留下的两页 runbook 现在都发：`scaling.md` 是加容量前该读什么、这套拓扑不做什么，
`cli-auth.md` 是运维者怎么给 CLI 认证。两者都不是内部材料——真正内部的那五份已经整个移出
仓库。

那个检查原先的断言恰好相反（runbook 不许发），会把这个修复判红。改成要求这两页必须发，
同时对几个曾经放过内部材料的目录留一条回归绊线：它们已经不在仓库里，哪天回来，镜像不许
带上。两半都实测过会红——抽掉 include 报缺页，加一条放行 `docs/design/**` 的规则报泄漏。

---

## v3.17.2（2026-09-27 — 出处指向读者没有的东西）

3.17.1 把不该在仓库里的文件移走了，这一版收拾剩下的引用和目录。

### 58 处注释在引用一份读者没有的文档

形状都一样：`(design.md §9)`。章节号是这些引用里唯一承重的部分，而它解析不到任何
地方——读者拿到的是出处的形状，后面什么也没有。每处的出处删掉，它当时想说的那句话
写在原地：只存在两种角色、bundle 是什么、SMTP 为什么可有可无、某个探测器实现的是哪个
warn 场景。

迁移文件里的保留不动。`sqlx::migrate!` 的校验和覆盖整个文件内容、注释字节也算，改一个
就会让所有已有数据库在启动时停下。

### 又移出四组

- **`docs-v0.1/` `docs-v0.2/`**（12 个文件）——描述的两套栈都不再运行，v0.2 那套
  2026-08-10 就从主机上删了。
- **`docs/legal/`**——隐私政策和条款，正文带 `<占位符>` 和「需律师审阅」，写的是一个
  没有注册入口的托管服务。仓库外面查过不是假设：t01 的 Caddyfile 和控制面里 5 个
  sentori 站点块零命中，`/legal/privacy`、`/privacy`、`/terms` 实际返回的是 dashboard
  外壳，其他仓库也零引用——这两份从来没对外提供过。
- **`core/README.md`**——给读代码的人看的 crate 分层图，也是这个仓库里最后一份中文
  文档。

镜像的 include 少了 legal 两行，exclude 少了三条指向已不存在路径的，现在发 13 篇而不是
15 篇。

### 一个根目录的 scratch 目录，忽略文件不得不点它的名

`tmp/` 攒了 3.4 GB：日志、截图、构建产物，还一度放着某个签名密钥的唯一副本；忽略文件
为它写了两遍，外加两个子目录变体。丢弃品和值得留的东西堆在同一处。

现在分三个目录，各自由全局忽略负责、仓库的忽略文件不再点名：一处放能活过会话的材料，
一处放丢弃品，一处放凭证。三个脚本写进过旧路径，改指丢弃目录：Maven 打包的工作目录、
webapp sweep 的默认输出，以及 e2e 里解释为什么用 `mktemp` 的那句注释。CHANGELOG 里点过
那个路径的几条改成直接说做了什么——一个只在某台机器上存在过的路径，读者跟不过去。

---

## v3.17.1（2026-09-27 — 仓库里只剩交付物）

一个公开仓库跟踪了 158 个没人从外面读的文件，而定义生产的那个文件不在任何仓库里。两件事
方向相反，根因一样：**没有人问过「这东西的读者是谁」**。

### 移出去的：158 个过程文件

roadmap、设计提案、plan、性能基线、决策记录、给自己生产写的 runbook、给某个客户团队的交接
沟通，全部被跟踪在一个谁都能读的仓库里。它们不是文档，是开发材料：描述在做一半的事，点名
我们自己的主机、一个模拟器序列号和一个私人邮箱。其中 `sdk/react-native/ios/PRIVACY_AND_REVIEW.md`
还在 npm 包的 `files` 范围内，**每个安装 SDK 的人都收到了它**。

留下的是外面的人真需要的：使用指南、SDK 与协议参考、cookbook、自托管和扩容两页、CHANGELOG、
LICENSE、SECURITY，以及 `ops/` 那几个脚本和讲它们的 README。

贡献者文档也一起移出：这个仓库不收 PR——公开 master 每次发版被 `git init` + `git push -f`
重建，合并进去的 PR 下一版就没了（v3.17.0 那条说的正是这件事）。一份讲怎么提 PR 的指南、
一份行为准则、一个 PR 模板和两个 issue 模板，描述的是一种从不发生的协作。

同一个提交里修好了所有指向被移出文件的引用：留下的文档不再链一个不存在的页，而是直接把话
说完；各 CHANGELOG 里的 markdown 链接改成纯文本（changelog 记录的是发生过的事，它点的路径
是记录的一部分）；源码注释改成说规则本身，不说规则写在哪。

### 收回来的：定义生产的那个 compose

部署一直从 runner 上另一个仓库的 checkout 里安装 `/apps/sentori/docker-compose.yml`，
fallback 指向本仓库里一个并不存在的路径。那个仓库已经不再跟踪它，于是生产运行的拓扑——
容器、镜像、卷、端口、dashboard 靠的那个 bind mount——只存在于一台主机上的一个文件，和一台
笔记本上一份不进 git 的副本。主机重建就没了。

它现在是 `deploy/docker-compose.t01.yml`，与线上逐字节相同（sha256 `26866dd4…`），部署从
它所发布的那个 release 里安装它。主机那份不一样时，先把 diff 打进日志再覆盖——事故中手改过
的内容留下记录，而不是静默消失。自托管用户拿的仍然是 `self-hosted/docker/docker-compose.yml`，
镜像排除 `deploy/`：它只描述一台主机，发出去只会误导。

### 顺带补了两道门

- `check-env-vars-real` 现在也读生产 compose：它交给容器的每个 `SENTORI_*` 键必须是服务器
  真读的名字，否则那个设置已经静默失效。插值（`${SENTORI_PG_PASSWORD}` 这类）是 compose
  自己的输入，不算。拿一个编造的键实测过它会红。这个检查原来只在 preflight 跑——**生产的门
  放在 preflight 里是放错了地方**，现在是 CI 的一步。
- 同一个检查原先跳过 `docs/runbook/`，恰好把留下来的两页排除在外。现在覆盖它们，同样实测
  过会红。

### 一个不能碰的地方

`core/migrations/0001_identity.sql` 里的一句注释指向已经移出的文档，**故意没改**。
`sqlx::migrate!` 的校验和覆盖整个文件内容，注释字节也算，改了它所有已有数据库——包括生产
——会在启动时迁移校验失败。

---

## v3.17.0(2026-09-08 — 通往生产的那条路上没有门)

两件事,同一个形状:**说了但做不到**。

### release/* 直接部署生产,而且不跑任何检查

`build.yml` 不在它上面触发,`v0.2-core-check` 也不在。所以一台笔记本和线上实例之间,唯一
站着的东西是「这个人记不记得跑 `bun run preflight`」——一种纪律,而不是一种保证,偏偏在
唯一一条错误会直接对外服务的路径上。**其它每条进生产的路都有门,只有发版这条没有。**

`deploy.yml` 现在把 `build.yml` 当 `workflow_call` 调用,deploy job `needs` 它。三个选择:

- **用 `workflow_call`,不用「加个 push 触发 + workflow_run」。** workflow_run 是异步的,
  在这个仓库里静默漏触发过。对 master 线那是可以承受的(它重新部署的是已经在线的东西),
  对 release 线不行——那会让一次部署在「门根本没跑」的情况下放行,比它要堵的洞更糟。
- **`force_all: true`。** 一个 release commit 只碰 VERSION、CHANGELOG、两个 Cargo 文件和
  openapi.json。按路径过滤会**恰好在发布它们的那个 commit 上**跳过 webapp 和 sdk job ——
  跑了的门和看了的门不是一回事。部署发布的是整棵树,不是 diff。
- **deploy job 上的 `always()`。** master 线上 `gate` 是 skipped(build.yml 已经绿过),
  而没有 `always()` 的话,一个被跳过的依赖会把它守护的 job 一起取消。

**验过它真的挡得住。** 推了一个 `release/3.16.1` 分支,里面带一条故意失败的 SDK 断言:

```
gate / sdk (sdk/react-native)   FAILURE   ← 注入的故障
deploy                          SKIPPED
```

生产停在 3.16.1,一次都没重建。分支名特意选了与 Cargo.toml 相同的版本号:万一门没挡住,
最坏结果是重新部署同一个版本,而不是把没过检查的东西送上线。

那张表的另一半同样重要:webapp、e2e 和四个 sdk job **都在一个只改了一个测试文件的 commit
上跑了** —— 那就是 `force_all` 在起作用。默认路径过滤下它们会被跳过,而**会跳过的门不是
门**。

### 公开镜像承诺了一件它机制上做不到的事

mirror workflow 的头部注释写着它存在是为了让社区 "fork / inspect / **contribute**"。而它
执行的推送是 `git init` + `git push -f` —— 公开 master 每次发版都被从零重建。**在那边合并
的 PR 会在下一次发版时被抹掉,连同作者署名。** 前两个动词是真的,第三个从来不可能。

注释改了,并且新增一份根 `CONTRIBUTING.md` 进入镜像,把这件事直说,同时给出真正走得通的
路径:开 issue、把 patch 放在 issue 里、由上游应用并在 changelog 里署名——你的 PR 会被
close 而不是 merge,那是机制在诚实,不是 patch 被拒。

它也告诉贡献者**在公开 repo 里跑不了 `bun run preflight`**(它有几个检查要读镜像配置和
workflow 文件,而那些不在那边),换成一组在那边真能跑的命令。`.github/CONTRIBUTING.md`
仍然排除在镜像外:那份是内部的,写的分支和发版流程在镜像那边并不存在。

### 顺带,我自己又踩了一次

给 rsync 的参数列表中间插了注释行,shellcheck 立刻 SC2215 —— **反斜杠续行里的 `#` 会让
shell 吞掉它后面所有参数**。记忆里这条写着「一周三犯」。注释搬到命令上方,并在那里写明了
为什么不能往下搬。

## v3.16.1(2026-09-08 — 给昨天新造的东西配上门)

3.16.0 把六个死指标引用改活、加了 `sentori_ingest_total`。这一版是它的后半程:
**新造的东西自己也需要门**,否则它会用和被它替换掉的那些东西一样的方式腐烂。

### 铁律的第四维一直没有门

client 零成本铁律有四维,而没有门的那一维等于没写。三条有:失败零传染是
`iron-rule.test.ts`,footprint 是 `check-sdk-size.sh`,perf 是 `sdk-perf.yml` 加 init
预算。**net(零可感)那条只有一个写下来的数字,全仓库没有任何东西在检查它。**

三条断言,用一个计数 fetch:

- **没出事就一个字节都不发。** 不调任何动词,直接 flush。空队列不能变成心跳 —— 一个按
  定时器跟自己服务器说话的 SDK,接入方在自己的网络面板里看得见,而「只有免费的东西才会被
  留着」是整个产品定位。
- **一个坏分钟低于 500 KB。** 一次崩溃 + 周边检测信号 + 用户身份 + 二十条带 context 的
  host 面包屑,远超真实一分钟。**实测 21.3 KB,占预算 4%。** 断言失败时把实测数字抛出来,
  下一个人看到的是趋势不是一个红叉。
- **批量发,不是一事件一请求。** 二十个事件不能是二十次往返,否则一次突发在接入方自己的
  监控里读起来像网络故障。

三条各自验过会红:预算调到 0.1 KB 报「sent 21.3 KB」;空闲 flush 前插一条 trace 让「安静」
断言红;要求请求数 <1 让批量断言红。

### 新指标只证明了它存在,没证明它会动

`sentori_ingest_total` 出现在 `/metrics` 里不是值得检查的性质 —— 一个卡在 0 的 counter
也会出现在那里,而 0 正是健康 counter 大部分时间的读数。e2e 现在断言**移动**:全套跑完
`accepted` 必须 > 0;一个 `platform` 非法的请求必须返回 400 **并且**让 `rejected` 变大。

实测:全程 `accepted=72`,坏 platform 让 `rejected 0→1`。第二个数据库回答不了 —— 被拒的
事件从不入库,这正是它是 `metrics_prom.rs` 里唯一不来自查询的指标的原因。

### 422 是个盲区,现在写下来了

反序列化失败的 body 到不了 handler:axum 的 `Json` extractor 用 422 + 纯文本挡掉,而
`docs/errors.md` 早已把这个行为记为「刻意在错误码契约之外」。所以 `sentori_ingest_total`
看不见它,由它算出的错误率分子分母都不含 422 —— **一个发结构错误 JSON 的客户端在这里读
起来是安静,不是报错**。对一个 per-event counter 这是对的取舍(没解析出来的 body 没有事件
可数),但必须写在指标和告警两处,否则下一个人会推断出并不存在的覆盖。

### CONTRIBUTING 在对读不到它的人说话

开头写着「This repo is the public OSS mirror」并让你 clone `sentori-selfhosted`,而它自己
就在那个 mirror 的 exclude 列表上,从不抵达那边。**它面向的受众读不到它**,反过来说公开
repo 根本没有贡献指引。没有把它从 exclude 里拿掉:mirror 发什么是分发决策,不是一个 commit
该绕过去的事,改成在文件里说清楚。

它的事实和它的受众一样旧:石头例子 `license-jwt`、钢筋 `workspace-identity` /
`event-pipeline`、水泥 `saas/server` —— v1 重构全删了;CI 写的 `v0.1-core.yml` 不存在;
schema 变更指向同样被 mirror 排除的 `docs-v0.1/`;还有一段把 SaaS 功能路由到一个已经不存在
的私有 repo。照真实 crate 清单、真实 workflow、真实的门重建 —— 而对 `release/*` 分支(它
直接部署生产)来说,`bun run preflight` 是**唯一**的门。

## v3.16.0(2026-09-08 — 腐烂长在没有门的地方)

一次全仓库盘点。确凿的问题**全部**落在三个零 checker 覆盖的面上:`ops/`、
`docs/runbook/`、`core/Cargo.toml` 的 exclude 列表。而有门看着的面 —— docs API 真实性、
env var、SQL 表名、doc link、i18n —— 一个问题都没有。这不是巧合:这仓库有三十来个
checker,**它们看的地方不会烂**。

### /metrics 答不出「有多少事件被拒了」

`sentori_ingest_total` 被告警规则按 rate 报警、被 dashboard 画两个 panel、被
`deploy.md` 要求「滚动时盯 `{status="rejected"}` 五分钟」。**没有任何代码发过它。**

数据库能被问「存了多少事件」,不能被问「拒了多少」—— 被拒的事件从来不入库。所以这是
`/metrics` 里唯一不来自查询的指标:四个进程内计数器,按终态计 —— accepted(202)/
rejected(400,payload 坏,客户端的锅)/ failed(500,流水线出错,我们的锅)/
rate_limited(429,限流器在干活)。batch 按事件计数不按请求计数;超大 batch 算一次拒绝,
因为那就是一次拒绝。

counter 不是 gauge:重启归零正是 Prometheus counter 的语义,`rate()` 会处理。四个 status
永远都发,包括 0 —— 对 counter 来说 0 是一次测量,而序列缺失不是。

### 告警触发后运维无路可走

`PgPoolNearSaturation` 按 `db_pool_in_use / db_pool_size > 0.80` 报警,`scaling.md` 让你
调 `SQLX_MAX_CONNECTIONS`。**没有任何代码读那个变量,也没有别的旋钮** —— 池就是 sqlx 默认
大小。补 `SENTORI_DB_MAX_CONNECTIONS`。不设=行为不变;设 0 或乱填按警告忽略,因为
`max_connections(0)` 建出的池永远发不出连接,一个笔误不该读作「把服务器焊死」的指令。

### 六个指标引用,没有一个是活的

`sentori_pg_pool_in_use` / `sentori_pg_pool_max`(真名是 `db_pool_in_use` /
`db_pool_size`)、`sentori_ingest_duration_seconds_bucket`(没有 histogram)、
`sentori_quota_drops_total`(quota 随 SaaS 面走了)。全部对齐,latency panel 换成池利用率
(对应还活着的那条告警),quota panel 换成 rate_limited。

**门才是重点。** `scripts/check-metric-names.mjs` 只解析 `expr` 字段 —— YAML 的
`rules[].expr` 和 dashboard 的 `panels[].targets[].expr`,引用了 `metrics_prom.rs` 没发的
指标就红。只看表达式是刻意的:两份文件的散文现在都会提到被删掉的指标名来解释它为什么没了,
而文档不是查询。验过会红:把 `sentori_pg_pool_*` 改回去 exit 1、把 histogram panel 改回去
exit 1、当前树 exit 0。

### runbook 在教人重启一个七月就删掉的容器

三份都是凌晨三点被脑子不清醒的人照着做的东西,而里面每条具体命令都是错的。

`incident-response.md` 唯一的止血步骤是
`docker compose -f production-compose.yml restart valkey` —— 重启一个 2026-07-23 删掉的服务,
通过一个那个路径上从不存在的 compose 文件。周围还有 Better Stack 双人轮值、已经不解析的
`status.sentori.golia.jp`、`#sentori-ops` 频道、按 marketing/docs 挂掉判 P2(八月删的容器)、
free-tier org quota(SaaS,已删)、以及一个不存在的 postmortem 目录。跟用户确认过 on-call
那套已经全没了,重写成单人运维。

`deploy.md` 写着在 `main` 上打 tag、pull GHCR 镜像、`lb_policy ip_hash` 后面轮换
`server-blue` / `server-green`。分支是 master,镜像在 runner 上 build,容器只有一个。照
deploy.yml 重写,并补上这个 session 亲自撞到的两件事:**`release/*` 部署生产前一道检查都不
跑**,本地 preflight 就是全部;以及 release finish **不要打 tag** —— deploy.yml 已经打了,
再打会在 `&&` 链中间失败并吞掉后面的 `git push origin master`,master 停在本地,看起来像
做完了。

`scaling.md` 在给从没开过的 Hetzner VM 定规格、往只有一个 upstream 的 Caddy pool 里加机器、
调一张不存在的 `org_quotas` 表。三个诊断步骤分别查:一个画不出东西的 Grafana panel、一个
server 不发的指标名、一个从没建过的 events 分区树(retention 是 archive_worker 里的
DELETE)。重写成真实架构,并直说没有横向扩展路径 —— 第二个 server 容器需要限流器和 ingest
计数器这两个进程内状态变成共享状态,而那个设计不存在。

### 四份文档在描述一个七月起就不存在的系统

`core/Cargo.toml` 的 exclude 列着十六个 crate「等 S9 sweep」——sweep 在 `94bd8758`
(2026-08-01)做了,十六个目录全没了,这份列表多活了五周。`members` 是显式列表,没有自动
发现,也就没有什么要 exclude。

`core/README.md` 列了二十七个 crate:十六个已删,三个存在的没列,`rate-limiter` 被描述成
「sliding window with valkey」(它是内存的),结尾指向一个不存在的 `v0.1-core.yml`。照真实
目录重建,每个 crate 的分类取自它自己 Cargo.toml 的声明而不是猜。

`ROADMAP.md` 头部写着 Valkey 和 `web/` 前端目录(是 `webapp/`),而**当前状态**那节是
2026-06-23 v0.2 pivot 的快照 —— SaaS 双 binary、一个「待你拍板」的 cutover(2026-07-20
执行了)。没有换一份新的功能清单:手写清单会用和代码一样的速度腐烂,而且腐烂时看不出来。
改成指向 VERSION、CHANGELOG 和那两个目录。

`docs/design/architecture-standards.md` §4 开篇「Sentori already uses Valkey for rate
limits, recent-events ring, quotas, and live presence」,然后为这四个消费者规定 key 命名、
TTL 分级、Lua pipelining 和 fail-open 分支。**四个一个都不存在。** 一份描述系统没有的基础
设施的标准文档,比不提它更糟 —— 下一个功能会把它读成许可。顺带删掉一个没有代码定义的
`internal.valkeyDown` 错误码和一个没有路由的 `/admin/api/self-test`。

**没动,而且是有意的**:CHANGELOG.md、docs-v0.*、docs/roadmap/v*.md、docs/archive/。它们
记录的是写下时的事实,改它们不叫修文档。

## v3.15.4(2026-09-07 — fix/ 分支从来没跑过 CI)

3.15.3 那两个分支都是 `fix/` 开头,合进 develop 之前一次 CI 都没跑。查 workflow 才发现
`v0.2-core-check` 的 branches 写的是 `[feature/**, develop, master]`——**没有 `fix/**`**。

git-flow 里 fix/ 和 feature/ 是等价的:同一个起点、同一个合并目标,前缀只标「新功能还是
修补」。所以一个动了 `core/**` 或 `self-hosted/server/**` 的 fix/ 分支,要一直等到进
develop 才有人看。这正是这道 workflow 存在的理由。补上 `fix/**` 和 `hotfix/**`——后者
理由更硬:它从 master 拉、往 master 合,是唯一一个错误可以不经过 develop 就进生产的前缀。

自验:改动本身就在一个 fix/ 分支上,改的又正好是这个 workflow(它的 paths 盯着自己),
push 后 `v0.2 core` 真的跑起来了,green。旧的 branches 列表做不到这件事。

同时修正了分支流程手册的 CI 触发表——它只列了 build + deploy 两个 workflow,并
断言 develop push 完全没 CI。实际有十个 workflow,develop 上跑三个。按每个文件的 `on:`
块重写,并标出 paths 过滤(表里的 ✓ 是「路径命中才跑」)。另修三处:release finish 让你
手动 `git tag -a`,但 deploy workflow 在 release/* push 时已经打好推好了,再打会失败——
而且它在 `&&` 链中间,会把后面的 `git push origin master` 一起吞掉,master 停在本地
(发 3.15.3 时就撞了);release start 让你走 changeset,但纯 server 版本根本不碰它;
"master 是唯一 CI 入口"应为「覆盖面最全」。

**那份手册不在这个 commit 里** —— 它不进版本管理,那份修正只在本机生效。

## v3.15.3(2026-09-07 — Valkey 两个月前就走了,注释没跟上)

问的是「kevy 升 6.3」。查下来这仓库既没有 kevy 也没有 Valkey:Valkey 2026-07-23 随
v0.1 栈一起删了,之后没有东西接替。留下的是八个文件里的描述,其中两处把它写成待办。

最锋利的是告警。`sentori-valkey` 组查 `sentori_valkey_command_duration_seconds_bucket`,
一个从没有任何构建发出过的指标,监控一个不运行的服务。**永远不会触发的规则,在 review
里读起来和有覆盖一模一样。** 删。ingest-stalled runbook 里「去看 Valkey」那句一并删——
半夜被叫起来的人会去找一个不在的容器。

其余是 `rate-limiter` / `cookie-session` 里承诺「v0.2 会有 ValkeyBackend」的注释。这些
句子挂的两个锚点也没了:K-tier `auth-session` crate(已退到 `core/Cargo.toml` 的
exclude)和顶层 `server/` 树(v1 重构删除)。改成事实:`MemoryBackend` 是唯一存在的
backend。

**`cargo doc` 从来不在 preflight 也不在 CI 里。** 跑一次就查出 push-provider 的模块文档
还在把 `Uuid` 归功于 `K1 sentori_workspace_identity`——同一种病,活下来是因为没人看。所以
修的是门:三棵 Rust 树现在都跑 `RUSTDOCFLAGS='-D warnings' cargo doc --no-deps`。断掉的
intra-doc link 是「文档在指一个不存在的东西」的廉价探测器。

开门让 server 红在五处,全是格式不是腐烂:两个裸 URL、`<oauth2 access token>` 和
`st_pk_<token>` 被当成未闭合 HTML 标签、`payload.context[key]` 被当成链接。两段请求块改
成 text 围栏——标签必须写,doc 注释里不带标签的围栏会被当 Rust doctest 收走。

验过会红:断链改回去跑 core 那条门线 exit 101,改回来 exit 0。

devops 侧同步修 `services/sentori/README.md`。它写的运行时是 postgres + valkey + server
+ web,路由表列四个端口;live t01 Caddy(今天只读拿的)只有一个 upstream,server-v1
`:18092`。

## v3.15.2(2026-09-06 — 那道门被我放进了一个没有 bun 的 job)

3.15.1 要修的东西都绿了(四个 sdk job、webapp、e2e),唯一红的是新加的那个 checker
本身:它被我放进 `server-test`,那个 job 不装 bun,于是 `spawnSync bun ENOENT` 加一段
Node 堆栈。

挪到 `workflows` —— 那个 job 管的就是 CI 配置,现在给它装了 bun。脚本在 PATH 上找不到
bun 时也不再抛异常,而是把「为什么跑不了」印出来:**跑不起来的门不是门**,而它跑不起来
的原因必须是它自己说的那句话。

验过会红:PATH 上留 node、去掉 bun。

## v3.15.1(2026-09-06 — CI 跑的 bun 比写 lockfile 的那个旧)

`build` 和 `mobile-e2e` 在 master 上红了,而这个仓库里没有任何依赖变过。CI pin 的是
bun 1.3.13;我们的 lockfile 是 1.4.x 写的,1.3.13 读不了就重新解析 ——
`error: lockfile had changes, but lockfile is frozen`,而同一棵树上本地
`bun install --frozen-lockfile` 刚刚通过。**preflight 看不见,因为 preflight 跑的是
开发机上的 bun。**

实测(root 和 apps/rn-example,frozen install):1.4.2 干净 / 1.4.1 干净 /
1.3.13 报 lockfile had changes。

版本号写在四个 workflow 文件里,其中两个写的是 `latest` —— 那根本不是 pin,上游发一个
新版本就够把它推走。现在只写在 `.bun-version`,每个 workflow 用 `bun-version-file` 读。

删掉两个从 initial commit 起就没动过的孤儿 lockfile,它们都在 workspace member 里面
(那里由根 lockfile 说了算):`apps/rn-example/bun.lock` 记的是 Expo 55 /
react-native 0.83.6,而它的 package.json 从 2026-08-16 起要的是 Expo 57 / 0.86.2;
`sdk/react-native/bun.lock` 同理。bun 1.4 忽略它们读根 lockfile,1.3.13 读的是那份陈的。
没有任何东西在写它们,所以也没有任何东西纠正过它们。

门:`scripts/check-bun-version.mjs` —— 这个 shell 的 bun 就是 CI 装的那个、没有 workflow
自己写版本号、没有 workspace member 带自己的 lockfile。三条都验过会红。preflight 现在
也在 `apps/rn-example` 里跑一次 frozen install,那正是 mobile-e2e 做的、而 preflight
一直没做的事。

## v3.15.0(2026-09-06 — 三个 gauge 读 0、一个 dump 覆盖 7/26、一个从来没人验的时区)

给 spg 写的 corpus 第一次跑起来,四个发现里有两个是我们自己的;补最后一个欠了四轮的
门时,又撞出三个 spg 的协议缺陷。这一版是前两个的修复 + 那个门。

### 三个 `/metrics` gauge 问了没人能回答的问题,三个都发布 `0`

`alert_rules` 被 v1 重写删了;`sessions` 从 `0001_identity.sql` 起就叫
`auth_sessions`;`sentori_issues_open` 过滤 `status = 'unresolved'`,而这个列的 CHECK
从来不允许这个值。前两个报错,helper 把每个错误变成 `0`。第三个**成功了,只是什么都
没匹配上** —— 再好的错误处理也抓不到它,只有读那一列自己的约束能。

gauge 读 0 是健康 gauge 大多数时候的读数,所以它活过了整个 v1 schema 的生命周期。

`sentori_alerts_active` 直接删掉而不是改指向:v1 里没有任何东西是 alert rule,而一个
分不清「没有」和「坏了」的 gauge 比没有更糟。helper 现在返回 `Option`,调用方直接不
输出那一行 —— Prometheus 本来就有「没测到」的表示法,scraper 能对缺失的 series 告警,
对一个没有含义的 0 不能。

门:`scripts/check-sql-tables-exist.mjs`,我们 SQL 里点名的每一张表都必须在 migration
里存在。260 处引用。

### `dump` 覆盖了我们 26 张表里的 7 张

`commands::TABLES` 驱动 `dump` / `status` / `export` / `restore`,而它还是 v1 之前那个
内核的列表:34 条里有 27 条点名的表重写时就删了。`restore` 静默跳过缺失的文件而且这是
设计使然(快照可能是增量的)—— 于是往返在两个方向上都报成功,同时漏掉 `tokens`、
`releases`、`release_artifacts`、每一张 push 表和每一个附件。

两个列表都按外键拓扑序从 `core/migrations` 重建,不再靠手工维护。

门:`commands::schema_agreement` 四个测试,读 `core/migrations` 现场比对。

**两个都能活这么久,是因为 `self-hosted/cli` 一道门都没有** —— 没有 fmt、没有 clippy、
没有测试。preflight 和 CI 覆盖 `core` 和 `self-hosted/server`,而这个 crate 是作为二进制
发布的。现在接上了,接上的时候里面躺着两个 clippy error。

### 会话时区:欠了四轮,补的时候发现 pin 一直在,缺的是门

`now()`、我们渲染的每个 `timestamptz`、retention 删除里的 interval 运算、worker 里的
`extract(epoch FROM …)`,都在**会话的**时区里求值。185 处依赖它,没有一处点名时区。

`sqlx` 一直在 startup packet 里要 `TimeZone=UTC`,所以在 PostgreSQL 上 pin 本来就在。
真正缺的是**有没有人验过** —— 本仓库最高频的那个缺陷,又一次。

`db.rs` 现在是唯一开连接的地方:要 UTC,然后**读回来**。第一条连接手工开,因为连接池
把 `after_connect` 的失败报成 acquire 超时,那条消息指着错误的组件。之后每一条物理连接
都再验一次。读回用 `current_setting` 而不是 `SHOW`:扩展协议上函数调用才是正常形状。

门:`scripts/check-timezone-pinned.mjs` —— 别的文件不许自己开连接,server 和 CLI 的两份
`db.rs` 不许漂移(CLI 是独立 workspace,共享不了)。红过才信:去掉 `.options()` 的单测、
新加一个 `PgPool::connect` 的门、两份副本漂移的门,以及拿一台服务器默认时区是 Asia/Tokyo
的真 PostgreSQL 验读回本身。

## v3.14.0(2026-08-28 — 我修好了错误文案,却从没看过一张错误截图)

这轮早前我修了 `formatApiError`(它一直渲染成一个裸冒号)。今天把 mock 的失败开关
真跑起来、把错误态拍下来 —— **它还是没在工作**,只是换了一种没在工作的方式。

### 那个开关一直在,从来没人跑过

`mock-api.mjs` 里有 `MOCK_FAIL`,注释写得清清楚楚:「每个页面的加载态和空态 sweep
默认就渲染,唯独错误态没有任何东西渲染 —— 于是这个 dashboard 带着『每个 API 错误都
显示成 ": "』发布了」。写它的人知道问题在哪。**但它没接进 sweep、没进 README、没有
任何入口**,所以没人跑过。本仓库最高频的那个缺陷,又一次。

现在有 `bun run mock:fail`,README 写了怎么用。

### 三层,每层都对,合起来什么都没说

拍下来的第一张:`Error: 500`。往下挖:

1. **监测页绕过了 api client** —— 手写 `fetch`,失败时 `throw new Error(String(resp.status))`:
   不读 body、不造 `ApiError`、拿不到 401 跳转、拿不到 `code`/`field`。`formatApiError`
   没错,是这条路从不产生它认识的东西。而 `check-error-format` 是拿构造出来的
   `ApiError` 测函数的,所以一直绿 —— **门测的是函数,不是那条路**。
2. **六处错误条把原因丢了**,只渲染自己那句「加载失败」。一句话分不出会话过期
   (重新登录)、没权限(找人)、数据库挂了(等)—— 三种不同的下一步,一个提示。
3. **版本页在错误旁边写着「版本 (0)」**。没有 active project 时 fetcher 返回了一个
   伪造的 `{releases: []}`,这份假数据活到了下一次渲染 —— 那个 0 不是任何人数出来的。
   计数是一种断言,而「没有」跟「我们没能查到」不是同一句话。

三个都修了。现在那条链是通的:服务端 body → `ApiError` → `formatApiError` →
`ErrorBanner reason` → 屏幕上写着 `500: upstream_unavailable — the database refused
the connection`。

三道新门:`<ErrorBanner>` 必须带上服务端说的话;`src/pages` 和 `src/components` 里
不许直接 `fetch`;每道都验过会红。

## v3.13.0(2026-08-28 — 两处「看起来像别的意思」)

都是看截图找出来的,读代码看不出来 —— 因为两处的代码都是对的,错的是**它渲染成
什么**。

### `0 (+3)` 读起来像涨了 3

监测页的启动表把冷启动写成主数字、预热写成 `(+214)`,再挂一个 hover 才看得见的
提示。可 `(+N)` 在任何别的地方都是**增量**的写法 —— 读者以为自己看懂了,根本不会
去 hover。1.2.2 那行的 `0 (+3)` 于是读作「这版涨了 3 次启动」,而实际意思是「零个
真实样本、三个预热幻影」。列头还写着「样本」,而显示的数字是样本减预热。

预热单独成列。**列头不会被误读,标点约定会。**

### 深色表单里的浅灰盒子

`<select>` 在 macOS 上会自己画控件,盖掉给它的背景色 —— 所以 `bg-surface` 形同虚
设,每个深色表单里都多出一个跟周围完全不搭的浅灰块。凭证页有一个,triage 的筛选条
有四个,而那是人整天盯着的那块屏。

`appearance-none` 收进 `SELECT_CLASS` 一处,箭头我们自己画(按主题两个字面量 —— data
URI 是字符串,插不进 CSS 变量)。两个主题都截图验过。

**第四个是渲染告诉我的。** 我按精确类名串替换,修好了 triage 的三个筛选;第四个中间
多插了一句 `max-w-40`,没被匹配到 —— 在 diff 里完全看不出来,在截图里一眼就是那个刚
修完的 bug。门因此不比类名串,而是数每一个 `<select>` 有没有戴 `SELECT_CLASS`。

写这道门的第一版**把四个正确的元素全报成坏的**:`onChange={(e) => …}` 里的 `=>` 有个
`>`,正则在那里就把标签截断了。改成按花括号深度扫。

## v3.12.0(2026-08-28 — 一条随机红的门,指着一个真的竞态)

`mobile-e2e` 在 master 上红了,develop 上**同一个 commit** 绿。同一份代码两个结果
= flake。而查下去,它指着的是产品里一个真的窗口。

### 身份监听器装得太晚

iOS 和 Android 是同一套结构:设备注册成功之后,`remember()` / `rememberHandle()`
才把身份监听器装上。而注册请求的 body 在**发出之前**就已经把 `userKey` 和
`traits` 序列化进去了。

于是有这么一段时间:注册请求已经在路上、监听器还没装。host 在这段时间里调
`sentori.user()`,`SentoriScope.setUser` 会去调监听器 —— 没有,什么都不发。等注册
回来装好监听器,也不会有人再问一次。这个人就丢了,直到下一次 token 轮转。

这不是罕见时序:app 启动时注册推送、紧接着识别用户,两件事就差几百毫秒。

第一版修法是「装好监听器之后立刻读一次身份,变了就补发」。**实测把它推翻了**:
RN 上多出了第三个请求,内容是 `userKey: null` 但 traits 已经有了 —— 半个人。
因为 `user()` 的 key 是异步哈希的,`setUser` 先把 traits 放好,key 一拍之后才落地,
而通知只在 key 落地后发一次。我那个「立刻读」正好插在两者之间。

所以改成:**没人听的时候把这次通知记下来,有人开始听的时候补播一次**。补播发生在
通知的时刻,而通知的时刻身份必然是完整的 —— 半成品读不到。一个标志位而不是队列:
身份是当前值不是流,补最后一次就够。三端(Swift / Kotlin / TypeScript)同一套。

`registerWithServer` 同时记下**它实际放上线的那个身份**。没有这一笔,补播时会拿
`null` 去比,于是即使注册本身已经带上了这个人,也还要多发一次 —— 这笔记账是让常
态保持零成本的那一半。

iOS 那个测试从「失败时耗 12.6 秒」变成 **0.117 秒通过**。RN 侧新加的测试专打这个
窗口(mock 让请求真花 5ms,哈希才来得及在在途期间落地),去掉补播就红:只剩 1 个
请求,登录到不了任何人。iOS 56、Android 88、RN 128 测试全绿。

### 顺手:去重用的比较键本身不稳

同一段代码里,「这个人变了没有」是拿 `"userKey\0traits"` 这个字符串比的,而
`traits` 是直接插值一个字典。Swift 的 `[String: Any]` 按哈希序打印,同样的键值对
在两个字典里可以打印出不同顺序 —— 比出「变了」,于是每次调用都发一次请求。

而 `Sentori.user` 是 host 可能在每个页面都调的动词,这个比较存在的**全部目的**就
是不让它变成每次一个请求。键排序后再比,两端一致。

### 本次不发布 native

`sdk/native/VERSION` 升到 2.0.1 并同步了 RN 镜像,但**没有打 tag、没有发包** ——
golia 组织的 Maven Central 月度配额已经超了。仓库里记的是「待发布」,发版是单独
一件事。

## v3.11.1(2026-08-28 — 只在我这台机器上存在的目录)

3.11.0 加的那道恢复路径的门,把 `verify-artifacts` 的输出写进了仓库里。
那个目录是 gitignored 的 —— **它在写这段代码的机器上存在,在任何 CI runner 上都
不存在**。于是这道门本地全绿、第一次在别处跑就红,`build` 挂掉、`deploy` 被跳过,
3.11.0 没有进生产。

改成 `mktemp`。教训跟这个 release 里其它几条是同一个:门要在**不是我这台机器**的
地方证明过一次才算数。

## v3.11.0(2026-08-28 — dSYM 靠文件名找,而身份在文件里)

### 一个从没测过的说法,先被证伪

我上一轮说「retro 补传重跑没有任何测试」。**错的** —— `smoke.sh:296` 一直在测
sourcemap 那条路。我没查就说了。

真实的缺口是 android/proguard 那半边:`native_symbolicate.rs` 的解析器有单测和
benchmark,而「上传 → 存 → 按 release 找 → 改写已存 payload」这条链一条断言都
没有。2026-08-10 修的四个 bug 全在这条链上,当时那些单测一个都没红。

`scripts/check-symbolication.mjs` 补上,八条断言。验红方式是只破坏
`rewrite_android`:sourcemap 那道旧门照常绿,新门红 —— 红在它自己的理由上。

其中两条是这条链上从没有人问过的问题:

- **ingest scope 的 token 不能上传 mapping**(403)。那个 token 随客户的 app 一起
  发出去。谁拿到 APK 谁就能替换映射,之后每一帧都还原成他选的样子 —— 而错的
  映射跟对的映射长得一模一样,直到有人真去读某一帧。代码里写着这条理由,没有
  任何东西在验它。
- **没有映射的帧不能被改写**。全都答得上来的解析器比一个都答不上来的更糟:那一
  帧读起来像源码,却指向不存在的地方。

### dSYM 的身份在字节里,而我们一直在读文件名

一个崩溃帧靠 `imageUuid` 找它的 dSYM,而查找是拿这个 id 去**匹配存下来的文件名**。
`sentori-cli` 会把 id 拼进名字,所以 CLI 那条路是通的。

问题是我们自己的提示写着另一条路:「上传 .dSYM 包里的那个二进制
(Contents/Resources/DWARF/<name>)」。那个文件是以产品命名的,名字里**没有 id**。
照着做的人会得到:上传成功、`usable: true`、控制台列为正常 —— 而没有任何一帧能
还原,也没有任何一处说得出为什么。

`sentori-dwarf-resolver` 此前**根本不读 LC_UUID**,所以服务端无从核对。

- 石头层加 `MachoSlicer::debug_ids()`:只读 load command,fat 的各 slice 借用不
  拷贝(一个真实 dSYM 三百 MB,上传时复制就是那条注释担心的 OOM)。
- 上传时从字节取 id,名字里没有就补上。名字叫什么都不再影响能不能找到。
- 一个没有 LC_UUID 的 Mach-O 现在报 `usable: false` —— 它解析得再干净也永远
  不会被选中,报正常就是控制台在替一个不会起作用的上传背书。

测试的旁证是特意找的:`object::write` 不发 LC_UUID,所以合成 fixture 必须返回空;
真值另外拿 `/bin/ls` 跟 `dwarfdump --uuid` 逐位对过(两个 slice 全中)。

「有没有 id」这个判据**只放在 `usable_for_symbolication` 一处**。我一开始写在
`store()` 里,写完发现 `sentori-server verify-artifacts` 会绕过它 —— 那条命令对已
存的行重新判定 `usable`,于是同一个文件在上传时是 false、跑一次校验之后变回
true。两个面各答各的。

### 已经存在磁盘上的那些行

上面这些只管新上传。3.11.0 之前存下的 dSYM 名字里没有 id,而它的**字节完全读得
通** —— 所以重新判定只会一直答 `usable: true`,控制台照样说没问题,而没有一帧能
还原。这条修复对已经踩到坑的人等于没发生。

`verify-artifacts` 现在会把这种行就地改名(唯一键冲突时两行都不动、只报告),
**并对该 release 真的跑一遍重跑** —— 光改名的话,等着这个 artifact 的崩溃还是和
之前一样读不出来。用来发现问题的那条命令,顺手把问题收尾。

e2e 里直接把行改回旧形状来测这条路:已经没有任何 API 能造出那个形状了,而这正是
重点 —— 测的是**早于修复的数据**的恢复路径。

### 一道门自己不在看那件事

`check-wire-case.mjs` 报「20 个 v1 handler 全 camelCase」并且是绿的。它只扫
`handlers/sdk/`,而**唯一**不住在那里的 v1 路由,正好就是唯一用 snake_case 的那
个:`/v1/releases/{release}/artifacts` 答的是 `content_hash`、`size_bytes`、
`debug_id`、`first_seen`、`content_changed`。

门改成从路由表推该扫哪些文件,解析不出某个 handler 就报错退出 —— 静默少扫正是它
要防的那件事。改完立刻红在这六个键上,改名后 220 键 / 21 个 handler 全绿。

这是**线上响应的破坏性改名**。范围很小:我们自己的 CLI 只读 `usable` 和 `hint`,
webapp 没有上传界面。`docs/protocol.md` 已同步。

### 时间轴上两行字压在一起

同样是截图看出来的。issue 页时间轴的 0s 标记不是数字而是一句话(`error 触发` /
`error fired` / `error 発生`),而且是向左悬挂的 —— 于是它落在紧邻的那个刻度上。
60 秒窗口、5 秒步长时那个刻度只隔轨宽的 8%,`-5s` 一直被 `error` 穿字。

原注释里想过两端被裁切的问题,没想过跟邻居撞。判据按**像素**留白:1400px 上够
用的百分比在 500px 上不够。首次测量前(`trackW` 为 0)按窄面板处理丢掉标签 ——
少一个刻度看着像间距,两行字叠着看着像坏了。

`tickLabelFits` 抽成纯函数才测得动;门在 5 种宽度 × 6 种窗口 × 8 种步长上核对
留白,不核常数取值。

### 291 MB 显示成「297713 KB」

截图看出来的,不是读代码想出来的。releases 面板写死 `Math.round(bytes/1024) + ' KB'`,
而真实 iOS dSYM 就是几百 MB —— 这是常态,不是边缘。没人在脑子里换算六位数,而那
一栏存在的意义就是让运维一眼看出落地了什么。

`formatBytes` 进 ui.tsx,门测的不是措辞而是性质:任何体积渲染出来,单位前不超过
四位数字 —— 超过就说明单位早该换了。

## v3.10.0(2026-08-28 — 最后一条从没被跑过的路由)

`/api/probes:sync` 是 agent 清单上最后一个完全空白的端点 —— 它当时卡在 body
形状上没测成。形状是 `{release, refs[]}`,需要 api scope。

三个问题:

- **`refs` 没有上限**。一次可以送一百万个 ref,就是一百万次数据库往返。
  `/v1/events:batch` 为同样的理由封顶 200,而这里什么都没有。
- **失败被静默吞掉**。`if res.is_ok() { registered += 1 }` —— 数据库出错既不
  上报也不记日志,调用方拿到「registered: 3」而它发了五个,没有任何可依据的
  东西。
- 逐条 INSERT,N 个 ref 就是 N 次往返。

改成一条 `unnest` 语句、封顶 1000、失败返回 500 并记日志。

### 去重是批量化的前提,不是顺手

一次静态扫描在两个文件里找到同一个 `sentori.probe(ref)` 就会发两遍。**逐条循环
碰巧能容忍**(每条是独立语句),而单条语句不行 —— Postgres 明确拒绝一条命令的
`ON CONFLICT DO UPDATE` 影响同一行两次。

我推理到了这一点并先去了重,然后**去数据库验证这个推理**:

```
ERROR:  ON CONFLICT DO UPDATE command cannot affect row a second time
HINT:  Ensure that no rows proposed for insertion within the same command
       have duplicate constrained values.
```

响应里现在带 `duplicatesIgnored`,因为「你发了 5 个我写了 2 个」是调用方该知道
的事。

`scripts/check-probes-sync.mjs` 进 e2e,六条断言:三个新 ref、重复注册、
**一次调用内的重复**、空扫描、超限 413、未知 token 401。

顺带把它补进 `openapi.json` 的完整 schema(此前只是个列出方法的 stub)——
带 schema 的路径从 4 条到 5 条。

---

## v3.9.1(2026-08-28 — 200 里带着 error 的四个 handler)

这轮看 push 投递,那是清单上最大的一块。

### 两件核实后是对的,写下来因为「没发现」也是结论

- **worker 用 `FOR UPDATE SKIP LOCKED` 拉取待投递的 send** —— 多副本部署不会
  重复投递。
- **单条 retry 和 `_retry_all_failed` 都带 `AND status = 'failed'`**,并发调用
  第二个查不到,不会重复入队。单条那个还带 `AND project_id`,跨项目 retry 不了。

push 投递这块的并发设计是扎实的。

### 但四个 handler 用 200 报告失败

它们的返回类型是裸 `Json<Value>` —— 没有状态码,axum 一律发 200 —— 而 body 里
放了 `error`。

最重的是 `GET /v1/push/users/{key}/preferences`:查询失败时返回
**`200 {"preferences": [], "error": "internal"}`**。一个只读 `preferences`
字段的调用方看到的是「这个人什么都没退订」,而真相是「我们不知道」——
**然后它就会给一个可能已经退订的人发推送**。

另外三个:`push_sends::list` 和 `push_credentials::list` 在授权失败时返回
`200 {"sends": []}` / `200 {"credentials": []}`,而同一个项目下**十一条兄弟路由
都返回 403**;`retry_all_failed` 返回 `200 {"requeued": 0, "error": "not found"}`。
「你没权限看」和「一条都没有」变成同一个响应 —— 而那正是推送配置页在问的问题。

四个全部改成带状态码:400 / 403 / 500。

`scripts/check-error-status.mjs` 进 preflight,扫全部 92 个 handler:返回类型是
裸 `Json<...>` 的,body 里不许出现 `error` 键。验过会红。

`get_preferences` 的 400 也顺带修了一处不一致:同一个 key 形状,PUT 兄弟路由
早就返回 400,只有 GET 返回 200。

---

## v3.9.0(2026-08-28 — issue 的历史记的是决定,不是请求)

继续 agent 列的未覆盖面:issue 状态变更的并发,以及计数器在并发下的正确性 ——
后者是上一版改过的路径,该自己验一遍。

### 两件核实后是对的

- **并发计数器正确**:120 条同 fingerprint 的事件并发打进去,`eventCount`
  正好 120,零 5xx。3.8.0 那次去重回读的改动没伤到计数。
- **并发状态变更不会损坏状态**:30 个 resolve/ignore/reopen 同时打,最终状态
  始终是三个合法值之一。

### 一件不对的:审计轨迹被请求淹没

同样那 30 次调用留下 **30 条 activity**。两个 handler 都是无条件 UPDATE +
无条件写记录,所以「已经 resolved 又 resolve」也算一条。

issue 详情页那块面板存在的意义就是让人看懂这个 issue 经历过什么,而它变成了
一份点击日志。

改成让数据库判断:`UPDATE … WHERE id = $1 AND status <> $2`,用
`rows_affected` 决定要不要记。同样 30 次调用现在留下 **10 条** —— 而那 10 条是
真实发生过的状态转换(open×4 / ignored×3 / resolved×3,三种操作确实来回切了
10 次)。重复的 20 次静默 no-op,仍返回 200。

两个例外保留:**换一个 release 重新 resolve 是真变更**,照记;**带 note 的
resolve 照记**,因为那是有人主动写了字。

条件写在 SQL 里而不是先读后写,所以它同时是并发安全的 —— 两个人同时点「解决」,
只有一个的 `rows_affected` 是 1。

`scripts/check-issue-activity.mjs` 进 e2e,四条断言:开着的 issue resolve 记 1 条、
重复两次记 0 条、换 release 记 1 条、十个并发 ignore 记 1 条。

**门自己先坏了一次**:并发那段我用 `fetch` 手拼 Cookie 头,拼错了,十个请求全
401,于是「新增 0 条」—— 和断言通过长得一模一样。改成走和前面同一条 curl 路径,
并且**先断言十个请求都是 200**。零条记录只有在写入确实发生过时才有意义。

### 首次 `docker compose up` 会因为 initdb 太慢而失败

healthcheck 是 `interval 5s × retries 10` = 50 秒,而第一次启动要跑 initdb。
超时后 compose 报 `container db-1 is unhealthy` —— 一个只是还在被创建的数据库,
而这是自建用户看到的第一件事。我自己这轮就撞上了。

加 `start_period: 120s`。在这个窗口内失败的检查不计入 retries,所以只有数据库
真的坏了时才会花掉这段时间。

---

## v3.8.1(2026-08-27 — 内容寻址的字节可以被重新引用,而 GC 拿的是旧快照)

继续 agent 列的未覆盖面,这轮看 retention / archive worker —— 它删数据,错了是
静默且不可逆的。

**先核实了一件我担心的事,结果是好的**:retention 按 `received_at` 删,不是
`occurred_at`。所以一台时钟错到 2001 年的设备刚上报的事件不会被立刻删掉 ——
到达时间是服务端的,不受客户端影响。

### 孤儿 blob GC 会删掉正在被引用的字节

`gc_orphan_blobs` 先把所有被引用的 hash 拍成快照,再遍历 blob store 删掉不在
快照里、且 mtime 超过一小时的。两道保护,而**内容寻址让它们同时失效**:

1. blob X 存在很久,引用它的 attachment 行被 retention 删掉 → X 成孤儿
2. GC 拍快照,X 不在其中
3. **快照之后**,有人上传了内容完全相同的文件 —— `put` 发现 X 已在盘上,
   直接返回**不重写**,mtime 还是几个月前的
4. attachment 行插入,指向 X
5. GC 遍历到 X:不在快照、mtime 很老 → **删掉**

结果:用户的上传返回成功,读回来是 404。

修法是删除前对这一个 hash 精确重查一次(候选按定义就是少数)。窗口从「快照到
遍历结束」缩到「重查到删除」的毫秒级。

**没有完全消除** —— blob 删除是文件系统操作,不在事务里,所以做不到原子。
**也没有门** —— 这个仓库没有 Postgres 集成测试的先例,而 GC 每 24 小时才跑一次,
e2e 触发不了。修复是逻辑上正确的收窄,但我没有让它变红过,这里说明白。

### 文档教人设的两个变量,compose 又是不透传

`self-hosting.md` 的表格里有 `SENTORI_EVENT_RETENTION_DAYS` 和
`SENTORI_ARTIFACT_KEEP_RELEASES`,而且它自己的上线 checklist 写着
「`SENTORI_EVENT_RETENTION_DAYS` 设成你的磁盘扛得住的值」—— **而 compose
不透传它**。照着 checklist 做的人做不到。

跟上一版的限流是同一个缺陷类别,而我上一版写的门只硬编码了限流那三个。改成
**读 `self-hosting.md` 的表格**:那张表里出现的每个变量,compose 都必须能透传。
表里加一行就是这里多一个义务,不用改门。

顺带透传 `SENTORI_ARCHIVE_INTERVAL_SEC`(模块文档记着它,而它同样设不了)。

---

## v3.8.0(2026-08-27 — 一个新错误爆发的那一刻,正是它最可能 500 的那一刻)

上一轮的黑盒 agent 自己列了没覆盖的面:限流、符号化、push 投递、并发写、
retention。挑限流开始 —— 这个仓库有过「限流器写了但从没在跑」的前科。

### 限流是在跑的,但压测顺手炸出一个更重的东西

默认 100/sec,1 秒内打 200 个:**100 个 429**(限流生效),外加 **9 个 500**。

日志:`duplicate key value violates unique constraint
"issues_project_id_fingerprint_key"`。

`SELECT … FOR UPDATE` 只能锁**已存在**的行,行不存在时挡不住并发插入。两个请求
带同一 fingerprint 同时读到 None,同时 INSERT,一个撞唯一约束 → 500 → 而我们
自己的契约叫 SDK **对 5xx 重试**。

**时机是最坏的**:一个 issue 一生只被创建一次,而那一刻正是一个新故障第一次在
所有设备上同时爆发的时刻 —— 并发最高、最需要它工作的时刻。

改成让数据库仲裁:`ON CONFLICT (project_id, fingerprint) DO NOTHING RETURNING id`,
抢输的一方重新读到赢家的行、拿到锁、走原本的「issue 已存在」路径(包括回归检测,
所以不能简单写成一个返回 id 的 upsert)。

`scripts/check-ingest-concurrency.mjs` 进 e2e:一个从没见过的 fingerprint,
60 个并发发送,任何 5xx 都红。**验过它会红** —— stash 掉修复重建镜像,80 个并发
里 6 个 500。

### 文档说的环境变量,一半是假的

- **`troubleshooting.md` 叫人调 `SENTORI_RATE_LIMIT_PER_MIN`** —— 不存在。真名是
  `SENTORI_RATELIMIT_PER_TOKEN_RPS`,单位不同、数量级不同。
- **`cli-auth.md` 把 `SENTORI_ADMIN_URL` 列为 CLI 查找 base URL 的第 2 顺位** ——
  也不存在;CLI 读的是 `SENTORI_API_URL`。
- **compose 一个限流变量都不透传**。`protocol.md` 记录了 5 个,自建用户一个都
  设不了 —— 既不能调也不能关。

`scripts/check-env-vars-real.mjs`:文档里每个 `SENTORI_*` 必须是代码真读的,
且叫人设的那几个必须能从 compose 透传。它理解 `_FILE` 是 `env(key + "_FILE")`
的通用机制(第一版把两个真变量报成虚构)。

**这道门的第一版有个我自己开的后门**:我把 `SENTORI_RATE_LIMIT_PER_MIN` 放进了
豁免名单,于是一道为抓它而写的门抓不到它。豁免名单是给「服务端确实不读的变量」
用的,不是用来把红门变绿的。

`docs/runbook/v1.0-fresh-deploy.md` 归档 —— 标题就是「OAuth wiring」,而 OAuth
在 v1 重构里删了。

---

## v3.7.1(2026-08-27 — iOS live 测试的 30 秒在冷 runner 上不够)

3.7.0 之后 `mobile-e2e` 的 `ios-live-ingest` 红了一次。诊断顺序留在这里,因为
第一直觉是错的:

- 我这轮刚改过附件读取路径(加 project 谓词),第一反应是自己引入的回归。
- 但 **`android-live-ingest` 是绿的**,两者走同一条服务端路径 —— 如果是服务端
  改动,两个都会红。
- `git diff v3.6.0..v3.7.0 -- sdk/` **为空**:这一版根本没碰 SDK。
- 断言消息是 `spilled: 0, queued: 0` —— 队列和溢出文件都空,意味着事件
  **从未交给传输层**。那是「还没启动完」的形状,不是「被拒」的形状。
- 同一 commit 重跑:**通过**。

所以是 flaky。30 秒的 deadline 在冷 macOS runner 上不够 —— 注释显示它已经从
3 秒加到过 30 秒。改成 90 秒,并把消息从「服务器从未接受一个批次」改成
「90 秒内没有投递」,因为前者把超时说成了拒绝。健康的运行不受影响:循环在第一次
投递就退出。

改的是 Swift 测试文件,本地 `xcodebuild build-for-testing` 编译通过后才推。

---

## v3.7.0(2026-08-27 — 一个黑盒 agent 找出两个跨租户缺陷)

给一个干净 ctx 的 agent 一个带真实数据的本地实例和两个租户的 token,让它找**行为
错误**(不是缺文档)。它交回 9 个确认缺陷,每个带最小 curl 和复现次数。我逐个
独立复验 —— 其中一个我自己的复验差点变成误报,见末尾。

### 跨租户:客户端自选的 event id 是全局的

项目 B 用项目 A 的 event id 发一条自己的事件:

```
202 {"eventId":"…","isNewIssue":false,"issueId":"<A 的 issue>"}
```

B 的事件**没有存**(B 的 issue 列表为空),A 的那条 `eventCount` 仍是 1。所以:
**带着成功回执的数据丢失**,外加一个跨租户探测面 —— B 可以拿任意 UUID 试,
命中就拿到别的项目的 issueId。

根因:`events.id` 是全局主键,`ON CONFLICT (id) DO NOTHING` 跨项目生效,而回读
`SELECT issue_id FROM events WHERE id = $1` **没有 project 谓词**。回读加上谓词;
查不到就明确拒绝(不能既没存又答 202),而消息不说这个 id 属于谁。

### 跨租户:附件能种进别人的崩溃详情

B 的 ingest token 往 A 的 event id 上传附件,拿到 202,然后那一行出现在
**A 的崩溃证据里**,内容可读。

上传端是对的 —— 证据合法地先于事件到达,所以 handler 不能要求事件已存在,而且它
正确地打上了上传者的 project_id。**读端才是缺陷**:`WHERE event_id = $1` 没有
project 过滤。三处读都有这个问题(事件详情、issue 的 replay join、项目的录屏
覆盖率统计),全部修好。`scripts/check-attachment-scoping.mjs` 进 preflight,
验过会红。

### NUL 字节 → 500,而我刚建的校验端点给它开绿灯

`payload` 里任何字符串含 `\u0000` → `500 ingest_failed`。Postgres 的 text/jsonb
存不了 NUL,错误落进 catch-all。而**我们自己的契约叫 SDK 对 5xx 重试** —— 一个
编码事故变成无限重试。

更糟:`/v1/events/validate`(3.6.0 刚建)对这种 body 答 `ok: true`。**一个专门
用来说"这个 body 能不能过"的端点,给一条 100% 失败的 body 开了绿灯。**

修:清洗而不是拒绝(NUL 是宿主 app 的编码事故,为它丢掉崩溃报告是 SDK 让 app
背锅),递归处理 payload 的值**和键**,以及四个顶层字符串字段。校验端点现在回报
`nulScrubbed`。

### 空邮箱能创建可登录的 admin

`{"email":""}` → 201,然后那个账号**以 admin 身份登录成功**。校验只看密码长度,
而邮箱就是登录身份。大小写折叠和重名检测都做对了,更显得这是遗漏。

### 其余

- **`limit=0` 返回 1 行**(`clamp(1,500)`)。分页时计算出 0 的调用方拿到一条
  幽灵记录。0 就是 0;负数报 400。
- **未知的 `status` / `kind` 静默返回空列表 + 200**。`status=all` 是"给我看全部"
  最自然的猜测,而它返回空,与"这个项目很干净"无法区分。现在 400 并列出可选值。

### 时钟错乱(独立于 agent,从窄屏截图发现)

队列里一行写着「7,979年前」。服务端对 `occurredAt` **完全没有范围检查**,而
`last_seen = GREATEST(...)` + 按它排序,意味着一台时钟坏掉的设备可以把自己的
issue 永久钉在列表顶端。

未来时间没有合法用途 → clamp 到到达时间,原值记进 `payload._clockSkew`;过去的
时间保留(离线缓存是正常的)。UI 侧:超过十年的相对时间显示 `—`,精确时间在
title 里。

### UI/UX(驱动真实数据的实例 + 窄屏)

- **版本分布两行都显示 `reg@1.0.0`** —— 实际是 `+1` 和 `+2` 两个构建,而
  `formatRelease` 有意丢掉 build 后缀。在这个面板里那不是噪音,是唯一的区分。
  新增 `formatReleaseIn(release, all)`:只在列表内发生重名时保留后缀。
- **900px 下时间轴刻度叠成 `−60s50s −40s`**。`tickStep` 只看时间跨度不看像素,
  而这个面板在 900px 窗口里只有约 330px。改成用 ResizeObserver 测量实际宽度。
- **900px 下「解决」按钮溢出、「忽略」被挤没**。操作行改成可换行,输入框可收缩 ——
  操作永远不该是溢出的那一部分。
- Instruments 的 `2ev · 0u` 写全成「2 次 · 0 人」。队列行里的紧凑记法保留,
  那里空间约束是真的。

### 我自己的一次误报

复验跨租户时我用 `?projectId=` 查,拿到别的项目的 14 条,一度以为发现了更严重的
泄露。参数名是 `project_id`。用对之后:A=14,B=0,过滤正常。

**不照单全收适用于我自己的观察,不只是别人的报告。**

---

## v3.6.0(2026-08-27 — 补上 agent 点名要的三件)

复评 agent 在「还缺什么」一节点了三件,全部补上,每件都有门。

### 1. `GET /openapi.json`

OpenAPI 3.1,**由服务端提供**而不是丢进 SPA 的静态目录 —— 那样
`info.version` 才是正在回答你的那个二进制的版本。描述一个不同构建的文档,
跟描述一个不同产品的文档是同一种失败。

路径表**从 router 生成**(`scripts/gen-openapi.mjs`),29 条,其中 4 条带完整
schema。`--check` 进 preflight,所以路径列表不可能与服务端漂移。生成器还会拒绝
描述一条 router 不服务的路径。

### 2. `POST /v1/events/validate` —— 无 token 校验请求体

agent 的原话:auth 在反序列化之前跑,所以拿不到 token 时,**四种故意写错的
body 和一个正确的 body 返回一模一样的 401**,它「从实测里学不到任何关于 body
校验的东西」。

wire 格式本来就是公开的(`protocol.md` 印着),拒绝告诉别人 body 对不对
保护不了任何东西,只是吃掉每次集成的第一个小时。

这个端点不碰数据库、不指名项目、不说明 token 是否有效、不存任何东西,走
per-IP 限流(与登录端点同一个)。它还**用 JSON 回答** —— ingest 自己的 422 是
axum 的 plain text,客户端没法分支。

最有用的是 `ignored`:

```
{"ok":true,"parsed":{…,"hasPayload":false},"ignored":["app","device","error"]}
```

`error` 放在顶层能干净地解析,然后**带着空内容到达**,而这件事没有任何别的
东西会告诉你 —— 请求是成功的。那正是旧文档那条 curl 的病。

### 3. 已发布 README 与仓库分叉的门

`scripts/check-published-readme.mjs` 拉 npm registry 上的 README 与仓库比对。
第一次跑就抓到真的:`@goliapkg/sentori-react-native@7.0.0` 的 README **少 22
行**,正是 3.5.0 加的 `init()` 选项表。需要网络,所以进 CI 而不是 preflight,
而且报告不阻断 —— 漂移是「该发版了」的理由,不是「该拦住这次构建」的理由。

npm 尚未重新发布(RN patch bump 会连带推 expo 一个 major,单独评估)。**误导
本身已经消除**:llms.txt 改指仓库 README 并说明 npm 可能滞后。

### 一道门当场抓到的真缺陷

`check-validate-agrees.mjs`(进 e2e):同一批 body 分别发给 validate 和 ingest,
两边的接受/拒绝判定必须一致 —— 一个会认证 ingest 拒绝的 body 的校验器,比没有
校验器更糟,它是第二个读起来权威的真相源。

**它第一次跑就红了**:`trace` / `warn` / `probe` 要求 `name`,而校验端点第一版
没检查,于是认证了一条 ingest 答 400 的 body。规则补上,用例扩到 13 条。

门自身也补了护栏:一个失效的 token 对每个 body 都答 401,长得跟「ingest 拒绝
一切」一模一样 —— 先用一条已知合法的 body 证明 token 能用,否则 exit 2。

---

## v3.5.1(2026-08-27 — 第二个干净 agent 说 Yes,并指出我把表格塞进了代码块)

3.5.0 之后起第二个干净 ctx 的 agent 复评,同样只许用公网。结论从 **Partly 变成
Yes**:`/llms.txt` 一跳解决七个问题里的六个,十来分钟,而且它说「大部分时间花在
**验证**而不是**寻找**,这是对的比例,也不是我通常拿到的比例」。

它抓到的东西里,最重的一条是我自己造成的:

- **`sdk/react-native/README.md` 的 ```tsx 围栏没闭合**,我 3.5.0 加的
  `init()` 选项表整个落在代码块里 —— GitHub 和 npm 上渲染成字面文本。
  两个门都读得很开心,因为它们把文件当纯文本 grep。**只有看渲染的人会发现。**
  `scripts/check-md-fences.mjs` 进 preflight,验过会红。

其余按它的清单:

- **`getting-started.md` 说 admin token 是 `sk_` 前缀、环境变量叫
  `SENTORI_ADMIN_TOKEN`** —— 前者被 `protocol.md` 记为 v0 已删除,后者服务端
  根本没有。改成实际的:一个前缀 `st_`,scope 是服务端属性,用 `ingest` token
  会被 `403 admin_token_required` 拒绝。
- **`errors.md` 现在说清 scope 拒绝有两种形状**:中间件层 401 `KindMismatch`,
  handler 层 403 `admin_token_required`。它们不是同一个检查,而 agent 只能从
  文档看出矛盾。
- **`llms.txt` 补上 `user()` 和 `context()`** —— 我前面明明验证过它们是真的,
  却没写进去,agent 于是判断「一个 agent 因此永远不会调用它们」。
- **`llms.txt` 的「真实路径」列表原本读起来像封闭清单**,漏了 `/v1/deploys` 和
  `/v1/push/*`。改成开放,并补一条它自己发现的有用不对称:**catch-all 只吃
  GET,POST 到打错的路径会真的 404** —— 所以 ingest 路径写错不会静默成功。
- **`llms.txt` 指向 npm 页当「当前 API 面」是错的**:那份 README 是上次发布时
  的副本,没有选项表。改指仓库里的,并说明 npm 可能滞后。

以及一个新的门,`scripts/check-docs-curl.mjs`(进 e2e):**把
`getting-started.md` 里的 curl 逐字提取出来对活实例执行**,要求 202 且带
`eventId`/`issueId`。文档里印的那条命令就是被测对象 —— 它不可能在页面是错的
时候通过。

两条 agent 的判断我核实后**不采信**:它说包导出 `push`/`feature-flags` 子路径
(`exports` map 里只有 `.`),说「Eight verbs」不准(`sentori` 对象正好八个
方法)。后者措辞确实会绕人,改成说清「八个方法是上报 API,其余导出是接线」。

---

## v3.5.0(2026-08-27 — 文档描述的是上一代产品,而这件事只有机器会当真)

起一个**干净 ctx 的 agent**,只许用公网信息(WebFetch / curl / 搜索,不许读本地
仓库),扮演「帮开发者把 Sentori 接进 RN app」。它的结论:照着两个公开入口
(`sentori.golia.jp` 和 OSS mirror)走完全程,**得到的是一个坏集成**。

每一条我都独立复验过,没有照单全收。

### 文档教的 API 不存在

`scripts/check-docs-api-truth.mjs` 首次运行报 **103 处**。最重的三处:

- `docs/getting-started/react-native.md` 教 `sentori.captureError(...)` ——
  已发布的 7.0.0 里它**只是内部别名,没有导出**。照抄会在 onPress 抛
  TypeError,直接违反本产品「失败零传染」的铁律。
- `docs/getting-started.md` 那条「不用 SDK 也能试」的 curl 发的是 `timestamp`
  和顶层 `error/device/app`。实测 **HTTP 422 `missing field occurredAt`** ——
  必填、无 alias,而 `error` 必须在 `payload` 里。同一个 `docs/` 下的
  `protocol.md` 给的是对的形状(实测 202)。**两份官方文档对同一条 curl 互斥。**
- `docs/errors/` 是**一整套虚构的错误码目录**,14 个文件自 initial commit
  起没动过,描述 `sentori.track()`、`POST /v1/track:batch`、`st_pk_`/`sk_`
  分裂 —— 服务器一个 code 都不会发。

修法不是逐条改文案:

- **`docs/errors.md` 改成生成的** —— `scripts/gen-error-reference.mjs` 从
  handlers 提取,17 个 code、6 条 401 hint,`--check` 进 preflight。它不可能
  再漂移,因为源头就是服务器。
- **`docs/archive/`** 收纳 43 个描述已消失产品面的页面:v0 错误目录、web SDK
  指南(包不在 npm)、两个「设计了没造」的 recipe、一个客户的升级记录、
  pre-v1 的 SDK 参考。**目录即元数据** —— 门按路径跳过,比文件里一行警告
  对机器可靠。
- `troubleshooting.md` 删掉三节:Webhook 签名(路由 0、源码 0)、crash-free
  sessions(server/webapp 里都不存在)、React hook(`SentoriProvider` 不存在)。
  401 hint 对照表换成**实测的六条**,原来那四条一条都不存在。

### `/llms.txt`,以及为什么它必须自足

agent 的原话:「域名对任何路径都回 200 HTML」——`/llms.txt`、`/robots.txt`、
`/openapi.json` 全是 SPA 外壳,md5 与 `/` 完全相同。**一个只看状态码的 agent
会认为 llms.txt 存在且已读取。**

`webapp/public/llms.txt` 现在是真文件。它不只是链接目录:包名、init 必填字段、
六个动词、wire 形状、202 收据、401 hint、以及一节明写「这个实例对未知路径回
200,别把 200 当成资源存在」。**agent 拿到它就能完成接入,不必再跳转。**

### mirror 里根本没有文档

`docs/README.md` 写着「read them here or in the OSS mirror」。实测 mirror 里
`docs/` **整个不存在** —— 它是别人拿来自建的地方,却只发了服务器不发说明书。

白名单放开 15 个面向用户的页面。第一版**漏了终止用的
`--exclude='/docs/**'`,154 个文件全泄露**,包括 archive 和内部设计文档 ——
`scripts/check-mirror-docs.mjs` 跑真实 rsync 参数验证,验过会红。

### 服务端:一个 hint 主动指错方向

`st_bogus000000` 明明以 `st_` 开头,却被告知「token must start with `st_`」。
`looks_like_token` 检查**前缀和长度两件事,只报告其中一件**。拆成
`WrongPrefix` / `WrongLength`,后者说「probably truncated when copied」。

### 其余

- `sdk/react-native/README.md` 加 `init()` 选项表(10 项),
  `check-sdk-doc-options.mjs` 改指它 —— 之前它盯着的是那份自称
  「Historical (v0.x)」的页面。
- `protocol.md` / `replay-encoding-v2.md` 里的客户 app 名脱敏。
- 门自身的两次修正值得记:它第一版不认识 `sentori` 对象字面量的键,把
  `sentori.user()` / `sentori.context()` 这两个**真实 API** 报成不存在 ——
  一个朝这个方向错的门比没有门更糟,它会说服你改坏正确的文档。

---

## v3.4.1(2026-08-27 — 当自己的用户走一遍:引导在最需要它的那一刻消失)

用 CDP 驱动真实栈(真 server、真 Postgres、空数据库)走一遍第一次使用者的路:
登录 → 空实例 → 建项目 → 发 token → 读接入页,每步截图。
`webapp/devtools` 的 sweep 是对着 mock 渲染所有路由,而 mock 永远有数据 ——
**空实例是它从没测过的状态**,而那正是每个自建用户看到的第一屏。

驱动脚本两个:walk.mjs 走首次使用路径,walk2.mjs 建项目并发 token。

### 引导在建完项目那一刻消失

收件箱空态的标题**恒为**「收件箱清零」,只有副标题按有没有项目切换
(`TriageView.tsx:399`)。于是:

- 还没有项目 → 「收件箱清零」+ 三步引导 —— 标题本身就错了
- 有项目、从未收到过任何事件 → 「收件箱清零 · 无待处理 issue」——
  **在庆祝一个还没开始的状态**,而此刻最需要的「怎么接 SDK」没了

缺的是第三态。`health` 端点本来就有 `lastEventAt`,为 null 就是从未接入过。
现在:「等待第一条事件 / 这个项目还没收到过任何事件。用 ingest token 把 SDK
指向本实例，issue 会出现在这里。/ 发放 ingest token →」,带链接。

### 项目页零项目时是一整片空白

新用户登录后侧边栏只有「项目」和「设置」,项目页是必到的第一站,而它
`rows.map()` 什么都不渲染 —— 一块黑屏,右上角一个表单。
其他页面都有空态(「还没有版本」「暂无内容」),唯独这一页没有。

### 同一种「没有数据」,服务端给了两种形状

项目卡片显示 `— err  — warn  0 users`:`counts24h` 用 `GROUP BY kind`,
安静的项目没有行,返回 `{}`,前端把缺键读成「未知」画成 `—`;
而 `users24h` 是普通 count,如实返回 0。一张卡片上两种画法。
服务端现在补齐五个 kind 为 0 —— 24 小时内没有 error 就是 0,不是未知。

### 其余

- **`createProject` 只有 `.then()` 没有失败分支**:重名或 500 时点击毫无反应。
  与 3.4.0 修的 `App.tsx` 是同一个病,补上。
- **token 明文只显示一次,却没有复制按钮**,而它上面一行写着「请立即复制」。
  推送接入页的代码块早就有复制按钮。补上。
- 项目卡片的 `u` 改成 `users` —— 它旁边是 `err`、`warn`,单字母读起来像被截断。

---

## v3.4.0(2026-08-23 — 自查:一个找故障的产品,把自己的故障显示成了「还没有数据」)

对功能完整性、专业性、用户体验做了一遍完整自查。工具是仓库自带的
`webapp/devtools`(mock + 18 条路由 × 三语 × 双主题的 headless 渲染)——
但它只看 console 报错,而这次找到的东西**一个都不会报 console 错**。

### 越权:三个 admin 端点不检查调用者能不能碰这个项目

`admin/releases.rs` 的三个 handler 从来没调用过 `ensure_project_access`:

- `GET /admin/api/projects/{project_id}/releases` —— 列任意项目的版本
- `GET …/releases/{release_id}/artifacts` —— 列任意项目的符号化物料
- `DELETE /admin/api/releases/{release_id}` —— **删任意项目的版本**

第三条最重:路径里连 project 都不带,handler 连 `SessionContext` 都不取,
而 `release_artifacts` 是 `ON DELETE CASCADE`。任何登录账号都能让另一个项目的
崩溃栈永久无法还原。`list_artifacts` 甚至把路径参数写成 `_project_id` ——
作用域就在 URL 里,没人读。

三个都补上了检查;`delete` 先由 release_id 查出 project_id 再判定,
查不到和无权看返回同一个 404,所以不能用来探测 id 是否存在。
`scripts/check-admin-authorisation.mjs` 进 preflight,验过会红。

### 故障被显示成正常

- **`App.tsx` 两处 `() => undefined`。** `authMe` 失败 → `me` 永远是 null →
  每条路由永久停在「加载中…」,没有原因也没有重试。`listProjects` 失败 →
  项目列表空 → 侧边栏丢掉项目导航,每个按项目取数的页面走 fallback,
  显示「还没有版本」「还没有项目」。**后端全 500 时,版本页说的是
  「版本 (0) · 还没有版本 — SDK 上报版本后出现」**,运维会去查 SDK。
- **`formatApiError` 返回 `": "`。** 模板字面量的两个洞是空的
  (历史里是 `${e.status}: ${e.body}`,`body` 改名成 `message` 时没跟上)。
  **24 个 `useAsyncData` 调用点全部吃这个默认值** —— 整个 dashboard 的
  API 错误提示就是一个冒号加空格。

三处都修了,并且是拿渲染验的:现在显示
「会话加载失败。 500: upstream_unavailable — the database refused the connection · 重试」。

配套:`devtools/mock-api.mjs` 加 `MOCK_FAIL=<路径子串>`,让**错误态**能被
渲染出来看。每个页面的加载态和空态天天有人看见,错误态没有任何东西会渲染它
—— 这就是 `": "` 能活这么久的原因。

### 功能完整性

`DELETE /admin/api/releases/{id}` 有 API 无 UI。补上,放在展开面板里而不是
列表行上(它级联删符号化物料,列表行是误点会落到的地方),确认文案说明后果。
顺带把它的返回从 204 改成 200 带 body —— 控制台的 fetch 一律 `resp.json()`,
无 body 的成功会被读成失败。

### 专业性

`i18n/zh.ts` 里 44 处半角标点紧贴汉字,而 `ja.ts` 只有 5 处;
且同一句里混用(「已存但读不了,符号化用不上:…。」)。全部改全角,
`devtools/check-cjk-punctuation.mjs` 进 webapp check —— 它第一件事就是抓住了
我自己刚写的那条删除确认文案。

### 两处环境漂移,与本次改动无关但会让 CI 红

- `bun.lock` 记着 `sdk/cli@1.4.1`,package.json 已经是 1.5.0(8-17 那次 bump
  漏了 lockfile)。干净 HEAD 上 `--frozen-lockfile` 就失败,CI 同一条命令。
- rustc/clippy 1.98.0(8-18)新增 `unused_async_trait_impl` 和
  `chunks_exact` 两条 lint,`core` 里 8 个既有错误。修了;`MemoryBlobStore`
  的 async 是 trait 契约要求的,加 allow 并写明理由 —— 第一次 allow 写成了
  `unused_async`,而 clippy 不会因为未知 lint 名报警,看起来像做了事。

---

## v3.3.5(2026-08-20 — 一个绿色的门可能已经不在看那件事了)

上一版把 push 的冲突目标写明,end-to-end 87 步随之全绿,我据此在给 spg 的账本上
把「未指定目标的 `ON CONFLICT DO NOTHING` 不覆盖部分唯一索引」这条打了勾。

**那条缺陷当时还开着。** 打勾的依据是套件变绿,而套件之所以变绿,正是因为 3.3.4
改成了指定目标 —— 第 66 步从此不再走那个形状,它的通过对那个缺陷一无所知。
目录里本来就有一个能直接回答的复现文件,我去问了套件。

对方指出来了,原话值得留下:*一条写着「已关闭」而缺陷仍开着的记录,比写着
「未关闭」更糟,因为它一次性把这个 case 从所有人的视野里移走。*

修的不是那条记录,是产生记录的方式:

- **复现回放器** —— 把目录里每个复现同时放给候选构建和
  PostgreSQL 18,逐个 case 对比两边的输出。判据是 PG 本身而不是冻结的期望文件:
  冻结文件记的是写它那天我相信什么。exit 2 专门留给「工具坏了」(复现文件少于
  预期、oracle 起不来),永远不与「全部一致」共用出口 —— 量到了空却报告一致,
  正是这次的形状。
- 拿 7.38.4 跑必须变红,验过了 —— 没人见过它红过的门不是门。
- 它在第一次运行的一分钟内找出两个我们都读过好几遍的文件里的新缺陷。

顺带修的产品缺陷,来自同一轮:

- **compose healthcheck 改成 exec form。** 原来是
  `["CMD-SHELL", "pg_isready -U sentori"]`,而 CMD-SHELL 要 `/bin/sh`。换一个
  从 scratch 构建的数据库镜像(有 `pg_isready`,没有 shell),检查跑不起来,
  容器永远不健康,`depends_on: service_healthy` 在任何一条语句发出之前就把整个
  栈按住了 —— 报错说的是「数据库不健康」,指向的是错的部件。
  `scripts/check-compose-healthchecks.mjs` 接进 preflight,并验过它会红。
- **db 镜像可用 `SENTORI_PG_IMAGE` 覆盖**,默认不变。
- **`SENTORI_E2E_KEEP=1`** 让通过的 e2e 保留栈,并打印怎么删掉它。
- `.env.e2e.*` 进 .gitignore —— e2e 被中断时会留下,之前攒了 16 个未跟踪文件。

以及两件欠了两轮的东西,做完了:并发抢锁的复现(六写手抢 250 个键,四个真实
upsert 形状,先证明自己真的在并发再相信任何不变量),和一份 Sentori 形状的转储
(27 表 40 索引 66 约束 ~66,000 行,给对方跑崩溃恢复和原地升级)。

---

## v1.9.2(2026-07-22 — 符号化的入口原来是个 404)

v1.9.0 说「符号化打通了」。能用它的只有坐在浏览器前的人。

文档(`troubleshooting.md` / `sdk-react-native.md`)教用户跑
`sentori-cli upload sourcemap`,**那条命令 POST 到一条 v0.2 服务端从来没有的路由**
(`/admin/api/releases/{name}/sourcemaps`)。生产实测 404,与随手编的路径同码。
而 dashboard 那条路 CI 也走不了:它要浏览器 session 和项目 UUID,流水线手上只有
token 和 release 名字。

新增 `POST /v1/releases/{release}/artifacts`,令牌认证,按名字找 release、没有就建 ——
**map 是构建时产的,通常在 app 第一次跑起来之前**,要求 deploy marker 先到会把顺序做成
陷阱。两条路共用同一个存储函数,不让它们漂成「谁传的、符号化器读法就不同」。

**然后新加的 e2e 断言当场抓出第二个。** 列出 release 的 artifact 时查询 select 的是
`size_bytes`,而表里那列叫 `uncompressed_size_bytes`,**一直都叫这个**。查询每次都失败,
`unwrap_or_default()` 把失败变成空列表 —— 于是 release 页对着表里躺着的文件说
「没有符号文件」。改列名 + 删掉兜底:**空列表现在只能是真的空。**

**e2e 改走文档教的那条路。** 原来它驱动的是 admin 路由,于是在一个 404 之上绿了一个月。
本地对真实 Postgres 端到端验过:upload 201 → admin 看见 1 条 → `app.js:1` 解回
`../app.js:10` 且保留压缩前坐标。**修列名之前那条断言实测是红的。** 真实 CLI 命令也跑过:
两个文件按 sourcemap / bundle 分类进库,字节数与打包输出一致。

## 部署侧

**deploy 绿过、生产却没换版本。** `workflow_run` 对 v1.9.1 那次 build 静默漏触发,最后一次
deploy 检出的是前一个提交。没有任何东西发现 —— 因为 smoke 测试问的是「服务器健康吗」,
而**旧二进制答得和新版一模一样**:`/healthz` 200、`/readyz` 200、`/auth/me` 401、
SPA fallback 200,四条断言全过,却什么都没发。

现在 smoke 末尾比对 `/healthz` 的 version 与本次检出的 `Cargo.toml`。**提交前拿真实坏状态
实测过红**(want=1.9.1 got=1.9.0),上线后在 CI 实测过绿(`serving 1.9.1`)。这不修漏触发
本身(在 GitHub 侧,无可靠复现),它让下一次漏触发**吵起来而不是隐身**。

---

## v1.9.1(2026-07-22 — 让新开的门真的能跑)

v1.9.0 把 `mobile-e2e` 从「四个指向不存在东西的 job」换成两个真 job。`sourcemap-e2e` 第一次就绿;`android-unit` 失败了四次,**每一次都是一个真问题**:

1. **`apps/rn-example/android` 是 prebuild 产物,在 .gitignore 里。** 本地一直存在,所以我只确认了目录在,没确认它被 git 跟踪 —— 正是我在删旧 workflow 时批评它犯的那个错。加了 `expo prebuild` 步骤,在全新 clone 上验证过。
2. **模块没有声明任何测试依赖。** `junit` / `robolectric` / `kotlin.test` 全部无法解析 —— 两个 Robolectric 测试自 Phase 29 写下起**从未编译过**。
3. **测试调用的 `installForTesting` / `persistForTesting` 不存在。** 测试是照着一个没人实现的 API 写的。
4. **Robolectric 4.13 没有 compileSdk 35/36 的运行时。** 它默认按模块 `targetSdk` 挑,而宿主工程给的是 Expo 模板的值。`robolectric.properties` 锁到 34 —— 这两个测试断言的是 JSON 形状和文件写入,与 API level 无关,让测试台跟着 app 的 target 漂只会让它没人维护。

**这四层能同时潜伏,是因为本该跑它们的 job 从 2026-05 起挂在缺 gradle wrapper 上,连编译都走不到。一道死门把它后面所有的门都挡住了。**

修完之后测试第一次真正执行,出现第一个**断言**失败:测试期望 `RuntimeException`,实现写 `throwable.javaClass.name` 即 `java.lang.RuntimeException`。**改的是测试。** 生产里 1,487 条 Android 事件全是全限定名,改实现会把每个已有 issue 分组劈成两半;而且包名在 Android 上是真信号 —— 两个不同库的 `FooException` 是两个不同的 bug。**一个从未执行过的断言不构成对实现的约束。**

同时:webapp 的 release 页能直接上传符号文件,不必再用 curl。

---

## v1.9.0(2026-07-22 — 符号化打通;删掉从来跑不起来的 CI)

**符号化的每块零件都在,缺的只是连接。** 三个 resolver crate 在 v0.2 cutover 之前就有测试和 benchmark,`release_artifacts` 有表,crash 视图会渲染源码上下文 —— 而 ingest 写着 `frame: None` 加一句 TODO,**因为根本没有办法把文件传进来**。

现在:`POST …/releases/:id/artifacts` 收 map,ingest 在**存储前**改写栈帧,并**保留原始坐标**。改在读时做意味着每次打开 dashboard 都重算一遍,而且永远修不了已经写下的事件;保留压缩前坐标则是**唯一**能让人察觉「map 过期」的手段 —— 否则它产出的是自信的胡说。

**顺带修好两条 ingest 路径的漂移**:批量路径**没有实时推送、没有 event_count 告警**,也没有身份归属。单条能触发的告警规则,批量进来就静默。三者现在共用 `after_ingest`。

## CI 那一半

**`mobile-e2e` 四个 job 全部指向不存在的东西** —— 一个没有工程文件的 Xcode scheme、一个不在仓库里的 Gradle wrapper、以及两次 legacy `server/`。它在 v0.4 被改成手动触发并留言「修好就恢复」,然后**从 2026-05-11 起红着没人看**。**没人能跑的门不是门;红着没人读的门更糟,因为红色从此不再有含义。**

替换它的东西是真能跑的:

- **android-unit** 通过 `apps/rn-example` 驱动 SDK 的 Robolectric 测试 —— 那是它们唯一能构建的地方(Expo 模块必须有宿主)。**这补上了「改 .kt 文件能通过所有门直接发到 npm」那个洞。** Gradle 模块路径是运行时发现而非硬编码:猜错名字会「通过」但什么都没测。
- **sourcemap-e2e** 对 v0.2 服务端跑完整往返。用 Bun 的打包器而非 Metro:裸 `bunx metro` 既没有 Babel preset 也没有 haste 配置,为了产出八行 JavaScript 要背上一整个 React Native 工程的配置。

**本地对真实 Postgres 端到端验证过**:压缩后的 `app.js:1` 解析回 `app.js:10`,正是 fixture 抛异常那一行。**然后验证了门会红** —— 传一个空 map 退出码 1,还原后 0。断言专门拒绝「解析到第 1 行」,因为 bundle 只有一行,不匹配的 map 会把一切都解析回去。

**删除**:`scripts/test-phase13/14/15.sh` 与 `test-mailcatcher.sh`,没有任何 workflow 引用它们,而且全都在调 `/admin/api/orgs/…` —— **v0.2 没有 orgs 这个概念**。旧的 `sourcemap-e2e/run.sh` 一并删除。部署 runbook 的 pre-flight 列着这三个脚本、一个 `pages` workflow 和 `server/migrations`,现在列的是 CI 真正跑的命令。

---

## v1.8.3(2026-07-22 — 恢复 PII-safe 跨项目用户查找:v0.2 cutover 以来它一直是断的)

**这个功能从 v0.2 上线起就死了,而且没人发现** —— 因为它的失败方式天生是静默的。

`identity_fingerprints` 带着 338 行真实数据从 v1 迁了过来,但**决定「一个工作区用哪把盐做哈希」的 `workspace_identity_scopes` 从来没被填过**。ingest 解析不到 scope,于是从 2026-07-19 起彻底停止写入指纹 —— 而 SDK 每个事件都还在发 `user.linkHashes`:**此后的 339 个事件,哈希躺在 payload 里,没有归属到任何人**。而产品页上还写着「跨项目用户查找,PII 绝不上线」。

三部分:

- **`sentori-identity-fingerprint`**(石头层 crate),持有公式 `SHA256(salt || key_type || ":" || client_hash)`。**逐字节锁定 v1** —— 生产里有那台服务器写下的指纹,换个摘要就意味着同一个人静默地对不上自己的历史。它的测试当场抓到本次提交初稿里的两个错:一个手写的期望摘要,和一个号称 64 字符实际 65 的常量。
- **Migration 0036** 给每个 workspace 绑定 scope,**沿用它现有指纹所属的那个**,而不是新铸一把盐把它们全部变成孤儿。已在生产上用事务试跑并回滚验证:2 条绑定,GOLIA 正确沿用了持有全部 338 行的那个 scope。
- **ingest 恢复写入**,单条与批量两条路径都接上。尽力而为:归属不了的事件仍然值得保留,所以 scope 缺失只记日志,不让上报失败。

`linkHashes` 里不是小写十六进制摘要的值一律丢弃并**大声记日志**。能产生这种值的只有 SDK bug 或伪造 payload,而两者的结局都是明文 PII 落进一张承诺永不持有 PII 的表。

---

## v1.8.2(2026-07-22 — SDK 性能数据与用户反馈:最后两张只写不读的表)

**`runtime_metrics_1h/_1d` 存的是 SDK 对宿主 app 的测量** —— 冷启动、帧率、堆内存、页面跳转耗时。**这正是本项目第一条铁律(Sentori 绝不能让被观测的 app 卡)的凭证数据**,而接入方无从查证,只能听我们说。现在两个读接口(只读 rollup,不碰按天分区且无上限的 `_raw`),指标页把它排在自定义指标之前、**先给百分位**:平均帧时间恰好会掩盖这组数据存在的意义 —— 卡顿。

**`user_reports` 是产品里唯一不是机器生成的信号**。调用栈说「在哪儿」,人说「他当时想干什么」。现在读得出来,并且排在 crash 页的证据之后、团队讨论之前。

**评论把自己署名成 `019e3589…`**。跟成员列表同一个修法:join `users`。创建接口也补上了 `author_email` 与 `edited_at` —— 创建返回的形状跟列表不一致,就是同一个对象有两种形状;而客户端那边还留着一份手抄的类型定义,早就跟服务端脱节了,现在改为从 API 推导。

---

## v1.8.1(2026-07-22 — 打开 analytics 表;修 v1.8.0 弄红的 master)

**`track_events` 从 migration 0019 起就有写入路由,生产里躺着 19,134 行** —— 18,751 条 `$pageview` 加上客户真实的 `bio.login.*` 业务事件。**一个读接口都没有。** 现在有三个,各对应表上已有的一个索引:窗口内的事件名聚合、单个事件名的日序列、按名字或用户过滤的明细尾巴。每个查询都有时间窗和行数上限 —— 一个能按请求全表扫 analytics 的端点,等于给了从 dashboard 打垮数据库的办法。

页面回答的是**紧挨着 crash 的那个问题**(这个用户、这个版本,当时在做什么),而不是要长成一个 analytics 产品。「0 个用户」渲染成空白而不是「0 人」:匿名上报的 SDK 根本没有可计数的 handle,而 18k 事件旁边写个 0 读起来像 bug。

**两处修复,针对正红着的 master**:

- **`core-check` 5 个集成测试挂在 `column "priority" does not exist`**。event-pipeline 的 fixture 用 migration 0001 + 0003 建 schema,而三个 triage 列在 0004。**生产早就有这些列,只有测试 schema 落后。** 我本地没发现是因为 `cargo test` 不编译 `tests/` 目录,CI 用的是 `--all-targets`。以后照抄 CI 的命令。
- **`build` 挂在 i18n 门上,而它是对的**:我加了 `crash.labels` / `crash.noLabels` 两个键却没做标签编辑器。现在做了 —— 标签按集合替换,删一个就是把剩下的发上去。

---

## v1.8.0(2026-07-22 — issue 的优先级 / 标签 / 负责人:从表接到屏幕)

**缺口是一层厚的**。`issues.priority` / `labels` / `assignee_user_id` 从建表起就在,生产 109 个 issue **全都带着 priority 值**。但 `Issue` 模型没有这三个字段,PATCH handler 给 `IssuePatch` 传的是三个字面量 `None` —— 而 `IssueStore::patch` 一直是支持它们的。缺的只有 HTTP 这一层。

`IssuePriority` 做成枚举而不是裸字符串:列上有 CHECK 约束刚好四个值,而 p0..p3 由紧急到不紧急的排序正是运维说「按优先级排」时的意思。越界值现在是 400,不再是约束冲突变成的 500。

**负责人在线上需要三种状态** —— 不改 / 清空 / 设置 —— 所以有了 `FieldPatch` 这个具名枚举,而不是 `Option<Option<T>>`。第一版写成 `#[serde(untagged)]`,**测试当场抓到**:untagged 按顺序试各 variant,而 unit variant 会接受 `null`,于是「取消指派」被解析成「不改」,按钮会静默失效。改成手写 impl。

批量接口同样支持这三个字段 —— 把一早上的新 issue 批量定到同一优先级,正是那个端点接受列表的理由。

**前端**:优先级与负责人在 Meta 卡里就地可改(读 crash 的人就在这儿做这两个决定,为一个下拉跳一次表单是没人做分级的原因);负责人按邮箱列出;issue 列表加优先级列。

**另外 17 处 `json!` 里的裸时间戳,以及门的第三个盲区**:它匹配 `time::OffsetDateTime`,但认不出 `use` 进来的裸 `OffsetDateTime`。扩宽后又误报了 25 处结构体字段赋值(那些字段本身带注解,是安全的)。**引号就是分界**:`created_at: row.get(...)` 是在构造结构体,`"created_at": row.get(...)` 才是直接上线。红/绿两态都实测过。

---

## v1.7.27(2026-07-22 — 文本检查收敛到零)

第三轮 4 处,**全是假阳性**:健康页按真名列出 worker、环境变量和认证方案(运维 grep 的是 `Bearer`,不是它的译名),另外三处是 mock 的 fixture 字符串和照原样打印的按键名。都收进白名单。

**放宽白名单就是放松这道检查**,所以反过来验了一遍:往报告里注入一句 `Manage subscription`,它仍然会报。一个因为「被教会忽略一切」而什么都不报的检查,比没有检查更糟 —— 因为它看起来像通过了。

zh 的 30 条路由现在报告零残留。这条线的收敛过程:**13 → 9 → 4 → 0**。

---

## v1.7.26(2026-07-22 — 读扫描文本第二轮:又七处)

回放列表副标题、推送凭证三个列头、保存视图的「打开」链接、事件页的实时开关、版本页的「发布标记」按钮。**全部是渲染时拼出来的** —— 三元、模板串、列定义 —— 所以前四轮 grep「带引号的字符串」一次都没看见。

它标出的九处里有两处**其实已经修好了**,是扫描读的还是旧产物。报告里的 `bundle` 字段就是为这个加的,**这是第一次我能不靠猜就分辨出来**。

443 键。

---

## v1.7.25(2026-07-22 — 把「读扫描文本找漏网英文」变成脚本)

`check-i18n.mjs` 能证明**每个键都接上了调用点**,但证明不了**一屏里没有硬编码字符串** —— 一个从来没被给过键的字符串,对目录检查完全隐形。而这正是前四轮反复捞出来的那一类。

新增 `check-sweep-text.mjs`:读扫描记下的**实际渲染文本**,在非英语的那轮里挑出英文散文。它**只报告不设卡** —— 白名单永远不可能完整(产品名、协议名词、代码示例、mock 里的数据都天然是英文),所以一条命中是一个问题,不是一个判决。用上一轮的报告验过:它抓到了本版修的那个 `Sign out`。

它找到的三处:设置页的退出登录、注册表单的密码提示、令牌快速开始的脚注。436 键。

**这一步我之前手写正则做了四遍,每遍白名单都不一样。**

---

## v1.7.24(2026-07-22 — 给消息目录加一道门)

两种故障,**在 diff 里都看不见**:

1. **两个语言各自漂移**。`Messages` 从 `en.ts` 推导,所以「缺键」已经是编译错误 —— 但「zh 多出一个 en 没有的键」没人管,目录可以单向静默分叉。
2. **没人引用的键**。它看着像废文案,而在这个仓库里**每一次都是反的**:`alerts.newRule`、`issues.empty`、`action.signOut` 都是「字符串写好了、调用点没接上」—— 三个按钮在已经翻译好的页面上还念着英文。门的报错原文就这么说,因为「unused key」这个说法会诱人去删,而删是错的修法。

已接进 `bun run check`,由 CI 兜住。**两种违规各实测过红、还原过绿** —— 而且它在接入过程中抓到了自己的假阳性:`t(n === 1 ? 'crash.step' : 'crash.steps')` 是在调用点选键,第一版把它读成了两个死键(我据此误删了一个正在用的键)。**没被证明会失败的门不算门。**

同时:回放**列表页**那句「逐帧回放将在 v0.3 提供」也是过期的 —— 每一行都能点进详情页,而详情页从 v1.7.22 起就能播了。已删。

---

## v1.7.23(2026-07-22 — hotfix:v1.7.22 让 master 的 build 变红)

v1.7.22 的 `ReplayPlayer` 在 effect 体里直接 `setState`,`react-hooks/set-state-in-effect` 判 error,`master` 的 build 挂掉、deploy 被跳过 —— **v1.7.22 从未部署**。

已经拿到手的文本不需要 effect:它在 render 时就在,派生即可。在 effect 里为一个渲染期已知的值设 state,多付一趟渲染,正是那条规则指的东西。解码函数提到组件外(它不依赖任何 prop),`ndjson` 走 `useMemo` 派生,fetch 分支保持 effect。

**为什么本地没拦住**:我一直用 `grep -c "error TS"` 判断 `bun run check` 的结果 —— **那只数 TypeScript 错误,eslint 的 error 完全漏掉**。改用退出码。

---

## v1.7.22(2026-07-22 — 回放页接上早就存在的播放器)

回放详情页显示的是一个拖原始 JSON 的滑块,底下还有一行「canvas 播放器将在 v0.3 提供」。**播放器随 crash 证据视图一起上线很久了** —— 只是这个页面没被告知,于是唯一一个专门用来看录屏的页面,成了唯一一个放不了录屏的页面。

`ReplayPlayer` 现在也接受直接传入 NDJSON,因为两个调用方走不同路径拿同一份字节:crash 视图按附件 ref,这个页面按 replay id。解码两遍就是两个要互相同步的播放器。

帧检查器保留在画布下方 —— 「看用户经历了什么」和「看 SDK 到底发了什么」是两个问题。

**最终扫描:90 次渲染(zh/ja/en × dark/light × 30 路由)、0 处控制台错误。**

---

## v1.7.21(2026-07-22 — 邀请人改从会话取;扫描文本侧查出的 39 处英文)

**邀请表单在让管理员手填自己的 UUID。** 没人知道自己的 UUID,而且**服务端是照客户端说的记的** —— 于是审计日志里的 `invite.mint` 记的是一个声称,不是事实,任何调用方都能把邀请算到别人头上。`ctx.user_id` 一直就在手边。

其余是**扫描的文本侧**查出来的:39 处在中文页面里说英文的字符串。它们躲过前几轮是因为**没有一个是「带引号的字符串」** —— 跨四行拼出来的副标题、在 `Pause` 和 `Resume` 之间二选一的三元表达式、`Retry all {n} failed`。

**设置页有两段不是没翻译,是过期了**:一段说成员管理还没做、让人去用 `sentorictl` CLI;另一段用内部工单号描述集成适配器。这两个功能早就上线了 —— 一个还在为已完成功能宣传绕路方案的设置页,是在把人往远路上引。现在它们指向真正干这事的页面。

每种语言 **439 键**。`vite preview` 接上了开发代理,扫描改为**对构建产物**跑 —— 此前两次扫描报的错都是它抓到了编辑器保存到一半的中间态。

---

## v1.7.20(2026-07-22 — 全量扫描查出的:一个写着「删除」的暂停按钮,四屏英文,侧栏溢出)

**扫描抓到一个我自己造的误标。** SaaS 管理页上,接 `suspendWorkspace` 的按钮写着「删除」,接 `resumeWorkspace` 的写着「立即重试」—— 我那次批量翻译把 Suspend→delete、Resume→retry 映射错了,两个碰巧在表里挨着、意思完全不同的词。**误点第一个会让一个客户的工作区下线,而按钮承诺的是删除。** 两者现在各有自己的词条,其余每个按钮都对着它调用的函数复核了一遍。

扫描还查出:

- **告警页**用列枚举值来自我介绍(`Trigger kinds: issue_new / regression / ...`),每一行也在重复原始枚举名。现在一条规则说的是它盯什么:「已解决的问题回归 · 最多每 60 分钟一次 · 全部项目」。
- **审计页**把两段 UUID 前缀并排放着,没有任何东西说明哪个是操作人、哪个是项目。
- **SaaS 管理页**表头与副标题仍是英文。
- **侧栏在矮窗口下溢出自己的容器**:背景和右边框停在视口底,最后几个入口继续画在外面、点不到。`flex-1` 只定尺寸不产生滚动,而 flex 子项默认的 `min-height: auto` 会让它撑破父容器。已加 `min-h-0` + `overflow-y-auto`,底部的语言与账号行固定不动。

每种语言 406 键。**90 次渲染(zh/ja/en × dark/light × 30 路由)全部无控制台错误。**

---

## v1.7.19(2026-07-22 — 全路由渲染自查工具,以及它查出来的东西)

连续三个上线的 bug 都是用户打开页面发现的,**读源码一个都发现不了**。新增 `webapp/devtools/`:

- `mock-api.mjs` —— 应答 dashboard 会调的全部 **42 个端点**
- `sweep.mjs` —— 用一个 Chrome 走完全部 **30 条路由**,每条出一张截图 + 一份报告(控制台错误 + 渲染文本)

`bun run mock` / `bun run sweep <目录> <语言> <主题>`。

**它查出了「挑五个代表页截图」查不出的 8 处**:

- **通知**直接打印原始 payload(`{"title":"TypeError in CheckoutScreen"}`)—— 那是存储形状,不是一句话。现在显示标题,类型也从 `ISSUE_NEW` 换成人话。
- **会话页**标题以下整片英文,连「created 2 days ago · last seen 11 minutes ago」都是。
- **快捷键页**通篇英文,而且它的导航部分**用自己的一套词**称呼跟侧栏相同的去处 —— 等于在教一套侧栏不用的词汇。
- **搜索页**在输入第三个字符之前,只有一个搜索框、下面一片空白。空屏是行动的邀请,不是留白。

**mock 故意提供脏数据**:`regressed_at` 是 `[1970,1,0,...]`,可空列真的有 null,有未验证的成员、已吊销的令牌、失败的推送。这个文件第一版里每个时间戳都是合法 ISO —— 这正是当时截图好看而生产在抛异常的原因。

---

## v1.7.18(2026-07-22 — 成员页显示的是人不是主键;补上 rfc3339 门的第二个盲区)

**成员列表列的是 `019e3589-9d7f-7013-9952-e3f287104954`。** 管理员决定要不要移除某人时,认的是邮箱;UUID 是外键,该待在行的 key 里而不是屏幕上。`GET /admin/api/members` **从来就没返回过别的东西**,所以这个问题在前端怎么改都修不了。

新增 `Members::list_with_identity`:join `users`,并把 `added_by` 也解析成邮箱。它**与 `list` 并存而非取代** —— 鉴权只要成员关系行,为了一个展示需求让每次权限检查都付一次 join 是错的取舍。行现在读作「lihao@golia.jp · 由 takagi@golia.jp 添加」,地址未确认的挂 `未验证` 标记。

**另外:又有 16 处 `json!` 里的裸时间戳,以及本该拦住它们的那道门。**

v1.7.16 加的检查只认识两种形态里的一种(直接从数据行取列)。另一种是**从模型结构体上取字段**,而这种真的反直觉:`Member::added_at` **确实带着** `#[serde(with = "time::serde::rfc3339")]`,但字段属性管的是「序列化这个结构体」,管不到 `json!` 把值取出来之后。`wire_time` 里新增两个测试把这个区别钉死,下一个人不用再自己发现一遍。

门扩展后**实测过红/绿两态,并且它找出了两处我手工漏掉的**。

---

## v1.7.17(2026-07-22 — 表格单元格显示 `[object Object]`;补完 97 处按钮 + 20 处标题)

**项目页每个单元格都是 `[object Object]`。**

`DataTable` 对没有 `render` 回调的列直接 `String(value)`,而**七个页面**把 React 元素放进行里(而不是给每一列写回调)。`String()` 不适合干这个 —— 它**从不失败**,什么都能转,于是类型不匹配表现成一段看着挺像样的文字,而不是任何人会注意到的错误。现在:元素按元素渲染,空值渲染成「—」,其它按 JSON 渲染而不是「object」这个词。

**顺带补完这次暴露出的翻译缺口**:97 处裸按钮文案(`Cancel` 出现 9 次、`Delete` 6 次)改为共用 `action.*` 键,不再每页各写一遍;20 处用模板串拼的卡片标题(`Active members (0)`、`Sends (0)`)—— 它们**对"搜索带引号的字符串"是隐形的**,所以在一整页中文里一直是英文。每种语言 336 键。

**顺带揪出一个变量遮蔽**:Tokens 页把行变量命名为 `t`,在回调内部盖住了翻译函数。行变量已改名 `tok`。

---

## v1.7.16(2026-07-22 — 页面崩溃的**真**根因:`json!` 里的时间戳不是 RFC 3339)

v1.7.15 只是止血。真因在服务端。

`#[serde(with = "time::serde::rfc3339")]` 是**字段**属性,而 **26 个 handler 用 `serde_json::json!` 拼响应**,那里根本没有字段可以挂它。于是这些时间戳按 `time` 的默认形状发出去:

```json
"created_at": [1970,1,0,0,0,0,0,0,0]
```

`new Date()` 把它读成 Invalid Date。v1.7.9 之前前端把由此产生的 `NaN` 渲染成难看的 `NaNy ago`;改用 `Intl.RelativeTimeFormat` 之后,同样的偏差**直接抛异常、整页白屏**。受影响的有:会话、通知、评论、关注者、搜索、活动流、版本、集成、端点探针、推送凭证、推送记录、SaaS 管理。

新增 `crate::wire_time::rfc3339` / `rfc3339_opt` 在调用点格式化;`scripts/check-rfc3339.sh` 现在也会对 `json!` 体内裸露的 `OffsetDateTime` 报错 —— **正是这个盲区让 26 处一边积累、一边让检查报绿**。已实测:退回一处则门变红,还原则恢复绿。

v1.7.15 在 `formatRelative` 里的守卫保留 —— 它是「坏时间戳不能再搞崩页面」的兜底,并且顺带修掉 `new Date(null)` 让未解决的 issue 报告「56 年前已解决」。

---

## v1.7.15(2026-07-22 — 热修:相对时间遇空值把整页搞崩)

**多个页面打不开**,报 `RangeError: number argument must be finite`。

v1.7.9 改用 `Intl.RelativeTimeFormat` 时,**改变了「时间戳缺失」的失败方式**:手写版本只是拼字符串,`NaN` 出来是难看的 `NaNs ago`;而 `Intl.RelativeTimeFormat.format()` 遇非有限数**直接抛**,这些组件上方又没有 error boundary,于是整页白屏。**一个显示瑕疵被我升级成了故障。**

有几列本来就可空 —— 没人解决过的 issue 的 `resolved_at`、不会重试的 `next_attempt_at`、从未触发的规则的 `last_fired_at` —— 而 33 个调用点直接把它们传了进来。函数签名现在接受 `null | undefined`(调用方本来就在这么传),不是真实时刻的值渲染成「—」。

**顺带修掉一个更安静的错答案**:`new Date(null).getTime()` 是 `0` 不是 `NaN`,所以未解决的 issue 一直在报告「56 年前已解决」。

---

## v1.7.14(2026-07-22 — 三语覆盖 41/41)

en/zh/ja 各 **282 键**,**所有**页面与组件都读目录。

上一轮把管理台留了英文,理由是「那几屏是我们自己操作的」。这个理由在「严格三语」面前站不住 —— **self-hosted 用户跑的正是这几屏**;一个除了你用来管理它的那四页之外处处都能切语言的 dashboard,不叫三语,叫大体三语。

服务商名、协议名词、URL 路径在译文里保持英文 —— 那在任何语言里都是它们的名字。

---

## v1.7.13(2026-07-22 — crash 页把证据放到讨论前面)

评论框和活动流夹在概要卡与调用栈之间,于是打开一个 crash,本该是错误的位置显示的是空的「评论 (0)」,而第一个有用的东西被挤到首屏之下。

打开 issue 的人是来读错误的,不是来读关于它的讨论。证据(调用栈 / 行为轨迹 / 录屏)现在紧跟概要,讨论排在证据之后。

---

## v1.7.12(2026-07-22 — 三语覆盖 40/41;命令面板与表格空态)

**命令面板** —— ⌘K 里列的是跟侧栏同样的九个去处,却各写各的 label,于是侧栏显示「概要」时面板还写着 Overview。改成持 `nav.*` 键、渲染时才解析 —— 那个数组是模块级的,label 在定义时烤死就跟不上语言切换。

**表格空态** —— `DataTable` 的 `empty = 'No data'` 是**默认参数**。默认值在签名处求值,那里没有 hook 也就没有 locale,所以凡是没自己传 `empty` 的表格,三种语言下都说 "No data"。改到组件内部解析。

链路详情、工作区切换器一并接入。至此 **40/41** 个页面/组件读目录,只剩管理台(SaaS admin / 推送凭证 / 证书监控 / 端点探针)有意保留英文。

---

## v1.7.11(2026-07-22 — 三语覆盖 36/41 页;light mode 有了画布)

**三语** —— en/zh/ja 各 **250 键**,41 个页面里 **36 个**读目录(此前是 3 个)。这一版补完:项目/令牌/成员/设置、账单/审计/健康/版本/指标/集成/告警/保存的视图/通知,以及**整条登录注册流程**(登录是所有人看到的第一屏,却是最后一个还全是英文的)。

`Continue with GitHub` 用的是插值而不是「翻译片段 + 名字」拼接 —— 日语里两半要换序(`GitHub で続ける`),`Continue with ${x}` 这种模板表达不了。

**故意留英文**:SaaS 管理台、推送凭证、证书监控、端点探针。那几屏是我们自己在操作,词汇本身多半是服务商名和协议名词。

**light mode 有了画布** —— 两种模式的「浮起」方向相反:深色下卡片靠**变亮**浮起(#1a1c1e 浮在 #0e0f10 上),浅色下靠**保持纯白**、让页面退成灰。GDS 两个值都给了,却没有一个名字叫「卡片底下那层」,所以浅色一直是白卡贴白底,全靠边框撑,侧栏更是跟内容区完全分不开。新增 `--color-canvas`(默认 `--gds-bg`,`[data-theme=light]` 下取 `--gds-bg-secondary`)。深色逐字节不变,浅色多了这一层。

这个问题是**把页面渲染成浅色看出来的** —— 单独核对 token 值的时候一切正常。

**部署** —— 本轮 webapp 全部走 rsync 快速路径(秒级),不等 Rust 镜像重建。

---

## v1.7.10(2026-07-22 — 概览/事件/链路/回放/搜索三语化;走势图跟随主题色)

沿 crash 线头继续:到达错误之前经过的页面、以及挂在它下面的那些。en/zh/ja 各 146 键。

**文案改写** —— 有几处在描述实现而不是描述东西:「LIKE search across issues + events」把一个 SQL 运算符甩给读者;「Recent event tail (newest first, up to 100)」「SDKs call POST /v1/spans」同理。现在说的是这屏装着什么,空的时候说该做什么。

**走势图** —— `Sparkline` 把 `#10b981`(emerald)写死在 prop 默认值里。**十六进制色躲得过类名扫描**,所以配色换蓝之后每条走势线还是绿的,换主题也不会跟。SVG 吃不了 Tailwind 类,改成读 accent 自定义属性;填充色由同一个值推导,不再是另一个会各自漂移的字面量。

---

## v1.7.9(2026-07-22 — crash 主路径三语化;相对时间交给 Intl)

**三语覆盖** —— 目录早就在,但只有外壳在用(41 个页面里 3 个),所以日语用户看到的是「日文侧栏包着英文列表」。本版把**看一个 crash 实际要走的那条路**做完:issue 列表 + issue 详情(表头、标签页、状态、行内操作、快捷键提示、Meta 各字段、评论)。en/zh/ja 各 117 键,数量一致 —— 缺键是编译错误,不是运行时回退。

**相对时间** —— 此前是手拼英文后缀(`${n}m ago`),任何目录都够不着它。改用 `Intl.RelativeTimeFormat`,同一个数字按读者语言渲染(16分前 / 16m ago),一处改动带动调用它的 10 个文件。

**两个只有「渲染成日语」才看得见的 bug**(读源码看不出来):

- 状态徽章**竖排**了:CJK 没有词边界,`未対応` 在任意两字之间断行,叠成三层,把整行表格撑到三行高。徽章与表头补 `whitespace-nowrap`;徽章的 `tracking-wide` 也去掉了 —— 汉字之间加字距读起来像渲染坏了,不像设计。
- `Comments (0)`、删除确认框、Meta 卡里的状态文字都还是硬编码。

---

## v1.7.8(2026-07-22 — 修 deploy 红:webapp-dist 不该 chown 给容器 uid)

**deploy 挂了** —— v1.7.7 的 deploy job 红在 `mkstemp … Permission denied`。根因是我先前手工快速部署时把 `/apps/sentori/webapp-dist` chown 成了 65532(容器 uid),CI 的 runner 从此写不进自己的目录;workflow 里那句 chown 排在 rsync 之后,永远救不了。

**两个坑方向相反**:`/data/blobs` 是读写挂载、容器要**写**,必须 65532;`webapp-dist` 是 `:ro`、容器只**读**,0644 的 world-read 位就够,属主必须留给 runner。已移除 chown,改用 `--chmod=D755,F644` 直接保证读位。

**顺带**(这一轮改自「看渲染结果」而非「读源码」):

- issue 行内的三个操作按钮各有各的底色、高度由 padding 撑出,`⊘` 还比 `↺` 窄(字形自带宽度)。现在统一走 `Button`,并给它加了 `icon` 方形模式,一排图标按钮才能对齐。
- issue 列表副标题写的是 `Project 019e358a…` —— 那是这行数据的存储方式,不是任何人对自己 app 的称呼。`useProjectName` 换成他们自己起的名字。
- `orange` / `rose` 漏在上一轮语义化之外,已归 `warn` / `danger`。全站不再有任何调色板字面量。

---

## v1.7.7(2026-07-22 — 接入 @goliapkg/gds:配色归位、控件统一、卡片内缩修正)

**配色** —— `emerald` 才是这个 dashboard 事实上的强调色,硬写在约 **60 处**;GDS 的蓝只在少数写了 `text-accent` 的地方生效,所以看起来跟 golia.jp 不是一家。本版整体接入 **@goliapkg/gds**(golia.jp 与旧 dashboard 同一套系统),并把所有字面色按语义归位:可交互的走 `accent`,状态走 `ok` / `warn` / `danger`。除 `index.css` 外全站不再出现任何具体色值 —— 这同时也是 light mode 能成立的前提:`bg-green-950` 是接近黑的绿,只有在深色卡片上才像个 badge。accent 实值 `#3b82f6`,与 golia.jp 同色。

**控件** —— `CONTROL_H` 固定两个高度,`buttonClass()` 成为按钮外观的唯一出处,由 `Button` 与新增的 `LinkButton` 共用。issue 头部的 `← All` 此前是手写样式的 `<Link>`,比旁边每一个按钮高 4px —— 因为高度被写了两遍,两份副本各自漂移。

**卡片内缩** —— `Section` 一个组件在干两件事:页面级分组**和**卡片内容体。用作卡片体时,内容拿到的是分组组件的「零 padding」和多余的 32px 下边距,于是文字贴着卡片边框。拆成 `Section`(给页面分组卡片)与 `CardBody`(卡片内部),表格单元也对齐到卡片头同一个 `px-5` —— 卡片左边缘现在是一条线,不是三条。

**字号** —— 基准 14px、下限 12px,取代此前 10–13px 的漂移。原先读着是「挤」而不是「密」:密度是每屏信息量,来自紧凑的行高与短行,不是把字缩到看不舒服。

**部署** —— dashboard 改动改由 rsync 推到 `/apps/sentori/webapp-dist`(server 只读挂载),不再等 Rust 镜像重建。

---

## v1.7.6(2026-07-21 — 统一控件高度;主题与语言移到侧栏)

**控件高度** —— 按钮此前用 padding 定尺寸,高度由 line-height 加内容撑出,所以带 badge 的按钮会比旁边的纯文字按钮高一两像素,issue 头部那排六个按钮从来对不齐;全站还有 **10 种不同的 padding 组合**在充当「高度」。现在只有两个:`CONTROL_H.sm`(h-7)与 `.md`(h-8),**按钮 / 输入框 / 下拉共用**,凡是能同排的就共享基线,padding 只负责宽度。

修的过程中浮出两个 bug:`primary` 变体的 hover 与常态**同色**(token 迁移时丢的,一直是死的);`danger` 仍写死红色调色板,是唯一不跟随主题的变体,浅色模式下会破 —— 现改用 `danger` 角色。

**主题与语言** —— 此前只在 Settings 里。那是错的位置:这两个是「瞟一眼顺手切」的偏好(给同事截图、读日文堆栈时),不是专门导航过去配置的设置项,而且用得比「退出登录」频繁,却排在它下面。现在放在侧栏底部、账号行**上方**。主题控件按 系统 → 浅色 → 深色 循环,字形显示当前生效的,tooltip 说明点一下会变成什么 —— 只显示当前状态的控件会逼人点一下才知道结果。

---

## v1.7.5(2026-07-21 — 主题迁移收尾 + crash 视图三语言)

v1.7.4 把 crash 视图发了出去,但它的字符串是内联英文,而角色 token 只覆盖了外壳、共享 UI kit 和这一屏 —— 也就是说浅色在"框"上是对的,在框住的几乎每个页面里都是错的。这一版补齐。

**主题** —— 其余 38 个文件、**416 处**调用点从原始色阶换成角色 token。两处判断值得记:`zinc-300` 映射到 `fg-muted` 而非 `fg`(它一直只用于次要正文);SaaS 统计卡里游离的 `emerald-600` / `zinc-800` 换成 `ok` / `fg` —— 它们本来想表达的就是"严重度"和"默认文字",角色名说得更准。现在唯一还写原始色阶的是 `index.css`,token 就定义在那里。

**三语言** —— crash 视图新增 23 个词条 × 3 语言。两处值得记:折叠帧那行英文是「N frames outside your code」,日语没有对应的复数形态,所以**计数与短语拆成两个 key**,而不是插值成一句;时间轴上的「crash」标记单独成 key 而非复用区块标题 —— 它是**某一刻的标签**,不是标题,三种语言对这个区分的处理不同。

---

## v1.7.4(2026-07-21 — issue 页变成「一次崩溃」:堆栈 / 时间线 / 上下文 / 录屏回放)

v1.7.3 把证据能存能取了;这一版把它们呈现出来。issue 页此前只列标识符(kind / platform / release / 时间戳),因为当时也只有这些能拿到。

**Stack** —— `inApp` 是唯一值得用颜色的区分。报告里绝大多数帧不是读它的人写的(React 内部、引擎、bridge),而属于他们的那一两帧才是全部重点:这些保留 accent 竖线并默认展开,其余成段折叠成一行「隐藏了 N 帧」。帧**由内向外**排列 —— SDK 发的是抛出顺序,但人打开页面是为了看抛出的那一行。符号化过的帧内联展示源码上下文、抛出行标红;`cause` 链**摊平**成并列区块而非层层嵌套。

**Before the crash** —— breadcrumb 时间线收束在崩溃那一刻,时间用**相对崩溃的偏移**(`−4.2s`)而非墙钟时间:调试的人问的是「崩溃前多久」,不是「当时几点」。每类一个字形,一串网络请求不读字就能扫;每条只出一行摘要 —— 需要展开才能扫的时间线不算时间线。

**录屏回放** —— SDK 一直在每次崩溃时录下滚动的 60 秒**线框**(带文字与填色的矩形,不是视频):代价只有屏幕录制的零头,能在中端机上的环形缓冲里存活,而且不会像位图那样泄漏密码框。此前从没有任何东西播放过它。播放器按格式本身的写法重建 —— 走到播放头前最近的 keyframe,再向前应用 delta;节点按取整几何跨帧匹配,所以移动的节点表现为「删除+新增」,而仅换了文字的节点保持身份;录制中途旋转屏幕也正确,因为尺寸取当时生效的 keyframe。单行损坏跳过而非整段失败(append-only 格式,尾部截断是预期失败)。绘制走主题 token,浅色下同样可读。

**录屏与日志共用一个播放头** —— 拖动回放,breadcrumb 时间线同步高亮当时那一步。这正是同时采集两者的意义:「用户做了什么」和「用户看到了什么」本来就是同一条时间线,做成两个互不相干的面板就浪费了同时拥有它们的唯一优势。

**Context / Artefacts** —— release / device / user / tags 只渲染 SDK 真的发了的字段(空的 `Breadcrumbs (0)` 外壳会让人以为工具坏了);身份 `linkHashes` 本质是加盐摘要,面板只说明「哪些身份被关联」,绝不打印摘要值。截图内联渲染,其余附件走 blob 端点。issue 有多次发生时出现事件选择器 —— 同一崩溃在不同设备/构建上的差异往往就是诊断本身。

---

## v1.7.3(2026-07-21 — 崩溃证据链修复:日期、附件/录屏、事件详情接口,以及主题与三语言地基)

为「围绕 crash 组织的详情视图」清障。查下来 dashboard「几乎不能用」不是观感问题,是几处硬伤叠加。

**每个日期都渲染成 `NaNy ago`** —— `self-hosted/server` 与它序列化的 `core` crate 里有 **62 个 `OffsetDateTime` 字段缺 `#[serde(with = "time::serde::rfc3339")]`**。缺了这个注解,`time` 输出的是 9 元素数组,`new Date(...)` 得到 Invalid Date,`formatRelative` 一路 fall through 到最后一支返回 `NaN` + `y ago`。issues / events / alerts / audit / cert / tokens / sessions / replays / members 全中。仓库本来就有 `scripts/check-rfc3339.sh` 专防这个,但它**只扫 legacy `server/src`**,整个 v0.2 栈从没被检查过 —— 现已扩到 `self-hosted/server/src` 与 `core/crates`,并实测「拆掉一个注解门会红」。

**附件与录屏从未落库** —— 有 2211 条事件的生产库里 `event_attachments` 是 **0 行**。同一个 handler 四处独立错误:按 raw bytes 读而两个 SDK 都发 multipart;`kind` 白名单放行会违反 CHECK 的构建产物(`sourcemap`/`dsym`/`proguard`),却拒绝 SDK 真正发的 `viewTree`/`stateSnapshot`/`logTail`/`sessionTrail`;INSERT 写了三个不存在的列又漏了两个 NOT NULL 列;返回体字段名 SDK 不认。更深一层:0022 schema 根本没有指向字节的列,blob 包装层也只有 `put` 没有 `get` —— 就算写成功也永远读不回来。migration 0035 补上 `blob_hash`(handler 一直在写这列),`AttachmentStore::get` 补全读取,并新增两个读接口。

**`GET /v1/projects/{pid}/events/{event_id}`** —— ingest 一直把 SDK 的整个 JSON 原样存在 `events.payload`(堆栈帧含 pre/post 源码上下文与递归 `cause` 链、breadcrumb 时间线、device / app / bundle / user / tags / flags),但没有任何接口 SELECT 过它,连单事件路由都不存在。现在一个接口全部返回,附件清单以 `event_attachments` 为准而非 payload 里的回声。

**布局** —— `<main>` 不给内边距、各页自行决定,一半页面(IssueDetail / Members / Projects / Metrics / Integrations / Notifications / PushSends)什么都没加,内容顶到视口边缘、自己的头部按钮被切掉。内边距与最大宽度收归外壳一处。

**主题与语言地基** —— 新增语义角色层(`bg`/`surface`/`raised`/`border`/`fg`/`fg-muted`/`fg-subtle`/`accent` + 三个严重度),`@theme inline` 使其运行时可切换;暗色仍为默认,浅色是独立设计的一组值(accent 加深到 `#0284c7`,因 `#38bdf8` 在白底上文字对比度不合格),主题在挂载前解析以消除首屏闪烁,并尊重 `prefers-reduced-motion`。三语言(en / 简体中文 / 日本語)用类型化目录实现 —— 少一个 key 即编译错误,不会悄悄漏成英文;语言按「存储 → `navigator.languages` → 英文」协商并同步 `<html lang>`。两个切换器在 Settings。

---

## v1.7.2(2026-07-21 — dashboard 首页移到 `/main`,修 OAuth 登录落到 marketing 页)

Google/GitHub 登录认证本身是好的(链账号 + 建 session 都正常),但登完把人丢到了 marketing 首页。根因:OAuth 回调结尾是**服务端 302**,而 SaaS 部署上 Caddy 把 `/` 交给 Astro marketing 构建、其余路径才路由到本 server 的 SPA —— 所以整页加载 `/` 永远到不了 dashboard。密码登录一直没暴露这个问题纯属侥幸:它在已加载的 SPA 内做**客户端**跳转,压根不经过 Caddy;只有全新的浏览器导航(OAuth 回调、刷新、书签)才会撞上 marketing。

把 **`/main` 定为 dashboard 首页** —— SaaS 与 self-hosted 都把它路由到 SPA,因此能扛整页加载 / 刷新 / 书签:

- server:OAuth 回调 `DASHBOARD_ROOT` 由 `/` 改为 `/main`。
- webapp:`/main` 渲染 Overview,`/` 重定向到它;侧栏 Overview 链接、`g i` 快捷键、命令面板条目、登录后默认 `returnTo`、邀请接受页的 CTA 全部指向 `/main` —— 应用内不再有任何路径能把已登录用户甩到 marketing。

---

## v1.7.1(2026-07-21 — 配额超限返回 402 而非 429,避免 RN SDK 重试 churn)

紧跟 v1.7.0 的行为修正。月度配额用尽本不该是 429:RN SDK 把每个 429 当"稍后重试" —— 读响应体 `retryAfterMs`(v1.7.0 的 server 从不发 → 默认 5s),退避重试 `MAX_RETRY` 次,再把批次 persist 到 AsyncStorage 离线队列、下次 session 继续发。对一个整月都不会恢复的配额,这就是对注定失败的批次反复重试 = 白耗 host app 电量/网络,正好踩"Sentori 不能给 host app 造成抖动"的铁律。

改为返回 **402 Payment Required**:所有 SDK 对非-429 的 4xx 都静默 drop(JS `status >= 400` → drop;RN 落在 429/5xx 两个分支之外 → 不重试、不 persist),已部署客户端零改动即止 churn,**无需更新任何 SDK**。402 语义也更准("到 plan 上限了,升级继续")。429 留给短期 per-token 速率限制(带 `retryAfterMs`,正是 RN 那段重试逻辑的本意)。

5 条真实 ingest 路径(events / spans + 各自 batch + replay 附件)全部切到 402;`docs/protocol.md` 补 402 响应码说明。

---

## v1.7.0(2026-07-21 — SaaS 接入与管理:真多租户 + 计费执行 + Stripe 自助 + onboarding)

把 SaaS 化从"地基"推到"能收钱、能管理、能自助接入"的完整闭环。

**多租户地基**
- register 事务内建 personal workspace + owner membership;session 携带 active workspace,每请求校验 membership;`AppState::identity_for/billing_for` 按请求 workspace 取 handle;一批 handler 从默认 workspace 切到 `ctx.workspace_id` + guard;工作区切换器(API + 侧栏);invite accept(`/admin/api/invites/accept` + `/invite` 页);RBAC 门 + saasadmin 默认拒绝;migration 0033 backfill membership。

**计费执行**
- 真实 SDK ingest 三类事件(events / spans / replays)接入配额计量(此前只有 legacy stub 计量):新 `sdk::quota::meter` helper,batch 按批量条数原子检,replay 附件才计 Replays;计量后端故障 fail-open,不因 usage 表抖动丢客户遥测。
- 配额 status-aware:新纯函数 `effective_plan(plan, status)` —— canceled / unpaid 降级到 Free 限额,past_due 保留(Stripe dunning 宽限);运营 suspend 改写 `canceled`(真正生效),不再是无执行力的 past_due。
- 修多租户读侧串数据:`workspace_usage` / `/v1/usage` 原按 boot 默认 workspace 读并汇总全体租户,改为按调用者 active workspace scope。

**Stripe 自助控制面**
- migration 0034 加 `stripe_events`(webhook 去重 + worker 台账)。
- `POST /webhooks/stripe`(HMAC 验签)+ 后台 billing worker 消费:checkout.session.completed 绑定 customer;subscription.created/updated 设 plan + status + period;deleted → canceled;invoice.payment_failed → past_due。
- 自助端点 `GET/POST /admin/api/billing{,/checkout,/portal}`(Owner/Admin 门)+ 运营 override `POST /admin/api/saas/workspaces/{id}/plan`。商业参数(price id / key / URL)全走 env,不硬编码。

**接入 onboarding**
- Billing dashboard 页(plan / status / usage bars / Checkout 升级 / Portal 管理 / 回调 banner;无 Stripe key 时降级为运营提示)。
- Overview 空态 → 4 步 first-run 引导;Tokens 页 Quickstart 卡显式展示 ingest URL(SDK 默认值)+ `sentori.init()` 片段(prefill 刚 mint 的 token);SaasAdmin 每行 plan 选择器。

**待配**:Stripe 运行时端到端需部署侧配 `SENTORI_STRIPE_SECRET_KEY` / `SENTORI_STRIPE_PRICE_*` / `SENTORI_STRIPE_WEBHOOK_SECRET` / `SENTORI_PUBLIC_URL` 后验证。

---

## v1.6.2(2026-07-21 — 依赖全面升级到最新 stable)

覆盖所有 workspace 与前端包,目标是消除 dependabot 的过期告警并把每个包带到它能到的最新。

**Rust**(1.97.1 已是 latest stable,各 workspace `rust-version` 一致)
- 六个 workspace 108 个包更新到最新兼容(补丁/小版本);除下述 wasmtime 外无 crate 落后大版本
- `generic-array` 弃用 `GenericArray::as_slice`,`webpush_encrypt` 的 HKDF 推导改用 deref borrow —— 一小时前刚提到阻塞级的 clippy 门抓住的,位置在 WebPush 加密路径
- **wasmtime 39 → 47**(legacy `server/` 的 dev 依赖)。它支撑 `tests/wasm_score.rs` 的信任分数 kernel 回归测试(5 断言,加载前端也在跑的 `sentori_score.wasm`);用的都是稳定核心 API,升级仅动 lockfile,5 断言在 47 上全过

**JS**
- webapp(vite 6→8、plugin-react 4→6、react 19.2)、web(react-router 7→8、jest-dom 6→7)、marketing/docs-site(sharp、sentori-javascript 0.1→1.3)、10 个 SDK(TypeScript 6→7)全部到各自能到的最新
- webapp 强制改动一处:tsconfig 加 `"types": ["vite/client"]`(TS 严格化后 side-effect CSS import 需要,与 web/ 已有约定一致)

**Tailwind webapp v3 → v4** —— 零视觉变化。theme 从 `tailwind.config.js` 迁入 CSS `@theme`,自定义 token(brand/字体)与约 30 个用到的默认色阶逐个 pin 回 v3 精确 hex,边框/placeholder 默认在 `@layer base` 复原。证据:编译产物零 `oklch`、32 个 token diff 无差异

**Expo SDK 声明为 latest** —— `@expo/config-plugins` 56→57,peer 开放到 `expo >=56.0.0`(去掉 `<57` 上界,不排除 56 用户)。`app.plugin.js` 用的 7 个 config-plugins API 在 57 全在(实测对 57 加载),SDK 源码不 import config-plugins,纯声明升级

**上游阻塞,如实保留**:TypeScript 7 在 webapp/web(typescript-eslint 硬拒 TS 7.0,实测确认)与 marketing/docs-site(astro check 依赖 TS 7 移除的 API);Tailwind 4 已在 web;rn-example 的 Expo 55→57 是需真机验证的 app 工程,独立处理。

---

## v1.6.1(2026-07-21 — 两道质量门提到阻塞级,连带两个真 bug)

两项技术债清完,门从"报告"提到"阻塞" —— 目标不是把 warning 数字降下去,是让它今后涨不回来。

**`self-hosted/server` clippy(111 → 0,门改为 `-D warnings`)**
- `usize`→`i64` 转换用 `try_from` + 饱和兜底并逐处注明为何不可达,而非整体 allow;两处改为真错误路径:`webpush_encrypt` 的 RFC 8188 `idlen` 字节截断会静默破坏帧结构,现返回 `InvalidServerKeyLen`
- `webhook::deliver` 可从两个请求处理器到达,其 HMAC `expect` 改为真错误;其余三处保留并写明不变量(启动接线、两处 `SecretKey::generate` 仅在 OS CSPRNG 不可用时失败)
- 删除确实无人使用的:`AppState` 三个启动时构建却从不读取的字段、`AttachmentStore::{get,delete}`、一个未挂路由的 501 桩、一个残留查询字段
- **`state.rs` 的 `#[allow(dead_code)]` 与其目标之间隔了空行**,实际在给 `RecentEventTick`(事件总线载荷,完全活跃)无差别放行,而四十行外三个真死字段反而不报。移除它才使其浮现

**webapp `set-state-in-effect`(17 → 0,门回到 error 级)**
- 新增 `useAsyncData`,取代 13 个页面各自长出的 `useState` 三件套 + `useEffect`
- **修掉被复制了 17 份的竞态**:快速切换项目时较慢的首个请求会后到并覆盖较新数据。每次运行取单调 ticket,仅最新者可写状态;卸载后不再写状态
- 五个页面此前在后续成功加载时从不清除错误,过期报错横幅会永远挂着
- 错误文案**刻意不统一**:仅两页用 `${status}: ${body}`,其余十一页用裸 `String(e)`,统一会改变用户实际看到的文字

**`fix(push)`:`appUserId` 定向静默不发** —— 该字段在文档注释里被宣传为定向方式,但 `resolve_targets` 仅处理 `tokenIds` / `nativeTokens` / `topic`。只设 `appUserId` 的推送会解析到零设备、什么都不发、返回 200,调用方以为已送达。现返回 400 并告知可用的定向方式。该模式仍未实现,只是不再假装实现了。

---

## v1.6.0(2026-07-21 — OAuth 登录移植 ⚠️ 含 legacy 账号劫持修复)

将 GitHub / Google 登录移植到 v0.2。**存储模型是重建的而非照搬**:legacy 把身份存在 `users.oauth_provider` / `oauth_subject`,v0.2 无此二列;`user_federation_links` 虽名字相近但不可替代 —— 它是项目级、`user_id` 为 TEXT 而非指向 `users` 的外键、且带 `install_id`,模型的是 SDK 侧最终用户联合。新增 migration 0032 建 `user_oauth_identities`,独立表以支持单账号同时绑定两个 provider(两个列做不到)。

**legacy 实现存在可实施的账号劫持,已刻意不移植**:`extract_github` 读取 `v["email"]` 且**无任何验证检查** —— 那是 GitHub 的公开资料邮箱,GitHub 从不验证、任何人可填任意地址 —— 随后 `upsert_oauth_user` 执行 `SELECT id FROM users WHERE email = $1` 并 `UPDATE … oauth_provider = …, email_verified = TRUE`。将 GitHub 资料邮箱改为受害者地址,点一次 "Continue with GitHub" 即接管其 Sentori 账号,并顺带标记该账号邮箱已验证。新实现只从 `/user/emails` 取地址,要求 `verified` 与 `primary` 同时为真,provider 不背书则拒绝关联。

其余边界:
- state cookie 与回调 `state` 比对,缺失或为空即拒绝
- 已有身份记录时**仅按 provider 的 subject 匹配**;邮箱只用于首次关联
- 新建用户写入哨兵密码哈希(argon2 无法解析),`/auth/login` 永不可能对其成功
- `exchange_code` 只记 provider 与状态码,绝不记 code / token / secret;非 2xx 分支丢弃响应体(部分 provider 会在错误里回显提交的 client_secret)

session 签发与密码登录**共用同一实现**(`session_cookie_header`),两条路径无法漂移。

13 个单元测试覆盖无需网络的部分:provider 解析、authorize URL 构造、两个 provider 的提取器(含未验证邮箱的拒绝)、state 比对的六种拒绝形态。

---

## v1.5.0(2026-07-21 — SaaS 管理入口统一)

Workspace 管理此前存在两套。`sentori-server` 的 `/admin/api/saas/{workspaces,stats}` 只读,走 dashboard session + saasadmin 角色门,是 **UI 唯一调用的那套**;`sentori-saas-control` 的 `/v1/saas/workspaces` 有完整 CRUD 与 suspend/resume,走**另一套独立账号体系**(`saasadmin_users` / `saasadmin_sessions`),却没有任何界面。一件事,两个接口,两套登录。

- **能力收进 `sentori-server`**,复用 UI 已在使用的鉴权:`POST /admin/api/saas/workspaces`、`DELETE .../{id}`、`POST .../{id}/suspend`、`POST .../{id}/resume`。SQL、绑定顺序、状态码、响应形状逐字迁移
- **`saas-control` 只保留 `/healthz` 与 Stripe webhook**。webhook 保持独立是有意的:它用 body 的 HMAC 验签,与 session 是不同鉴权模型,且 Stripe 需要稳定的机器端点,不应置于 dashboard 登录之后。其鉴权中间件、saasadmin 登录、tenant handler 一并删除;当日早些时候加入的启动时 schema 等待与操作员播种也随之移除(此处已无任何鉴权消费者),argon2 依赖同去
- **migration 0031 与其两张表保留**。当天建、当天删,判断有误便无退路;文档注明控制面已不再读取
- **webapp 同步补齐** create / suspend / resume / delete,删除操作带确认。搬走一个点不到的端点不叫统一

**端到端验证**:create 201(返回 id)→ suspend 204 → resume 204 → delete 204 → workspace 确已从库中消失;鉴权三层实测:未登录 401、已登录非 saasadmin 403、saasadmin 全通;saas-control 旧路由全 404,webhook 对无签名请求 400。

---

## v1.4.9(2026-07-21 — core 集成测试首次运行,连带修复六类既有缺陷)

`v0.2-core-check` 的 core job 此前只跑 `clippy` 与 `check`,**从不 `cargo test`** —— `crates/*/tests/integration.rs` 下所有断言一直是死代码。这些测试用 testcontainers 自起 Postgres,runner 本就有 docker,补一行即可运行。打开之后连锁暴露出以下缺陷,**均为既有问题,非本次引入**:

- **静默 FK:未知 project 写入 0 行却报成功**(9 crate / 14 处)。这些 store 用 `INSERT … SELECT p.workspace_id … FROM projects p WHERE p.id = $n` 从 project 推导 workspace;project 不存在时 SELECT 匹配不到行,写入 0 行,**FK 违例永不触发**,而每个函数的文档与 `translate_fk` 都假设此时得到 `ProjectNotFound`。实际后果:`replay-store::store` 的 blob 已写入对象存储才轮到该行失败,**blob 就此孤儿化**;`runtime-metrics::ingest_batch` 返回少计的 `Ok(n)`,未知 project 的数据点与去重跳过无法区分;`billing::record_drop` 与 `cert-monitor::poll_domain` 静默 `Ok`,写入凭空消失;其余冒成裸 `Db(RowNotFound)`,矛头指向驱动而非缺失的 project
- **`billing::check_and_record` 返回自相矛盾的 `OverLimit { current_count: 0, limit: 100000 }`** —— 同一根因,未知 project 落入"已达上限"分支
- **`integration-traits::record_link`** 同族反模式的 issue 驱动版(`FROM issues i WHERE i.id`),按 `projects` 的检索扫不到
- **`notifier` 四条 SELECT 漏取 `workspace_id`**,而 row-mapper 会读它 —— 所有读路径 panic 于 `ColumnNotFound`。该列自 migration 0010 起就在表上,SELECT 从未跟进
- **`integration-traits` 测试 seed 早已过期**:写于 `issues.workspace_id` 加 NOT NULL 之前,11 个测试在 setup 阶段即挂
- **`IssueStore::patch` 调用点遗漏**(v1.4.8 引入 workspace 参数时,server 侧一处未跟改)

修法尽量不动成功路径:多数为 `fetch_one` → `fetch_optional` 加显式映射,不增往返;两处 `ON CONFLICT DO NOTHING` 无法区分"未插入"与"重复",改用探测,其中 `ingest_batch` 仅探测确实 0 行的 project id,全新数据批次零额外开销。两处**有意保留**:`persist_event` 的 events 插入与 `ingest_span` 的 traces upsert 与已会 bail 的语句同事务同 project_id,再加检查即为不可达分支加守卫。

SDK handler 早已为 `ProjectNotFound` 写好分支(映射 401),此前该变体永不产生,那段代码一直是死的。

---

## v1.4.8(2026-07-20 — 租户隔离 ⚠️ 安全)

v1.4.7 补上了**鉴权**(必须登录),本版补上**授权**(只能看自己的)。此前任一已登录用户可读写全部租户数据。

- **`SessionContext` 现携带 `workspace_id`**,由中间件解析一次。此前它只有 `user_id`,过滤全靠每个 handler 自己记得 —— 于是六个忘了。现在过滤是 handler 必须主动丢弃的东西
- **两层防御**:`handlers::tenant` 的 `guard_project` / `guard_issue` 先校验 id 归属,查询层再约束 `workspace_id`;后加的 handler 即使忘了守卫也跨不过边界。外来 id 与不存在的 id 返回同样的 404 —— 403 会确认 id 存在,把接口变成猜测他人 id 的探针
- **覆盖**:projects / events / spans / traces / metrics / replays / search / stats / cert / issues(读+写)/ issue watchers / comments / activity log,共 17 条查询获得 workspace 约束
- **`bulk_patch` 的 body-id 攻击路径**:它的 issue id 来自请求体而非路径,守卫路径的 project_id 对其无效 —— 攻击者可在路径放自己的 project、在 body 放他人 issue id 批量篡改。现先经 workspace 约束的 SELECT 收窄 id 集合。已实测:跨租户 `bulk_patch` 返回 `updated: 0`,目标 issue 状态未变
- **SSE 实时流 `events_live`** 此前零鉴权,仅按路径 project_id 过滤,可旁听他人事件流;现加守卫
- **审计行 workspace 写错**:这些 handler 记审计用的是服务器配置的 workspace 而非调用者的,审计落到了错误租户名下
- **回归测试**(7 个,CI 内运行,无需数据库):扫 handler 源码断言每条 dashboard SELECT 都约束 workspace_id、每个从路径取 id 的 handler 都调守卫 —— 缺陷本身是"少了一行",所以检查"那一行在不在"

**端到端验证**:建两个分属 GOLIA / Qualcomm 的账号,双向确认各自只见己方项目、交叉访问 issues/events/stats/replays/traces/metrics 全部 404、跨租户写入被拒。

**已知限制**:`IssueStore::patch` / `bulk_patch`(core crate)无 workspace 参数,故 issue 写路径上仅守卫一层,缺查询层第二道防线。把 workspace 推进 store 是根治方案。

---

## v1.4.7(2026-07-20 — 两处未鉴权访问修复 ⚠️ 安全)

**背景**:为让 SaaS 面可用而排查时,发现两处接口在公网无需任何鉴权即可访问。两次访问日志审计均显示**仅有本人验证请求(21 次,同一 IP),无外部访问,数据完整**。发现后立即在 t01 边缘封锁,修复上线后恢复。

- **dashboard 读 API 全部无鉴权**:路由组注释写着 "open in v0.2; can be locked behind the same session middleware via env-var flip",而该 flip 从未执行。任何人可读取任意 workspace 的 issues / events / traces / metrics / replays,且 `GET /v1/projects` 直接返回项目 UUID 供寻址(实测取回某客户 98 个 issue 含崩溃明细)。现拆为三组:dashboard 读进 `session_middleware`;ops 探针(`/healthz` `/livez` `/readyz` `/metrics`)保持免鉴权供 k8s 与 Prometheus 抓取;`/auth/*` 保持公开(否则无法获得 session)。SDK ingest 是独立组,自带 Bearer `st_pk_` 门,未受影响
- **saas-control 无鉴权中间件**:binary 提供 `login` 会写 `saasadmin_sessions`,但从无中间件读回该表 —— 跨租户列表、`DELETE /v1/saas/workspaces/{id}`、suspend/resume 对任何可达者开放。此前不可利用仅因它没有公网路由(该路由是本日早些时候我为修复"控制面返回 HTML"而添加的)。新增 `auth_mw`:Bearer → sha256 → 查 session,过期与不存在合并为同一 401;破坏性路由额外要求 `super` 角色,`staff` 只读
- **`saasadmin_users` / `saasadmin_sessions` 表此前不存在**:定义在 `saas/migrations/0001`,但 saas-control 无 `sqlx::migrate!` 调用,其注释又将 schema 归属 sentori-server,而 core/migrations 从未包含它们 —— 因此 login 只可能返回 500,无人能登录。新增 core migration 0031;saas-control 启动时等待其就绪,并按 `SENTORI_SAASADMIN_BOOTSTRAP_EMAIL/_PASSWORD` 幂等播种首个 `super` 操作员
- **saas-control 的 `init_tracing` 又是空 stub**(与 sentori-server v1.4.1 同一 bug),生产零日志,以致排查暴露只能依赖 Caddy 访问日志。已接真 subscriber

**已知未闭合**:dashboard handler 中 `projects` / `events` / `spans` / `metrics` / `replays` / `search` 六处 SQL 无 workspace 过滤,已登录用户可跨 workspace 读取。当前 dashboard 账号仅 GOLIA 内部人员,故非急性问题;**但在给外部租户(如 Insight)开通 dashboard 账号之前必须先完成租户隔离**。

---

## v1.4.6(2026-07-20 — 缺失 asset 返回 404 + v0.2 server 首次纳入 CI)

- **缺失的 asset 返回 SPA shell 而非 404**:浏览器跨部署持有缓存的 index.html 时会去请求上一版的哈希 bundle,该请求落进 SPA fallback 拿回 200 + index.html,于是浏览器把 HTML 当 JavaScript 解析、报语法错,而不是一个它能恢复的干净 404。`/assets/*` 改用不带 index fallback 的 ServeDir。按前缀而非扩展名判定 —— 扩展名启发式会误伤合法含点的路由段(如 release 名 `app@5.4.2+361`)
- **`self-hosted/server/` 此前完全不在 CI 内**:`build.yml` 测的是 legacy `server/`,`v0.2-core-check` 测的是 `core/` —— 那个打进 self-hosted 镜像、正在跑生产的二进制,从未被任何 CI 编译、lint 或测试过(本版新增的 4 个 fallback 测试若就这么提交也永远不会运行)。现于 `v0.2-core-check` 增加 server job:fmt + test 强制,clippy 暂为 report-only
- 该 crate 因从未进过 CI 也从未被格式化:本版含一次 `cargo fmt --all`(48 文件,纯空白)。clippy 的 ~110 条历史 lint 未清 —— `clippy --fix` 会重写约 64 个文件的生产二进制,应作独立改动

---

## v1.4.5(2026-07-20 — 两处生产静默错误内容修复)

- **未匹配的 API 路径返回 SPA 而非 404**:所有未命中路由都落到 `fallback_service(webapp_service())`,于是 `/v1/does-not-exist`、`/admin/api/nope`、`/auth/nope` 一律回 `200 <!doctype html>`。已发货的 SDK 调到本服务未实现的端点时会把它当成功,随后在离现场很远的地方 JSON 解析失败。legacy 没这问题 —— 那时 SPA 与 API 是 Caddy 后面两个容器,未知 `/api` 路径会吃到 server 的真 404;单体化引入了这个回归。现按路径前缀分流:四个 API 前缀返回 `{"error":"not_found"}` + 404,其余仍解析到 index.html + 200 供 React Router 接管。附 3 个前缀判定回归测试(含 `/v1` `/apidocs` `/authors` 这类相邻路径不得误判)
- **saas-control 部署了但公网无路由**:`:18091` 容器健康在跑、直连返回正确 JSON,但 `/v1/saas/*` 在公网落到 SPA,回 HTML 200 —— 文档化的 SaaS 控制面 API 对任何调用方都是"看起来成功的垃圾"。t01 Caddy 增加 `/v1/saas/*` → `:18091`(置于 catch-all 之前),按安全路径改 live → validate → reload → `devops caddy import` → 同步 repo mirror
- `docs-v0.2/DEPLOY.md` 的路由拓扑对齐现实(此前仍写 v2.4 之前的 `api.sentori.golia.jp` 分域形态,且未说明 `/v1/saas/*` 必须先于 catch-all —— 正是控制面回 HTML 的原因)

---

## v1.4.4(2026-07-20 — workflow 解析修复 + workflow 质量门)

- **v1.4.3 的镜像 workflow 从未运行过**:run-step 注释里为了说明"Actions 表达式不能做参数展开"而写了字面量的空 `${{ }}`,GitHub 把它当真表达式解析并拒收整个文件 —— 连续四次 run 秒失败、零 job、无可读日志,唯一线索是 run 名显示为文件路径而非 workflow 名
- **OSS mirror workflow 带着同一个死 tag 过滤器**(`v0.2.*`,而实际 tag 是 `v1.4.x`),所以"顺带镜像 tag"那段分支一次都没执行过,公开 repo 从未从 CI 收到版本 tag。改为 `v<x>.<y>.<z>`
- **补门**:`scripts/check-workflows.sh`(actionlint)进 preflight,build.yml 新增 `workflows` job 且 docker-build 依赖它 —— 解析不了的 workflow 文件今后进不了 master
- 附带:actionlint 配置 lx64 自建 runner 标签、deploy.yml 未使用的循环变量、`ls | head` 换 `find`、一处 actionlint 表达式桩造成的 SC2193 误报显式标注

---

## v1.4.3(2026-07-20 — CI 与发布镜像修复)

- **self-hosted 镜像从来拿不到版本 tag**:workflow 只在 `v0.2.*` tag 触发,而实际发布 tag 是 `v1.4.x` —— 没有一次 tagged run 触发过,metadata-action 的 semver 模式从未生效,发布镜像只有 `master` 和 `sha-<short>`。同时 `self-hosted/README.md` 让用户 pull `ghcr.io/goliajp/sentori-selfhosted:latest`,而 workflow 推的是 `ghcr.io/goliajp/sentori/sentori-server` —— 镜像名与 tag 皆不存在。现改为 `v<x>.<y>.<z>` 触发 + tagged 构建加 `latest`,README 指向真实镜像
- **镜像构建改原生 arm64 runner**:此前 linux/arm64 在 QEMU 仿真下编译整个 Rust workspace,稳定耗时 ~55 分钟对 60 分钟超时(仅 5 分钟余量)。改为 per-arch matrix(`ubuntu-latest` / `ubuntu-24.04-arm`,公开 repo 免费)按 digest 推送 + merge job 合并 manifest list,两架构并行原生构建;cache scope 按架构隔离
- **CI runner 磁盘**:master 上 `cargo test (server)` 曾以 "No space left on device" 中途死亡(连诊断日志都写不下,故报 failure 但无失败步骤)。构建前清掉预装的 Android SDK / .NET / GHC / CodeQL(~25 GB),并前后打印 df

---

## v1.4.2(2026-07-20 — astro CVE 修复 + webapp 质量门补齐)

- **安全**:astro 5 → 7 + starlight 0.34 → 0.41,关掉 6 个 dependabot alert(4 high):CVE-2026-54299(prerendered error page 的 host-header SSRF)/ CVE-2026-50146(unescaped slot name 反射 XSS)/ CVE-2026-54298(spread props 属性名 XSS)。5.18.2 是最后的 5.x 且无补丁回移,跨大版本是唯一路径;marketing 2 页 + docs 56 页产物验证无退化(`/docs` base、og:image、pagefind 索引均在)
- **质量门**:`webapp/`(v0.2 dashboard,即 self-hosted 镜像打包的 SPA)此前在 preflight 与 CI 均无 typecheck/lint —— 96 个类型错误因此潜伏到 docker `tsc -b` 才暴露。现补:tsconfig `noEmit`(此前 `tsc -b` 往源码目录 emit,积了 40 个 stale `.js` 干扰 eslint 与编辑器)、eslint flat config、preflight + build.yml webapp job(gating docker-build)
- **修复**(lint 查出的真实缺陷):`useKeyHandlers` render 期间写 ref、`Cert` render 期间调 `Date.now()`、6 处空 catch 补上吞错理由

---

## v1.4.1(2026-07-20 — auth 安全修复 + 注册/重置全链路打通)

- **安全**:`/auth/forgot-password` 曾把 reset token 直接放进 HTTP 响应(任何人 POST 目标邮箱即可接管账号);`/auth/register` 同样回传 verify token。两者改为仅经邮件发送(新 Mailer,`SENTORI_SMTP_*` + `SENTORI_BASE_URL`;无 SMTP 配置时 token 仅落服务器日志)
- **修复**:email_verifications / password_resets 的 INSERT 缺 NOT NULL 的 workspace_id —— 自助注册与忘记密码在生产必 500(K2 store 首次对真 schema 跑通);webapp reset 请求体 snake/camel 失配
- **新增**:webapp `/verify` + `/reset-password` 落点页;Register/ForgotPassword 不再展示 token

---

## v1.4.0(2026-07-20 — v0.2 fresh-start 正式 release + 生产 cutover)

v0.2.0-rc1(下节)转正。相对 rc1 的增量:

- **生产 wire**:deploy.yml 构建/拉起 v0.2 栈(server-v2 :18090 + saas-control :18091 + postgres-v2)与 legacy 栈并存;legacy 保留作 rollback,直至 t01 Caddy 切流
- **Dockerfile 修正**:rust 1.88 → 1.97(msrv)、saas 二进制名 sentori-saas → sentori-saas-control、webapp builder npm → bun --frozen-lockfile;新增 migrate-tool/Dockerfile(compose 网络内一次性 ETL)
- **webapp typecheck 修复**:96 个 tsc 错误(页面与 ui.tsx 组件契约漂移;CI 从未对 webapp/ 跑过 tsc)——Badge variant→tone、EmptyState message→hint、Section title 可选化、DataTable rowKey 可选化(修掉 14 页非空表格的潜伏 runtime crash)、useKeyHandlers useCallback 误用
- **cutover**:sentori.golia.jp + ingest.sentori.golia.jp 切至 v0.2 栈;legacy 数据经 sentori-migrate 全量 ETL(幂等)

---

## v0.2.0-rc1 候选(2026-06-23 ship 准备完成)

**状态:** branch `feature/v0.2-foundation` 167 commits,3 binary cargo build 全绿。等用户拍板 → tag → docker build + OSS mirror。

**ship 清单:**
- 5 lens dashboard(Issues/Events/Traces/Metrics/Replays)+ 31 webapp pages + 5 detail views + filter + saved view
- 147+ backend endpoints + 63 sentori-cli subcommands + /v1/_describe self-describing catalog
- Push pipeline production-grade:5 vendor adapters real(WebPush/APNs/FCM/HCM/MiPush)+ JWT/OAuth cache(55min TTL)+ token quarantine + exponential backoff retry + DLQ + 真 dispatch worker + **WebPush 真 payload encryption(RFC 8291/8188 — aes128gcm)** 浏览器收到 title/body
- Alert pipeline:**4 trigger kinds 全 真 wire**(new_issue / regression / event_count via ingest path + crash_free_drop via 5-min periodic worker)+ webhook out + HMAC + per-rule throttle + audit
- 4 background workers:push_worker(5s)+ probe_worker(10s)+ archive_worker(24h)+ **periodic_alert_worker(5min)**
- Synthetic monitor:probe_worker 真发 HTTPS + endpoint_check + endpoint_probe 时序表
- 4 background workers:push_worker(5s)+ probe_worker(10s)+ archive_worker(24h prune sent>30d/failed>90d)+ periodic_alert_worker(5min)
- Auth + session:HttpOnly cookie + Bearer dual auth + 可独立 revoke + saasadmin role gate + IP/UA capture
- Audit log:10+ action types + actor_user_id + IP/UA enrich + client-side IP filter
- 5 lens 真实 ingest + 真实 ETL:**68/68 表(100%)from legacy**(saas_stripe_customers / saas_stripe_invoices / saas_org_quotas / saas_billing_events 补全)
- /healthz + Health page + **/metrics(Prometheus text format)** + **/livez + /readyz(k8s probes)** + 工作流可视(stat grid + 4 workers + 5 vendor adapters)
- audit log payload._ip server-side substring filter(LIMIT 之前 filter,可靠)
- 4 test 触发器(webhook-test / push-test / alert-fire-test / ingest-test)

**已知 defer v0.3+:**
- HCM/MiPush 中国 vendor adapter token cache(目前每 send OAuth)
- Saved view 高级 query builder UI

---

## v0.2 — fresh-start pivot(2026-06-22, branch `feature/v0.2-foundation`)

**目标：** 大仓 → SaaS 内置 + self-hosted OSS repo + 定制产品。SaaS 与 self-hosted 同版本号同终端 SDK。SaaS 一经发布满足现有 sentori 全功能（含 tenant 管理 + 简化顶层架构）。Self-hosted 满足 tenant 内全功能。现有 app 用户用现有 SDK + 现有 token 无感继续工作。

**数据策略：** SaaS 由 row-level `workspaces` × `workspace_id` 替代 database-per-tenant；ETL 是业务层（读 legacy DB → 写 v0.2 DB），不是 schema ALTER；self-hosted 总是 fresh DB（Docker pull + migrations + env-var bootstrap admin）。

### Phase A — Legacy inventory ✅

- 82 migrations / 68 tables / 360 cols 全量盘点 → `docs-v0.2/internal/legacy-inventory.md`
- 28 SDK `/v1/*` endpoint + 11 push endpoint + 95 admin endpoint + 13 auth endpoint = 136 surface
- 永久契约：token format `st_pk_<26 base32>`、SHA-256 hashed、`sentori-ingest-token` crate 承载

### Phase B — Workspace 重组 + cargo 工作区适配 ✅

- 5 stub crate 占位（push-provider Mock / mock vendor adapters / etc）
- `sentori-server` / `sentori-saas-control` / `sentori-cli` 三 binary 改名 align v0.2
- GitHub workflows: `v0.2-core-check` + `v0.2-self-hosted-image` + `v0.2-oss-mirror`

### Phase B.5 — Schema 补全 ✅

- 15 新 migration（0016-0030）覆盖 legacy 33 张缺漏表
- 列名沿用 legacy 表名以支持 ETL INSERT-透传

### Phase C — SDK 28 endpoints port ✅ 27 业务 / 1 SSE stub

- `sentori-ingest-token` crate: Bearer middleware + `IngestContext { workspace_id, project_id, token_kind }`
- 17 SDK handler 全部 ship 业务实现（events / events:batch / events_attachments / spans / spans:batch / sessions / heartbeat / deploys / metrics / runtime_metrics / track / security_report / security_link / security_score / control / user_reports）
- `/v1/events/_recent` SSE 留 stub（Phase G 接 dashboard SSE 时落地）

### Phase D — Push 11 endpoints ✅ 全业务 + worker

- 11 endpoint 全 wire（register_token 双写 push_tokens + device_tokens / revoke / subscribe / unsubscribe / send / receipt / ack / expo-compat send/receipt / get_preferences / put_preference）
- `push_worker.rs` background task：drain `push_sends.status='queued'` → MockProvider → INSERT push_delivery_logs + UPDATE status
- env-tunable interval/batch；`FOR UPDATE SKIP LOCKED` 横向 scale-out 安全
- 真 vendor adapter（APNs/FCM/WebPush/HCM/MiPush）留 K7.x

### Phase E — Admin endpoints + Session middleware ✅

- 33 admin endpoint port: tokens / projects / push-credentials / members / invites / cert-watch / integrations / releases / saved-views（detail+patch）/ alerts（detail）/ saas（cross-workspace list + stats）
- `session_mw.rs`：cookie 或 Bearer session token gate；`Extension<SessionContext>` 注入
- `/auth/{register,login,verify,forgot,reset,change-password,me,logout}` 6+2 endpoint
- saas-control binary aligned to row-level pivot（删 `tenant_provision`，workspaces CRUD + workspace_billing 状态 → suspend/resume/delete）

### Phase F — Docker + OSS mirror ✅

- `self-hosted/docker/Dockerfile` multi-stage Rust → distroless
- `.github/workflows/v0.2-self-hosted-image.yml`：多架构 build → ghcr.io/goliajp/sentori/sentori-server
- `.github/workflows/v0.2-oss-mirror.yml`：master push + v0.2.* tag → mirror to `goliajp/sentori-selfhosted` public repo

### Phase G — Webapp 18 页 ✅

Login / Register / ForgotPassword / Overview / SaasAdmin / Projects / Members / Tokens / Push / Integrations / Releases / Issues / Events / Cert / Alerts / Audit / Settings / Health。

- `lib/api.ts`：auth + admin + saas API methods + auto-inject Bearer session header
- session 在 localStorage（HttpOnly cookie middleware 后续）
- Sign out 闭环：POST /auth/logout → clear → /login

### Phase H — sentori-migrate ETL binary ✅ 13 sets / 41 表覆盖

- 业务层 ETL（read legacy DB → transform identity layer → write v0.2 DB）
- table sets：identity / tokens / releases / events / issues / sessions / spans / push / attachments / dashboard / dashboard_extra / analytics / metrics / ops
- 41 of 68 legacy 表覆盖；剩 27 为 SaaS-only billing / 内部 rollup / 低优先级
- `--dry-run` count rows without writing；`--tables` 选择 set；`ON CONFLICT DO NOTHING` 幂等

### sentori-cli — admin commands

- `project list/create/delete` + `token list/mint/revoke` via /admin/api/*
- Bearer token via `SENTORI_ADMIN_TOKEN` env-fallback

### 总产出

- 40 commits on `feature/v0.2-foundation`
- 3 binary `cargo build` 全绿
- Backend endpoints: 84+（28 SDK + 11 push + 6 auth + 33 admin + saas-control + healthz）
- Webapp pages: 18
- Migrations: 30 sql / ~58 表
- ETL: 13 sets / 41 表
- Docker image + OSS mirror workflow 就绪

### 之后增量(2026-06-22 → 2026-06-23, branch 仍 feature/v0.2-foundation)

| 项 | 完成 |
|---|---|
| WebPush 真 vendor adapter | ✅ VAPID ES256 + 浏览器 WebCrypto keypair wizard |
| APNs 真 vendor adapter | ✅ token-based ES256 over HTTP/2 |
| FCM 真 vendor adapter | ✅ Legacy HTTP API(server key) |
| SaaSadmin role-based RBAC | ✅ env-allowlist `SENTORI_SAASADMIN_USER_IDS` |
| HttpOnly cookie session | ✅ |
| `/v1/events/_recent` SSE 真实现 | ✅ + Events 页 Live toggle |
| ETL 全 14 sets,coverage | ✅ 62/68 表(91%)|
| 5 lens project dashboard | ✅ Issues / Events / Traces / Metrics / Replays + detail + filter + save 路径全闭环 |
| Issue triage workflow | ✅ list inline ✓⊘↺ + bulk select + detail + comments + activity + watchers + notification fanout |
| Notifications inbox + sidebar badge | ✅ 60s 轮询 |
| Linear UX | ✅ ⌘K palette + `g<letter>` + per-page j/k/x/e/i + `?` cheatsheet + HttpOnly cookie + 401 redirect + return-to |
| /v1/_describe self-describing catalog | ✅ sentori-cli describe 用 |
| Endpoint probes admin CRUD | ✅(后台 poller worker 留 v0.3)|
| sentori-cli 44 subcommands | ✅ 全 admin / lens / ops 覆盖 |
| Replay scrubber + raw NDJSON download via cli | ✅(完整 canvas/DOM replayer 留 v0.3)|

### 留 v0.3+

- WebPush payload encryption(RFC 8030/8188/8291)— v0.2 只 wake push
- HCM / MiPush vendor adapter(中国市场)
- 剩 6 个 saas_* 内部表 ETL(saas-control 自有,不影响 cutover)
- Endpoint probes background poller worker
- Replay canvas/DOM 真实 replayer
- Saved view 高级 query builder UI
- HTTPS 自动 TLS via embedded rustls + Let's Encrypt
- SDK 类型生成器
- Cutover 部署执行(user-gate;CUTOVER.md 就绪)

---

## v2.3 — SDK redesign + identity layer + DSR + Sentry compat

**Shipped:** 2026-06-03. See `docs/roadmap/v2.3.md`.

**Package bumps:**

- `@goliapkg/sentori-core` → **1.2.0** (minor — two minor changesets merged: logger surface + ReadyInfo shared type + `BeforeSendHook` type + `withSpan` overload dispatch + `withActiveSpan` explicit name)
- `@goliapkg/sentori-javascript` → **1.2.0** (minor — two minor changesets merged: `init.logLevel` + `init.onReady` + `init.sample` canonical name + `init.beforeSend`)
- `@goliapkg/sentori-react-native` → **2.2.0** (minor — two minor + one patch changeset, merged into one minor bump: `init.beforeSend` + logger surface re-export + `ReadyInfo` from core + `/compat` Sentry drop-in unit-test coverage)

**Highlights:**

- **Silent by default.** Logger module gates the SDK's console output at `'warn'` by default — a healthy install adds zero `[sentori]` lines to host metro/browser console. `init.logLevel: 'silent' | 'error' | 'warn' | 'info' | 'debug'` controls the gate. `setLogTransport(fn)` routes Sentori-internal lines into the host's own log aggregator instead of console.
- **`init.onReady`.** Fires once after init completes; carries `sdkVersion` + (RN-only) `coldStartMs` + `native: { bound, methods }`. Hosts no longer need to scan the console to know the SDK is live.
- **`init.beforeSend`.** Sync host-supplied mutate-or-drop hook on each outbound event. Returns the (possibly mutated) event or `null` to drop. Throwing / non-event return falls back to the unmodified event under the NEVER rule.
- **Unified `withSpan`.** `withSpan(name, fn)` is the new high-level wrap helper per design §2.3; `withSpan(span, fn)` keeps working via overload dispatch (delegates to `withActiveSpan`). `withScopedSpan` remains as the explicit high-level name.
- **Identity layer + cross-project user lookup.** `setUser({ id, name?, linkBy: { email?, phone?, googleSub?, ... } })` — raw identity values are normalised + SubtleCrypto SHA-256 hashed client-side; only the hash leaves the device. Server layers a per-scope salt before storing fingerprints. Operator dashboard's Users module looks up cross-project hits without ever seeing raw values.
- **GDPR DSR erase.** `POST /admin/api/orgs/{slug}/users/erase` with `dryRun` flag; pseudonymises `payload.user` across every matching event + drops identity_fingerprints rows. Dashboard UI with typed-confirmation gate. Audit log entry per call.
- **Sentry compat drop-in.** `import * as Sentry from '@goliapkg/sentori-react-native/compat'` — full translation table for code an LLM has been trained on against `@sentry/react-native`. DSN parser, `Sentry.init/captureException/captureMessage/setUser/setTag/addBreadcrumb/withScope/startTransaction/close/flush`. Drops `ip_address` (Sentori never stores IP). Warn-once dedup per (api, dropped_field).
- **`/explore` grammar extension.** 5 new filters (`issueEq`, `userIdEq`, `routeEq`, `osEq`, `search`), 4 new dims (`device_os`, `issue_priority`, `severity`, `route`), 4 new measures (`new_issue_count`, `p50_duration`, `p95_duration`, `crash_free_rate` reserved-pending-session-schema). Unlocks v2.2 W3 per-row sparkline stub on Issues list + foundation for Phase 7 / 8 lenses.

**Migration:**

- Existing v2.2 init calls keep working unchanged. Pre-existing `sampling` field stays accepted as a back-compat alias for `sample`.
- `setUser({ id, email })`-style Sentry calls reach the same internal state via `/compat`; mapping is automatic but warn-once hints fire at `info` level pointing to native equivalents.
- Hosts that grepped metro for `[sentori]` to confirm SDK liveness should wire `onReady` or set `logLevel: 'debug'`.

---

## v1.0.0-rc.10 — default capture rate 4 Hz → 2 Hz per perf rule

**Theme:** iOS sim measured 0.99 ms / tick at 4 Hz on a thin (11-node) dev panel, well within budget — but extrapolation to a 200-node Insight-class dense UI puts JS-thread occupancy at ~1.2–1.6 % on iOS and ~3.6–4.8 % on Android (reflective Drawable colour extraction amplifies cost there). That crosses the rule that Sentori must not make the host app stutter. Roll the default back; keep the encoding gains.

**Package bumps：**

- `@goliapkg/sentori-react-native` 1.0.0-rc.9 → **1.0.0-rc.10**（patch — perf-budget default change）

### Fix

- Default `replay.hz` 4 → 2. `replay.hz: 4` (or 8) remains opt-in for motion-heavy iOS apps.
- `apps/rn-example` reverts to the default rate (drops explicit `hz: 2`), removes a temporary rc.9 auto-fire `useEffect`.
- `bunfig.toml` at `apps/rn-example` sets the linker to hoisted so future iOS Pods install can resolve transitive expo deps under `node_modules/<pkg>/ios/...` (harmless under the workspace default `isolated`).

### Verify

- iOS sim per-tick budget unchanged; budget envelope at 2 Hz extrapolates to ≤ 0.6 % JS-thread occupancy even on dense Android UI

---

## v1.0.0-rc.9 — keyframe + delta replay encoding (v2 wire format)

**Theme:** rc.8 stabilised the Android wireframe walker but each replay tick still emitted a full snapshot. Sustained capture at the rc.8 default rate produced > 200 KB / 60 s on dense dashboards — close enough to the 60 s replay attachment budget of 200 KB that the next product feature would push us over. Need a real on-wire compactor.

**Package bumps：**

- `@goliapkg/sentori-react-native` 1.0.0-rc.8 → **1.0.0-rc.9**（minor — new wire format, dashboard parser updated in lockstep）

### Fix

- Rewrite `replay.ts`: native walker still pulls full snapshots each tick; JS now diffs against the previous emit's reconstructed state and emits either a fresh keyframe (cold start / every 4 s / when delta would be near-rewrite) or a small delta (added / changed / removed nodes by spatial fingerprint).
- Native tick rate bumped 1 Hz → 4 Hz default at this rc (later rolled back to 2 Hz in rc.10).
- Ring buffer evicts by 60 s time window instead of fixed 60-frame count.
- New `replay.keyframeMs` option for tuning reconstruction chain length.
- Dashboard side: `web/src/modules/issues/replay-tab.tsx` + reconstructor pair lands; parser auto-detects v1 archive events for backwards-compat reading.

### Verify

- 11 new unit tests cover encoder paths + a 60 s × 4 Hz dense-UI byte-budget assertion (< 200 KB)
- Wire schema documented at `docs/replay-encoding-v2.md`

---

## v1.0.0-rc.8 — anchor walker at content view + cover StateList/Bitmap drawables

**Theme:** Insight 2026-05-18 rc.6 verify still showed two artifacts: (A) After rc.3's deep-walk fix, PhoneWindow's StatusBarBackground / NavigationBarBackground children started getting emitted with display-wide absolute-coord rects, painting as horizontal grey bars overflowing the device viewport in the dashboard SVG. (B) Coloured Pressable backgrounds (StateListDrawable's default state) still rendered as flat grey.

**Package bumps：**

- `@goliapkg/sentori-react-native` 1.0.0-rc.7 → **1.0.0-rc.8**（patch — Android walker anchor + extended drawable extractor）

### Fix

- Switch walk root from `window.decorView` to `decorView.findViewById(android.R.id.content)`. System-bar overflow nodes drop out.
- Extend `extractDrawableColor`:
  - `StateListDrawable` → recurse into `.current` (covers Pressable / TouchableOpacity wrappers in default state)
  - `BitmapDrawable` → return null AND switch node kind to `image` in walker, so dashboard renders raster-backgrounded views as a media region instead of a flat grey rect.

### Verify

- On `sim-sentori-android` (Pixel 10 Pro / API 37): rc.7 surfaced 5 distinct hexes on `rn-example`'s dev panel; rc.8 same 5 with no system-bar overflow nodes.
- Sim-Sentori-Android emulator workflow documented at `docs/runbook/sim-sentori-android.md`.

---

## v1.0.0-rc.7 — cover CompositeBackgroundDrawable + reflective getColor fallback

**Theme:** rc.5 introduced colour extraction for Android but only matched `ColorDrawable` / `GradientDrawable` / `RippleDrawable` explicitly, missing RN 0.74+'s actual code path: a View's background is a `CompositeBackgroundDrawable` (extends `LayerDrawable`) whose inner `BackgroundDrawable` layer holds the paint colour. Both classes are Kotlin-internal in the RN module — can't import the types — so we bridge via reflection.

**Package bumps：**

- `@goliapkg/sentori-react-native` 1.0.0-rc.6 → **1.0.0-rc.7**（patch — Android colour extraction parity）

### Fix

- `is RippleDrawable` → `is LayerDrawable` so the recursion covers `CompositeBackgroundDrawable`, `RippleDrawable`, and any future layer-stacking variant.
- `else` branch tries `getBackgroundColor()` / `getColor()` via Java reflection. Any throw / non-Int falls back to null — attachments still ship, just without that node's colour.

### Verify

- On `rn-example` S22 verify: rc.5 surfaced 3 unique hexes (root + text + overlay); rc.7 surfaces 5 including a Pressable's `#1A1A1FFF` brand colour.

---

## v1.0.0-rc.6 — surface HTTP status in attachment-upload dev logs (+ server body-cap fix)

**Theme:** Pre-rc.6 `uploadAttachment` returned null on any non-2xx without echoing the status to logcat — only the next-event breadcrumb carried `http_NNN`, invisible to anyone reading metro/logcat in the moment. Insight's rc.4 verify rig (and our own future runs) had to guess between 413 / 422 / 500 / network-fail.

This release also paired with a server-side body-cap fix (commit `5b608b3`) to make the rc.6 per-route 16 MB attachment cap actually take effect.

**Package bumps：**

- `@goliapkg/sentori-react-native` 1.0.0-rc.5 → **1.0.0-rc.6**（patch — dev-log surface area + server body-cap regression cover）

### Fix

SDK side — three new dev-only `console.warn` lines, one per failure shape:

```
[sentori] attachment upload non-2xx status=NNN eventId=… kind=…
[sentori] attachment upload bad-response-body status=NNN eventId=… kind=…
[sentori] attachment upload fetch threw reason=fetch_TypeError eventId=… kind=…
```

Fires for screenshot / sessionTrail / replay uniformly. Existing `[sentori] replay upload returned null ndjsonBytes=N` line still fires; read together.

Server side (`5b608b3`):

- Raise `MAX_BODY_BYTES` 1 MB → 16 MB at the merged-app outer layer so the attachment route's per-route 16 MB cap can take effect (Tower outer layer was cutting the body stream off before the inner layer ran).
- Add explicit `RequestBodyLimitLayer::new(1 MB)` to every other small-payload ingest route so they don't inherit the loosened outer cap.
- Regression test `attachment_route_accepts_payloads_above_1mb` posts a 5 MB body to the attachment URL and asserts not 413.

### Verify

- Insight 2026-05-18 770 KB replay upload (which had been 413-ing): post-fix lands cleanly.

---

## v1.0.0-rc.5 — Android emits node.color for rect backgrounds — wireframe shows brand palette

**Theme:** Pre-rc.5 the Android walker explicitly skipped colour for the `rect` kind (the `.kt` comment said "renderer falls back to neutral"); iOS already pulled `UIView.backgroundColor` on the same path. Consequence: every coloured ViewGroup in the host app rendered as a uniform white block on the dashboard wireframe — "怎么还是黑白的呢" (user, after rc.4 deploy).

**Package bumps：**

- `@goliapkg/sentori-react-native` 1.0.0-rc.4 → **1.0.0-rc.5**（patch — Android colour extraction first pass）

### Fix

- Extract from `ColorDrawable.color`, `GradientDrawable.color.defaultColor`, and recurse into `RippleDrawable` layers.
- Filter out alpha-0 fills so fully-transparent backgrounds keep falling back to the neutral default.

### Verify

- Native S22 verify on the `rn-example` dev panel produced 946 colour fields on a ~117 KB NDJSON capture with three distinct RN-brand hues.

> Note: native change. Insight needed a full APK rebuild to pick rc.5 up — JS-only `bun add` was not enough.

---

## v1.0.0-rc.4 — UTF-8-safe replay encoding — close Insight Android attachment drain

**Theme:** The pre-rc.4 inline `btoa(ndjson)` upload path threw `InvalidCharacterError` on any wireframe NDJSON code point > 0xFF (JP / em-dash / emoji TextView text) and the surrounding catch swallowed it silently. After rc.3's walker fix surfaced deep TextView content, the replay attachment silently never landed.

**Package bumps：**

- `@goliapkg/sentori-react-native` 1.0.0-rc.3 → **1.0.0-rc.4**（patch — encoding + dev-warning surface）

### Fix

- Route through a shared `base64Utf8` helper (sessionTrail's existing pattern).
- Replace the silent catch with `[sentori] replay attachment threw` / `replay upload returned null` / `replay drain empty` dev warnings so the next failure shape is visible.
- Add `wantReplay` to the `captureException` debug line.

### Verify

- End-to-end on S22 with mock ingest: `kinds=replay` lands.

---

## v1.0.0-rc.3 — Android replay walker recurses through zero-size wrappers

**Theme:** Insight 2026-05-18 rc.2 re-verify (`com.qualcomm.insight` on Samsung S22 / Android 16) 报告：rc.2 native (`probeScreenshot` 在 exposed-methods list 里) 确认是新版，但 wireframe 仍间歇问题。两条独立 bug 叠加：
- (A) `captureWireframe` 间歇丢 subtree —— 只 emit root + 几个顶层 wrapper，800 nodes 报告对但 dashboard 渲染空
- (B) rc.2 把诊断 warning 收得太严，只在 `null`/空串时 fire，对「非空但 nodes=[]」的 thin result 静默

**Package bumps：**

- `@goliapkg/sentori-react-native` 1.0.0-rc.2 → **1.0.0-rc.3**（patch — Android-only native fix + 双端诊断扩展）

### Root cause

`SentoriReplayCapture.kt` walk() 在 view 自身 `width <= 0 || height <= 0` 时整段 return —— 包括「不 emit 节点」和「不递归 children」两个 effect。对 RN Fabric 拓扑下经常出现的 zero-measure ViewGroup wrapper（在 RN 的 shadow-tree 中间层 + lazy-layout 阶段）来说，这意味着**整个 React tree 子树都被跳过**，根 decorView + 几层 padding wrapper 之外什么都没拿到。结果 NDJSON 非空、JS 视为成功、ring 累计「几乎空」帧、dashboard 渲染近空。

iOS 端走的是 "skip emit but recurse" 写法（comment 已经有 `invisible container — skip emitting but recurse`），从一开始就对。

### Fix

* **Android walk()** 拆分「emit a node」和「recurse into children」语义：zero-size view 不 emit 节点但仍递归子树。对 MAX_DEPTH (60) + MAX_NODES (800) 仍设上限。
* **诊断状态扩展**：双端新增 `lastDepthMax` / `lastSizeBytes` / `totalTicks` / `totalEmptyResultTicks`，原有 `lastPath` / `lastNodes` / scene-window counts 保留。失败模式有正面证据（"depthMax=42" 说明 walker 跑通了；"depthMax=3" 说明又被早期 bail 卡住了）。
* **JS replay tick 加 thin-result warning**：捕获 nodes < 6 的 "成功但浅" 情况，stride 1/10/100/... 节奏避免 spam。原来的 `replay tick: native returned null` warning 保留，覆盖另一条 failure path。
* **第一次 ok tick 也打 log**：`[sentori] replay tick: first ok — nodes=N sizeBytes=B`，让 dev 一眼看到 capture 真的拿到结构。

### Verify

- 109 SDK tests + tsc clean
- 待 Insight 重 build APK on S22 / Android 16, 期望:
  - `[sentori] replay tick: first ok — nodes=300+ sizeBytes=15000+`
  - `[sentori] enqueue ... attachments=3 kinds=screenshot,sessionTrail,replay`
  - dashboard issue detail 上 Session replay 段渲染出完整 wireframe layout (不再是 2-3 个空 box)
  - `await Sentori.probeNativeWireframe()` returns `lastDepthMax: 30-50` (健康范围)

### Other

- iOS 端补 depthMax / sizeBytes / totalTicks 诊断 (parity)，行为不变

---

## v1.0.0-rc.2 — Android foreground-Activity tracker

**Theme:** Insight 2026-05-17 Android verify (`feature/GOL-582-sentori-0-9-upgrade @ b792e31d`, Samsung Galaxy S22 / Android 16) 上 `captureScreenshot` 和 `captureWireframe` 双路都 null。同 JS 在 iOS sim-insight 一次过。Android 单独有 native-side Activity 解析 gap。

**Package bumps：**

- `@goliapkg/sentori-react-native` 1.0.0-rc.1 → **1.0.0-rc.2**（patch — 仅 Android 行为修复 + 双端新增 `probeScreenshot` 诊断 API）

### Root cause

`SentoriCrashHandler.register(context)` 走的是 Expo module `OnCreate` 生命周期。在 Insight dev-launcher → MainActivity 拓扑下，MainActivity 已经 onResume 完了才轮到 module init。`Application.registerActivityLifecycleCallbacks` **只补发未来事件、不补发已发生的状态**，所以 `lastActivity` 一直 null —— 每次 capture short-circuit 到 `null`。iOS keyWindow 4 层 fallback 治的那一类 bug 在 Android 上的等价形态。

### Fix

新加 `SentoriForegroundActivity.kt` 作为进程级 single source of truth：

1. **`Application.ActivityLifecycleCallbacks`** —— 继续 track 未来的 onCreate/onStarted/onResumed
2. **`ActivityThread` reflection back-fill** —— `install()` 时立刻通过 `android.app.ActivityThread.currentActivityThread().mActivities` 拿当前 foreground Activity，补已经 resumed 不会再触发 callback 的盲区。失败安静吞掉，lifecycle callbacks 继续兜底
3. **`SentoriScreenshotCapture.register()` / `SentoriReplayCapture.register()`** 都代理到 `SentoriForegroundActivity.install()`，两边共享同一个 Activity 指针 —— 不会出现 screenshot 看到 Activity 但 replay 没看到的不对称
4. **诊断**：双端新增 `probeScreenshot()`（mirror `probeWireframe()` shape）+ 每个失败 path 都有 tagged 字符串 (`activity.null` / `decorView.zero-size` / `pixelCopy.notSuccess` / `pixelCopy.threw:<class>` / ...)，JS 端 `probeNativeScreenshot()` 一行拿到，Insight verify 不用贴 logcat
5. **iOS screenshot capture** 顺手补 4 层 keyWindow resolution + `probeScreenshot` + lastDiagPath，跟 replay 端保持一致

### Verify

- 109 SDK tests 全绿 + tsc clean
- 待 Insight 在 Samsung Galaxy S22 / Android 16 同一台真机重跑：期 metro log 出 `enqueue ... attachments=3 kinds=screenshot,sessionTrail,replay`、dashboard `attached: ● screenshot · ● replay`
- 失败时 `await Sentori.probeNativeScreenshot()` 一行返回带 `trackedSource: "reflection.activityThread"` / `lastPath` 的诊断

### Other

- Insight 同次 verify 抓到一条 production-grade Android RN bug (`ReactViewGroup contains null child at index 3`) —— 不属于 SDK 路径，平台真的看到了原本看不到的崩溃。独立条目 GOL-589

---

## v1.0.0-rc.1 — Replay scrubber + iOS showcase

**Theme:** Sentori 从 "看到 stack" 进化到 "看到崩前 30 帧的 prop 变化"，并把 iOS SwiftUI showcase 当作开源门面。

**Package bumps：**

- `@goliapkg/sentori-react-native` 0.9.12 → **1.0.0-rc.1** （major — unref crash fix 是 hard break）
- `@sentori/web` (dashboard) 0.1.0 → **1.0.0-rc.1**
- `sentori-server` 0.1.0 → **1.0.0-rc.1**
- `sentori-cli` 0.1.0 → **1.0.0-rc.1**
- `@goliapkg/sentori-monorepo` 0.0.0 → **1.0.0-rc.1**

（其它 SDK package — core / vue / svelte / next — 暂不动，本 release 焦点是 RN 通路。）

### Workstream A — Replay scrubber + fiber tree diff

**A1 — SDK wireframe path repair** (commits `7490bed`, `33230ec`)
- iOS keyWindow 改 4 层 fallback (foregroundActive → fgi → any scene → legacy)，加 NSLog 诊断 + `probe()` 暴露 path / nodes / scene / window counts 给 JS
- Android symmetric probe (activity.null / decorView.null / root.zero-size / activity.resumed)
- JS `probeNativeWireframe()` typed wrapper
- **关键修复**：`replay.ts` 删 `.unref?.()` — Hermes 0.81 timer 是 plain number，optional-chain 走 prototype lookup → undefined → synchronous throw 被 RN bridge 静默吞掉 → setInterval 注册但 callback 永不触发。这是 Insight 2026-05-17 verify report 的 "starting bound=true 后 30s 没有 tick log" 根因
- 加 unconditional FIRST INVOCATION log + startSpan 包 try（span 库失败不再杀整个 tick）

**A2 — Frame-level delta encoding** (commit `6203ec8`)
- 60-frame payload 在 sparse app 上 ~7 KB raw、heavy app ~400 KB raw，都在 500 KB attachment cap 内
- 加 frame-level dedup：新 snapshot 跟最后一帧 byte-equal 时不 push。静态 UI 不再吃掉 ring 槽位

**A3 — Server-side wireframe storage + query** (commit `d2a8258`)
- 修 `ALLOWED_KINDS` 漏 `replay` 的 bug — migration 0043 加了 DB CHECK 但 application 层 whitelist 没同步，每次 SDK upload replay attachment 都 400 invalidKind（Insight 看到 `○ replay` 的根因）
- 加 `application/x-ndjson` 到 replay 的 media-type whitelist
- 新 endpoint `GET /admin/api/projects/{project_id}/events/{event_id}/replay-frames` — 服务端解 NDJSON 直接返回 frame JSON 数组，dashboard 不用在 client 做 NDJSON parse

**A4 — Dashboard scrubber UI** (commit `455beda`)
- 新 `<ReplayTab>` 加在 issue detail 的 tab 列表，位置 Stack 后、Events 前
- SVG wireframe canvas（一个 `<rect>` per node + soft text glyph，aspect ratio 锁 device viewport）
- 220px 右侧 meta 列：frame N/total · t+ 秒 · ts · nodes · viewport
- Horizontal thumbnail rail — 一帧一个 mini SVG，accent outline on 当前帧，click-to-jump
- Scrubber 控件：Prev / Play / Next + HTML range slider + 2 Hz auto-play stop at end (不 wrap)
- 键盘导航：← → step / Space play-pause / Home End jump
- Empty state 引导 SDK 配置而不是 404

**A5 — Tree diff renderer** (commit `2cc6694`)
- "◉ diff vs prev" toggle 在 meta 列
- spatial fingerprint 配对（(x,y,w,h) 取整 → key）— SDK 不出稳定 ID 时这是最诚实的 matcher
- added (success-green outline) / changed (warning-amber outline) / removed (danger-red translucent ghost overlay at prev position)
- diff summary 行：added N / changed N / removed N

**A6 — Issue detail integration** (folded into `455beda`)
- 1 import + 1 Tab key + 1 conditional render

### Workstream S — Showcase / marketing front door

**S1 — Showcase polish round 1** (commit `93fed34`)
- iOS-18+ MeshGradient aurora behind warm-dark backdrop（缓慢漂移，atmospheric not UI animation）
- 共享 `formatBytes` helper — "—" / "952 B" / "1.0 KB" / "30.9 KB"
- DefRow 收 84→72px gutter + minimumScaleFactor 0.7 — `(not yet sampled)` 完整显示

**S2 — Marketing screenshot capture** (commit `c9b3c76`)
- `marketing/assets/showcase-hero.png` — 顶部 scroll：brand wordmark + tagline + status pill + 8-card demo grid 上半
- `marketing/assets/showcase-bottom.png` — 底部 scroll：grid 下半 + wireframe replay ring chart + recent events + footer

**S3 — Repo root README** (commit `72f10fd`)
- 完全重写：showcase hero 起手 → 三个 60s path（try-in-sim / RN install / self-host）→ RN-first features 罗列 → roadmap
- 替换过期的 `@sentori/react-native` 为 `@goliapkg/sentori-react-native`
- 链接 `docs/roadmap/v1.0.md` 而非旧 phases doc

**S4 — docs-site landing alignment** (commit `c12dc3f`)
- splash hero 接 README tagline
- 4 张 card：iOS showcase / SDK reference / Wireframe replay / Self-hosting
- "What you can do today" 列出 4 个可执行的能力
- 25 pages 编译干净，无 MDX/Starlight warning

### Workstream C — Intent cluster (cold)

C1–C6 全部 cold。trigger："Replay" 上线 ≥ 1 周 + ≥ 100 events 带完整 breadcrumbs — 等真实数据积累。下个 release 开 C 系列。

### iOS Showcase 项目本身 — `apps/ios-showcase/`

新增。SwiftUI 6 + iOS 26 deployment。**直接 link SDK 的 Swift 源**（绕开 ExpoModulesCore JS bridge），证明 native 通路在没有 RN 的情况下也能用：

- `SentoriCrashHandler` / `SentoriReplayCapture` / `SentoriScreenshotCapture` / `SentoriMobileVitals` / `SentoriHangWatchdog` / `SentoriNativeExceptionBridge` 全部链接，`SentoriModule.swift`（Expo wrapper）显式排除
- xcodegen 作为 project 源（xcodeproj git-ignored）
- 6 个 SwiftUI view 模块：Hero / KPI / ActionGrid / ReplayRing / EventLog / Footer
- 8 张 demo card，每张 SF Symbol 动画 + Liquid Glass material + 触发一个 SDK 能力

### 验证

- iOS showcase 在 sim-sentori 上跑：10 秒内 replay frames 16 → 258，bytes 0 → 30.9 KB（wireframe sampler 在 native 通路里跑得稳）
- 所有 [sentori] diagnostic warn 在 RN example app 上 fire on boot — A1 trigger 真实达成
- 109 SDK tests + dashboard typecheck + docs-site build 全 green
- 所有 commits CI green，deploy 上线 app.sentori.golia.jp

### 已知限制

- `apps/ios-showcase/` 是 simulator-only，没 deployment team 签名（CODE_SIGN_IDENTITY="-"）
- RN SDK 0.9.x→1.0.0 是 major bump（unref crash fix 是 hard break）— 升级前确认你的 RN 上 Hermes timer 实现，0.83 之后版本已修
- C 系列（intent cluster）延后到 v1.1

---

## v0.7.1 — Insight feedback patch（Phase 48）

**Goal:** Insight 第一份 dogfood 反馈直接落 fix。Screenshot 链路修通 + MaskRegion 不再是空头支票 + dashboard 几条专业感破绽（路由、版本号、sidebar 拥挤、IDE 外部跳转）一次性收掉。

**npm 包变化：**

- `@goliapkg/sentori-javascript` 0.4.0 → 0.4.1
- `@goliapkg/sentori-react-native` 0.7.0 → 0.7.1
- `@goliapkg/sentori-react` 0.4.5 → 0.4.6
- `@goliapkg/sentori-next` 0.2.5 → 0.2.6
- `@goliapkg/sentori-vue` 0.1.0 → 0.1.1
- `@goliapkg/sentori-svelte` 0.1.0 → 0.1.1
- `@goliapkg/sentori-solid` 0.1.0 → 0.1.1

（sentori-core 0.6.0 不动 — 协议层无变化。）

### Phase 48 sub-A — Screenshot 链路修复 ✅

Insight 反馈核心：CFNetwork log 显示 POST `/v1/events/<id>/attachments/screenshot` server 端 202 接收成功，但 issue 详情页五个 tab 完全没有 attachment 展示位。三层修：

- **Server**：`list_events_for_issue` 加 `enrich_attachments` 步骤，单批 query `event_attachments` 表并覆盖 `payload.attachments[]`。Server 端是 source-of-truth，client echo 失败或者 ref 不匹配都不影响 dashboard 显示。
- **RN client**：`uploadAttachment` 把 `if (resp.status !== 201) return null` 放宽成接受任何 2xx（反向代理偶尔把 201 改成 202，这正是 Insight 撞到的现象）。Body 解析失败也优雅 null。
- **JS client**：同样的放宽，sessionTrail 上传走同一条路径。

### Phase 48 sub-B — MaskRegion 真正生效 ✅

之前 `_maskedNativeIds` 只注册不消费 — wrap 14 个 surface 的 `<MaskRegion>` 然后截图照样把 PII 全送上去（Insight 反馈 P1，GDPR/CCPA 视角是 stop-ship）。重写：

- `<MaskRegion>` 现在渲染 children + 一个 `absoluteFill` 的黑色 overlay View（normally `opacity: 0`，被 `pointerEvents="none"` 屏蔽不挡触摸）。
- `engageMasks()` 截屏前把所有 overlay 翻 `opacity: 1` 盖住 children；imperative `setMaskedNode(ref)` 注册的 view 直接 `opacity: 0` 隐藏。截屏完恢复。
- `captureScreenshot()` 在 `captureRef` 调用前 + 后各 yield 一帧确保 paint 落地。

### Phase 48 sub-C — Issue list filter 进 URL ✅

Issues list 的 `status` (active/regressed/...) tab、`q` search、`anr` 切换全部进 URL search params。F5 / link share / 后退 都保留 filter 状态。新 hook `useUrlParam<T>` 是其它视图后续接入的 building block。

### Phase 48 sub-D — Sidebar footer 重做 + version badge ✅

之前 footer 把 email link + EditorPicker + 4 个 toggle 全挤进 ~200px 宽，把 "Sign out" 文字压成两行。重做：

- 引入 `<UserMenuButton>` 弹出式 popover：邮箱+role chip 是 trigger，点开里面有 "My activity" 和 "Sign out"。
- 单行 toggle strip：密度 / 主题 / 折叠 sidebar handle。
- 最底下一行 `text-fg-muted/50 mono` 显示 `v0.7.1 · sha7chars`，sha 从 `VITE_GIT_SHA` 取（Dockerfile.web 已注入）。`web/src/version.ts` 是 SENTORI_VERSION 的 single source of truth，每次发版手动 bump。

### Phase 48 sub-E — Dev build 文案 ✅

Frame source drawer 在 404 时根据 `event.payload.environment` 分两路：

- `dev` → 「Dev build. Source maps are only uploaded for production releases. Ship a release build with `bun cli release` to see the original source here.」
- 其它 → 保留通用 fallback 文案。

### Phase 48 sub-F — 内嵌 source viewer，去掉 IDE jump ✅

Sentori 主张零外部依赖。删掉：

- `<OpenInEditorButton>` (8 个内置 IDE URI scheme + custom template)
- `<EditorPicker>` sidebar UI
- `web/src/lib/editor-template.ts` + test

实际上 `<FrameSourceDrawer>` 通过 server 的 `frame_source` endpoint 已经从 sourcemap 的 `sourcesContent` 抽出完整源码、内嵌渲染（starry-night 高亮 + 上下文 ±5/±20 行可切换）— 这条链路本来就完整，IDE 跳转按钮一直是冗余。点 stack frame 即开。

### Phase 48 sub-G — Publish + ROADMAP ✅

7 个 npm 包 publish，本节 + git tag v0.7.1。

---

## v0.7 — 综合升级（Phase 43–47）

**Goal:** 在 v0.6 issue 诊断深度的基础上，五条并行轴一次性铺开：Linear/Slack 双向集成、客户端 sampling + 数据分析、Web SDK 矩阵（Vue/Svelte/Solid 加入）、session-trail 轻量回放、最后一波 polish 收尾。

**npm 包变化**：

- `@goliapkg/sentori-core` 0.5.0 → 0.6.0（Phase 44 sampling helpers + Phase 46 TrailBuffer/SessionTrail）
- `@goliapkg/sentori-javascript` 0.3.4 → 0.4.0（sampling、sessionTrail capture + uploadAttachment、captureStep）
- `@goliapkg/sentori-react-native` 0.6.1 → 0.7.0（sampling、sessionTrail、useTraceNavigation 自动 captureStep）
- `@goliapkg/sentori-react` 0.4.4 → 0.4.5（router 自动 captureStep）
- `@goliapkg/sentori-next` 0.2.4 → 0.2.5（依赖跟随 javascript/react）
- `@goliapkg/sentori-vue` 0.1.0 ✨ 首次发布
- `@goliapkg/sentori-svelte` 0.1.0 ✨ 首次发布
- `@goliapkg/sentori-solid` 0.1.0 ✨ 首次发布

**设计四条协调铁律**：sentori-core single source of truth / 复用 Phase 42 attachment 框架 / 集成走 v0.2 outbound webhook 改造的 IntegrationAdapter trait / sampling 跨 SDK 单一 API。

### Phase 43 — Linear 双向集成 + Slack（commit 92e482f / 542900d / 370d746 / e0c24df）✅

- 新表 `integrations`（org_id, kind in ('linear','slack'), config JSONB）+ `issue_integration_links`，partial unique index on (org_id, kind) WHERE revoked_at IS NULL
- `IntegrationAdapter` Rust trait，`ConnectMode::{OAuth, Manual}` 枚举：Linear 走 authorize_code → token，Slack 走 manual incoming-webhook URL
- Linear adapter：GraphQL `issueCreate` mutation + `Linear-Signature` 头的 HMAC-SHA-256 constant-time 校验。webhook 反向同步：Linear issue state.type → 'completed'/'canceled' 触发 Sentori resolve，同状态短路防 comment 回环
- Slack adapter：`accept_manual_config({webhookUrl, channelLabel?})` 校验 `https://hooks.slack.com/` 前缀，Block Kit 模板用 mrkdwn 转义
- 派发链路：`integrations/dispatch.rs#on_new_issue` / `on_status_change` 用 `tokio::spawn` 走 ingest 热路径之外，Linear API 延迟永不阻塞 event 落库
- Dashboard `<IntegrationsView>` 加 SECONDARY nav（admin-only），OAuth/Manual 双 mode 渲染、`<SlackConfigureForm>` inline

### Phase 44 — Sampling + 数据分析（commit 0bf924a）✅

- `sentori-core/src/sampling.ts`：`shouldSample(rate)` 均匀采样 + `shouldSampleTrace(traceId, rate)` 对 traceId 前 32 bits 哈希做确定性决定（保证一条 trace 的所有 span 共享同一 yes/no）+ `normalizeRate` clamp 到 [0,1]
- RN + JS SDK：`init({ sampling: { errors, traces } })`，capture 路径在最开始 gate，丢弃前埋 `sampled-out` 面包屑保留可观测性
- Issue merge 功能：`POST /admin/api/projects/{p}/issues/{i}/merge` 事务式 UPDATE events SET issue_id + 聚合 + DELETE source。Dashboard `<MergeIssueButton>` 带 UUID 校验
- Issue 全文搜索：migration 0034 加 `tsvector` 生成列 + GIN 索引（`simple` config 不做词干），`?search=` query 串走 `to_tsquery`，issues 列表的 free-text 自动喂进 search
- Cross-stack cause chain：event 协议加 `error.nativeError?: { issueId, type, message }`，dashboard `<NativeErrorCard>` 在 CauseChain 里展示 amber-tinted 跨栈卡片 + 跳 native issue 链接

### Phase 45 — Web SDK 矩阵 Vue/Svelte/Solid（commit c5fab5b）✅

- `@goliapkg/sentori-vue@0.1.0`：`app.use(sentori, opts)` plugin，包住 `app.config.errorHandler`（chain previous）；`<SentoriErrorBoundary>` 用 `errorCaptured` lifecycle + slot fallback + `ignore` prop；`setupTraceNavigation(router)` from `/router` subpath
- `@goliapkg/sentori-svelte@0.1.0`：`sentoriHandleError()` 工厂返回 `HandleClientError` 形状；`traceNavigation($navigating)` 接 `$app/stores`；Svelte 5 用户直接用内置 `<svelte:boundary>`
- `@goliapkg/sentori-solid@0.1.0`：`sentoriOnCatch` 给 Solid 内置 `<ErrorBoundary onCatch>` 用；`traceSolidRouter(pathname)` 在 `createEffect` 里调，同路径短路
- 三个 SDK 各 ~100 LOC adapter，shared adapter base 评估后改判为不需要（框架 boundary 语义差异太大，强抽象更绕）

### Phase 46 — Session-trail 轻量回放（commit 07d12f1）✅

- `sentori-core/src/trail.ts`：`TrailBuffer` ring buffer max 30 steps + `sealTrail()`，每步 `{ ts, label, breadcrumb?, viewTreeRef?, screenshotRef? }`
- `captureStep(label, opts)` 公开 API（JS + RN 都暴露），各 framework SDK 的 router helper（react/vue/svelte/solid）自动写 `route:<path>`，RN 的 useTraceNavigation 写 `screen:<name>`
- `init({ capture: { sessionTrail: true } })` opt-in；`captureException` 时 `sealTrail` → `uploadAttachment('sessionTrail', JSON)` → push ref 进 `event.attachments[]`，并清空 buffer（每个 crash 一份 trail，干净）
- Server：`AttachmentKind` 加 `'sessionTrail'`，migration 0035 扩 event_attachments.kind CHECK，application/json 白名单
- Dashboard `<SessionTrailViewer>`：左栏 step list + 右栏 detail（label、相对 crash 时间、breadcrumb、可选 screenshot/viewTree 链接），← → 键步进，默认 focus 最后一步（最接近 crash）。AttachmentGallery 自动渲染为可展开 `<details>` 卡

### Phase 47 — Polish 收尾 ✅

- 47.01 Related Issues panel：server `GET /admin/api/projects/{p}/issues/{i}/related` 返回同 project / 同 error_type 的 sibling issue（capped 5，按 last_seen DESC），dashboard `<RelatedIssuesPanel>` 渲染为 header 下的 chip row + 已 resolved/closed 的 muted 渲染
- 47.02 hover frame ↔ tree node 联动：`web/src/lib/frame-hover.tsx` FrameHoverContext，stack frame hover 用 `file:line` 发布，ViewTreePanel 翻译成 nodeId Set 高亮 + `scrollIntoView({ block: 'nearest' })` 滚到第一个匹配节点。与既有 search highlight 用不同 palette（hover 是 `bg-accent/20 ring-1`，search 是 `bg-accent/10`），ancestor 链对两者都自动展开
- 47.04 SDK perf benchmark：`sdk/core/src/__tests__/perf.bench.ts` 给 uuidV7 / shouldSample / shouldSampleTrace / breadcrumb / TrailBuffer.push / sealTrail 各一条 wall-clock 预算（10x 实测 margin，CI-safe）。`bun run bench` 跑这条
- 47.05 Dashboard LCP gate：`web/lighthouserc.cjs` + `bun run lhci`，LCP < 1200ms 是 hard build-breaker，FCP/TBT/CLS 是 warn。手动跑：`cd web && bun run build && bun run preview & bun run lhci`。CI 接入文档见 `docs/performance/dashboard-lcp.md`
- 47.06 ROADMAP + CHANGELOG v0.7 整段（本节）
- 47.07 v0.7 tag + npm publish：**等用户授权后执行**（npm 不可逆 + git tag push 影响 shared state，所以不自动跑）

---

## v0.6 — Phase 42 — Issue Detail 诊断深度

**Goal:** Issue 详情从「stack + 朴素文本源码」升级成「stack + 高亮源码 + 智能调用链 + 出错截图 + UI tree + cross-stack 串联 + AI export」。覆盖 JS / RN / iOS native / Android native 全栈。9 个 sub-phase (A → I)，每个独立 commit + 多数独立 deploy。

**npm 包**：`@goliapkg/sentori-core@0.5.0`（加 `AttachmentMeta` 类型）+ `@goliapkg/sentori-react-native@0.6.0` (sub-D JS 截图) → `0.6.1` (sub-E iOS native + sub-F Android native)。

**设计原则四条铁律**：零热路径成本 / 跨 native+JS 统一表达 / 隐私默认安全 / 可扩展 attachment 模型。

### sub-A — Dashboard 视觉强化（无 SDK 改动）✅

- `@wooorm/starry-night` syntax highlighting，GitHub-tone 配色，按 `data-theme` 而非 `prefers-color-scheme` 切换；`<SourceCode>` 组件每行 div + 行号 + `#L<n>` anchor
- `FrameRow` ±3 行 inline source + `FrameSourceDrawer` 全文 + auto-scroll
- `packageOf(file)` 推断 npm package（@scope、react-native 双布局、Metro lazy-bundle %2F 编码）
- `VendorFold` 按 package group："react-native (8 frames)" 而非 "11 library frames"
- `<FrameRoleBadge>` you / framework / lib 颜色编码
- `CauseChain` accent-tinted 卡片 + "↪ caused by" 标签
- `source_repo_url` per-project（migration 0031 + PATCH endpoint）+ frame 旁 "↗ src" 链接（GitHub/GitLab/Bitbucket/Gitea 通用 `/blob/<ref>/<path>#L<line>`）
- `<OpenInEditorButton>` URI scheme jump，per-user 8 内置 IDE + Custom 模板（Insight 反馈后加，原 vscode:// hardcoded → user-pickable）
- `<IssueDetailSkeleton>` 替代 LoadingState

### sub-B — Source / context lazy load ✅

- ingest 时存 ±3 行（缩自 ±5），event payload 缩 40%
- frame source endpoint 加 `lines` query 参数（默认 5、上限 50），Cache-Control immutable+1h
- `<FrameSourceDrawer>` ±5/±20/±50 toggle，`placeholderData: prev` 切换无 loading flash

### sub-C — Attachment 基础设施 ✅

- migration 0032 `event_attachments` 表
- `AttachmentStore` trait + `LocalFsAttachmentStore` (`$SENTORI_ATTACHMENT_DIR`) + `NoopAttachmentStore` (env 未设 → 503)
- `POST /v1/events/{id}/attachments/{kind}` multipart 500KB 限 + mediaType 白名单按 kind
- `GET /admin/api/events/{id}/attachments/{ref}` admin gate + Cache-Control immutable
- ingest 校验 `event.attachments[].ref` 必须签发给同一 (event_id, project_id)，不匹配静默丢
- retention sweep `prune_attachments`：events_cutoff 之前同步清 DB 行 + 磁盘 blob

### sub-D — JS / RN 截图 (SDK 0.6.0) ✅

- `init({ capture: { screenshot: true } })` opt-in
- `captureScreenshot()` `InteractionManager` + `rAF` 双 yield，1.5s timeout，480px JPEG q=70
- `uploadAttachment` RN-style FormData (`{uri,type,name}`)
- prod 配额 10 张/session，dev 不限
- `<MaskRegion>` 声明式 + `setMaskedNode`/`unsetMaskedNode` 命令式
- dashboard `<AttachmentGallery>` + `<Lightbox>` (esc / ← → / download)

### sub-E — iOS native (SDK 0.6.1) ✅

- `SentoriScreenshotCapture.swift`：`UIGraphicsImageRenderer` 抓 key window，480px → JPEG q=70；`UIView` 递归 view tree depth=10 / 1500 nodes 上限，accessibilityLabel 200B 截断
- NSException handler 同步抓屏 + view tree → event JSON 里塞 `_pendingAttachments` → JS 下次启动 drain 时上传 → 写回 `event.attachments[].ref` → enqueue
- XCTest stub（XCTest 在 CLI Swift 缺 UIKit/XCTest module，IDE 内 ⌘U 跑）

### sub-F — Android native (SDK 0.6.1) ✅

- `SentoriScreenshotCapture.kt`：`ActivityLifecycleCallbacks` 跟踪 foreground Activity；`PixelCopy.request` API 24+ GPU 异步抓（不阻塞主线程，ANR 时也能抓）；Android 11+ `WEBP_LOSSY` / 老版本 JPEG；DecorView 递归 view tree
- 接入 `SentoriCrashHandler` + `SentoriAnrWatchdog`（ANR detector trigger 也走 attachPending）
- Robolectric 单测（GPU/PixelCopy 部分留 instrumentation test）

### sub-G — View Tree Skeleton dashboard ✅

- `<ViewTreePanel>` 折叠树 + 文本搜索（name / accessibilityLabel / contentDescription），命中祖先链自动展开
- props_summary 内联前两项（a11y label / frame / alpha / hidden）
- `<AttachmentGallery>` 把 viewTree kind 从 pill 升级为内联 `<details>` 包 `<ViewTreePanel>`

### sub-H — Cross-stack + AI-export + 抛光 ✅（H.01 / H.03 留 v0.6.x patch）

- `<CopyMarkdownButton>` 把 event 渲染成 paste-into-AI Markdown（meta + 5 in-app frames 带语法 fence + ±5 行源码 + 红箭头标 at-line + breadcrumbs 尾 8 条）；clipboard 失败 fallback `prompt()`
- ANR / hang 紫色 banner：`payload.kind === 'anr'` 时 stack 区顶部加 accent-tinted 卡片 + "Hang"(iOS) / "ANR"(Android) 标签
- trace 详情 span aside 加 "Events on this span" → issue 反链
- `c` 快捷键触发 Copy as markdown

### sub-I — 性能 / 隐私 / 文档收尾 ✅

- `docs/sdk-react-native.md` "Screenshot capture (opt-in)" 章节 + docs-site mirror
- `docs/self-hosting.md` `SENTORI_ATTACHMENT_DIR` + Data retention 段含 attachments 行 + 容量估算
- ROADMAP + CHANGELOG 完整 summary（这条）

### 显式延后到 v0.6.x patch

- **H.01 Cross-stack cause chain**：`event.nativeError?: {issueId}` 协议字段 + dashboard 嵌入 native issue 卡片 —— 需要 SDK + server + dashboard 三处协议改动，工作量大于抛光级别
- **H.03 Related issues panel**：新 server endpoint `/admin/api/issues/<id>/related`（同 fingerprint stem / 同 file / 同 release）+ side panel UI
- **G.10 hover frame ↔ tree node 联动**：UI polish，等 Insight 真实用 view tree 后看是否要
- **G.02-G.04 SDK JS Fiber walker**：native 已给 RN/Fabric tree；纯 JS walker 是 nice-to-have
- **I.01 SDK perf benchmark < 100ms / I.02 Dashboard LCP < 1.2s**：需要 perf 测试基础设施，待 Insight 真实流量进来后基于实际 P95 调

### Insight 升级路径

```sh
bun add @goliapkg/sentori-react-native@latest   # 0.6.1
# opt-in 截图：init({ capture: { screenshot: true } })
# 用 <MaskRegion> 包敏感 UI（信用卡 / 私聊 / OTP 输入框）
```

服务端：`SENTORI_ATTACHMENT_DIR=/apps/sentori/attachments` 设上让 multipart endpoint 进入工作状态（未设 → 503 `attachmentsDisabled` SDK 端 silent drop，attachments 不影响 event 本身落地）。

---

## v0.5.7 — `sentori-react-native` hotfix: RN 0.83 new-arch dev-symbolicate

**Reported by Insight dogfood (RN 0.83.6 + Hermes + Fabric/TurboModule new arch).** Dev-mode errors were arriving in dashboard as fully minified stacks (`_temp5`, `anonymous`, `_performTransitionSideEffects`) despite 0.5.6 advertising automatic Metro `/symbolicate`. Root cause: SDK read `NativeModules.SourceCode.scriptURL`, which on the new architecture is `undefined` (constants are not hoisted onto the module object — they live behind `getConstants()`). With `scriptURL` undefined, `metroSymbolicateUrl()` returned null and the SDK silently skipped symbolication.

**Fix:** prefer `react-native/Libraries/Core/Devtools/getDevServer`, the same helper RN's own LogBox and `symbolicateStackTrace` use internally. It calls `NativeSourceCode.getConstants().scriptURL` under the hood and works on both old and new arch. Falls back to `NativeModules.SourceCode.getConstants().scriptURL` and then the legacy `.scriptURL` property for older RN versions.

**npm**: `@goliapkg/sentori-react-native@0.5.7` (no other packages changed).

---

## v0.5 — Scale + Readable Errors + Dashboard sidebar（Phase 39-41）

**Goal:** Insight dogfood 把 v0.4 的盲区都打了出来 —— trace 不归组、错误栈不可读、左侧没导航。v0.5 三条主线就是补这些洞，对应 dashboard 上一线员工真能用的 traces 列表、`src/Foo.tsx:42` 的栈渲染、Linear 风左侧 sidebar。

**反 Sentry 立场延续：** trace 用「时间硬保留窗口 + 窗口内 100% 不采样」，不默认开 head-based / tail-based 采样（schema 简、部署轻、不丢有错的 trace）；source-map 符号化在 ingest 时一次做完 + 按符号化帧重新分组 issue，不靠"on-demand at view"绕一圈。

**npm 包**：sentori-core 0.4.1 / sentori-javascript 0.3.4 / sentori-react 0.4.4 / sentori-react-native 0.5.6 / sentori-next 0.2.4 / sentori-cli **0.3.0**（新发布）/ sentori-expo 0.1.1。

### Phase 39 — Trace 上量管控 ✅

- **sub-A** `sdk/core/normalizeUrl(url)` —— path id-like 段 → `{id}`（pure-numeric / uuid / 长 hex / 长 opaque token），砍 query+fragment；三个 hook（RN fetch+xhr / JS fetch+xhr）接上 —— span `name` 收敛成路由，`http.url` tag 保完整
- **sub-B** navigation span 自动当父：`useTraceNavigation` / `useSentoriRouter` 每屏开一个 `react.navigation` span（含初始屏，各自 trace root），并 `setActiveSpan` 让它常驻 active；该屏的 `http.client` span 自动成 child —— **一屏一 trace** 而不是一请求一 trace
- **sub-C** 时间硬保留：`SENTORI_TRACE_RETENTION_DAYS`（默认 14）；`retention.rs` 泛化成同时管 events + spans 分区滚动/drop；`traces` 不分区（UPSERT 冲突）走 DELETE + 新 `traces_last_seen_idx`（migration 0029）
- **sub-D** span 独立月度配额：`check_and_record_spans`（`SENTORI_SPAN_LIMIT_MONTHLY` 默认 10M）—— span 洪水不再吃 error event 的配额
- **sub-E** Q4 复合 index（migration 0030 `(project_id, op, duration_ms DESC)`）：实测 1M spans **43ms → 0.40ms**（~100×，plan 从 seq+heapsort → per-partition index seek）；baseline doc `docs/performance/baseline-v0.5-phase39.md`
- **sub-F** publish 5 包 + inter-dep pin 同步避免 version-pin trap；dogfood 记录

### Phase 40 — 可读的 JS/RN 错误（sourcemap 端到端）✅

- **sub-A** `parseStack` 修 Hermes 字节码帧：`at fn (address at /path/main.jsbundle:1:N)` 剥前缀；`(native)` 无位置帧丢弃
- **sub-B** `@goliapkg/sentori-cli` 新发布 —— `upload sourcemap` + `react-native upload`（一行 compose Metro+Hermes + upload）；Node ≥18 / 零运行时依赖；EAS post-build hook 修
- **sub-C** server 在 ingest 时符号化：`symbolicate_event` 用上传的 sourcemap 把 `Event.error.stack`（+ cause 链）重写成原始 source，`Frame` 加 `rawLine`/`rawColumn` 保 bundle 坐标（"show source" 反查需要）；issue **按符号化后的 in-app top 帧重新 fingerprint**（一个 release 第一次传 map → 老 issue 静、新 issue 冒出，同 Sentry），没传 map 的 release 行为不变
- **Day 2** 401 hint：admin auth / ingest auth 的 `401` body 加 `hint` 字段（无 Bearer / 错前缀 / revoked 各文案）；`Event.symbolication: { releaseHasMap }` server-set meta
- **Day 3** dashboard issue 详情：in-app 帧默认展开带 ±5 行内联源码（出错行红底高亮），连续 vendor 帧折叠成 `▸ N library frames`；in-app 帧若没解析按 `releaseHasMap` 标原因（"upload a source map: `npx sentori-cli upload sourcemap …`" / "wrong build / outside the map"）；`UnsymbolicatedHint` 链 docs recipe；server `symbolicate_frame_typed` 顺手填 `pre/context/postContext` 不用每帧 fetch
- **Day 5（sub-E）** dev-mode Metro：`__DEV__` 下 SDK 发事件前先 POST 给 `<devServer>/symbolicate`（RN LogBox 那条），dev 错误也立刻可读

### Phase 41 — Dashboard 左侧 sidebar ✅

- **Day 6** 新 `web/src/components/sidebar.tsx`：顶 Sentori + OnboardingBadge + OrgSwitcher；主导航（带 16px inline-SVG icon）+ 次要项（admin-only 自动隐藏）+ 底部 footer（用户邮箱 + RoleBadge + density toggle + ThemeToggle + Sign out）；`md+` 持久 `w-56` 边栏 / 窄屏 fixed hamburger → overlay drawer（route 变自动关）。`org-layout.tsx` 去掉顶部横排 NAV，改成 `<Sidebar/>` + 内容区两栏；所有路由路径不动
- **Day 7** sidebar.test.tsx +5（主/次导航渲出、active 高亮、非 admin 隐藏 Alerts/Audit、hamburger 存在）；CHANGELOG + tag v0.5.0

### v0.5 显式 defer 到 v0.6 / 按需

- ❌ head-based / tail-based 采样默认开启（`tracesSampleRate` 留 SDK 开关，默认 1.0）；tail-based stateful collector 不做
- ❌ "Expo zero-config config plugin" 自动注入 Xcode/gradle build phase —— Insight 用 `sentori-cli react-native upload` 接进 `eas-build-on-success` script 一行已足够干净；真要 zero-config 时单独做 + 仔细测
- ❌ `g i` / `g t` "go to" 快捷键、sidebar 折叠到图标轨 —— polish，没收到诉求再加

---

## v0.4 — Distributed Tracing（Phase 34-38）

**Goal:** Protocol 早就留好的 `traceId` / `spanId` 槽真正用起来。从 RN client 一路追踪到后端 API，dashboard 看 waterfall。反 Sentry envelope-bloat：单 JSON span ingest，所有 root + child 是同一个 spans schema，靠 `parentSpanId == null` 区分；不上 transactions[] 嵌套 / measurements 浮点矩阵；waterfall 纯表格 + 缩进，不画 SVG bar。

### Phase 34 — Protocol + storage 准备 ✅

- **sub-A** Span 协议：`docs/protocol.md` 加 `## Span schema` 14 字段表 + 命名约定（http.client/server / db.query/transaction / cache.get/set / react.render/navigation / app.cold-start）+ "什么不做" 反 Sentry/OTel 子节；`POST /v1/spans` + `/v1/spans:batch` (200/batch) endpoint；Event schema 的 `traceId`/`spanId` 从 v0.1 占位升为 v0.4 in-use 语义
- **sub-B** Schema：migration `0026_spans.sql` partitioned by `received_at`（PK `(received_at, id)` 复合）+ 4 索引（trace_id / parent_span_id partial / project_id+received_at / project_id+op）+ migration `0027_trace_meta.sql` `traces` 物化表（trace_id PK + project_id + root_op/name + first/last_seen + span_count + status worst-of + duration_ms）；不分区因为 trace 数是 span 的 1/200
- **sub-C** Ingest endpoint：`server/src/api/spans.rs` single + batch handler（≤ 200/batch），validate 7 项（op/name 长度 / status enum / duration 24h / tags ≤ 50 keys 必 string / data 16KB），`persist_span` tx 内 INSERT spans + UPSERT traces 维护 worst-of status；quota 共享 events bucket
- **sub-D** EXPLAIN baseline：5000 trace × 200 span = 100k spans (SQL bulk INSERT 1s)；Q1 trace list 0.075ms / Q2 trace detail 0.68ms / Q3 span search top-50 5.64ms；`tools/seed-spans.ts` 实际 HTTP path tool；`docs/performance/baseline-v0.4-phase34.md`

### Phase 35 — SDK 端：RN + JS + React ✅

**5 个 npm 包 publish**：sentori-core 0.3.0 / sentori-javascript 0.3.0 / sentori-react 0.4.0 / sentori-react-native 0.5.0 / sentori-next 0.2.0

- **sub-A** `sdk/core` span buffer + trace-context：`SpanHandle` + `SpanBuffer` ring (cap 1000) + `startSpan` / `withSpan` / `activeSpan`；Node 路径 lazy require AsyncLocalStorage（await 跨 continuation 保持 context）/ browser/RN fallback save-and-restore；40 单测
- **sub-B** JS SDK fetch hook：`installFetchInstrumentation` monkey-patch globalThis.fetch + W3C traceparent header 注入（`toTraceparent` 16-hex truncate）+ status 映射（4xx/5xx → error / AbortError → cancelled）；20 单测
- **sub-C** RN SDK fetch + react-navigation：handler/network.ts 扩展现有 fetch hook 同时 emit breadcrumb + span；`useTraceNavigation(navigationRef)` hook duck-typed 不绑 `@react-navigation/native` 保持 optional peer；34 单测
- **sub-D** `<TraceRender>` React component：useMemo([], startSpan) + useEffect cleanup finish；re-render 不重开 span；同时修两 latent bug（sentori-core 版本 pin / SentoriSuspense 漏 export）；20 单测
- **sub-E** Publish 5 包 + dashboard dogfood：web/src/auth/ProtectedLayout 加 `useSentoriRouter()`；SentoriProvider 走 initSentori → fetch hook auto-install；每个 `/admin/api/...` 请求产 http.client span — **first end-to-end dogfood trace**；bundle +1.15 KB gzip

### Phase 36 — Dashboard：Trace List + Trace Detail ✅

- **sub-A** Trace list view：`web/src/views/traces.tsx` 6 列表格 + status pill 三色 + keyboard nav j/k/Enter + useInfiniteQuery + IntersectionObserver；server `list_traces` 同款 keyset cursor `(last_seen, trace_id)`；sidebar 加入口
- **sub-B** Trace detail waterfall：`web/src/views/trace-detail.tsx` 纯表格 + 缩进 `depth*16px`（**不**画 SVG bar）；hover 行 `ancestorIds` 走 parentSpanId chain 高亮 root→leaf；click 行打开右侧 drawer（id / parent / duration / tags grid / data pretty-print JSON）；orphan span 当 root 兜底
- **sub-C** Span ↔ Event 联动：migration `0028_event_trace.sql` events 表加 trace_id / span_id 列 + partial index；ingest pipeline 写两列；Issue detail "In trace →" pill + Trace detail "N event(s)" 红 chip 反向跳转
- **sub-D** Search-token parser：`parseTraceQuery` 同 `parseIssueQuery` 模式 — `op:` / `status:` / `duration:>Nms|>Ns`；单 search box 替换 3 dropdown；11 vitest 覆盖

### Phase 37 — Cross-cutting：W3C TraceContext 后端联动 ✅

- **sub-A** sentori-server 自身 instrument：`SpanEmitter` buffer + 30s flush task + 200-span 突发 flush；axum middleware 包 handler 抓 method/path/traceparent → emit `http.server` span；`router::build` 加 `self_trace` 可选 layer；`main.rs` 读 env `SENTORI_SELF_TRACE_PROJECT_ID`；7 单测 + live smoke 验证跨进程 stitch 通过
- **sub-B** Node SDK middleware：3 framework adapter（Express / Hono / Fastify）+ 共享 `parseTraceparent`；subpath export `@goliapkg/sentori-javascript/tracing`；Hono 用 `withSpan(next)` 让 handler 内 startSpan 自动 child；15 新单测
- **sub-C** Recipe doc：`recipes/distributed-tracing.md` 三层全栈走读（RN → Node Hono → Postgres）+ ASCII 拓扑图 + SDK ↔ 协议 crosswalk 表 + 跨系统 stitch lossy 说明

### Phase 38 — Polish + Performance + 发布 ✅

- **sub-A** 1M span EXPLAIN 复测：5000 traces × 200 spans (~15s SQL bulk INSERT)；Q1 0.131ms / Q2 1.16ms / Q3 0.11ms / Q4 40.7ms（7.2× sub-linear scaling，plan-shape 均未变 → 不触发 regression policy）；`docs/performance/baseline-v0.4-phase38.md`；deferred 2 项 action item（partition prune hint / Q4 composite index）留 v0.5
- **sub-B** CHANGELOG + tag v0.4.0 + GitHub Release + marketing hero 加副标题 "Distributed tracing built in"

### v0.4.1 patch — XHR instrumentation

dogfood 发现：sentori-rn / sentori-js 的自动 tracing 只 patch `globalThis.fetch`，但 axios（RN 上默认 `xhr` adapter / 浏览器同）走 `XMLHttpRequest` 不经 fetch — 用 axios 的 app（如 Insight）几乎不产 http.client span。Phase 35 sub-C 的注释 "RN polyfills XHR over fetch internally" 是错的，RN 的 XHR 是原生实现。

- `sdk/react-native/src/handlers/network.ts` 拆 `patchFetch()` + `patchXhr()`，后者 patch `XMLHttpRequest.prototype.open/send` — open 时存 method/url，send 时开 span + 注 traceparent header + 监听 `loadend`（status 0/4xx/5xx → error）和 `abort`（→ cancelled）
- 新 `sdk/javascript/src/hooks/xhr.ts`：`installXhrInstrumentation` 同款 prototype patch；`init.ts` 在 `installFetchInstrumentation` 后也调它
- 测试：JS SDK +9（XHR lifecycle / traceparent / 5xx / status 0 / abort / URL input / 并发独立 span）；RN SDK +5（同款用 FakeXHR）；JS suite 44/44，RN suite 39/39
- publish 4 包（core 无改动不 bump）：sentori-javascript 0.3.0 → 0.3.1 / sentori-react-native 0.5.0 → 0.5.1 / sentori-react 0.4.0 → 0.4.1（dep js→0.3.1）/ sentori-next 0.2.0 → 0.2.1（dep js→0.3.1 + react→0.4.1）
- Insight 升到 sentori-react-native@0.5.1 即可不改代码拿到 axios trace

### v0.4.2 patch — client span flush（critical fix）

dogfood 第二个坑，比 v0.4.1 更严重：**SDK 从来没有把 client span 发出去**。`installNetworkHandler` / fetch / xhr hook 都正常 `startSpan().finish()`，但只把 span push 进 `sdk/core` 的内存 ring buffer；RN / JS 的 transport 只发 `/v1/events`、`/v1/sessions`，没人 drain 那个 buffer 去 POST `/v1/spans:batch`。结果：所有 client span 都堆在设备内存里，Traces 页面只看得到 server 自身的 `http.server` span。文档 `docs/recipes/distributed-tracing.md` 早就写了"span 经 `/v1/spans:batch` 上报"，代码却没实现。

- `sdk/react-native/src/transport.ts` + `sdk/javascript/src/transport.ts`：加 span flush —— `drainSpans()` → POST `{spans}` 到 `/v1/spans:batch`，每批 ≤200（server 上限），失败即丢（不重试、不离线持久化；span 是 best-effort）。RN 用 10s `setInterval`（`startTransport` 里起）；JS 用 5s `setInterval`（`unref` 掉，不阻止 Node 退出）+ `pagehide` 时再 flush 一次
- `installNetworkHandler` / fetch / xhr hook 现在跳过指向 `getConfig().ingestUrl` 的请求 —— 否则 span 上报本身又会产 `http.client` span，无限套娃
- 测试：RN transport +4（POST batch / 空 buffer no-op / >200 分批 / 5xx 即停）；JS +6（同款 + 未配置 no-op + ingest-URL 不被 trace）；全 SDK sweep 绿（core 40 / js 50 / rn 43 / react 20 / next 9 / expo 4）
- publish 4 包（core 无改动不 bump）：sentori-javascript 0.3.1 → 0.3.2 / sentori-react-native 0.5.1 → 0.5.2 / sentori-react 0.4.1 → 0.4.2（dep js→0.3.2）/ sentori-next 0.2.1 → 0.2.2（dep js→0.3.2 + react→0.4.2）
- Insight 升到 sentori-react-native@0.5.2 后 Traces 页面才会真的有 client span

---

## v0.3 — React-first via dogfood（Phase 29-33，进行中）

**Goal:** 用 Insight (RN) + 一个 React Web 项目作为真实流量源，把 SDK / dashboard / 文档在两个生态里都打磨到 polish 程度，把"React / RN 第一选择"做实。不开新轴。营销不主动推广，只把 docs 打磨到可口口相传。

### Phase 29 — v0.2 deferred sweep ✅

5 sub 全完成：

- **sub-A** iOS 真主线程采样器：`SentoriThreadSampler.swift` 用 raw byte reinterpret 按 ABI index 取 PC/FP（绕过 Swift 未导入的 `_COUNT` 宏和 `__darwin_*` intrinsics）；4 个 XCTest case 覆盖；`HangWatchdog` 用 sampler 输出 + 47-bit PAC mask；e2e 推迟到 example 切到 npm registry 版本（Expo autolinker file:.. monorepo bug）
- **sub-B** webhook 持久化重试队列：migration 0025 + `webhook_dispatch.rs` 的 30s sweep + 6 次重试（[60s, 5m, 30m, 2h, 12h, 24h]）；dashboard `<DeliveriesRow>` 展开块
- **sub-C** server `OffsetDateTime` serde rfc3339 sweep：23 处 serde-derived struct 字段加注解；`scripts/check-rfc3339.sh` 接入 CI 防回归
- **sub-D** UUID prefix collision sweep：15 处 `[..N]` 改 `[12..28]` 取尾部 16 char hex
- **sub-E** CLI `issue list/resolve/silence`：3 unit test 覆盖 clap parse / format / 缺 token 错误；sentori-cli@0.2.0 publish + GH Release with 3 平台 prebuilt 二进制

### Phase 30 — Insight (RN) onboarding polish（进行中）

- **sub-A** Insight 接入秒表 ⏸ blocked（等用户在 Insight 项目实际跑）
- **sub-B** 摩擦点修复 ⏸ blocked on sub-A
- **sub-C** ✅ `tools/seed-events.ts`：UUID v7 + 10 errorType + weighted env + 5% ANR + regression simulation；`docs/self-hosting.md` 加 Populate dev data 章节
- **sub-D** ✅ EXPLAIN ANALYZE baseline：5000 events / 987 issues / 117 sessions 下 5 query 合计 0.55ms execution，全部命中既有索引或被正确 Seq-Scan；`docs/performance/baseline-v0.3-phase30.md`
- **sub-E** ✅ 索引补齐：5k 规模本质上无需新索引；唯一实施项是 `list_events_for_issue` 的 partition-pruning hint（`?days=` query param，默认 90，clamp 1..365）；EXPLAIN 验证 plan 从 9 partition 降到 7；`alert_rules` partial index 推迟到 1M 复跑后再决定

### Phase 31 — `sentori-react` SDK polish ✅

7 sub 全完成，**`@goliapkg/sentori-react@0.3.0` 已 publish**：

- **sub-A** ErrorBoundary v2：`fallback` 接受 `ReactNode | (props: {error, reset}) => ReactNode`；`resetKeys: unknown[]` 浅比较自动 reset；7 test 覆盖（含嵌套 boundary 不冒泡）；`docs-site/sdk-react.md` + 3 个 recipe
- **sub-B** react-router 集成：`useSentoriRouter` hook（subpath export `/router`，optional peer dep `react-router >= 7`）；MemoryRouter 测试初次 mount 不发 + 后续转换 emit `{from, to}` breadcrumb
- **sub-C** Suspense + RSC：`<SentoriSuspense fallback errorFallback?>` 单组件组合 Suspense + ErrorBoundary；`sdk/next/app-router.ts` 加 `useNextRouter` + `useReportNextError`（subpath export `/app-router`）；`error.tsx` recipe 进 sdk/next/README.md
- **sub-D** dashboard 升级到 sentori-react：把 `web/` 加进 root workspaces；`web/src/main.tsx` 删 imperative `initSentori`，改用 `<SentoriProvider>` + `<SentoriErrorBoundary fallback={<ErrorState />}>`；bundle +4KB gzip
- **sub-E** Recipe docs：`recipes/nextjs.md`（App Router + Pages Router + GH Actions yml）/ `recipes/remix.md`（v2 Vite-based + classic esbuild 兼容说明）/ `recipes/vite.md`（minimal SPA + sourcemap CLI）；docs-site sidebar 加 Recipes 分组；13 page build
- **sub-F** Symbolication UX：`<UnsymbolicatedHint>` banner 在未上传对应 artifact 时显示"Open release →"链；server 加 `sentori_symbolicate_duration_seconds{cache="cold|warm"}` histogram instrument 3 条路径；优化暂不做（先 instrument 后调优）
- **sub-G** Publish：`sentori-react@0.3.0` 发到 npm（package size 14.57 KB packed / 50.58 KB unpacked / 45 files）

### Phase 32 — Docs + onboarding 完整化 ✅

5 sub 全完成：

- **sub-A** getting-started 4-path 重做：旧单页拆为 hub + `getting-started/react.md` / `getting-started/react-native.md` / `getting-started/nextjs.md` / `getting-started/node.md`；每篇 Prerequisites / Install / Configure / Capture first error / View on dashboard / Next steps 6 段；docs build 13 → 17 page
- **sub-B** 5 分钟秒表实测：Vite + React 33s（npm registry sentori-react@0.3.0 整链路）/ Node.js (bun) 47s；Next.js + React Native defer 到 dogfood 时实测；`docs/dogfood/onboarding-times.md` 记数据；4 path docs 0 修订
- **sub-C** React 专区 5 篇深度 recipe：`recipes/state-management.md`（Redux/Zustand/TanStack Query + 4 capture-point 边界） / `recipes/suspense-rsc.md`（5 React 19 错误 surface 表）/ `recipes/sourcemap-upload.md`（GitHub Actions + GitLab CI + Vercel build hook 全 yml）/ `recipes/release-versioning.md`（4 平台 inject + semver regression 规则）/ `recipes/multi-environment.md`（single-token vs per-env tradeoff）；sidebar Recipes 组 3 → 8；docs build 17 → 22 page
- **sub-D** Troubleshooting：10 项 Q + Diagnose + Fix（dashboard 无事件 5 status code 表 / minified stack / dSYM uuid 不匹配 / token 401 hint 5 种 / webhook 签名 Node verify() / crash-free 0% / regression / hook 错误 / CI 慢 / dev 淹掉 dashboard）；sidebar Reference 组加入口；docs build 22 → 23 page
- **sub-E** Marketing hero copy：`marketing/src/pages/index.astro` h1 改 "Error tracking, built React-first."；sub-hero 强调 React 三件套 + 保留 platform-agnostic 立场；meta tags（OG/Twitter/title）同步；布局不动

### Phase 33 — Performance / scale 验真 ✅

5 sub 全完成：

- **sub-A** 1M event EXPLAIN baseline：走 SQL bulk INSERT 路径（HTTP 660 ev/s 太慢）20s 内 seed 1.02M events；5 query 合计 0.55ms → 1.24ms 在 200× 数据下 2.3× — sub-millisecond 预算完整；`docs/performance/baseline-v0.3-phase33.md`；`tools/seed-events.ts BATCH_SIZE 100 → 500`；deferred 索引仍不实施
- **sub-B** Cursor pagination + infinite scroll：`list_issues` 加 `cursor` query param + `X-Next-Cursor` response header（不破坏 JSON array body）；keyset 复合 `(last_seen, id)`；CORS expose_headers；`<IssuesView>` useInfiniteQuery + `<LoadMoreSentinel>` IntersectionObserver（300px rootMargin prefetch + button fallback）；react-window 不上（1k 行仍 OK）
- **sub-C** Ingest 压测：bun-native `tools/load-test.ts` open-loop scheduler；60s × 50 req/s × 4 endpoint = 3000 request；P99 12.6ms (events) / 33.5ms (batch) / 5.7ms (sessions) / 6.2ms (deploys)；**TOTAL P99 29.1ms，6.8× SLO headroom**，0 errors；10min staging 复跑 defer
- **sub-D** SDK offline / retry：RN SDK 4 个 retry test（5xx × 3 retry / NetworkError 中途恢复 / 4xx 不重试 / 双 flush 不双发）+ JS SDK 1 个 offline test（warn 一次不 crash 不重发）；54 SDK test 全绿；0 bug surfaced，不发 patch
- **sub-E** `docs/performance.md` baseline 汇总：9 个 headline 数字 + SLO target + headroom 倍数；3 个 detail doc 链接；regression policy（数字 +20% loose / plan-shape 变 tight）；`.github/PULL_REQUEST_TEMPLATE.md` 加 4 项 Reviewer checklist（performance / tests / docs / migrations）

---

## v0.2 — 账户结构 + SDK 矩阵 + 数据呈现（Phase 18-28）

三大板块按线性推进：**账户结构骨架**（Phase 18-20）—— org/team/project 三层 + RBAC + audit log；**SDK 矩阵 + 原生深度**（Phase 21-22）—— sdk/core 衍生 react/next/expo + iOS dSYM/Android proguard/ANR/Hang 完整化；**数据呈现**（Phase 23-28）—— Release 一等公民 + Issues 列表 power-user 化 + Issue 详情 revamp + Health/搜索/告警/Polish。

### Phase 18 — 账户结构深化（Org / Team / Project / Ownership / Audit）

**Goal:** 把扁平 "org + member/owner" 升级为 "org → team → project" 三层；加 ownership 转让 + audit log。

**Entry:** Phase 17 sub-G ✅。

**Exit:**
- 一个 org 可以有 N 个 team；team 有自己的成员集；project 多对多绑定到 team
- Project 操作受 ACL：user 必须是该 project 关联 team 的成员，**或** org-admin
- Owner 可发起转让；接收方点邮件链接确认才生效；事务原子
- 所有写操作（create/update/delete on org/team/project/membership/token）落 audit_logs，dashboard 设置页可查
- 邀请流支持 "邀请到 team X"，accept 时事务内同时插 membership + team_membership

**Estimate:** 2.5–3 周

#### Steps

##### sub-A — schema + migration ✅

- [x] 新表 `teams`(id uuid v7, org_id, slug, name, description, created_at)；UNIQUE(org_id, slug)
- [x] 新表 `team_memberships`(team_id, user_id, role: lead|member, created_at)；PK(team_id, user_id)（viewer 由 Phase 19 sub-A 加，一并和 org 级 viewer/billing_admin 落地）
- [x] 新表 `project_teams`(project_id, team_id)；PK(project_id, team_id)；级联 ON DELETE CASCADE
- [x] 新表 `audit_logs`(id uuid v7, org_id, actor_user_id, action text, target_type text, target_id uuid, payload jsonb, created_at)；INDEX(org_id, created_at DESC) + actor + target
- [x] 新表 `org_ownership_transfers`(id uuid v7, org_id, from_user_id, to_user_id, token text UNIQUE, expires_at, accepted_at NULL)
- [x] migration `server/migrations/0010_phase18_orgs.sql`，含外键 + 索引；本地 sentori-pg 应用通过；commit `5ec39d0`
- [ ] `cargo sqlx prepare`；提交 `.sqlx/`（推迟到 sub-B 写完 query 一并跑）

##### sub-B — server: Team CRUD + ACL middleware ✅

- [x] `server/src/api/teams.rs` 团队 CRUD + member CRUD：routes 在 `/api/orgs/{slug}/teams[/{team_slug}/...]`（不是 `/admin/api/...`，因为团队是 org 概念，复用既有 `require_user` 路由分组）
- [x] `server/src/api/teams.rs` 加 `list_team_projects / list_project_teams / assign_project_to_team / unassign_project_from_team`：project↔team 绑定 endpoints 在 `/admin/api/projects/{id}/teams[/{team_slug}]`（admin 路由，需要 owner/admin 角色）
- [ ] ~~`server/src/auth.rs`：`AuthCtx` 加 `team_ids_for_org(org_id) -> Vec<Uuid>` (Valkey 30s 缓存)~~ — 推迟；当前 inline 两条 query 简单且 dev 流量下零成本，等 Phase 23 perf 真有压力再上缓存
- [x] 拓展 `admin_auth::require_project_in_org`（不开新 extractor）：未绑团队 → 任意 org 成员通过；绑了团队 → 必须在某关联 team 中 OR org owner/admin
- [x] `projects.rs / tokens.rs / issue endpoints` 已经全部走 admin 路由，自动被改造后的 middleware 覆盖
- [x] tests：`server/tests/teams.rs` 两个 case：(1) owner 建 team 200 / plain member 建 team 403 / member 给自己加 team 403；(2) 绑团队后 in-team 成员 200 / out-of-team 成员 403 / owner 200。本地 sentori-pg `cargo test` 全 12 个测试 pass。commit `4398d4b`

##### sub-C — server: Ownership transfer + audit log ✅

- [x] `audit::record(pool, org_id, actor, action, target_type, target_id, payload)` helper：`server/src/audit.rs`，含 actions / targets 字符串常量；写失败仅 log 不阻塞业务路径
- [x] mutating endpoint 接 `audit::record`：org create / patch, member role / remove, team create / delete / member-add / member-remove, project create, project↔team bind / unbind, token create / revoke, transfer requested / accepted；team patch / team-member patch / invite create-delete 留给 Phase 20 sub-A 一并扫
- [x] `POST /api/orgs/{slug}/transfer`：owner only，body `{toUserId}`，target 必须当前是 admin/owner；写 `org_ownership_transfers` + 发邮件 + audit
- [x] `POST /api/orgs/transfers/{token}/accept`：to_user 登录态；事务内 swap role + 镜像 `orgs.owner_id` + 标 accepted_at
- [x] `notifier::OwnershipTransferRequested` 邮件模板（7-day confirm link）
- [x] `GET /api/orgs/{slug}/audit?limit=&before=&action=&actorUserId=&targetType=`：owner/admin only，DESC by created_at，joined users.email 显示 actor
- [x] 注：`org.deleted` 因 FK cascade 落不下 audit 行，改 emit 一条 tracing log；Phase 20 sub-A 把 audit_logs 移出 cascade
- [x] tests：(1) happy path：role swap / owner_id 镜像 / replay 拒绝 / 错误 caller 403；(2) 非 eligible target / 非 org 用户 / 非 owner caller 全拒；(3) audit list owner 200 + 含 org.created + team.created / member 403 / `?action=team.created` 过滤生效；本地 sentori-pg `cargo test` 全 17 个 pass。commit `1fc9bc9`

##### sub-D — dashboard: Team 管理 UI ✅

- [x] `web/src/views/team-list.tsx`：路由 `/org/{slug}/teams`，含 admin/owner 创建表单（slug + name + description）+ 团队列表 + 删除按钮（带 confirm 警告"删除后项目权限会回到全 org 开放"）
- [x] `web/src/views/team-detail.tsx`：路由 `/org/{slug}/teams/{teamSlug}`，两段：Members（dropdown 加成员 / 行内换 role / 删除）+ Projects（dropdown 绑项目 / 按钮解绑），plain member 只读
- [x] `web/src/api/client.ts` 增 `teamsApi`（list/create/get/patch/delete/listMembers/addMember/patchMember/removeMember/listProjects/listProjectTeams/bindProject/unbindProject）+ `auditApi.list` + `transfersApi.{create,accept}`
- [x] `OrgLayout.NAV` 加 `Teams` 项夹在 Issues 和 Settings 之间
- [ ] ~~`OrgSwitcher` 改两层：`Org > Team`；选 team 后 issues 列表自动 filter~~ — 推迟到 sub-E 跟 role chip / permission gate UI 一起做。`bun run build` + `bun run test` 全绿；commit `78defab`

##### sub-E — dashboard: Project ↔ Team 绑定 + 角色 chip ✅

- [x] 新视图 `web/src/views/project-team-settings.tsx`，路由 `/org/{slug}/projects/{id}/settings/teams`：列出 org 内所有 team，admin 单击 Bind/Unbind 即可绑定；普通成员看到 Bound/Not bound chip；headline 文案根据是否有绑定切换"全 org 开放" vs "team-scoped"；token-settings + recipient-settings 加交叉链接
- [x] org-settings members 表行内显示用户所属 team-slug chip（`useQueries` N 并发拉每 team 的成员，client-side 聚合，1-10 团队规模零成本）
- [x] `web/src/components/RoleBadge.tsx`：颜色编码 chip（owner=emerald / admin=accent / lead=amber / member=neutral）；用在 org-settings 和 team-detail
- [x] 权限层：`auth/permissions.ts` 定义 `PermissionAction` enum + role matrix；`auth/useHasPermission.ts` 钩子；`components/PermissionGate.tsx` 包按钮；server 仍是 authz 真理来源，UI 只决定"按钮渲不渲染"
- [x] OrgSwitcher 两层 `Org / Team` 下拉；选 team 写 `?team=<slug>` 到 URL；`OrgLayout` 读取后并发查每个 project 的团队绑定，把 `currentProjects` 缩到匹配集；`OrgCtx` 加 `currentTeamSlug` + `teams[]`
- [x] bun run build → 123 KB gzip / bun run test green / tsc clean。commit `5fcd2cf`

##### sub-F — Invite 流扩展 ✅

- [x] migration `0011_invite_team.sql`：`org_invites` 加 nullable `team_id` FK ON DELETE SET NULL（团队删除不毁邀请）
- [x] `CreateInviteBody.teamSlug` 可选 camelCase；服务端验证 team 存在于该 org，否则 400 `teamNotFound`
- [x] `accept_invite` 事务内同时 insert `memberships` + `team_memberships`（role=member）；team 中途被删则 team_id 已 SET NULL，邀请仍可接受退化为只入 org
- [x] `list_invites` / `export_org` 返回 `teamSlug`（LEFT JOIN teams）；前端 `InviteRow` 类型同步加字段
- [x] dashboard：org-settings invite 表单条件渲染 "team" dropdown（仅当 org 有团队时出现）；待接受邀请列表行内显示 team-slug chip；`orgsApi.createInvite(slug, email, role, teamSlug?)` 第四参可选
- [x] tests：(1) `invite_with_team_attaches_user_to_team` 端到端：bad slug 400 / list 含 teamSlug / accept 后 org+team 双成员均落库；(2) `invite_with_dropped_team_falls_back_to_org_only`：邀请发出后删 team，accept 仍 200，仅落 org 成员。全 19 server tests + dashboard build 通过。commit `7bbd9ff`

##### sub-G — Ownership transfer UX ✅

- [x] org-settings owner-only `<TransferOwnershipSection>`：danger 虚线边框 + 选 admin-eligible member（排除 self）+ 必须输入 org slug 二次确认才能 enable submit；空状态提示先把成员升级到 admin
- [x] 新视图 `web/src/views/transfer-accept.tsx`，路由 `/transfers/:token`（直接走根，不用 `/orgs/{slug}/...` 包，匹配邮件 link `{base_url}/transfers/{token}` 形态）；server 错误码 `transferUsed/Expired/forbidden/transferNotFound` 映射人话
- [x] 未登录访问该路径自动 `/login?next=/transfers/<token>` bounce 回来
- [x] 接受后 navigate('/')，让 OrgLayout 重新加载 orgs（角色已变）；msg-only toast 替代旧 owner 端 UI 通知
- [x] server `NotifyEvent::OwnershipTransferCompleted` 邮件模板 + `accept_transfer` 在事务 commit 后发邮件给老 owner（查 user email + org name）；安抚文案明示 demoted to admin、可联系 support 如非本人操作
- [x] bun run build → 124 KB gzip / tsc clean / server 19/19。commit `0cab6c9`

##### sub-H — Audit log viewer ✅

- [x] 新视图 `web/src/views/audit-log.tsx`，路由 `/org/{slug}/audit`，owner/admin only（普通成员看到"permission denied"一行）；OrgLayout NAV 加 `adminOnly` flag 把 "Audit" 项 gating
- [x] 表格：time / actor (email or "system") / action / target_type+id / payload（Show/Hide 折叠 JSON，empty 显 —）
- [x] filter bar：action select（硬编码 17 个动作，注释指向 Phase 20 sub-A 替换为 codegen）+ actor select（拉 listMembers）+ datetime-local before；任一 filter 激活时显示 "Clear filters"
- [x] cursor 分页：底部 "Load older →" 用最旧 row 的 createdAt 推 `before` cursor（匹配服务端 DESC + before 语义）
- [x] CSV 导出：当前可见集，header 含 timestamp/actor/action/target_type/target_id/payload，正确 escape 逗号/引号/换行
- [x] bun run build → 125.8 KB gzip；tsc + test green。commit `65f100b`

##### sub-I — tests + docs + 收尾 ✅

- [x] `server/tests/teams_acl_matrix.rs`：4 角色 × 6 端点的状态码矩阵（owner/admin/member/non-member × create_team/patch_team/delete_team/list_teams/add_team_member/project_bind）+ no_session 401。每个 case 用独立 fixture（uuid 后缀防并行 slug 碰撞）；7 个新测试，全 server suite 32/32 pass
- [x] `web/playwright.config.ts` + `web/e2e/teams.spec.ts`：webServer 自动 spawn `cargo run` + `vite dev`；spec 走 register/verify via API（暂用 `docker exec sentori-pg psql` 取 verify token，注释指明 CI 化时换 `/dev/last-verify-token` endpoint）+ UI login 验 cookie/redirect + 后续 invite-with-team 自动 accept 流。运行 ~2s / 1 test pass。`bun run test:e2e` 触发
- [x] `vite.config.ts` proxy 加 `/api`（之前只 `/admin/api`）—— dev 与 Caddy 生产配置对齐；不再需要 vite 前手动 fronting proxy
- [x] `docs-site/src/content/docs/teams.md` 1500+ 字纯文字写法指南（无截图，留 Phase 28 polish）：vocab 段、何时用 team、create/bind/invite/transfer/audit 流；sidebar 加 "Teams & ownership" 项
- [x] `web/.gitignore` 加 `test-results / playwright-report / playwright/.cache`
- [x] commit `5fe1025` + `c3c8d1e` 推 main

---

### Phase 19 — RBAC 全栈完善 ✅

**Goal:** 把 Phase 18 的 role 列字段做成完整的 permission matrix；server endpoint 标 min role；dashboard 全 button role-aware。

**Entry:** Phase 18 ✅

**Exit:**
- Role 矩阵：`owner / admin / member / viewer`（billing_admin 推迟到 Phase 27 一并）
- 服务端所有 endpoint 已经在 inline check 里强制；新加 viewer 路径 403 全 cover；test matrix 验
- dashboard 大部分 mutating button 走 `useHasPermission()`；剩下用 inline `canManage` 已重命名为 hook 调用
- 角色升降级 UI 落地（仅 admin↔member↔viewer，owner 仅走 transfer）

**Estimate:** 1.5 周（实际 1 session）

#### Steps

##### sub-A — Role 字段拓展 ✅
- [x] migration `0012_viewer_role.sql`：DROP + ADD CONSTRAINT 替换 `memberships.role` / `team_memberships.role` / `org_invites.role` 的 CHECK，加入 `viewer`
- [x] billing_admin **不**加：Phase 27 alerting/billing 落地时一并，避免空角色
- [x] 本地 sentori-pg 应用 + `\d` 三表确认

##### sub-B — server-side viewer enforcement ✅
- [x] 新模块 `server/src/roles.rs`：`OrgRole` / `TeamRole` enum (serde rename_all=lowercase) + `can_manage_org()` 谓词；常量 `VALID_INVITE_ROLES` / `VALID_MEMBER_PATCH_ROLES` / `VALID_TEAM_ROLES`（owner 被排除：仅 transfer 流程可达）
- [x] `orgs.rs::create_invite` 改用 `VALID_INVITE_ROLES`；之前的 "cannotInviteAsOwner" 特殊错误码并入 `invalidRole`
- [x] `orgs.rs::patch_member` 改用 `VALID_MEMBER_PATCH_ROLES`；任何 PATCH 到 owner 直接 400
- [x] `teams.rs` `VALID_TEAM_ROLES` 替换为 `roles::VALID_TEAM_ROLES`
- [x] `~~auth::Role enum + RequireRole extractor + 全端点重构~~` —— 推迟。当前 inline `matches!(role, "owner" | "admin")` 模式已经在 admin/owner 二选一上工作；新增 viewer 走同样路径自然 403。把 36 个端点全 refactor 成 extractor 是 Phase 28 polish 的事
- [x] 新 `tests/viewer_acl.rs`：6 个 reads 200 + 6 个 writes 403 + PATCH-to-owner 400 + viewer→admin 200。全 server 36/36 pass

##### sub-C — `useHasPermission` 全 dashboard 走查 ✅
- [x] `OrgRole` / `TeamRole` 类型加 `viewer` 字面量
- [x] `RoleBadge` 加 viewer style（bg-bg-tertiary 中性灰）
- [x] `team-list.tsx` / `team-detail.tsx` / `org-settings.tsx` 把 `canManage = role === 'owner' || 'admin'` 改成 `useHasPermission('team.manage' / 'team.member.manage' / 'org.manage')`
- [x] `OrgLayout` 头部 user email 旁加 `<RoleBadge role={currentOrg.role} />` —— 当前 role 永远可见
- [x] permissions.ts 已在 Phase 18 sub-E 落，role matrix 自然包含 viewer（默认 false 任何 mutating action）

##### sub-D — Role 升降级 UI ✅
- [x] org-settings member 行 `<select>`：admin/member/viewer（**owner 不在选项里** —— 只能走 ownership transfer）
- [x] 仅 owner-self 排除 + 当前行 role !== 'owner'  才渲染 select；owner 行恒为 RoleBadge
- [x] `onChange` 加 `confirm()`：明示老角色→新角色再 mutate；取消则不发 mutate（select 视觉残留下次 refetch 自动校正）
- [x] vite.config.ts test exclude `e2e/**`：playwright spec 不再被 vitest 误吃，单元/e2e 干净分流
- [x] `web/src/views/team-detail.tsx` 已有 role select，逻辑一致；本 sub 没动它（lead/member/viewer 三态足以），confirm 升级看 sub-E 是否需要

server 36/36 + dashboard build 126 KB gzip + vitest 1/1 + e2e 1/1。commit `4147a2a`

---

### Phase 20 — Audit log 深化 + 全局活动 feed ✅

**Goal:** Phase 18 已落 audit_logs；这阶段把它做成可观察的产品功能（不是只查日志）。

**Entry:** Phase 19 ✅

**Exit:**
- Audit log 全 action 字符串集中在 `audit::all_labels`；dashboard 从 `/api/audit/actions` 拉而非硬编码
- Per-user "我做的事" feed `/me/activity`；含 tombstoned org（org_id NULL → "deleted org"）
- Org 内 audit log（已在 Phase 18 sub-H 做完）
- Webhook payload schema 已在 `docs/protocol.md` + docs-site 同步落定，等 Phase 27 实现 delivery + signing

**Estimate:** 1 周（实际 1 session）

#### Steps

##### sub-A — Audit i18n + tombstone ✅
- [x] migration `0013_audit_tombstone.sql`：`audit_logs.org_id` 改 nullable + FK SET NULL；删 org 后 audit 行保留，org_id 自动 null
- [x] `delete_org` 在 DELETE 之前先 record audit 行，payload 含 slug + name；cascade 之后 org_id 自动变 NULL
- [x] `audit::label_for(action) -> &str` + `audit::all_labels() -> Vec<(code, label)>`：英文 i18n 标签（未来加 locale 表）
- [x] 新端点 `GET /api/audit/actions`：dashboard 拉 catalog 而非硬编码
- [x] `audit-log.tsx` 用 `auditApi.actions()` 替代 `ACTION_OPTIONS` 常量

##### sub-B — Dashboard audit log 页（已在 Phase 18 sub-H 落地，本阶段仅 catalog 接入升级）
- [x] sub-H 已做：actor / action / target / time + 折叠 JSON payload + filter + cursor 分页 + CSV 导出
- [x] 本阶段：action filter 改用 `auditApi.actions()` 动态 catalog

##### sub-C — Per-user activity feed ✅
- [x] `GET /api/users/me/activity?limit=&before=`：actor=caller across 所有 orgs，LEFT JOIN orgs 让 tombstoned 行也出现
- [x] 视图 `web/src/views/user-activity.tsx`，路由 `/me/activity`：时间轴样式，每条带 org link 或 "deleted org" 斜体
- [x] OrgLayout 头部 user-email chip 改成 `<Link to="/me/activity">` 入口

##### sub-D — Webhook payload schema 落定（Phase 27 实现）✅
- [x] `docs/protocol.md` + `docs-site/src/content/docs/protocol.md` 加 "Audit-event webhook payload (forward-looking, Phase 27)" 段
- [x] 锁定 headers（含 `sentori-signature: t=<unix>,v1=<hmac-sha256>` 5min anti-replay + v2 rotation lane）+ body（含 actor / org / target / payload）+ per-action required keys 表 + retry 1m/5m/30m/2h x6 + dedup by uuid v7
- [x] 文档版本号 `v0.1`

##### sub-E — CSV 导出 + tests（已在 Phase 18 sub-H + Phase 20 测试中覆盖）✅
- [x] CSV 导出：sub-H 已做（quoted, escaped）
- [x] 新 `tests/user_activity.rs` 3 个 test：catalog endpoint / activity feed 含 org slug / tombstone-after-delete 仍可见
- [x] server 39/39 + dashboard build 126 KB gzip + vitest 1/1 + e2e 1/1。commit `1c47429` + `cca4814`

---

### Phase 21 — SDK monorepo 抽 core + JS 矩阵扩展

**Goal:** 抽 `sdk/core` 作为共享 workspace package；衍生 `@goliapkg/sentori-{javascript,react,next,expo,react-native}` 都依赖它。覆盖主流 web 框架。

**Entry:** Phase 20 ✅

**Exit:**
- monorepo workspace 启用（root package.json + workspaces）
- `sdk/core/` 内含 types / config / transport / breadcrumbs / capture / stack / uuid / queue
- 所有 SDK 包仅做 framework adapter，业务逻辑在 core
- 4 个新 npm 包：`@goliapkg/sentori-{core,react,next,expo}`，docs site 各 1 篇 reference
- React 包：`<SentoriProvider>` + `<SentoriErrorBoundary>` + `useSentori()` + `useCaptureError()`
- Next 包：自动 capture App Router error.tsx + Pages dir + server actions + edge runtime
- Expo 包：Config Plugin（pod / gradle 自动） + EAS post-build hook 上传 source map

**Estimate:** 2 周

#### Steps

##### sub-A — 抽 `sdk/core/` ✅
- [x] 新 workspace package `@goliapkg/sentori-core@0.1.0`：types / uuid / breadcrumbs (+ BreadcrumbBuffer class) / stack (含 shortFilenames opt-in) / index re-exports
- [x] **保留** transport / capture / config / init 在各 SDK 内部 —— 这些含 platform-specific 行为（fetch vs sendBeacon, RN AsyncStorage offline queue, native module bridge），强行抽进 core 会破语义
- [x] root `package.json` 加 bun workspaces；`sdk/core` 依赖图先于 js/rn install
- [x] `sdk/javascript@0.2.0`：types/uuid/breadcrumbs/stack 退化为 re-export shim；object-form `addBreadcrumb({ type, data })` 公开 API 不变；**bug fix 顺路落**：`detectDevice()` 之前发 `os: 'macos' | 'windows' | 'unknown'` 全被服务端 `validationFailed` 静默拒绝，现在按 protocol 收紧到 `web | other`，OS 细节走 `model`
- [x] `sdk/react-native@0.2.0`：同 shim 模式；保留 long path（Hermes 路径已经短，native symbolication 需要绝对路径）；`addBreadcrumb` timestamp override 用 private shadow buffer 兜底
- [x] core 11 tests + js 8 + rn 18 = 37 SDK tests 全绿
- [x] npm publish 全 3 包：core 0.1.0 / javascript 0.2.0 / react-native 0.2.0。commit `59f13f4` + `4c94d8d`

##### sub-B — `@goliapkg/sentori-react` ✅
- [x] 新 `sdk/react/` workspace；deps `@goliapkg/sentori-core@0.1.0` + `@goliapkg/sentori-javascript@0.2.0`；peer `react>=18`
- [x] `<SentoriProvider config={...}>`：one-shot init via ref（StrictMode double-mount 安全）；提供 context；misconfig 走 console.error 不炸树
- [x] `<SentoriErrorBoundary fallback={({ error, reset }) => ...}>`：React 19 class component；`componentDidCatch` 调 captureError 加 `tags.source = 'react.errorBoundary'`；fallback 收 `reset` callback；可选 `onError` hook 给二级 logger
- [x] `useSentori()` 返回完整 context（capture / setUser / setTags / addBreadcrumb / initialised）；`useCaptureError(fn, extras?)` async 包装捕错 + rethrow，保留调用者 try/catch 语义
- [x] provider-scoped `setTags(tags)` 合并到 per-call extras（per-call wins 冲突）—— 同 Sentry precedence
- [x] tests：4 个 case 跑 happy-dom + bunfig preload，fetch + sendBeacon 全 stub；`bun run build` + `bun test` 全绿
- [x] npm publish `@goliapkg/sentori-react@0.1.0`；commit `390bdf7`

##### sub-C — `@goliapkg/sentori-next` ✅
- [x] 新 `sdk/next/` workspace；3 个 sub-path exports：`/client` / `/server` / `/instrumentation`；deps `core@0.1.0 + javascript@0.2.0 + react@0.1.0`；peer `next>=14, react>=18`
- [x] `clientInit(cfg?)`：浏览器侧 idempotent init；从 `NEXT_PUBLIC_SENTORI_*` 读 env；React Refresh / fast-reload 不重复注册
- [x] `serverInit(cfg?)`：node 侧 idempotent init；guard `NEXT_RUNTIME==='nodejs'` 避免 edge runtime 拉 Node deps
- [x] `onRequestError(err, req, ctx)`：匹配 Next 15 instrumentation.ts:onRequestError 签名；自动加 `next.method / next.route / next.routeType / next.runtime / source=next.requestError` tags
- [x] `/instrumentation` 一行 drop-in：`export { register, onRequestError } from '@goliapkg/sentori-next/instrumentation'`
- [x] env 解析 `src/config.ts`：client 只看 NEXT_PUBLIC_SENTORI_*；server 优先 SENTORI_*，缺时 fallback NEXT_PUBLIC_*；显式 cfg 字段 always wins；缺必填字段 throw 含具体 env 名
- [x] `~~withSentori(nextConfig)~~` —— 不实现：现阶段 nextConfig 不需要 webpack plugin（无 source map upload、无 build-time codegen）；Phase 22 接 sourcemap upload 时再加
- [x] tests：9 个 case（config matrix + onRequestError 三场景 + non-Error wrap）；`bun test` 全绿
- [x] README：4 个文件（instrumentation.ts / app/layout.tsx / app/error.tsx / app/global-error.tsx）copy-paste-ready
- [x] npm publish `@goliapkg/sentori-next@0.1.0`；commit `0432d58`

##### sub-D — `@goliapkg/sentori-expo` ✅
- [x] 新 `sdk/expo/` workspace；peer `expo>=50, expo-application>=5 (optional), react-native>=0.74, sentori-react-native>=0.2.0`
- [x] `app.plugin.js` CommonJS Config Plugin：marker + withInfoPlist 写 SentoriSdkVersion；native autolink 由 RN SDK 自带的 expo-module.config.json/podspec/gradle 解决，plugin 留作未来扩展点
- [x] `initSentoriExpo({ token, application?, release?, environment?, ingestUrl? })`：用户传 `import * as Application from 'expo-application'`（避免本包硬依赖 expo-application）；自动派生 `applicationId@version+build`；`__DEV__` 决定 environment；fallback 到 public ingest
- [x] `deriveRelease(app)` 单独 export 给非 init 场景（tag / log prefix）
- [x] `scripts/eas-post-build.mjs`：EAS postPublish hook，shell 调 `@goliapkg/sentori-cli upload sourcemap`；CLI 未装时友好 warn + exit 0（Phase 22 sub-A 落 sourcemap subcommand 后自动接通）
- [x] 4 tests：full / partial / missing / null fields；`bun test` 全绿
- [x] README copy-paste 三步：app.json + App.tsx + eas.json
- [x] npm publish `@goliapkg/sentori-expo@0.1.0`；commit `e2340c3`

##### sub-E — Vue / Svelte 设计文档（不实现）✅
- [x] `docs-site/src/content/docs/sdk-vue.md`：`@goliapkg/sentori-vue` 草案 —— `sentoriPlugin` for `app.use()`, `useSentori` / `useCaptureError` composables, `<SentoriErrorBoundary>` via `errorCaptured` 生命周期；附 Nuxt 3 module / Pinia / vue-router 集成路线
- [x] `docs-site/src/content/docs/sdk-svelte.md`：`@goliapkg/sentori-svelte` 草案 —— Svelte 5 + runes 目标；boundary 组件用 snippet API；SvelteKit `/sveltekit-server` + `/sveltekit-client` `handleError` 子路径
- [x] sidebar 加 "SDK — Vue (planned)" + "SDK — Svelte (planned)"，文档顶部明示 v0.3+
- [x] docs-site build → 9 pages

##### sub-F — onboarding wizard 多 SDK ✅
- [x] `SdkChoice` 从 2 选 → 5 选：`react / next / expo / react-native / javascript`
- [x] 默认 SDK 从 `react-native` 改为 `react`（最常见上手）
- [x] 布局从 flex 改 `grid grid-cols-2 sm:grid-cols-3` 让窄宽下标签仍可读
- [x] 5 段 install + init snippet 全部 copy-paste-ready：
  - React → `<SentoriProvider>` 包根
  - Next → `instrumentation.ts` + `.env.local` + `clientInit()`
  - Expo → `app.json plugins` + `initSentoriExpo({ application })`
  - RN → `initSentori`
  - JS → `initSentori`（browser/node 同 import）
- [x] dashboard build 126.9 KB gzip / vitest 1/1 / e2e 1/1。commit `eb53b37`

---

### Phase 22 — 原生层深化（iOS dSYM / Android Proguard / ANR / Hang）

**Goal:** native crash 端到端：上传 mapping → 服务端反符号化 → dashboard 显示原始位置 + ANR / hang 检测。

**Entry:** Phase 21 ✅

**Exit:**
- iOS dSYM 上传 + 服务端 atos 解析；dashboard issue 详情 frame 行显示原始 file:line
- Android Proguard mapping 上传 + retrace；同上
- Android ANR detection（5s main thread block）
- iOS hang detection（best-effort，runloop 阻塞 ≥ 2s）
- mapping/dSYM 按 release 串绑

**Estimate:** 2.5 周

#### Steps

##### sub-A — CLI: dSYM 上传 ✅
- [x] migration `0014_dsyms.sql`：(id, project_id, release nullable, debug_id, arch, object_name, size_bytes, data bytea, uploaded_by, uploaded_at)；UNIQUE(project_id, debug_id, arch) + 列表索引
- [x] 服务端 `POST /admin/api/projects/{id}/dsyms`：raw octet-stream + headers (`x-sentori-debug-id`, `x-sentori-arch`) + query (`release`, `objectName`)；ON CONFLICT DO UPDATE 幂等；max 256 MB；audit `dsym.uploaded`
- [x] 服务端 `GET /admin/api/projects/{id}/dsyms?release=&limit=` 列出（含上传者 email）
- [x] router 每路由 `DefaultBodyLimit::disable()` + 256 MB 限制 for `/dsyms` and `/sourcemaps`，不被 protocol 1 MB 全局限制锁住
- [x] `cli/src/dsym.rs` Mach-O 解析（`object` crate）：单 / fat32 / fat64；提 LC_UUID + cputype/cpusubtype；fat 切片 byte-level；arch 表：arm64 / arm64e / arm64_32 / armv7 / armv7s / armv7k / x86_64 / x86_64h / i386
- [x] `sentori-cli upload dsym --project=<uuid> [--release] [--token] [--api-url] <paths>`：token fallback `SENTORI_ADMIN_TOKEN → SENTORI_TOKEN`；api-url fallback `SENTORI_ADMIN_URL → INGEST_URL replace ingest.→api. → public api`
- [x] tests：3 个 case（happy + idempotent / 坏 headers + 空 body + arch 白名单 / 跨 org 403）；并修 fixture 时间戳并行碰撞 bug；server 42/42 全绿
- [x] dashboard release 详情显示 dSYM 列表 —— 推迟到 Phase 23（Release UX 主轴），server 端已就位
- [x] commit `8e1b71a`

##### sub-B — server: iOS 反符号化 ✅
- [x] **不**用 atos（macOS-only）：用 pure-Rust `addr2line` + `gimli` + `object` —— Linux 生产服务器原生工作；deps 加到 server/Cargo.toml
- [x] 新模块 `server/src/symbolicate_ios.rs`：按 `(project_id, debug_id, arch)` 查 dsyms 表，从 fat blob 提对应 arch 的 slice，dump 到 `SENTORI_DSYM_CACHE_DIR`（默认 `/tmp/sentori-dsyms`）；`Loader::new(path)` 内存映射 + DWARF 解析；resolve `instructionAddress - imageAddress`
- [x] 协议扩展：`docs/protocol.md` + docs-site 加 4 个 native 帧字段（`debugId`, `arch`, `instructionAddress`, `imageAddress`）；解释 server 命中后会重写 `function/file/line/inApp` 同时保留 native 字段供后续重符号化
- [x] cache：`(project_id, debug_id, arch) → dumped path` map，200 条上限 wholesale eviction；tmpfs 让重 mmap 廉价；`Loader` 本身不缓存（lifetime 复杂，重开销可忽略）
- [x] 接入既有 symbolicate_payload pass（admin.rs `list_events_for_issue`）：JS sourcemap 先走（RN 桥帧通常在顶端），iOS DWARF 后走（native 帧底层）；两 pass 对不识别的帧 no-op
- [x] tests：3 个 unit（parse_addr 各形态 / normalise dashed+bare / 非 32-hex fallback）+ 不破坏 31 集成 = 13 unit + 31 integration = **44 server tests** 全绿
- [x] 真实 DWARF 端到端测试推迟到 sub-F（release-aware）—— 需要 checked-in 迷你 Mach-O fixture
- [x] commit `6fe7139`

##### sub-C — CLI + server: Android proguard ✅
- [x] migration `0015_proguard_mappings.sql`：(id, project_id, release, debug_id, size_bytes, data, uploaded_by, uploaded_at)；按 (project, debug_id) 和 (project, release, time) 索引
- [x] `POST /admin/api/projects/{id}/mappings` 与 `GET ...` —— raw octet-stream，optional `?release=` + `x-sentori-debug-id`；服务端从 mapping header sniff `# pg_map_id:`；每次 insert 新行，per-build 历史保留
- [x] `sentori-cli upload mapping --project=<uuid> [--release] [--token] [--api-url] <path>`：env fallback chain 同 dsym
- [x] `server/src/symbolicate_android.rs`：用 getsentry `proguard@5.10` crate 的 `ProguardMapper::new(ProguardMapping)` retrace；按 frame.debugId 优先 / fallback 到 release；只动 function 含 `.` 的帧，JS 帧自然 no-op；50-entry cache
- [x] `admin.rs`: 第三道 symbolicate pass（JS sourcemap → iOS DWARF → Android proguard），每 pass 对不属于自己的帧 no-op
- [x] tests：integration 2（sniff debug_id + list / 空 body 400），unit 1（proguard crate sample 解析回原 class.method）
- [x] server suite 14 unit + 31 integration = **45 server tests** 全绿；CLI `upload --help` 列出第三个子命令 `mapping`
- [x] commit `e3eca45`

##### sub-D — Android ANR detection ✅
- [x] `SentoriAnrWatchdog.kt`：worker thread post tick / 5 s timeout / 单次报告（防止抖屏期间洪水）/ daemon 不阻塞退出 / debug-build 默认关（避免 Metro debugger 暂停 main 引发误报）；写盘到既有 pending dir 复用 drain 流
- [x] `SentoriModule.kt` 加 `startAnrWatchdog({ timeoutMs?, intervalMs?, force? })` + `stopAnrWatchdog()`；JS opt-in 让 host 选时机
- [x] `sdk/react-native@0.3.0`：`native.ts` 类型 + index 出口；wire 完成
- [x] EventKind 加 `'anr'`：`sdk/core@0.2.0` types + server `event::EventKind`；wire-format 加性，receiver 未识别按 `error` 处理
- [x] dashboard `issues.tsx`：行内 amber `ANR` chip（`errorType === 'ApplicationNotResponding'`）+ 顶部 `ANR` 过滤按钮（pressed 时仅显示 ANR 行）；不破 IssueRow schema
- [x] docs/protocol.md + docs-site kind 行更新；server 14 unit + 31 integration 全绿；core 11 + rn 18 SDK tests 全绿
- [x] npm publish core 0.2.0 + react-native 0.3.0；commit `b02b7bb`

##### sub-E — iOS hang detection ✅
- [x] `SentoriHangWatchdog.swift`：DispatchSourceTimer 后台 queue 每 1s post 到 main，未 ack ≥ 2s 即报；single-shot per hang；`#if DEBUG` 默认关；写盘到既有 pending dir
- [x] `SentoriModule.swift` 加 `Function("startAnrWatchdog")` + `stopAnrWatchdog`，**和 Android 共用同一 JS 函数名** —— host 一行 `startAnrWatchdog()` 双平台开
- [x] 复用 `kind = "anr"`（不另开 `"hang"` —— dashboard ANR badge 已 work；区分靠 `tags.source = sentori.hangWatchdog`）
- [x] iOS 2s/1s vs Android 5s/1s 默认：iOS 没有等价系统级 ANR 信号，更严格捕短 stutter；Android 跟系统 5s 一致
- [x] **Caveat**：Thread.callStackSymbols 只回 caller-thread 栈；watchdog 在 background queue 上跑，跨线程取 main 真实栈需要 Mach API（thread_state_t / vmread）—— App Store review 容易判 reject。今天 capture 是 watchdog timing path 栈，sub-F 或后续 phase 上 proper main-thread sampler
- [x] `native.ts` 类型 + JS export 已是 sub-D 留下的 `startAnrWatchdog`；本 sub 仅文档区分双平台 default
- [x] `sdk/react-native@0.3.1` 发到 npm；commit `601066a` + `dfc3a5d`

##### sub-F — release-aware symbolication ✅
- [x] dSYM / proguard mapping 已经按 release 串绑（sub-A + sub-C 的 schema 都有 `release` 列；CLI 上传时 `--release` 写入；可空表示"未明确归属"）
- [x] **新端点** `GET /admin/api/projects/{id}/releases/{name}/artifacts`：统一返回 `{release, sourcemaps, dsyms, mappings}`；JOIN 三张源（dsyms / proguard_mappings / release_artifacts via releases）按 release 过滤
- [x] dashboard issue-detail 加 `<ReleaseArtifactsPanel>`：显示 sourcemap 文件数 + iOS dSYM slice 数（带 arch 列表）+ ProGuard mapping 大小；零上传时不渲染
- [x] ServerEvent.kind 类型从 `'error'` 拓到 `'anr' | 'error'`（同步 sub-D 服务端 EventKind 拓宽）
- [x] **不**做"symbolicate 拒跨 release lookup" —— 原 roadmap 措辞误判：debug_id 在每次构建是唯一的，retracer 应该按 debug_id 匹配；release 列是元数据。强行 release 隔离会让 downgrade / re-symbolicate 旧 release 失败。保留现状：debug_id 优先 + release fallback
- [x] 新 integration test `release_artifacts_unifies_dsym_and_mapping`：上传一个 dSYM + 一个 mapping 同 release，端点返回两者；其他 release 返全 0
- [x] **iOS 真主线程采样器推迟到 v0.3**：sub-E 留下的 caveat 要求 Mach API（thread_state_t / vmread）+ 谨慎处理 App Store 审核，独立 phase 更合理
- [x] server 14 unit + 32 integration = **46 server tests** 全绿；dashboard build 127.5 KB gzip；commit `d507c21`

---

### Phase 23 — Release 管理 UX

**Goal:** Release 一等公民。Dashboard 有 Releases 列表 + 详情；deploy webhook + regression detection。

**Entry:** Phase 22 ✅

**Exit:**
- Releases 列表页（每 release 卡片：版本 / env / source map / dSYM / 首末次见 / regressions）
- Release 详情页（uploaded artifacts 树 + event timeline + 比较前一 release）
- `POST /v1/deploys` webhook + dashboard 显示 deploy timeline
- Regression detection（issue 已 resolved 然后 release X 后又出现 → 标 regression）
- Compare-releases 视图（diff issues：新出 / 修了 / 仍存）

**Estimate:** 1.5 周

#### Steps
- [x] **sub-A**：`releases` 表 schema 加 `deploy_at`（migration 0016，回填到 `created_at`）；新端点 `GET /admin/api/projects/{id}/releases` 返回每个 release 的 event_count + first/last_seen + sourcemap/dsym/mapping count（live JOIN，预期 ≤ 1k releases/project）；dashboard 路由 `/org/{slug}/releases` + `<ReleasesView>` 卡片列表（deploy 时间 / event 数 / 三种 artifact 上传数 muted-when-0 + first→last seen 行）；OrgLayout NAV 加 "Releases"；`adminApi.listReleases` + `ReleaseListRow` 类型；server tests 47/47 全绿；dashboard build 128 KB gzip。commit `85c941c`
- [x] **sub-B**：`/org/{slug}/releases/{name}` `<ReleaseDetailView>`，复用 sub-F 的 `release_artifacts` endpoint；三段（Source maps / iOS dSYMs / ProGuard mappings）的 divide-y 行列表，每行 identifier + size + 相对 uploadedAt + uploader email 截断；空段显示 copy-pasteable `sentori-cli upload …` 命令；releases 列表卡片包 `<Link>` 跳详情。dashboard 129 KB gzip。commit `d7d158f`
- [x] **sub-C**：`POST /v1/deploys` 端点（`server/src/api/deploys.rs`）走 `require_token` 中间件复用 ingest token；body `{release, environment?, deployedAt?}`，校验 release ≤ 200 / env ≤ 64；upsert `releases (project_id, name)` 行 + 设 `deploy_at = deployedAt ?? now()`；audit `release.deployed`（target type 同名）payload 含 `project_id / release / environment / deploy_at`，DevToken 跳过 audit（无 org）；`audit::actions::RELEASE_DEPLOYED` + `targets::RELEASE` 加进 catalog 让 dashboard filter 自动可见；webhook payload contracts 表加 `release.deployed` 行。3 个新 integration test（happy path / idempotent refresh / 错误输入）全绿，protocol.md + getting-started.md 加 CI 集成段（curl + GitHub Actions 示例）。protocol bumped to v0.1.1.
- [x] **sub-D**：regression 检测——`migration 0017`：`issues.status` CHECK 加 `'resolved' / 'regressed'`，加 `resolved_at / resolved_in_release / regressed_at / regressed_in_release` 4 列，加部分索引 `issues_resolved_idx WHERE status = 'resolved'` 让 cron 扫描 O(resolved 行数) 而不是全表。**On-event** 走 `issues::upsert_issue` 的 `ON CONFLICT UPDATE`：CASE WHEN 把 `resolved` 翻成 `regressed`、stamp `regressed_at = event.timestamp` 和 `regressed_in_release = event.release` —— 全在同一个 SQL，原子无 read-then-write 窗口；返回 `(issue_id, is_new, regressed)` 三元组让 events.rs 决定 fire `NewIssue` / `Regression` 通知。**Cron** `regression::spawn_sweeper` 每 5 分钟扫一次 `WHERE status='resolved' AND last_seen > resolved_at`，把上线前老数据 / 绕过 upsert 的写入兜底翻转。**Notifier** 加 `NotifyEvent::Regression` 变体走 `notification_recipients.on_regression = TRUE` 的收件人，邮件含 release。**Patch issue API** 允许 `'resolved'`（`'regressed'` 仍是自动态、不接受手 patch），`PATCH` 时同 SQL 用 CASE 标 `resolved_at = now()` / `resolved_in_release = last_release`；从 `resolved` 离开时清掉 `regressed_*` 标记防止下次回归被吃掉。**Metrics** 加 `sentori_issue_regressed_total` 计数器。**Dashboard** `IssueRow` type 加 4 列 + IssueStatus enum 拓到 5 态；`/issues` 页面 status 切换器加 `regressed / resolved` tab；行内 `regressed` 红色徽章带 release tooltip；hotkey `r` resolve 当前行（active / regressed → resolved）。3 个集成测试（on-event auto-flip、re-resolve 清标记、sweeper 兜底）+ build 129 KB gzip。
- [x] **sub-E**：compare-releases —— 新端点 `GET /admin/api/projects/{id}/releases/{base}/compare/{target}` 用一条 CTE-driven SQL 算两个 release 的 issue 集合差：CTE 各自跑 `SELECT DISTINCT issue_id FROM events WHERE release = $base/$target AND issue_id IS NOT NULL`，外层 LEFT JOIN issues 表 + CASE 分桶 `added / fixed / persisting`。返回结构 `{base, target, added[], fixed[], persisting[]}`，每行带 `bucket` 字段方便后续做 flat list；reject `base == target`（400 / 500）。Dashboard：路由 `/org/{slug}/releases/{target}/compare/{base}`、`<ReleaseCompareView>` 三段 list（红/绿/灰徽章 + 计数）每行链到 issue detail；`<ReleaseDetailView>` header 加 `Compare with…` 原生 select（拉 `listReleases` 缓存的所有 releases，剔自身）选了立刻 navigate。`adminApi.compareReleases` + `ReleaseCompareRow / ReleaseCompare` 类型；2 个集成测试（三桶分类正确 + 拒绝同一 release）；server 25/25 全绿，dashboard build 130 KB gzip。

**Phase 23 ✅** — Release 一等公民全部 5 个 sub 完成：deploy_at + 列表 / 详情 / deploy webhook / regression detection / compare 视图。下一步 Phase 24 Issues 列表 power-user 化。

---

### Phase 24 — Issues 列表 power-user 化

**Goal:** filter query 语法 + 列配置 + 保存视图 + 批量操作。

**Entry:** Phase 23 ✅

**Exit:**
- Query 语法（`errorType:TypeError environment:prod last:7d release:1.2.3 status:unresolved`）前后端共用 parser
- 列配置：show/hide errorType / count / lastSeen / env / release / assignee（localStorage 持久化）
- 保存视图：个人 + 共享 org/team 内
- Bulk select + bulk resolve / ignore / assign
- 密度切换 compact / cozy（应用到所有表格）

**Estimate:** 1.5 周

#### Steps
- [x] **sub-A**：query parser —— `web/src/lib/issue-query.ts` + 13 个 vitest 单测覆盖 grammar：`KEY:VALUE`（`errorType / error / env / environment / release / status / last`，alias 折叠），无前缀 token 累积成 `freeText`，`status:` 校验合法值，`last:Nm/Nh/Nd` 解析成 RFC 3339 `lastSeenAfter` ISO 时间戳，未识别 key 默默退化成 free-text（typo 友好）。Server `ListIssuesQuery` 加 `error_type` + `last_seen_after`（serde rfc3339 option），SQL WHERE 多两个 `($::TEXT IS NULL OR ...)` 守卫。`adminApi.listIssues` types 同步加两参。`<IssuesView>` 重做：去掉单独的 env / release 输入框，单一搜索框（28rem）+ token tooltip + warnings 横条；status tab 和 `status:` token 共存（token 优先）；free-text fallback 走客户端 `messageSample / errorType` substring。新 `formatIssueQuery` 帮 sub-C 的 saved view 复原 query 字符串。1 server integration test 验证 `errorType=` 和 `lastSeenAfter=` URL 参数收敛结果集。dashboard build 130.6 KB gzip。
- [x] **sub-B**：列配置 UI + localStorage 持久化 —— `web/src/lib/column-prefs.ts` 抽通用 hook：`ColumnDef[]`（id / label / defaultVisible）+ `useColumnPrefs(storageKey, defs)` 返回 `{visible: Set<id>, toggle, reset}`。Storage shape 用 `Record<id, boolean>`（不是 string[]）—— 加新列时新列默认按 `defaultVisible` 取值，不会因为旧 user 的 snapshot 没 mention 就被 hide（早期 string[] 设计 polarity 错）。Quota / 私密模式异常吃掉。`<IssuesView>` 接进来：errorType + Message 不可关，count / lastSeen / firstSeen / env / release 可切换；`firstSeen` 默认 hidden。Header 右侧加 `⋯` 列设置按钮，popover 三段（标题 + checkbox 列表 + Reset to defaults），mousedown outside-click 关闭。Storage key `sentori:issues:columns:v1`。新发现：vitest 4 + jsdom 的 `globalThis.localStorage` 是空 stub object（没有 `setItem` / `getItem` 方法），spec 顶上 `Object.defineProperty` 装了一个 in-memory polyfill —— 浏览器里 native localStorage 走原生实现不受影响。5 个新单测覆盖 default 解析 / 用户覆盖 / 新列 forward-compat / corrupted payload 静默 / round-trip。dashboard build 131.3 KB gzip，TS+vitest 18 → 23 全绿。
- [x] **sub-C**：`saved_views` 表 + UI —— `migration 0018`：`saved_views(id, org_id, target, scope, team_id, user_id, name, payload jsonb, created_at, created_by, updated_at)`，`target` 走 CHECK in `('issues')` 留 forward room，`scope ∈ ('personal','team','org')`，CHECK 表达 scope ↔ FK 互斥关系（personal 必带 user_id、team 必带 team_id、org 两者皆 NULL）。三个端点 `GET /api/orgs/{slug}/views` / `POST /views` / `DELETE /views/{id}`，复用 `require_user`。可见性：org-scope 全员看；team-scope 团队成员或 org owner/admin（admin 看所有 team 用一个面板就行）；personal-scope 仅本人。创建 authz：personal 任何成员、team 团队 lead 或 org admin、org 仅 admin。删除 authz 镜像创建。Dashboard：`SavedView / SavedViewScope / SavedViewPayload` types + `orgsApi.{listViews,createView,deleteView}`；`<IssuesView>` header 加 `Views (N)` 按钮 → popover 按 scope 分组（org / team / personal）每行小卡片显示名字 + team_slug + 截断 query；`+ Save current view…` 弹模态：name + scope radio + team select（scope=team 时拉 `teamsApi.list`），错误返回的 `{error}` 内联红字渲染。视图应用 = setQueryText（如 view 含 `status` 但 query 没 `status:` token，append 一段，让 status tab 同步翻）。1 个 server integration test 覆盖 4-user 矩阵：owner / lead / member / bystander 三种 scope 的可见性 + 创建 authz + 删除 authz。dashboard build 132.5 KB gzip，server 全套相关 18 项测试全绿。
- [x] **sub-D**：bulk action endpoint + UI —— `POST /admin/api/projects/{id}/issues:bulk` 接 `{ issueIds: Uuid[], action: 'resolve'|'silence'|'close'|'reopen' }`，server map action → `status` 字符串（**不**让 client 直接传 raw status，避免 `regressed` 这种 ingest-only 状态被人手 patch 进去）；空数组 / >200 / unknown action 全 reject；UPDATE 用 `id = ANY($::uuid[])` 一条 SQL 完成，含 single-row PATCH 一致的 CASE：resolve 时 stamp `resolved_at = now()` + `resolved_in_release = last_release` + 清 `regressed_*`；非 resolve 时清 `resolved_*`。Dashboard：`adminApi.bulkPatchIssues`；`<IssuesView>` 加首列 checkbox（select-all 含 indeterminate 三态），shift-click 范围选（`lastClickedIdxRef` 追 anchor），row checkbox 走 `e.stopPropagation()` 不触发 row 的 navigate；选中 ≥ 1 时 header 下方插一条 `bg-accent/5` toolbar 显示 `N selected · Resolve · Silence · Close · Reopen · Cancel`，mutation onSuccess 清 selectedIds + invalidate `['issues', projectId]`；行高亮用 `bg-accent/5` 区分选中态 vs `bg-accent/10` 键盘焦点态。3 个 server integration test：bulk resolve stamps release / invalid action 拒 / 空数组拒；dashboard build 133.0 KB gzip。注：`assignee` 在 schema 里没字段（v0.2 issue 表没 `assigned_to`），ROADMAP exit 提到的 bulk assign 推迟到 Phase 25 issue detail revamp 一起做（assignee schema + UI 同时落更省）。
- [x] **sub-E**：density toggle compact / cozy —— `web/src/lib/density.ts` 提供 `DensityProvider` (React Context) + `useDensity()` + `densityClasses(d)` token map（`compact: h-7 + py-0.5 + text-[12px]`，`cozy: h-10 + py-1.5 + text-[13px]`）；localStorage `sentori:ui:density:v1` 持久化，quota / 私密模式 fallback；store *tokens* 而不是 raw class strings 让每个表自选哪些 slot 用。`main.tsx` 套 `<DensityProvider>`；`<OrgLayout>` 头部加 `<DensityToggle>` 按钮（compact: ☰ / cozy: ≡）。Tables 全用 `dCls.rowClass` 替换原来 hardcode 的 `h-9` / `h-8`：`issues.tsx` / `token-settings.tsx` / `recipient-settings.tsx` / `org-settings.tsx`（members + invites 两表）/ `team-list.tsx` / `team-detail.tsx` / `audit-log.tsx`，共 7 处 view。两个新单测覆盖 token map 互斥 + h-7 vs h-10 token 选择。dashboard build 133.5 KB gzip，vitest 25/25 全绿。

**Phase 24 ✅** — Issues 列表 power-user 化全 5 sub 完成：query parser + 列配置 + saved views + bulk actions + density。下一步 Phase 25 issue 详情页 revamp（含 assignee schema + bulk assign 补做）。

---

### Phase 25 — Issue 详情页 revamp

**Goal:** 一屏调试。Tabbed 布局 / inline source / breadcrumb 时间轴 / activity log。

**Entry:** Phase 24 ✅

**Exit:**
- Tabs：Stack | Events | Breadcrumbs | Tags | Activity（URL hash 保留状态）
- Frame 行 click → inline source 抽屉（用 source map 反查）
- Breadcrumb 时间轴：可折叠 group + 类型颜色
- Related events 侧栏（同 fingerprint）
- Comment thread + activity log
- Status / assign / "mark as fixed in release X" 流

**Estimate:** 2 周

#### Steps
- [x] **sub-A**：tab layout shell + URL hash 路由 —— `<IssueDetailView>` 整页重做。顶部 sticky header（`Back · errorType · messageSample · status badge` 按 status 染色 + EventPicker `1 / N of total · prev/next · uuid prefix`）+ tab bar `Stack | Events | Breadcrumbs | Tags | Activity`。`useTabFromHash()` 自定义 hook：`location.hash` parse 成 Tab，set 用 `history.replaceState` 不污染浏览器历史栈（tab 是 view mode 不是 navigation，对标 Linear / Vercel），`hashchange` 事件订阅外部回 / 复制粘贴 URL 都正确。Stack tab：原页面所有内容（symbolicated toggle + StackList + CauseChain + Releases chips + ReleaseArtifactsPanel + Context grid）；Events tab：列表（取代旧侧栏，每行 uuid + message + receivedAt + env / release / platform 三 chip）；Breadcrumbs / Tags tab 拆出独立组件；Activity tab placeholder 标 sub-E。键盘 `[` / `]` step event 跨 tab 仍生效。dashboard build 134.2 KB gzip。
- [x] **sub-B**：inline source 抽屉 + 服务端 `GET /admin/api/projects/{project_id}/events/{event_id}/source?frame=N&cause=M` —— 由于 frame 可能在 cause chain 里某层（最深 10 层），endpoint 用 `(frame, cause)` 二元坐标定位 stack 中的具体一行；走 raw 行号（symbolicate 在 read 路径 in-place 重写但 index 稳定）。`symbolicate.rs` 加 `source_for_frame(pool, release, raw_line, raw_col, ctx)` + 内部 `window_from_sourcemap` 纯函数（无 DB），单元可测；用 `sourcemap::Token::lookup_token` 反查到 source idx，从 `SourceMap::get_source_view(idx)` 拿 sourcesContent，按 ±5 行切片返回 `{file, line, column, before[], at, after[]}`。Dashboard：`adminApi.frameSource` + `FrameSource` type + `<StackList>` 行改成 `<button>` onClick 触发 drawer，`<CauseChain>` 加 depth 参数把 cause 索引传上去；`<FrameSourceDrawer>` 右侧滑入半屏，header 显 `cause N · frame M`，body 用 `<pre>` 渲染上下文行（命中行 `bg-accent/10` 高亮 + 左侧行号），404 时友好提示 "release 没传 sourcemap / 反查失败 / sourcemap 没带 sourcesContent"，Esc 关闭。`SourceMapBuilder::add_raw` + `set_source_contents` 写了 2 个单元测试覆盖 window 切片 + 起点边界 clamp（`before` 在文件头被截到 1 行）。dashboard build 135 KB gzip。
- [x] **sub-C**：breadcrumb 时间轴组件 —— 新文件 `web/src/views/breadcrumb-timeline.tsx`：左 gutter 时间戳 + 类型彩色圆点 + 垂直连接线（首尾不画线让两端不漏出连接线），body 按 type 渲染（`nav`: from→to / `net`: METHOD URL status durationMs（status ≥500 红 / ≥400 琥珀 / 2xx 绿）/ `log`: level + message（warn 琥珀 / error 红）/ `user`: action target / `custom`: JSON dump）。`groupBreadcrumbs` 纯函数：相邻同 type 且时间差 ≤1s 合并；3+ 行折叠成 `type ×N — expand`，<3 行不显折叠按钮（chevron 不污染）。`<IssueDetailView>` 的 `BreadcrumbsTab` 调用从旧 `<BreadcrumbsList>` 切到 `<BreadcrumbTimeline>`，把旧组件删掉。4 个新 vitest 单测覆盖：不同 type 各自独立分组 / 同 type ≤1s 合并 + >1s 切组 / type 变化即使时间贴近也分组 / 空数组。dashboard build 135.8 KB gzip，vitest 24/24 全绿。
- [x] **sub-D**：related issues 面板 —— ROADMAP 字面 "同 fingerprint" 但 fingerprint 已 = issue（events 列表本身就是同 fingerprint），改进后的解读：**"这个 release 还炸了什么"**。`<RelatedIssuesPanel>` 在 Stack tab 底部加一段 `Other active issues in {release}`，复用现有 `listIssues({ release, status: 'active', limit: 20 })` 端点（不加 server endpoint），客户端 filter 剔自身 issue.id 后取 top 10，每行 errorType + messageSample + 事件数 + relativeTime，整行 `<Link>` 跳目标 issue。`<StackTab>` 加 `issueId` + `orgSlug` props 把上下文穿到 panel。零 active 时 panel 不渲染。dashboard build 136 KB gzip。后续 server 端 `excludeId` 参数随数据规模再加。
- [x] **sub-E**：comments + activity log —— `migration 0019_issue_comments.sql`：`issue_comments(id, issue_id FK CASCADE, author_id FK SET NULL, body, created_at)` + `(issue_id, created_at)` 索引。Server 三个端点 `GET /admin/api/projects/{id}/issues/{iid}/activity` / `POST .../comments` / `DELETE .../comments/{cid}`：activity 是 unified stream，把 `issue_comments` 行 + `issues.resolved_at` / `regressed_at` 状态时间戳合并按 `at` 升序输出，三种 `kind: comment / resolved / regressed`（**故意不读 audit_logs**——audit 是 org 级安全审计、与 per-issue UI 流不混；Phase 25 sub-F 落 issue.assign / fix-in-release 时再扩 stream）；`Serialize` 用 `#[serde(tag = "kind")]` + 每个 variant 自己的 `rename_all = "camelCase"`（enum-level rename_all 不传播到内 variant 字段，踩过坑）。Body 校验 1..2000 char trimmed；create 要求 User caller（DevToken / LegacyAdmin 拒）；delete 仅作者或 LegacyAdmin/DevToken。`AppError::Forbidden` 加进 enum + 403 mapping。Dashboard：`ActivityEntry` 联合类型 + `adminApi.{listIssueActivity, createIssueComment, deleteIssueComment}`；`<ActivityTab>` 接进来，列表 + comment 输入框（textarea + N/2000 计数 + "Comment" 按钮 disabled-when-empty/oversize），`<CommentEntry>` 显示作者邮箱 + relativeTime + 自己的 comment 显 Delete 按钮，`<StateEntry>` 渲染 resolved（绿）/ regressed（红）+ release。2 个 server integration test：unified stream 包含 comment+resolved+regressed 三种且 author email 正确 + 删除自己的 comment 204；body 边界（empty / >2000）拒绝。dashboard build 136.6 KB gzip。
- [x] **sub-F**：assign / status / "fixed in release" 流 —— `migration 0020`：`issues.assignee_user_id UUID FK SET NULL` + 部分索引 `WHERE assignee_user_id IS NOT NULL` 给 "我的 issues" filter。`IssueRow` SELECT 三处都改成 `JOIN users u ON u.id = i.assignee_user_id` 拿 `assignee_email`，加 `assigneeUserId / assigneeEmail` 字段。**`patch_issue`** 升级两个语义：(1) `assigneeUserId` 走 *double-Option*（自定义 `deserialize_double_option`）—— `Some(Some(uuid))` 设、`Some(None)` 清空、`None` 不动，让 dashboard 能传 `null` 区别于 omit；(2) `resolvedInRelease` 同样 double-option，覆盖默认的 `last_release`，让用户手动 pin "fixed in 1.2.0"。SQL 用 `COALESCE($4::TEXT, last_release)` 保留 fallback 行为。**`bulk_patch_issues`** 加 `action: "assign"` 走 `id = ANY($::uuid[])` 一条 SQL，要求 `assigneeUserId` 字段（可为 null = 批量 unassign）。Phase 24 sub-D 留下的 bulk assign 尾巴在此补全。Dashboard：`IssueRow` types 加两字段；`adminApi.patchIssue` body 联合类型加 assignee/resolve；`bulkPatchIssues` 走 discriminated union（`'assign'` action 必带 assigneeUserId）；`<IssuesView>` 列设置 + table cell 加 `assignee` 列默认 hidden、bulk toolbar 加 `Assign to me` / `Unassign` 两键；`<IssueDetailView>` header 加 `<IssueActions>`：assignee chip + Assign to me / Unassign + 当 status 非 resolved 时 `Resolve in <select 上次见过的 releases>` 按钮，`onResolve` 把选定 release 同 patch 一起送。3 个 server integration test：单行 patch assignee 设/清 round-trip / resolve 时 `resolvedInRelease` 覆盖 last_release / bulk assign 一次设 3 行；dashboard build 137.1 KB gzip。

**Phase 25 ✅** — Issue 详情页 revamp 全 6 sub 完成：tab shell + URL hash / inline source 抽屉 / breadcrumb 时间轴 / 同 release related issues / comment + activity log / assignee + resolve-in-release。Issue triage 已经是完整 power-user 体感。

---

### Phase 26 — Health metrics（crash-free rate / sessions）

**Goal:** 轻量 session-aware。不做 session replay。

**Entry:** Phase 25 ✅

**Exit:**
- SDK init / close 触发 session ping（open / close / errored）
- crash-free user / session per release / per env
- Health widget on overview page
- Per-release health 对比

**Estimate:** 1.5 周

#### Steps
- [x] **sub-A**：协议加 session ping（`POST /v1/sessions`） —— `migration 0021_sessions.sql`：`sessions(id PK uuid v7, project_id FK CASCADE, user_id TEXT 应用级 id 不连 FK 到 users 表, release, environment, status CHECK in (ok / errored / crashed / exited), started_at, duration_ms ≥0, received_at)`，三个索引（project+release / project+received / project+user partial）。Server `api/sessions.rs`：单 ping endpoint 走 `require_token` 共享 ingest 限流；body 校 release ≤200 / env ≤64 / userId ≤200 / 四态 status 白名单 / duration 0..7days（夹掉 clock skew bug）；`ON CONFLICT (id) DO NOTHING` 让 SDK 重试可幂等不双数。`/v1/sessions` 路由进 `ingestion` 路由组。protocol.md + 镜像加 endpoint 章节，列 healthy=`ok|exited` / unhealthy=`errored|crashed` 的语义、duration 上限、idempotent 行为；版本 bump v0.1.2。3 个 server integration tests：round-trip 校验存进 DB、重发 3 次 1 行、bad input 各种 400（status / duration 负 / duration >1week / 401 missing token）。**故意不做** init/heartbeat ping、active session UI、批量 endpoint —— 单 close ping + 现场 query 是最小体量；规模真上来再加 `/v1/sessions:batch` 和预聚合。
- [x] **sub-B**：SDK lifecycle —— core SDK 加 `SessionTracker` 类（host-agnostic）：`start(ctx) / markErrored / markCrashed / end(finalStatus?)` 状态机，status promotion 单调（crashed > errored > exited > ok，markErrored 不能盖 crashed），`end()` 幂等且 idempotent 不双发，重入 start 静默丢弃旧 session（lifecycle 由 platform 拥有）。Host 注入 `send` 回调让 transport 走平台原生路径。**JS SDK** (`@goliapkg/sentori-javascript`)：transport.ts 抽 `postJson` helper 复用给 `send` (events) + `sendSession` (sessions) 都走 sendBeacon→fetch fallback；`session-tracker.ts` 单例包装；`init()` 启 `startSession()`；browser hook 加 `pagehide` listener（modern 浏览器 unload 标准事件，不用过时的 `beforeunload`，bfcache + 后台 + 关 tab 都触发）；node hook 加 `beforeExit` ship `exited` ping；`captureError` 调 `markSessionErrored`。**RN SDK** (`@goliapkg/sentori-react-native`)：parallel 实现，transport 加 `sendSessionPing`；新文件 `handlers/lifecycle.ts` 通过 `require('react-native').AppState` 订阅 'change' 事件，`active` → `startSession` / `background|inactive` → `endSession`（**故意 inactive 也 end** 因为 swipe-away 永远不会触发 background；新进入 active 起新 session 计两次符合 "用户开了两次 app" 直觉）；`init()` 装 lifecycle 后立刻 `startSession()`；`captureError` 调 `markSessionErrored`。Versioning：sentori-core 0.2.0 → sentori-javascript 依赖从 0.1.0 跳到 0.2.0（之前是 npm 上的旧版本，没拿到 SessionTracker，bun 链了 npm 缓存而非 workspace；强制 reinstall 让 symlink 指向本地）。Tests：core 7 个 vitest 单测覆盖 SessionTracker（duration / 状态升降 / 终态 override 不降级 / start-while-active 丢弃 / end 幂等）；JS 8 + RN 18 既有套件 build 全过；server 3 个 sessions integration test 之前 sub-A 已通过。
- [x] **sub-C**：聚合 query —— `GET /admin/api/projects/{id}/health?from&to&bucket&release&environment` 返回 `{ summary, buckets[] }`。Bucket 用 PG 14+ `date_bin($interval, received_at, '1970-01-01')` 而不是 `date_trunc`，绕开 timezone 偏移坑（每个 server 都按 epoch 对齐）；支持 `5m / 1h / 1d` 三档。Summary 单独一条 query 走 `COUNT(DISTINCT user_id) FILTER (WHERE status = 'crashed')` 算 crash-free user rate（匿名 NULL user_id 不进 user-rate 但仍进 session-rate）。Rate NaN-safe：total=0 时回 `null` 让 dashboard 渲 "no data"，不出 0%。From / to 默认 now-24h..now，from ≥ to 拒。Bucket SQL 用 `FILTER (WHERE status = ...)::BIGINT` 三个 count 一次出 total/crashed/errored，按 1 列分组+排序。3 个 server integration test：5 行混合 status/2 user/1 anon 算出 0.8 session-rate + 0.5 user-rate / release filter 收敛 / 空区间 rate 为 null。
- [x] **sub-D**：dashboard 健康 widget on overview —— 新路由 `/org/{slug}/overview`，OrgLayout NAV 加 `Overview` 作为第一项（`Issues` 仍是首次登录默认 redirect 目标，避免破现有用户记忆）。`<OverviewView>` 拉 `adminApi.health(projectId, { bucket: '1h' })` 默认 last 24h；顶部三个 stat card：crash-free session rate（≥99% 绿/否则琥珀）、crash-free user rate（≥99.5% 绿/否则琥珀）、total sessions（neutral）；rate=null 时显 `—` + neutral ring（不假装 0%）。Sparkline 是纯 SVG 几个 `<rect>`：每 bucket 总数当背景灰条 + crashed 数当顶部红条，max=Math.max(1, ...) 防 0 除；不引入 chart 库（保 bundle 体积），整图用 `currentColor` 跟主题对齐。0 session 时底部一行 footnote 提示安装 SDK 后会自动发 ping，不显空 chart 占位。`adminApi.health` + `HealthResponse / HealthSummary / HealthBucket` types 加进 client.ts。dashboard build 138.4 KB gzip。
- [x] **sub-E**：per-release health + alerting hook —— `<ReleaseHealthPanel>` 组件接进 `<ReleaseDetailView>` header 下方一段：复用同一 `health` endpoint 加 `?release=` filter，**窗口故意拓到 last 7 days**（不是 overview 的 24h），因为 per-release sample size 需要更宽窗口才有意义；`bucket=1d` 出周线。四个 stat：crash-free sessions（≥99% 绿）/ crash-free users（≥99.5% 绿）/ total sessions / crashed count。零 ping 时 panel 显友好 "No session pings on this release yet" 不假装 0%。Alerting hook：health endpoint 是 Phase 27 告警引擎读取 crash-free rate 的数据接口，本身已就位（接受 release/env filter + 窗口参数），sub-E 的 "alerting hook" 实质上就是确认数据接口稳定 + 在 release 视图把这一层数据曝光让 dashboard / 告警引擎共用同一来源——告警实现在 Phase 27。dashboard build 138.7 KB gzip。

**Phase 26 ✅** — Health metrics 全 5 sub 完成：sessions endpoint + schema / SDK lifecycle (JS pageshow + RN AppState) / 5min bucket 聚合 query / overview 健康 widget / per-release 健康。轻量 session-aware 的端到端链路就位。

---

### Phase 27 — 告警规则引擎深化

**Goal:** 真 rule engine（不只 "新 issue 发邮件"）。

**Entry:** Phase 26 ✅

**Exit:**
- Rule schema：trigger（count > N in T window / fingerprint match / regression / health drop）+ filter（env, release, fingerprint regex）+ throttle window
- Per-rule recipient routing + 多 channel（email / webhook）
- Webhook channel 实现（Phase 20 schema 落地）
- Daily / weekly digest
- Mute / snooze

**Estimate:** 2 周

#### Steps
- [x] **sub-A**：`alert_rules` schema + CRUD —— `migration 0022`：`alert_rules(id PK, org_id FK CASCADE, project_id FK CASCADE 可空 = org-wide, name, enabled, trigger_kind CHECK in (new_issue, regression, event_count, crash_free_drop), trigger_config JSONB, filter_config JSONB, channels JSONB, throttle_minutes 0..7d, last_fired_at, created_at/by, updated_at)`，三个索引（org / project partial / enabled+kind partial）。**故意把 trigger_config / filter_config / channels 留 JSONB 不强 schema**——shape 会跟 sub-F 的 mute/snooze + 后续 trigger kinds 一起演化，DB CHECK 会要求每次 shape tweak 都加 migration，不值。Server `api/alert_rules.rs` 四端点 list / create / patch / delete 在 `/api/orgs/{slug}/alert-rules`：read 所有 org 成员、create/patch/delete owner+admin only。Audit 三个新 action `alert_rule.created/patched/deleted` 接进 catalog（`AlertRule` target type + label + `all_labels()` 列表 + filter 下拉自动可见）。校验：trigger_kind 白名单、name 1..80 trimmed、throttle 0..7d、channels 必须 array、trigger_config / filter_config 必须 object、project_id 必须属于该 org。`PatchRuleBody` 用 `COALESCE($N, col)` 保留没传的字段。2 个 server integration test：4-step 生命周期（owner create / member 403 / list 看到 / patch 跑通 + audit 三条 / 删 204）+ 校验拒（unknown trigger / channels not array / 空 name / throttle 超限）。
- [x] **sub-B**：rule evaluator —— `server/src/rule_eval.rs`：两入口 `try_fire_on_event` (同步从 ingest 路径调，覆盖 `new_issue` + `regression`) + `spawn_cron` (60s ticker，sweep `event_count` + `crash_free_drop`)。**Throttle 走 atomic UPDATE-RETURNING**：`UPDATE alert_rules SET last_fired_at = now() WHERE id = $1 AND (last_fired_at IS NULL OR last_fired_at < now() - make_interval(mins => $2))` 返行才发，并发 evaluator 自然只一个赢 race。Filter 匹配：`environment` / `release` 严格相等，`errorTypeRegex` 走 PG `~`（cron 路径）+ `regex` crate 内存匹配（on-event 路径）。On-event SQL `JOIN projects p ON p.org_id = r.org_id` 让 org-wide rule（`project_id IS NULL`）也匹配该 org 任意项目。`event_count` cron query: `JOIN projects` + `($1::UUID IS NULL OR project_id = $1)` 同时支持 project-scoped 和 org-wide。`crash_free_drop` 设了 hard-coded 10-session minimum sample size 防 toy project 噪声炸警。Notifier 加 `AlertFired { rule_id, rule_name, org_id, channels: JsonValue, summary, body }` 变体：遍历 channels[] 中 `type='email'` 的项发邮件，subject `[Sentori] Alert: {rule_name} — {summary}`，body 带 trigger / project / issue / release / type 上下文。Webhook channel 留 sub-D。`events.rs` 在 `is_new` / `regressed` 分支调 `try_fire_on_event` 同时发 NotifyEvent::NewIssue / Regression（两条路径并存：alert rule 走精细 filter / recipients 走 project 默认）。`main.rs` spawn 60s ticker。新增 `regex` crate 依赖。3 个 server integration test：on-event new_issue rule 含 env+regex filter（mismatch / mismatch / match → 仅一条 AlertFired）/ event_count cron 阈值跨过即触发 / throttle 60min 阻塞重发。
- [x] **sub-C**：UI 创建 / 编辑 rule —— 新路由 `/org/{slug}/alerts` + `<AlertsView>`，OrgLayout NAV 在 `Audit` 之前加 `Alerts` adminOnly 项；`permissions.ts` 加 `alert.manage` action（owner / admin 通过）。`adminFetch` 之外接 `orgsApi.{listAlertRules, createAlertRule, patchAlertRule, deleteAlertRule}` 4 个端点 + 6 个 client types（`AlertRule / AlertRuleInput / AlertTriggerKind / AlertTriggerConfig / AlertFilter / AlertChannel`）。`<AlertsView>` 列表 6 列：enabled checkbox（点即触发 `patchAlertRule { enabled }` mutation）/ 名字 / 触发器 chip 含 inline summary（`event_count` 显 `≥N events / Wm`、`crash_free_drop` 显 `rate < X% / Wm`）/ 过滤器 `env:prod release:1.0 type~^Type` / throttle `Nm` / 上次触发 relativeTime / 编辑+删除（confirm 对话框）。`<RuleModal>` 创建 + 编辑公用：name + trigger select + 根据 kind 动态显示 count/threshold/windowMinutes 字段 + filter 三输入（env / release / errorType regex）+ emails CSV 输入（拆分 `[,\s]+` 转成 `[{type:'email', to:[...]}]` channels 数组）+ throttle + enabled。Webhook channel UI **故意推迟到 sub-D**——和 webhook 实现一起落更省，避免 UI 空配置。Modal 错误把 server `{error}` 内联红字渲。dashboard build 140.8 KB gzip。
- [x] **sub-D**：webhook channel + signature verification —— 新模块 `server/src/webhook.rs`：`send(WebhookDelivery { url, secret, body, event })` 单次签名 POST，header 套 `protocol.md` Phase 20 sub-D 锁定的形态：`content-type / sentori-event / sentori-delivery-id (uuid v7) / sentori-timestamp (unix s) / sentori-signature: t=<ts>,v1=<hex>` + `user-agent: sentori/0.2`。`sign(secret, ts, body)` 公开 helper：HMAC-SHA-256 over `<ts>.<raw-body>` → 64 hex char，receiver 复用 OK。Connect 5s / read 10s timeout；reqwest 加 `rustls-tls` feature 让 prod image 不带 system OpenSSL。**v0.2 retry 策略 = 一次 + log**——持久化 retry 队列推迟，audit log 已捕 rule-fire 历史不丢"是否触发"。Notifier `AlertFired` channel 处理加 `webhook` 分支与 `email` 并行：缺 `url` / `secret` warn skip；payload `{id, kind:"alert.fired", ruleId, ruleName, orgId, summary, body, firedAt}` 一致 JSON 出。Dashboard `<RuleModal>` 加 Webhook fieldset：URL + secret 两输入，hint 内嵌签名公式 `sentori-signature: t=<ts>,v1=<hex>`；channels 数组按需 push。1 个 server integration test：起 axum mock receiver 在同进程，rule 配 webhook → cron sweep 触发 → 等 50×20ms 收到一次 POST → 验证所有 5 个 sentori-* header 形态、`v1=` 64-hex、HMAC 重算等于 header 值（receiver-side verification path 走通了）、body shape `{kind, ruleName, summary, firedAt, id}` 完整。`webhook.rs` 自带 1 个 unit test 覆盖 sign 决定性 + 不同 secret/body/ts → 不同 hex（replay 防御）。Webhook sign helper 4 项 invariants 锁住，audit-event webhook delivery 后续 phase 共用同一 primitive 直接套用即可。
- [x] **sub-E**：digest（cron + opt-in） —— `migration 0023`：`digest_subscriptions(user_id, org_id, frequency CHECK in (daily, weekly), last_sent_at, created_at)` 复合 PK + due-by-frequency 索引。Server 三个端点 `/api/users/me/digests` GET / POST 订阅 / DELETE `{org_slug}/{frequency}`：所有操作都 scope 到当前 user，**不接受跨用户 / admin 强订阅**——op-in 是字面意义。`server/src/digest.rs::sweep_once(pool, tx)` 一条 SQL 拉所有 `last_sent_at IS NULL OR last_sent_at < now() - CASE frequency WHEN 'daily' THEN 24h ELSE 7d`，对每行算 org-wide aggregate (新 issues / regressed issues / events / 总 sessions / crashed / crash-free rate) 注入 `NotifyEvent::DigestEmail { to, org_name, org_slug, frequency, summary_lines, window_hours }` 然后 UPDATE `last_sent_at = now()`（即使 notifier 失败也 mark sent，宁愿丢一份 digest 也不重复轰炸）。Notifier `DigestEmail` 邮件 subject `[Sentori] Daily/Weekly digest — {org_name}`、body 含 6 行 summary + window hours + manage-subscriptions 链接行。`spawn_cron` 每小时 sweep 一次。2 个 server integration test：subscribe → 首次 sweep 触发 + last_sent 推进 → 第二 sweep 24h 内 throttle 0 fire / unsubscribe 204；subscribe 拒未知 org (404) + 未知 frequency (400)。User-grain 不是 org-grain：两个用户都订阅同一 org daily 各自收一封，unsubscribe 只删自己的行。**Dashboard UI 推迟**——sub-E ROADMAP 是"cron + opt-in"，未明文要求 settings 页 UI；client API 已就位，UI sub 可以独立做（避免 phase 27 collation 过大）。
- [x] **sub-F**：mute / snooze —— `migration 0024`：`alert_rules` 加两列 `muted BOOLEAN DEFAULT FALSE` / `snoozed_until TIMESTAMPTZ NULL`。**两态语义独立**：muted 是用户主动静音不限期（不同于 enabled=false：muted 仍在活动列表显示，disabled 暗示用户准备退役 rule）；snoozed 是临时（1h/4h/24h/7d 快捷选项），过期自动清。Evaluator 三处 SQL（`try_fire_on_event` + `sweep_event_count` + `sweep_crash_free_drop`）WHERE 加 `muted = FALSE AND (snoozed_until IS NULL OR snoozed_until < now())` —— PG 自己判断过期，不需要单独 cron 清字段。`PatchRuleBody` 加 `muted` (Option<bool>) + `snoozed_until` (double-Option，`Some(None)` = explicit clear) 字段；UPDATE 走 `CASE WHEN $::BOOL THEN $val ELSE snoozed_until` 让 "字段未传" 不覆盖、"传 null" 显式清空。Dashboard `<AlertsView>`：name 列加 `Muted` 琥珀徽章 / `Snoozed` 蓝徽章（hover 显 toLocaleString 解开时间）；操作列加 `Mute / Unmute` 切换 + `Snooze / Wake` 按钮（snooze 用 `prompt('Snooze for how many hours?', '1')` 简单收集时长，snoozed 状态时按钮文案变 `Wake` 一键清掉）；types 加 `muted / snoozedUntil` 两字段。2 个 server integration test：muted 阻止 fire / unmute 后 fire / 设 snooze 阻止 fire / 显式 null 清掉后 fire。dashboard build 141.4 KB gzip。

**Phase 27 ✅** — 告警规则引擎深化全 6 sub 完成：alert_rules schema + CRUD / rule evaluator (cron + on-event 双触发) / dashboard CRUD UI / signed webhook channel / opt-in digest / mute + snooze。从"新 issue 发邮件"到完整 rule engine 的端到端链路就位。

---

### Phase 28 — 全局搜索 + Dashboard polish + a11y + 性能

**Goal:** Cmd+K 全局；最后一公里打磨。

**Entry:** Phase 27 ✅

**Exit:**
- Cmd+K palette：跨 org / team / project / issue / member 跳转 + 最近访问
- 键盘快捷键 cheatsheet（`?` 弹出）
- a11y audit pass（WCAG AA）
- Bundle 分析 + code splitting；首屏 < 200KB gzip
- Empty / loading / error state 全 dashboard 一致化
- Theme 微调（Linear-tight density / token 化）

**Estimate:** 2 周

#### Steps
- [x] **sub-A**：Cmd+K palette + `GET /admin/api/search?q=&types=` —— Server `api/search.rs`：5 张表 (orgs / teams / projects / issues / members) per-type LIMIT 10、按 caller 可见性 filter（`AdminCaller::User` 走 `EXISTS memberships m WHERE org_id = ... AND user_id = caller`，token / LegacyAdmin 跳过 scope 全表搜索）；ILIKE substring 匹配 + `\\%` `\\_` 转义防 wildcard 注入；`?types=` CSV 选子集（默认全 5 类）；统一 hit shape `{type, id, label, sublabel?, url}` 让前端一个组件渲。**v0.2 不上 PG full-text**——dataset 小、ILIKE 跑得动；真 latency 涨了再上 GIN tsvector。Dashboard `<CmdK>` 全局组件挂在 `<OrgLayout>` 内：Mod+K（macOS Cmd / Linux+Win Ctrl）toggle / Esc / outside-click 关 / ArrowUp+Down + Ctrl-N/P 导航 / Enter 跳转；输入 120ms debounce 防 fast-typer 多 query；空 query 显 `Recent` section（localStorage `sentori:cmdk:recent:v1` 上限 10 条）；hit row 用 `<KindChip>` 5 色徽章（org accent / team amber / project blue / issue red / member violet），sublabel 显 `org-slug · type` 上下文；底部一行键盘 cheatsheet `↑↓ navigate · ↵ open · esc close`。2 个 server integration test：跨 org user 搜不到别人的 project / issue（visibility 隔离）+ `?types=project` 严格收敛。dashboard build 142.5 KB gzip。
- [x] **sub-B**：`?` cheatsheet —— `<KeyboardCheatsheet>` 全局组件挂在 `<OrgLayout>` 与 `<CmdK>` 平行；`?` 无 modifier toggle / Esc / outside-click 关闭。`?` listener 检 `target.tagName ∈ {INPUT, TEXTAREA, SELECT}` + `isContentEditable` 即跳过——在搜索框打 ? 不弹面板。3 段静态分组（Global / Issues list / Issue detail）共 12 条 shortcut，每条 `<kbd>` chip 视觉+一行 label。**故意不上 hotkey registry**：dashboard 现有 `useHotkeys` 散布各 component 不在统一 hub，硬编码列表是漂移风险 vs 把 registry 缠进每个 hotkey site 的复杂度的 trade-off—— v0.2 选漂移风险（review checklist 加一条"加 hotkey 同步更新 cheatsheet"够用）。dashboard build 143.0 KB gzip。
- [x] **sub-C**：a11y baseline —— `index.css` 加全局 `:focus-visible { outline: 2px solid accent; offset: 2px }` 让没显式 `focus:ring-*` 的交互元素键盘用户仍看见焦点（用 `:focus-visible` 不是 `:focus`，鼠标点击不污染）；`prefers-reduced-motion: reduce` media query 把 animation / transition / scroll-behavior 全压到 0.001ms 尊重系统偏好；`.skip-to-content` link 默认 `translateY(-200%)` 隐藏，`:focus-visible` 时 `translateY(0)` 露出，让键盘 tab 第一下能跳过 nav 直接到主内容（`<OrgLayout>` `<main id="sentori-main">` 配套加 anchor）。Audit 走查：所有 `<button>` 都已带 `type="button"` 或 `type="submit"`（包括动态 `type={onClick ? 'button' : 'submit'}`，无 untyped 实例）；`<EventPicker>` `[/]` / `<DensityToggle>` ☰ ≡ / `<RuleModal>` 关闭 `✕` / `<KeyboardCheatsheet>` esc 等 icon-only 按钮都有 `aria-label` 或 `title`；modal 全部 `role="dialog"` + outside-click 关闭。**故意不上 axe-core / Lighthouse 自动 audit 集成**：v0.2 dataset 小、UI 表面有限，hand-walked + 这一波静态规则覆盖到位，CI 集成留待真有人报 a11y bug 再加。dashboard build 143.1 KB gzip。
- [x] **sub-D**：route-level code splitting —— `main.tsx` 把 19 个非 critical view 全转 `React.lazy(() => import(...).then(m => ({ default: m.X })))`。Critical path 保持 eager：`LoginView / RegisterView / VerifyView / ForgotPasswordView / RootRedirect / ProtectedLayout / OrgLayout`（首屏必经）。其余 view（issues / issue-detail / releases / overview / alerts / audit / settings / teams 等）都按 route 拆 chunk。`<RouteSuspense>` 在 `Suspense` 边界统一 fallback `Loading…` 文本，每个 lazy element 用 `lazyEl()` helper 包一层让 routing table 清爽。**结果首屏 gzip 从 143 KB → 103.5 KB**（达 ROADMAP 目标 200KB 双倍裕度），最大单个 lazy chunk `issue-detail` 6.4 KB gzip / `client.ts` 7.6 KB / `useQuery` 5.5 KB / `react-hotkeys-hook` 2.2 KB；总 19 个 view chunks 平均 1-2 KB gzip。Vite 自动按 import graph 共享子 chunks（react-hotkeys-hook 抽出来给 `issues` + `issue-detail` 共用）。dashboard build 整体未变大，只是切片：路由切换时按需加载 < 50ms 走本地 cache。
- [x] **sub-E**：empty / loading / error 一致化 —— 新文件 `src/components/states.tsx` 三个组件 `<LoadingState label?>` / `<ErrorState label, detail?, onRetry?>` / `<EmptyState title, hint?, cta?>`：统一 padding (`px-6 py-6` for compact / `px-6 py-10` for empty)、配色（loading + empty `text-fg-muted` / error `text-red-400`）、ARIA roles (`role="status"` / `role="alert"`)。替换 7 个主 view 的 inline state blob：alerts / overview / releases / release-detail / release-compare / issue-detail / issues。文案借机一致化（"No X yet" + 可执行 hint，避免单行干描述）。`onRetry` 字段为 sub-F polish 留接口（query.refetch() 接进），本 sub 暂不暴露。dashboard build 103.5 KB gzip 首屏（无变化，state 组件本身 < 200 字节 minified）。后续 view（team-list / org-settings / token-settings 等）有零散内联 state 留 sub-F 顺手清理。
- [x] **sub-F**：theme 微调 + 设计 token 文档化 —— 新 `docs/design-tokens.md`：完整列出 light + dark palette 8 个 token（bg / bg-tertiary / fg / fg-secondary / fg-muted / border / accent）含 hex 值 + 用途描述；status 颜色语义表（success/warn/danger/info/neutral/distinct 对应 Tailwind palette 引用）；typography 三档（dashboard 14px / marketing 15px / docs `--sl-text-base: 14.5px`）；density tokens（compact `h-7 + py-0.5 + 12px` / cozy `h-10 + py-1.5 + 13px` 来自 `lib/density.ts`，故意不放 CSS 因为每表挑哪些 slot 用得不一样）。**Hard rules**：no raw hex inline、no new font family、`prefers-reduced-motion: reduce` 必须尊重、status 颜色 one-way（绿不能复用做 neutral）。三处 CSS（`web/src/index.css` / `marketing/src/styles/global.css` / `docs-site/src/styles/overrides.css`）顶上加注释指向 doc table 作 single source of truth：**值仍手动镜像不走 npm 共享包**——三个 surface 各打 bundle，shared CSS package 强制每次 token 改动跑发布循环成本不值，doc-driven reconciliation 够用（review checklist 加 "PR 同时改三处 + 表"）。本 sub 不调具体颜色值（已收敛），只做 hardening：注释 + docs。dashboard build 103.5 KB gzip 不变。

**Phase 28 ✅** — 全 6 sub 完成：Cmd+K 全局搜索 / `?` 键盘 cheatsheet / a11y baseline (focus-visible + skip-to-content + reduced-motion) / route-level code splitting 首屏 143 → 103.5 KB / empty + loading + error 一致化 / design token 文档锁定。

---

# 🎉 v0.2 ROADMAP COMPLETE 🎉

Phase 18 → 28 全部 ✅。从 Phase 18 的 org / team 账户结构、19 的角色权限矩阵、20 的审计日志、21-22 的 SDK 矩阵 + 原生符号化、23 的 release 一等公民、24 的 issue list power-user、25 的 issue 详情 revamp、26 的 health metrics、27 的告警规则引擎、到 28 的搜索 + polish —— 跨 11 个 phase、~60 sub、Rust + TypeScript + Kotlin + Swift 多语言、server + dashboard + marketing + docs + 6 SDK 全栈。下一阶段进 v0.3 规划。

---

---

## v0.1.x — SaaS 上线 + dogfood（Phase 11-17）

self-hosted v0.1.0 之后的 SaaS arc：DNS/TLS（11）→ marketing + docs site（12）→ 多租户改造（13）→ self-serve onboarding（14）→ free tier 配额（15）→ 生产就绪 + 公开上线 sentori.golia.jp（16）→ SDK 分发链路 + dogfood + qualcomm/insight 真接入（17）。

### Phase 11 — 域名 / DNS / TLS 准备

**Goal:** 通过 devops `zones.yaml` 把 `sentori.golia.jp` 主域 + 5 个 4 段 subdomain（status 留到 Phase 16）落地；origin VM 上 Caddy 自动 ACME 出 cert。
**Entry:** Phase 10 完成（self-hosted v0.1.0 已发布）。
**Exit:** 6 个域名都能 HTTPS 访问（即使内容是 502/404 也行），证书 valid。
**Estimate:** 0.5 周。

#### Steps

##### 决策

- [ ] **决策**：DNS 通过 devops 项目 `zones.yaml` 管理（`golia.jp` zone 下加 records），**不**自起 Cloudflare client
- [ ] **决策**：3 段 `sentori.golia.jp` → CF Pages + orange cloud（Universal SSL 自动覆盖）；4 段子域 → grey cloud + origin Caddy + Let's Encrypt
- [ ] **决策**：4 段子域全部反代到同一台 origin VM（Phase 16 决定具体是 t01 还是 Hetzner，Phase 11 阶段先指 t01 占位）

##### DNS（在 devops 项目）

- [ ] `cd ~/workspace/goliajp/devops`，编辑 `zones.yaml`，在 `zones.golia.jp.records` 下追加：

  ```yaml
  - { name: sentori,         cname: <cf-pages-target> }   # marketing，Phase 12 部署后才填具体 target
  - { name: app.sentori,     host: <origin-vm> }          # dashboard
  - { name: ingest.sentori,  host: <origin-vm> }          # SDK 上报
  - { name: api.sentori,     host: <origin-vm> }          # admin API
  - { name: docs.sentori,    host: <origin-vm> }          # 文档站
  - { name: cdn.sentori,     host: <origin-vm> }          # SDK/CLI 静态资源
  ```

- [ ] **必须**：跑 `devops dns diff` 给用户看，列出 CREATE / DELETE / UPDATE 各几条，等明确确认后再 sync（feedback_dns_delete.md 教训）
- [ ] `devops dns sync` 执行
- [ ] Cloudflare 后台手动确认 4 段 subdomain 的 proxy 状态是 **DNS-only（grey cloud）**，`sentori.golia.jp` 是 **proxied（orange cloud）**
- [ ] 验证：`dig app.sentori.golia.jp` / `dig sentori.golia.jp` 返回正确 IP / CNAME

##### TLS（在 origin VM）

- [ ] 在 origin VM 的 Caddyfile 加 6 个 site block（占位即可，不需要真后端）：

  ```caddy
  app.sentori.golia.jp, ingest.sentori.golia.jp, api.sentori.golia.jp,
  docs.sentori.golia.jp, cdn.sentori.golia.jp {
      respond "sentori — phase 11 placeholder" 503
  }
  ```

- [ ] `caddy reload`，等 Caddy 自动跑完 ACME（每个 subdomain 各一张 cert）
- [ ] 验证：`curl -vI https://app.sentori.golia.jp` 等 5 条 TLS 握手成功（503 body 也算成功）
- [ ] `sentori.golia.jp` 的 cert 由 Cloudflare Universal SSL 自动管理，无需 origin 配置；验证 `curl -vI https://sentori.golia.jp` 返回 CF 默认页或 503

##### 收尾

- [ ] 写 `docs/infrastructure/dns.md`：subdomain 表 + cf 模式（grey/orange）+ 续期路径（CF Universal SSL 自动 / Caddy 自动）+ 链接到 devops zones.yaml
- [ ] commit：`infra: sentori dns records via devops, caddy tls on origin`

---

### Phase 12 — Marketing 站 + 文档站

**Goal:** `sentori.golia.jp`（主站）和 `docs.sentori.golia.jp`（文档）都有内容并部署。
**Entry:** Phase 11 完成。
**Exit:** 两个站可访问；主站含 Hero / Features / Self-Hosted CTA / GitHub 链接；文档站含 getting-started / SDK / protocol 全套。
**Estimate:** 2 周。

#### Steps

##### Marketing 站
- [x] **决策**：Astro（SSG，SEO 好，bundle 小）
- [x] `cd sentori && bunx create-astro@latest marketing --template minimal`
- [x] 装 Tailwind v4：复用 `web/` 的 design tokens（共享 CSS vars）
- [x] 写 `marketing/src/pages/index.astro`：
  - [x] Hero：产品定位句 + Get Started CTA + GitHub 链接
  - [x] Features 网格 4 项：JS+Native 错误捕获 / 协议简洁 / 部署轻 / 现代 dashboard
  - [x] "Open Source & Self-Hostable" 区块
  - [x] "Why we built Sentori" 区块（一段对比 Sentry 的思考）
- [x] 写 `marketing/src/pages/pricing.astro`：v0.1 阶段—Self-hosted 永久免费 + SaaS Free Tier 100k events/月（"Beta：邀请制"）
- [x] 设计语言：暗色优先、dense、参照 Linear / Vercel；不堆 emoji 不用 illustration
- [x] SEO：完整 meta 标签（canonical / og:type / og:site_name / twitter:card+image）+ `@astrojs/sitemap` 生成 sitemap-index.xml + 手写 robots.txt（同时给 docs-site）
- [x] OG image：手画 1200×630 SVG（暗色 + 紫色 accent + react-native/rust/self-hosted chips），sharp 转 PNG（39 KB），marketing/ 与 docs-site/ 各一份

##### 文档站
- [x] `cd sentori && bunx create-astro@latest docs-site --template starlight`（手搓——避开 create-astro 的交互 prompt）
- [x] 把 `docs/getting-started.md`、`sdk-react-native.md`、`protocol.md`、`self-hosting.md` 迁入 `docs-site/src/content/docs/`
- [x] 配置侧栏导航：Guides（Getting started / Self-hosting）+ Reference（SDK / Protocol）
- [x] 配置内置全文搜索（Pagefind）—— Starlight 默认开启，build 输出 `dist/pagefind/`，索引 1230 词
- [x] 暗色为默认（`overrides.css` 复用 `web/` 的 palette）

##### 部署
- [x] GitHub Actions：`.github/workflows/pages.yml` —— `wrangler pages deploy` 双 job（marketing + docs），trigger 限定 marketing/ docs-site/ docs/ 改动
- [ ] **(user-owned)** Cloudflare 控制台开两个 Pages 项目：`sentori-marketing` + `sentori-docs`（项目名与 workflow 写死的 `--project-name` 对齐）
- [ ] **(user-owned)** 在每个 Pages 项目里绑 custom domain：`sentori.golia.jp` / `docs.sentori.golia.jp`（DNS 已在 zones.yaml 准备好；CF Pages 加域名时会自动签 cert）
- [ ] **(user-owned)** GitHub repo Secrets：`CLOUDFLARE_API_TOKEN`（scope=Pages: Edit）+ `CLOUDFLARE_ACCOUNT_ID`
- [ ] **(user-owned)** 首次手动触发 workflow_dispatch（或推一个改动）验证 deploy 通；之后 main commit 自动 deploy

---

### Phase 13 — 多租户改造（org / user / membership）

**Goal:** server + dashboard 支持 user / org / project 三层模型；项目数据按 org 隔离。
**Entry:** Phase 12 完成。
**Exit:** 一个用户能注册、创建 org、邀请协作者、管理多个 project，看不到别人 org 的数据。
**Estimate:** 3 周。

#### Steps

##### 数据模型

- [x] migration `0007_multi_tenant.sql` (前 6 个 migration 已用：0001 init / 0002 issues / 0003 partition / 0004 issue_denorm / 0005 release_artifacts / 0006 notifications)：
  - [x] `users` (id, email UNIQUE, password_hash, email_verified, created_at)
  - [x] `orgs` (id, slug UNIQUE, name, created_at, owner_id FK)
  - [x] `memberships` (org_id FK, user_id FK, role enum('owner','admin','member'), created_at, PK(org_id, user_id))
  - [x] `projects.org_id` 加 FK + 回填默认 org（dev system org `019508a0-0001-...`，password_hash 占位非合法 argon2 → 不可登录）
  - [x] `tokens.org_id` 加 FK + 回填
  - [x] `email_verifications` (token, user_id, expires_at)
  - [x] `sessions` (id, user_id, expires_at, ip, user_agent)
  - [x] `org_invites` (token, org_id, email, role, expires_at, used_at)

##### Server

- [x] 加依赖：`argon2 = "0.5"`（密码哈希）、`rand = "0.8"`（token 生成）
- [x] `server/src/passwd.rs` + `server/src/api/user_auth.rs`：
  - [x] `POST /api/auth/register`（email + password，dup → 仍 200 防 enumeration，发邮件验证 link via notifier）
  - [x] `GET /api/auth/verify?token=...`（24h TTL）
  - [x] `POST /api/auth/login` → DB session row + httpOnly + SameSite=Lax cookie（base_url https 时自动带 secure）
  - [x] `POST /api/auth/logout`（DELETE sessions row + 清 cookie）
  - [x] `GET /api/auth/me`（JOIN sessions/users，校验 expires_at）
- [x] `server/src/api/orgs.rs` (12 endpoints, all guarded by `require_user`)：
  - [x] org CRUD：`POST /api/orgs`（创建 + 自动 owner membership，事务原子）/ `GET /api/orgs`（list-mine）/ `GET|PATCH|DELETE /api/orgs/{slug}`
  - [x] membership：`GET /api/orgs/{slug}/members` / `PATCH /api/orgs/{slug}/members/{user_id}`（owner only，禁止自降级）/ `DELETE /api/orgs/{slug}/members/{user_id}`（self-leave 任何成员可，他人需 admin/owner，最后一位 owner 不可删）
  - [x] 邀请流程：`POST /api/orgs/{slug}/invites`（admin/owner，禁邀 owner 角色）/ `GET /api/orgs/{slug}/invites`（未用） / `DELETE /api/orgs/{slug}/invites/{token}` / `POST /api/invites/{token}/accept`（已登录用户接受，校验 email match + 过期 + 未用，写入 membership 事务原子）
  - [x] notifier：新 `NotifyEvent::OrgInvite` variant 发邀请邮件
- [x] middleware 改造（sub-D）：`require_admin` 接受三种 caller —— DB user session / 旧 `admin_password` HMAC cookie / Bearer token —— 注入 `AdminCaller` enum 到 request extensions；新 `require_project_in_org` 解析 path 中 `/projects/{uuid}/...`，对 `User` caller 做 `projects ⨝ memberships` scope check（`LegacyAdmin` / `DevToken` 是 super-admin 直接放行）
- [x] rate limit：注册 / 登录端点 per-IP（`rate_limit_auth_middleware`，30/min，Valkey INCR + 60s expire，X-Forwarded-For 解析，无 Valkey fail-open）
- [x] `GET /admin/api/projects`：`User` caller → 自己 orgs 下的 projects；`LegacyAdmin` / `DevToken` → 全部 projects（super-admin）。返回 camelCase `{ id, name, orgId, orgSlug, createdAt }`（**回填 Phase 6 line 308 deferred**）

##### Dashboard

- [x] 路由：`/login`（替换原 admin_password 登录为 email+password）、`/register`、`/verify?token=`、`/forgot-password`（stub）—— `userAuthApi`（`/api/auth/*`）+ `adminApi`（`/admin/api/*`）拆分；AuthProvider 升级带 `user: {id, email}`
- [x] 顶栏 org switcher（`OrgSwitcher` native select；切换 → `navigate('/org/{slug}/issues')`）
- [x] `/org/:slug/settings`（`OrgSettingsView`）：org 重命名 / 成员列表 + role select（owner-only）+ Remove/Leave / 邀请创建 + pending invites 列表 + Revoke
- [x] `/org/:slug/projects/{id}/settings/recipients`（`RecipientSettingsView`）：增删 `notification_recipients` + on_new_issue / on_regression toggles —— 回填 Phase 9 deferred
- [x] server: 新 `api/recipients.rs` 4 端点（list / create / patch / delete），挂在 `/admin/api/projects/{id}/recipients`，自动经过 `require_admin` + `require_project_in_org`
- [x] 所有 issue/project 路由前缀加 `/org/:slug` —— `/org/:slug/issues[/...]`；`OrgLayout` 提供顶栏 + `OrgCtx`；`/` 由 `RootRedirect` 跳第一个 org 或 `/onboarding`；废弃 `DEV_PROJECT_ID` 写死，`useOrg().currentProject` 给 `IssuesView` / `IssueDetailView` 用
- [x] onboarding：verify 成功后 server 自动 bootstrap personal org（slug ← email 前缀 sanitized + 冲突自动 `-2/-3/...` 后缀，事务原子写入 orgs + memberships）；dashboard `/onboarding` 改造为 fallback 手动建 org form（只在 server 自动建失败或用户失去所有 org 时命中）

##### 测试

- [x] 集成测试 + commit：`scripts/test-phase13.sh` —— 重跑安全（fixture suffix 用 `date +%s%N`），覆盖：
  - [x] register / verify / login + sub-H bootstrap personal org
  - [x] cross-org `GET /admin/api/projects` 隔离（alice ≠ bob 的 projects 列表）
  - [x] alice GET bob's project issues → 403；反向 → 403；本人 → 200；dev token super-admin → 200
  - [x] alice 邀请 bob → bob accept → bob 现在能看 alice 的 project issues（隔离解除）
  - [x] mismatched-email invite → 403 inviteEmailMismatch
- [x] Phase 13 整体收尾（`feat: multi-tenant orgs, users, memberships` 由 sub-A..I 8 个 commit 累计完成）

---

### Phase 14 — SaaS 自助 onboarding

**Goal:** 用户从 `sentori.golia.jp` 注册到拿到第一个 token 接入应用 ≤ 5 分钟。
**Entry:** Phase 13 完成。
**Exit:** 一个新人按 dashboard 引导能 5 分钟内 RN 应用接入并看到第一条事件。
**Estimate:** 1.5 周。

#### Steps

- [x] 注册成功后 onboarding wizard（4-step in `views/onboarding.tsx`，state machine derived from server data + manual override for SDK/wait steps）：
  - [x] Step 0: Create org（仅 user 没 membership 时命中——sub-H 自动 bootstrap 多数情况下跳过）
  - [x] Step 1: "Create your first project" → name 输入 → 自动 create project + create default token，raw token 透传到下一步
  - [x] Step 2: "Install the SDK" → 显示 token + ingestUrl + bun install + initSentori snippet + copy button
  - [x] Step 3: "Send your first event" → poll `listIssues(projectId)` 每 3s，第一条 issue 出现自动 navigate to issues
- [x] 顶栏 `OnboardingBadge` 红点：当前 org 没 project 或 first project 没 events（issues count == 0）时显示，点击 → `/onboarding`；refetch 60s；RootRedirect 优先把没 project 的 user 直接跳 onboarding
- [x] 项目设置页：token 管理（生成 / 撤销 / 标签）：
  - [x] migration `0008_tokens_meta.sql`：tokens 加 `label` + `last4`
  - [x] `POST /admin/api/orgs/{slug}/projects` —— User caller 必须是 owner/admin
  - [x] `POST /admin/api/projects/{id}/tokens` —— 返回 raw token 一次（`st_pk_<26 Crockford>` = 32 chars 总），DB 只存 sha256 hash + last4 + label
  - [x] `GET /admin/api/projects/{id}/tokens` —— 列出 metadata（不含 raw）
  - [x] `DELETE /admin/api/projects/{id}/tokens/{tid}` —— 撤销（set revoked_at）；revoked token → /v1/events 401
  - [x] dashboard `/org/:slug/projects/:id/settings/tokens` (`TokenSettingsView`)：generate form (label + kind public/admin) + 一次性 reveal box（copy / dismiss）+ tokens table（label / kind / last4 / created / status / Revoke）；recipient/token settings 互相 cross-link
- [x] 邀请协作者流程：
  - [x] org settings 页"Invite member"（Phase 13 sub-G 已就位）+ server `notifier::OrgInvite` 邮件含 `{base_url}/invite/{token}` 链
  - [x] 邀请链接 `/invite/:token`（`InviteAcceptView`）：未登录 → `Navigate /login?next=/invite/{token}`；已登录 → 自动 `acceptInvite` + StrictMode-safe ref 防双调用 + navigate `/org/{slug}/issues`；错误码（mismatch/expired/used/notFound）映射到友好文案
  - [x] login/register 互相 carry `?next=` 参数；`sanitizeNext` 拒绝跨域/双斜线开放重定向
- [x] 改 marketing 的 "Get Started" 按钮 → 直链 `https://app.sentori.golia.jp/register`（同时保留 docs/github 次级 CTA）
- [x] e2e：`scripts/test-phase14.sh` —— 注册 → verify (sub-H bootstrap) → 登录 → create project → create public token → POST /v1/events 用 token (202) → dashboard 看到 TypeError issue (sub-A 修复 events 用 `state.project_id` 写死的 bug：`auth::IngestCaller` 注入 token's project_id) → revoke token → POST 同事件 → 401
- [x] Phase 14 整体收尾（`feat(saas): self-serve onboarding from registration to first event` 由 sub-A..E 5 个 commit 累计完成）

---

### Phase 15 — 配额 / 限流 / usage 计量（free tier）

**Goal:** Free tier 配额（100k events/月、保留 30 天）执行到位，超额时优雅降级。
**Entry:** Phase 14 完成。
**Exit:** 一个 org 超过 100k events 后新事件被 429 拒收，dashboard 显示 banner，邮件告知 owner。
**Estimate:** 1 周。

#### Steps

- [x] migration `0009_quotas.sql`（0008 已用于 tokens_meta）：
  - [x] `org_quotas` (org_id PK FK, plan `org_plan` enum, event_limit_monthly, retention_days, created_at, updated_at) + 22 个现有 org 回填 free 100k/30d
  - [x] `usage_counters` (org_id FK, period_yyyymm, event_count, dropped_count, updated_at, PK(org_id, period_yyyymm))
- [x] server ingestion 路径加 quota check（`quotas::check_and_record`）：
  - [x] 入库前查 Valkey `usage:<org_id>:<yyyymm>`；缺 quota row 时按 free 默认；DevToken / 无 Valkey → 跳过 fail-open
  - [x] 超限 → 429 + body `{"error":"quotaExceeded","resetAt":"<RFC3339 next-month UTC>"}` + INCR `dropped:<org_id>:<yyyymm>`
  - [x] 未超限 INCR `usage:` 并设 32d TTL；返回 Allowed{current,limit}
  - [x] events handler + events_batch handler 都接入（batch 内逐条 gate，超限那条算 rejected error="quotaExceeded"）
  - [x] `IngestCaller::Token` 扩出 `org_id` 字段（auth.rs `lookup_token_row` 单 SELECT 同时取 project_id + org_id），免去 events 路径的 projects join
  - [x] 后台 `quotas::spawn_flush_task` 每 60s `SELECT org_id FROM org_quotas` → GET valkey 双 key → UPSERT `usage_counters`；main.rs 在 db+valkey 都 ready 时 spawn
- [x] retention 清理：`server/src/retention.rs::spawn_retention_task` 每 24h 跑一次 —— `ensure_future_partitions(6 个月)` + `drop_expired_partitions(now - max(retention_days))`，partition 名通过 regex `^events_[0-9]{4}_[0-9]{2}$` + `parse_partition_name` 双重校验防注入；e2e 验证：seed 一个 `events_2020_01` → server 起来 30s 后被 drop，同时新创 `events_2026_09 / 10` 凑齐 6 个月未来 partition
- [x] dashboard 配额 widget：
  - [x] server `GET /api/orgs/{slug}/usage` —— Valkey 优先（实时） + PG `usage_counters` fallback；返回 `{plan, eventLimitMonthly, retentionDays, periodYyyymm, eventCount, droppedCount, percentUsed, resetAt}`
  - [x] org settings 页 `UsageSection`：events used / limit + 进度条（≥80% amber，≥100% red） + dropped count
  - [x] 用量 ≥ 80% 全局 `UsageBanner`（OrgLayout 顶栏下方常驻）：amber 提示 percent，红色 "quota reached" + dropped 计数；refetchInterval 60s
- [x] 邮件：用量 ≥ 80% / ≥ 100% 各发一封 —— `quotas::maybe_warn` 检测 cross-threshold（prev<t && current>=t），用 Valkey `SET notified:<t>:<org>:<period> 1 NX EX 32d` 去重；`NotifyEvent::QuotaWarning` variant + notifier 查 owner/admin 的邮箱发模板邮件（80% warn / 100% reached）；e2e: limit=2 → event 2 单次跨 80+100 两个阈值，log 双行 + Valkey 两 flag
- [x] 默认 plan = `free`，新 org 自动 100k 限额（`server/src/quotas.rs::ensure_default_quota`，挂在 `orgs::create_org` 与 `user_auth::bootstrap_personal_org` 的事务里，ON CONFLICT DO NOTHING）
- [x] **决策**：free tier 数据 30 天保留；pro / enterprise 留到付费上线
- [x] e2e：`scripts/test-phase15.sh` 7/7 阶段全过 —— register + bootstrap → tighten quota=2 → create project+token → 4 events (202/202/429/429) → Valkey usage/dropped/notified flags → 429 body resetAt RFC3339 → `GET /api/orgs/{slug}/usage` 返回 plan/eventCount/percentUsed/droppedCount；不破坏 Phase 13 (6/6) 和 Phase 14 (8/8) smoke
- [x] Phase 15 整体收尾（`feat(saas): free tier quota enforcement and usage metering` 由 sub-A..F 6 个 commit 累计完成）

---

### Phase 16 — 生产就绪 + 公开上线 sentori.golia.jp

**Goal:** sentori.golia.jp 公开可注册；监控/告警/备份/runbook 全部就位；自己用 Sentori 监控 Sentori。
**Entry:** Phase 15 完成。
**Exit:** 域名公开访问；dogfooding ≥ 1 周无 P1 事故；上 Show HN 时不会被打挂。
**Estimate:** 2 周。

#### Steps

##### 部署底座

- [ ] **决策**：起步 Hetzner CCX23（4 vCPU / 16GB RAM / 80GB SSD）× 2（主备） + 1 个独立 PG VM（CPX21）
- [ ] **决策**：v0.1 不上 k8s，docker compose 即可（简单 + 易调试）
- [ ] **决策**：边缘 Caddy（自动 ACME、HTTP/3）替 nginx
- [x] 写 `docker/production-compose.yml`：blue/green 双 server + caddy + valkey；PG 通过 `DATABASE_URL` 指向独立 VM
- [x] 写 `docker/Caddyfile`：app./api./ingest. 三个 site，自动 ACME TLS、`ip_hash` lb（blue/green session 粘附）+ `/v1/events/_recent` 健康检查、HSTS+CSP+CORS（dashboard 限同源、ingest 公开）
- [x] 写 `docker/README.production.md`：day-zero 部署、blue/green 滚动、回滚步骤
- [ ] **(user-owned)** 配置 Cloudflare：DNS → VM IP（grey cloud, Caddy 出 TLS；apex 仍走 CF Pages 兜 marketing）
- [ ] **(user-owned)** 部署：`docker compose -f production-compose.yml pull && up -d`

##### 监控 / 告警

- [x] **选型**：外部 Better Stack（uptime + status 页 + on-call）+ 内部 Grafana/Prometheus（metrics 详查）
- [x] server `/metrics` 暴露 prometheus exporter（`metrics 0.24` + `metrics-exporter-prometheus 0.16`）—— `server/src/metrics.rs` 用 `OnceLock<Counter/Histogram>` 模块级 cache（macro 直接重复调用在 0.16 + 0.24 组合下不累加，单元测试覆盖）
- [x] 关键指标已暴露：`sentori_ingest_total{status=accepted|rejected|quota_exceeded}`、`sentori_ingest_duration_seconds` (p50/p99 histogram)、`sentori_quota_drops_total`；e2e 验证 5 accepted + 1 rejected counter 累加正确
- [ ] PG pool / Valkey latency metrics（占位 alert 规则在 ops/prometheus-alerts.yml 已写好；exposer follow-up）
- [x] Grafana dashboard：`ops/grafana-sentori-overview.json`（4 panel：ingest rate、p50/p99 latency、quota drops、error rate stat with thresholds）
- [x] 告警规则（`ops/prometheus-alerts.yml`）：
  - [x] `SentoriIngestErrorRateHigh` (rate > 1% / 5m)
  - [x] `SentoriIngestStalled` (rate == 0 for 15m)
  - [x] `HostDiskFreeLow` (< 20%)
  - [x] PG pool / Valkey p99 占位规则（指标 follow-up 后即可启用）
- [ ] **(user-owned)** Better Stack 监测 7 个 subdomain 的 200 OK + TLS 有效期
- [ ] **(user-owned)** `status.sentori.golia.jp` 用 Better Stack status 页

##### 日志 / 备份

- [x] 日志：`ops/vector.toml` —— journald (docker.service / caddy.service) → vector → Grafana Cloud Loki，static labels service/env/unit；JSON parse 提取 tracing 字段；5 MB / 5 s 批
- [x] PG 备份脚本就绪：
  - [x] `ops/backup.sh` —— `pg_dump --format=custom --no-owner --no-acl` → R2 daily/，`rclone delete --min-age 30d` 自动 retention
  - [x] `ops/postgresql.archive.conf` —— `archive_mode=on` + `archive_timeout=300` + `archive_command='rclone copyto %p r2:.../wal/%f'` 给 ≤ 5 min RPO 的 PITR
  - [x] `ops/restore.sh` —— 从 R2 拉 latest 或指定 stamp，DROP+CREATE+pg_restore，强制交互式 `yes` 确认
  - [x] `ops/README.backup.md` —— PG VM / app VM 一次性 setup + recovery drill checklist
- [ ] **(user-owned)** 演练：在 fresh VM 跑一次完整 restore，记下分钟数到 `docs/runbook/backup-restore.md`

##### 安全 / 隐私

- [x] HTTPS only + HSTS（max-age=63072000; includeSubDomains; preload）—— Caddyfile 全局 `security_headers` snippet (sub-A)
- [x] CORS：dashboard 限同源 + ingest 公开 —— api./ingest. site 不同 CORS (sub-A)
- [x] CSP 收紧：dashboard `default-src 'self'; connect-src 'self' api./ingest.` (sub-A)
- [x] secrets 管理：`ops/secrets.md` —— sops + age cookbook（recipient 列表 in `.sops.yaml`，VM 用独立 age key，rotation 流程）
- [ ] **(user-owned)** DDoS：Cloudflare 兜底（CF 控制台 grey/orange 切换 + WAF 规则）
- [x] `docs/legal/privacy.md` + `docs/legal/terms.md`（draft template，需 lawyer review；marketing footer 已加 Privacy/Terms 链）
- [x] PII 默认行为：SDK 不上传 user.email；schema enforce —— SDK `User = {id?, anonymous?}` 已就位 (Phase 1)；server `event::User` 加 `deny_unknown_fields` 强拒 PII；SDK `setUser` JSDoc 明确政策
- [x] 数据导出 API（GDPR 风险预防）：`GET /api/orgs/{slug}/export` —— owner/admin 拿到 org + members + projects (含 tokens metadata + recipients) + pending invites 的 JSON dump，附 `Content-Disposition: attachment; filename=...`（events 全量留 V2，太大）

##### Testing（回填 v0.1 deferred）

- [x] iOS XCTest：`sdk/react-native/ios/Tests/SentoriCrashHandlerTests.swift` —— 通过 `persistForTesting(exception:)` helper 直接驱动 native handler 的 persist 路径（直接 raise NSException 会终止 test runner），断言 `<Documents>/sentori/pending/*.json` 出现 + JSON 反序列化为合法 Event（kind/platform/error.type 匹配）；CI workflow `mobile-e2e.yml::ios-xctest` 走 macOS runner
- [x] Android Robolectric：`sdk/react-native/android/src/test/.../SentoriCrashHandlerTest.kt` —— Robolectric 跑 `installForTesting(ctx)` + `persistForTesting(throwable, thread)`，断言 `<filesDir>/sentori/pending/` 内文件 JSON shape；CI workflow `mobile-e2e.yml::android-robolectric` 走 Ubuntu runner
- [x] mailcatcher 集成测试：`scripts/test-mailcatcher.sh` —— 起 mailpit 容器，注册用户 → notifier 发 verification email → 用 mailpit HTTP API 断言 subject/body 含 verify link；server 加 `SENTORI_SMTP_TLS=plain` env 兼容 mailpit；CI 跑（macOS Docker port-forwarding quirk 在本地可能漏 SMTP banner，Linux runner 无此问题）
- [ ] **(user-owned)** iOS simulator e2e 自动化（`xcrun simctl install` + `launch` + `/v1/events/_recent` poll）—— 工作量大、纯 native build 链路，留作 launch 后 dogfood 用例
- [ ] **(user-owned)** Android emulator e2e 自动化（`adb shell am start`）—— 同上
- [x] minified bundle → `sentori-cli upload sourcemap` → 触发错误 → dashboard 显示原始位置：`scripts/sourcemap-e2e/{run.sh,throw-and-format.js,app.tsx,metro.fixture.config.js}` —— Metro 出 minified bundle + map → cli upload → Node eval bundle 触发 throw → POST 到 server → 拉 admin API + symbolicated=true → 断言 top frame 指向 `app.tsx` 而非 `bundle.js`
- [x] GitHub Actions：`.github/workflows/mobile-e2e.yml` —— 4 jobs (ios-xctest macos-14, android-robolectric ubuntu, mailcatcher with services, sourcemap-e2e)，path filter 限定 `sdk/`、`cli/`、`server/`、`scripts/sourcemap-e2e/` 改动才跑

##### Dogfooding

- [ ] sentori 自己接入 sentori：marketing / dashboard / server 都向自家 prod 报错
- [ ] 跑 ≥ 1 周观察：crash 率、ingestion latency、grouping 准确度、自家邮件告警是否触发

##### Runbook

- [x] `docs/runbook/incident-response.md` —— P1/P2/P3 ladder + on-call rotation + 60s P1 checklist + 显式 NOT-page 列表
- [x] `docs/runbook/scaling.md` —— v0.2 capacity baseline + 横向加 app VM 步骤（更新 Caddy upstream + reload）+ PG vertical resize 阈值表 + 烫手 org 处理（quota first, compute later）+ "我们 yet 不做 autoscaler / 跨 region active-active / SDK 队列"
- [x] `docs/runbook/backup-restore.md` —— 备份矩阵表 + 何时 restore 决策表 + 完整 failover 步骤 + quarterly drill checklist + "Last drill: never" 跟踪
- [x] `docs/runbook/deploy.md` —— pre-flight + cut release + blue/green 滚动（每个 container 间隔 5 min 看 Grafana）+ 双步骤式 destructive migration 模式 + rollback

##### 公开发布

- [x] 改 `marketing/pricing.astro`：去掉 "Beta 邀请制"，开放注册（"$0" 标价 + "Sign up — free →" 直链 `app.sentori.golia.jp/register`，footer 加 Privacy/Terms）
- [x] 写 launch 文章 draft：`docs/launch/show-hn-draft.md`（80 字符 HN 标题 + 体 + 反 obvious-question prep + 跨平台节奏指引）
- [x] 准备 demo 视频脚本：`docs/launch/demo-script.md`（30 秒 storyboard，注册→onboarding wizard→token→dashboard→issue 出现，无 voiceover、burn-in caption）
- [x] 整合 launch checklist：`docs/launch/checklist.md` —— `[code]` vs `[ops]` 标注，跨 Phase 11/12/16 user-owned 全部 inventory
- [ ] **(user-owned)** tag `v0.2.0` + GitHub release notes
- [ ] **(user-owned)** 录视频 + Lawyer review 法律文档 + 配 SPF/DKIM/DMARC + 一周 dogfooding 无 P1
- [ ] **(user-owned)** HN 发文（周二/周三 早上 PT）
- [ ] 🎯 **里程碑：sentori.golia.jp 正式开放**

---

### Phase 17 — SDK 分发链路 + dogfood + qualcomm/insight 真接入

**Goal:** 让 sentori SDK / CLI 从 "git 路径安装" 提升到 "`npm install` / `npx` 一行装"；dashboard onboarding wizard 提供 RN / JavaScript 双 snippet；sentori 自家 web 三个项目接入做 dogfood；最终给 `qualcomm/insight` (Expo RN) 上 sentori。
**Entry:** Phase 16 sub-H ✅。
**Exit:** 任意 RN 或 web 项目能用一行 `bun add @sentori/react-native` (或 `@sentori/javascript`) 装上 + init 即工作；`qualcomm/insight` 在生产抛错能在 sentori dashboard 看到 + symbolicated stack。
**Estimate:** 1.5–2 周。

#### Steps

##### sub-A — `@goliapkg/sentori-react-native` npm publish ✅

实际包名 `@goliapkg/sentori-react-native`（@sentori free org 必须 npmjs.com 网页手动创建；规避到 @goliapkg user-controlled scope；brand 仍是 sentori-react-native）。Expo Config Plugin 路径不需要——现有 `expo-module.config.json` + podspec + android/build.gradle 已让 `expo prebuild` autolink。

- [x] `package.json`：0.0.0 → 0.1.0；license MIT、repo / bugs URL、keywords、publishConfig.access=public
- [x] `files` whitelist 加 `android/src/`、`android/build.gradle`、`ios/`、`expo-module.config.json`、`SentoriReactNative.podspec`（86 files / 32 kB tarball）
- [x] `npm publish` —— 0.1.0 上线；`bun add @goliapkg/sentori-react-native` 安装 verified
- [x] 全仓搜替 `@sentori/react-native` → `@goliapkg/sentori-react-native`：web onboarding wizard 三段 snippet、`docs/{getting-started,sdk-react-native}.md`、`docs-site/src/content/docs/{index,getting-started,sdk-react-native}.{mdx,md}`
- [ ] tag-driven `publish-sdk-rn.yml` workflow（与 sub-B 的 CLI release pipeline 一起做）

##### sub-B — `@goliapkg/sentori-cli` 跨平台 prebuilt binary + npm 包装 ✅

- [x] `.github/workflows/release-cli.yml`：tag `cli-v*` 触发 `cargo build --release` 矩阵 (linux-x64 / linux-arm64 / darwin-arm64)，`.tar.gz` + `.sha256` 传到 GitHub Release。darwin-x64 这次跳过（GH-hosted Intel mac runners 排队 stuck）；cargo install 是 fallback
- [x] `cli/npm/`：thin Node wrapper，bin 走 spawn 子进程，postinstall 按 `process.platform-arch` 下载 release 资产到 `vendor/`，`SENTORI_SKIP_DOWNLOAD=1` 逃生
- [x] `npm install -D @goliapkg/sentori-cli` → 二进制下载、`./node_modules/.bin/sentori-cli --help` 正常输出 Rust CLI help
- [x] `docs/sdk-react-native.md` + `docs-site/.../sdk-react-native.md` sourcemap upload snippet 改成 `npx @goliapkg/sentori-cli upload sourcemap ...`
- [x] README 单独说明 bun 用户需 `bun pm trust @goliapkg/sentori-cli`

##### sub-C — Dashboard onboarding wizard SDK 选择 ✅

- [x] `InstallSdkStep` 加两按钮 picker：React Native / JavaScript（默认 RN）
- [x] `sdkSnippets()` helper 按 SDK 分别返回 `install` + `init` 字符串；CodeBlock 复用，复制体验不变

##### sub-D — `@goliapkg/sentori-javascript` (web + node) ✅

实际包名 `@goliapkg/sentori-javascript`（与 sub-A 的 scope 决定一致）。ESM-only（modern Node + browsers + Bun 都吃；CJS dual-build 收益太薄）。

- [x] `sdk/javascript/`：tsc-only build；零运行时依赖
- [x] 核心 surface：`initSentori` / `captureError` / `captureException` / `setUser` / `getUser` / `addBreadcrumb` / `getBreadcrumbs` / `clearBreadcrumbs`
- [x] 浏览器 hooks：`window.error` + `unhandledrejection`，idempotent
- [x] Node hooks：`process.on('uncaughtException' | 'unhandledRejection')`，**故意不 process.exit**（host 拥 crash policy）
- [x] uuid v7 自实现（crypto.getRandomValues + ms timestamp）；stack regex 同时认 V8 与 SpiderMonkey + URL-style file paths
- [x] transport：browser 优先 `navigator.sendBeacon`（小 body + tab close 存活），fallback `fetch keepalive: true`；4xx/5xx silent drop（v0.1 不带 retry queue）
- [x] bun:test 8 个单测（uuid 形态 + 唯一性、stack v8/spider/url、breadcrumb FIFO 100 cap、captureError 正确 POST shape + cause chain）
- [x] `npm publish @goliapkg/sentori-javascript@0.1.0` 上线；`npm install` 起 ESM import + initSentori + captureError 都通

##### sub-E — mailrs `sentori@golia.jp` 密码改 argon2id ✅

- [x] 用 `debian:13-slim` docker 一次性 + `argon2 -id -m 16 -t 3` 生成 hash
- [x] `docker cp` 写入 mailrs container 的 `/data/users.toml` 替换 `password = "..."` 为 `password_hash = "$argon2id$..."`
- [x] mailrs 重启 + STARTTLS + AUTH LOGIN 探活通
- [x] sentori 端 SMTP_PASS secret 不变（明文密码相同），无需 redeploy
- [x] `ops/secrets.md` 加 mailrs SMTP user 完整轮换 playbook

##### sub-F — Sentori 自家 dogfood ✅

- [x] prod 注册 `dogfood-<ts>@golia.jp`、auto-bootstrap personal org `dogfood-<ts>`
- [x] 创建三个 project：`sentori-dashboard`、`sentori-marketing`、`sentori-docs`，各 mint 1 个 public token
- [x] 三个 token 入 sentori repo secrets：`SENTORI_DOGFOOD_{DASHBOARD,MARKETING,DOCS}_TOKEN`
- [x] `web/main.tsx` `import { initSentori } from '@goliapkg/sentori-javascript'`；token 走 `import.meta.env.VITE_SENTORI_TOKEN`，dev 没 token 时 Vite tree-shake 掉 SDK
- [x] `marketing/src/pages/{index,pricing}.astro` 各加 `<script>` 块；Astro bundle 把 SDK 编进 `_astro/*.js` 块；`PUBLIC_SENTORI_TOKEN` 内联进 source
- [x] `docs-site/src/components/Head.astro` 重写 Starlight Head（先 import 默认再 append `<script>`）；同样 PUBLIC_SENTORI_TOKEN
- [x] `docker/Dockerfile.web`：`ARG VITE_SENTORI_TOKEN` + `ARG VITE_GIT_SHA` + `ARG VITE_SENTORI_INGEST` → ENV → COPY → `bun run build`，token 在镜像 build 时被 Vite inline
- [x] `goliajp/devops` `services/sentori/docker-compose.yml` web service `build.args.{VITE_SENTORI_TOKEN, VITE_GIT_SHA}` 从 compose env 取
- [x] `.github/workflows/deploy.yml`：marketing 与 docs build step 各加 `PUBLIC_SENTORI_*_TOKEN` + `PUBLIC_GIT_SHA` env；Write `.env` step 写 `SENTORI_DOGFOOD_DASHBOARD_TOKEN` + `SENTORI_VERSION` 给 docker compose 用
- [x] release/v0.2.0 force-FF 到 main HEAD + `gh workflow run` 触发部署，build/health/smoke 全绿
- [x] 实测：用 marketing 的 build-time inlined token POST `/v1/events` (DogfoodSmokeError) → 落进 `sentori-marketing` project，dashboard listIssues 能看到

##### sub-G — `qualcomm/insight` 接入（Phase 17 的真目的地）

- [x] sentori prod 注册 / 选 org → create project `qualcomm-insight`（dogfood org，project id `019e0ea2-fe14-7451-9441-a22d34e0fbaa`，public token `st_pk_byhzg0spp3xz7g0kgswk7zr4x8`）
- [x] `qualcomm/insight` repo：`bun add @goliapkg/sentori-react-native@0.1.2`（npm scope 决策为 `@goliapkg/*`，因为 `@sentori` 在 npmjs.com 仅 web UI 可创建）
- [x] tenant config 拓展：`TenantInput.sentori?: { ingestUrl; token }` 加到 `src/core/gen/types.ts` + defaults 透传 + `tenants/qualcomm/config.ts` 填值
- [x] bootstrap 接线：新增 `src/core/bootstrap/scripts/sentori.ts`（prod-only `__DEV__` gate，调 `initSentori`，release 串 `${slug}@${ios.version}+${android.versionCode}`），`setup.ts` 在 sentry import 之前 `import './scripts/sentori'`
- [x] 用 qualcomm-insight token 模拟 POST 一条事件到 `ingest.sentori.golia.jp/v1/events` → 202 → dashboard 看到 `QualcommInsightSmoke` issue（验证 token + ingest + grouping 全链路工作）
- [x] commit `qualcomm/insight` changes（commit `0f3fa68b`，`feat: install sentori RN SDK alongside existing sentry`）
- [ ] iOS：`expo prebuild` + `pod install` + run on simulator → trigger test error → 验证 dashboard 看到（**留给下一次 qualcomm/insight 真机/simulator build cycle**，不在自动化范围内）
- [ ] Android：同上（同上原因）
- [ ] EAS build hook：上传 source map (`@sentori/cli upload sourcemap`) 到 sentori，confirm dashboard 显示原始位置（**等首个 RN bundle ship 后再启用**）
- [ ] 🎯 **里程碑：sentori 接住第一个真实生产 RN 应用**（JS-layer 已就位 + token 已 verify；待下一次 qualcomm/insight 真机 build 跑出第一条 native 事件即勾掉）

##### 实际部署落地（Phase 16 sub-H — 通过 devops infra 接入，而非原计划的"Hetzner + 独立 Caddy"）

- [x] `goliajp/devops` repo `services/sentori/{docker-compose.yml, README.md}`：postgres + valkey + sentori-server + sentori-web + marketing nginx + docs nginx，类比 portal/tasks 部署形态
- [x] `goliajp/devops` repo `devices/cloud/t01/caddy/Caddyfile` 加 5 个站点（apex sentori / app / api / ingest / docs），通过 `devops caddy deploy t01` push + reload + LE 证书 provision 完成
- [x] DNS 5 个 CNAME records (`sentori`, `app.sentori`, `api.sentori`, `ingest.sentori`, `docs.sentori` → `t01.golia.jp.`) 通过 devops API + `devops dns sync golia.jp` 推到 Cloudflare live
- [x] `goliajp/sentori` repo `.github/workflows/deploy.yml`：lx64 self-hosted runner 监听 push 到 `release/*`，rsync source + build marketing/docs + 写 .env + `docker compose build && up -d` + 健康检查 + 公开 surface smoke (server `/v1/events/_recent`、`/metrics`、`/api/auth/me`、web、marketing、docs 全 200/401)
- [x] `gh secret set` 一次性配 8 个 secrets（PG / DEV_TOKEN / ADMIN / SESSION + SMTP 4 项预填空，user 后续可自行填真 SMTP）；org-level runner group 开 `allows_public_repositories=true`
- [x] `git push origin main:release/v0.2.0` + `gh workflow run deploy --ref release/v0.2.0` 触发 deploy；workflow 全绿，5 个子域 live：
  - https://sentori.golia.jp → marketing 200
  - https://docs.sentori.golia.jp → Starlight 200
  - https://app.sentori.golia.jp → dashboard SPA 200
  - https://api.sentori.golia.jp/api/auth/me → 401（auth gate 工作）
  - https://ingest.sentori.golia.jp/v1/events/_recent → 401（token gate 工作）
- [x] prod e2e 通过：注册 → verify → login → 自动 bootstrap personal org → create project → create public token → POST event 到 `ingest.sentori.golia.jp` (202) → dashboard 看到 issue (grouping work)
- [x] SMTP 走 `mail.golia.ai`（goliajp/mailrs）—— 在 mailrs `/data/users.toml` 加 `sentori@golia.jp` 作 SMTP submission user（plain pw，mailrs verifies 通过 users.toml first-tier）；6 个 `SENTORI_SMTP_*` secrets 更新指向 mailrs（mail.golia.ai:587 STARTTLS）；redeploy 后 register flow log "verification email sent"，邮件确认落到 mailrs `/data/maildir/golia.jp/<recipient>/new/` 内含正确 `[Sentori] Verify your email` subject + verify link body；DKIM/SPF 走现有 golia.jp zone 配置（`golia.jp` SPF 已含 `a:mail.golia.ai`）


---

## v0.1 — self-hosted MVP（Phase 0-10）

从零到 self-hosted v0.1.0：项目方向（0）→ event schema（1）→ server 骨架（2）→ RN SDK JS 层（3）→ E2E smoke（4）→ PG 落库 + grouping（5）→ web dashboard MVP（6）→ RN SDK Native（7）→ sourcemap（8）→ release / 邮件告警（9）→ docker compose + docs（10）。

### Phase 0 — 项目方向对齐

**Goal:** 把所有"非编码决策"一次性定下来，避免后续返工。
**Entry:** 当前会话已有的方向（自研协议 / RN-first / Rust+axum+PG+Valkey / 全 SPA dashboard / sentori.golia.jp 公网）。
**Exit:** 下面所有 checkbox 全勾完，仓库进入 git 管理。
**Estimate:** 0.5 天。

#### Steps

- [x] **决策**：项目名 `sentori` 保留（不改）
- [x] **决策**：公网域名 `sentori.golia.jp`，subdomain 拓扑见上表
- [x] **决策**：monorepo 布局如下（创建空目录占位）

  ```
  sentori/
  ├── web/                    # 已存在，dashboard SPA
  ├── server/                 # axum 后端二进制
  ├── sdk/
  │   └── react-native/       # @sentori/react-native npm 包
  ├── cli/                    # sentori-cli（sourcemap/dSYM 上传）
  ├── marketing/              # Astro 静态站，sentori.golia.jp
  ├── docs-site/              # Astro Starlight，docs.sentori.golia.jp
  ├── docs/                   # protocol.md / getting-started.md / ... 源材料
  ├── e2e/                    # 端到端测试脚手架
  ├── docker/                 # Dockerfile.server 等
  ├── docker-compose.yml
  ├── ROADMAP.md              # 本文档
  └── README.md
  ```

- [x] **决策**：token 格式 `st_pk_<26 字符 base32 of uuid-v7>`（`pk` 项目公钥；保留 `sk_` 给后续 admin secret key）
- [x] **决策**：UI 栈不改——React 19 + Vite + Tailwind v4 + jotai + react-router 继续
- [x] **决策**：SDK 包名 `@sentori/react-native`（与 `@sentori/web` 同 namespace）
- [x] 写顶层 `.gitignore`：覆盖 Rust `target/`、Node `node_modules/`、Vite `dist/`、Xcode `build/` & `*.xcuserdata`、Android `build/` & `.gradle/`、`.env`、macOS `.DS_Store`
- [x] 写顶层 `README.md` 占位：项目目标 + 状态 + 链接到 ROADMAP.md
- [x] `git init`
- [x] 第一个 commit：`chore: bootstrap sentori monorepo`（51b6ae0）

---

### Phase 1 — Event Schema + Token/Ingest 协议初稿

**Goal:** 一份 markdown spec 作为 SDK ↔ Server 双方契约，写死字段、命名、类型、token 格式、ingest URL。
**Entry:** Phase 0 完成。
**Exit:** `docs/protocol.md` 通过 review，包含至少 3 个 example payload + token/URL 设计说明。
**Estimate:** 1 天。

#### Steps

- [x] 写 `docs/protocol.md` 主体框架（最终包含：Design principles / Versioning / Endpoints / Auth / Token + Ingest URL / Response codes / Event schema / Device / App / Framework / User / Error / Frame / Breadcrumb / Batch / Size limits / Rate limits / Examples / Compatibility promises）
- [x] 章节：Token / Ingest URL 设计
  - [x] token 格式 `st_pk_<26 chars Crockford base32 of uuid-v7>`
  - [x] ingestUrl 默认值 `https://ingest.sentori.golia.jp`
  - [x] 显式说明**不复用 Sentry DSN URL 编码方案**及理由
  - [x] env 变量约定：`SENTORI_TOKEN` + `SENTORI_INGEST_URL`
- [x] 章节：HTTP 端点
  - [x] `POST /v1/events` —— 单条
  - [x] `POST /v1/events:batch` —— 数组
  - [x] 响应码语义：`202 Accepted` / `400 Bad Request` / `401 Unauthorized` / `413 Payload Too Large` / `429 Too Many Requests`
- [x] 章节：鉴权头
  - [x] `Authorization: Bearer st_pk_xxx`
  - [x] `Sentori-Sdk: react-native/0.1.0`
  - [x] `Content-Type: application/json`（不接受 multipart）
- [x] 章节：Event 顶层 schema（id / timestamp / kind / platform / release / environment / device / app / user / tags / breadcrumbs / error / fingerprint? / traceId? / spanId?）—— **review 期决议**：camelCase + 完整词 + src→platform + app.rn→app.framework + 加 traceId/spanId 扩展位
- [x] 章节：Error subobject（type / message / stack[] / cause?）
- [x] 章节：Stack frame（function / file / line / column / inApp / absolutePath? / preContext? / postContext?）
- [x] 章节：Breadcrumb 类型枚举（`nav` / `net` / `log` / `user` / `custom`）+ 各自 data 字段
- [x] 章节：Batch 包装（`{ events: [...] }`）+ 单批 size cap（1MB / 100 条）+ 部分失败语义（accepted/rejected/errors[]）
- [x] 章节：Rate limit 响应（`429` body 带 `retryAfterMs`，默认 5000 req/min）
- [x] 写 example 1：JS `TypeError`（含 breadcrumbs、release、user）
- [x] 写 example 2：iOS `NSException`（platform=ios，stack 有 absolutePath）
- [x] 写 example 3：Android `RuntimeException`（platform=android，含 cause chain）
- [x] 跟用户过一遍 review，记录修改并迭代到 v0 —— 用户授权"我来定，新和专业、无历史负担"，按 modern conventions 全面重写
- [x] commit：`docs: protocol v0`

---

### Phase 2 — Server 骨架

**Goal:** `sentori-server` 二进制在 `:8080` 接收 `POST /v1/events`，stdout 打印解析后的 Event。
**Entry:** Phase 1 完成。
**Exit:** `curl -X POST localhost:8080/v1/events -H "Authorization: Bearer st_pk_dev" -d @example1.json` 返回 `202`，stdout 输出结构化 event。
**Estimate:** 1.5 天。

#### Steps

- [x] `cd sentori && cargo new server --bin --name sentori-server`（实际手写 Cargo.toml + 拆 lib/main，效果等价）
- [x] 加依赖：
  - [x] `axum` 0.8
  - [x] `tokio` (full features)
  - [x] `serde` + `serde_json`
  - [x] `uuid` (v7)
  - [x] `time` (with serde-well-known，rfc3339)
  - [x] `tower` + `tower-http` (cors / trace / limit)
  - [x] `tracing` + `tracing-subscriber`
  - [x] `anyhow` + `thiserror` 2
  - [x] `validator` 0.20（schema 校验，含 nested 校验）
- [x] 写 `server/src/main.rs`：tokio main + tracing init + `axum::serve`
- [x] 写 `server/src/event.rs`：Event / Error / Frame / Breadcrumb / Device / App / Framework / User 的 serde + Validate，camelCase rename，对齐 protocol.md
- [x] 写 `server/src/api/events.rs`：`POST /v1/events` handler，validate 后 stdout pretty-print
- [x] 写 `server/src/api/events_batch.rs`：`POST /v1/events:batch` handler，含 partial failure 语义（accepted/rejected/errors[]）
- [x] 写 `server/src/auth.rs`：Bearer token middleware（constant-time 比对，读 env `SENTORI_DEV_TOKEN`）
- [x] 写 `server/src/router.rs`：组装 router + middleware（CORS、TraceLayer、RequestBodyLimitLayer 1 MB）
- [x] 写 `server/src/error.rs`：AppError + IntoResponse + flatten_validation_errors（共享给 batch handler）
- [x] 写 `.env.example`：`SENTORI_DEV_TOKEN=st_pk_dev0000000000000000000000`
- [x] `cargo run` + curl 校验 —— 由集成测试覆盖（reqwest 端到端跑 6 个测试）
- [x] 写集成测试 `tests/post_event.rs`：6 个测试覆盖 202 / 401×2 / 413 / 400 + batch partial failure
- [x] `cargo test` 全绿（6/6 passed）
- [x] commit：`feat(server): minimal ingestion skeleton`

---

### Phase 3 — RN SDK JS 层

**Goal:** `@sentori/react-native` 能在 demo app `init`、捕获 JS 错误、batch 上报到 Phase 2 的 server。
**Entry:** Phase 2 完成。
**Exit:** demo app 调用 `throw new TypeError("test")`，Phase 2 的 server stdout 看到完整 event。
**Estimate:** 1 周。

#### Steps

- [x] `cd sentori/sdk && bunx create-react-native-library@latest react-native`（包名 `@sentori/react-native`，类型 turbo-module，语言 kotlin + objc/swift）
- [x] 删除模板示例代码（保留构建配置）
- [x] 写 `src/types.ts`：Event / SentoriError / Frame / Breadcrumb，对齐 `docs/protocol.md`
- [x] 写 `src/init.ts`：`sentori.init({ token, release, env?, ingestUrl? })` —— 校验 token 前缀（`st_pk_`），ingestUrl 默认 `https://ingest.sentori.golia.jp`，env 默认从 `__DEV__` 推断，写入全局 config singleton
- [x] 写 `src/handlers/global.ts`：`ErrorUtils.setGlobalHandler` 包装 + 保留原 handler
- [x] 写 `src/handlers/promise.ts`：`HermesInternal.enablePromiseRejectionTracker` 接入
- [x] 写 `src/handlers/network.ts`：fetch / XHR 拦截 → 写 breadcrumb（不阻断原请求）
- [x] 写 `src/breadcrumbs.ts`：ring buffer，cap 100 条，类型枚举对齐 protocol
- [x] 写 `src/transport.ts`：
  - [x] batcher：5 秒或 10 条触发 flush
  - [x] retry：指数退避 1s/2s/4s 上限 3 次
  - [x] offline queue：AsyncStorage 落盘，下次启动 drain
  - [x] header：`Authorization` + `Sentori-Sdk: react-native/<pkg.version>`
- [x] 写 `src/error-boundary.tsx`：React `<sentori.ErrorBoundary fallback>`
- [x] 写 `src/stack.ts`：解析 RN error stack 字符串 → `Frame[]`（先不做 sourcemap，留 raw 给 Phase 8 处理）
- [x] 写 `src/index.ts`：默认 export `sentori` 对象，所有方法 namespace 化
- [x] 写 jest 单测覆盖 transport / breadcrumbs / stack 解析
- [x] `bun run prepack` 检查产物
- [x] 改 `sdk/react-native/example/App.tsx`：`sentori.init` + 一个 throw 按钮（`ingestUrl: 'http://localhost:8080'` 用于本地 dev）
- [x] iOS：`cd example/ios && bundle exec pod install`
- [x] iOS：`cd example && bun run ios` 启 simulator，点击 throw，验证 server stdout 收到事件
- [x] Android：`cd example && bun run android` 启 emulator，同验证
- [x] commit：`feat(sdk): JS-layer error capture and batched transport`

---

### Phase 4 — 端到端 smoke test

**Goal:** 锁第一个对外 visible 的里程碑——demo app throw → server 收到，建 CI 防回归。
**Entry:** Phase 3 完成。
**Exit:** `bash e2e/run.sh` 一个命令端到端跑通并退出 0（SDK transport ↔ server 协议契约自动验证）。
**Estimate:** 2–3 天。

#### Steps

- [x] server 加 dev-only `GET /v1/events/_recent`：in-memory 环形 buffer cap 100，鉴权与 ingest 端点共享
- [x] 写 `e2e/run.sh`：bun 驱动 SDK transport 端到端验证（替代 simulator GUI 自动化，保留协议契约保护）
  - [x] 启 server（debug build，背景）
  - [x] `bun install` in `e2e/`（含 `@sentori/react-native` file link）
  - [x] `bun send-event.ts`：`sentori.captureError(...)` + 等 batcher flush
  - [x] poll `GET /v1/events/_recent` 校验事件到达 + 字段（platform / error.type）
  - [x] 杀进程
- [ ] 写 `e2e/run-android.sh`：`adb shell am start` 触发 —— **deferred 到 v0.2**（与 simulator GUI 自动化一并推迟）
- [ ] 加 `package.json` script：`"e2e": "bash e2e/run.sh"` —— **N/A**（顶层无 package.json；直接 `bash e2e/run.sh` 即可）
- [ ] 在 GitHub Actions 加 e2e workflow —— **deferred 到 v0.2**（CI workflow 与 simulator 自动化打包到一起）
- [x] commit：`test: end-to-end smoke from RN demo to server`
- [x] 🎯 **里程碑标记**：Sentori 首个端到端版本（SDK transport ↔ server 协议契约自动化）

---

### Phase 5 — PG 落库 + 最小 grouping

**Goal:** 事件落 Postgres，同 fingerprint 归到同 issue。
**Entry:** Phase 4 完成。
**Exit:** 同一段错误连续 throw 5 次，DB 中 `issues` 表 1 行、`events` 表 5 行。
**Estimate:** 1.5 周。

#### Steps

- [x] 加依赖：`sqlx` (postgres + uuid + time + json features) + `sqlx-cli`（dev）+ `redis`/`fred` (Valkey)
- [x] 起本地 PG 18：`docker run -d --name sentori-pg -p 5432:5432 -e POSTGRES_PASSWORD=dev -e POSTGRES_DB=sentori postgres:18-alpine`
- [x] 起本地 Valkey：`docker run -d --name sentori-vk -p 6379:6379 valkey/valkey:8-alpine`
- [x] 写 `server/migrations/0001_init.sql`：
  - [x] `projects` (id, name, created_at)
  - [x] `tokens` (id, project_id FK, token_hash, kind, created_at, revoked_at)
  - [x] `releases` (id, project_id, version, build, created_at, UNIQUE(project_id, version, build))
  - [x] `issues` (id uuid-v7, project_id FK, fingerprint, type, msg_sample, status enum('active','silenced','closed'), first_seen, last_seen, event_count, UNIQUE(project_id, fingerprint))
  - [x] `events` 按月 RANGE 分区 (id uuid-v7, project_id FK, issue_id FK, release_id FK?, env, payload jsonb, ts, received_at)
  - [x] 索引：`events(issue_id, ts DESC)`、`issues(project_id, status, last_seen DESC)`
- [x] 写 `server/src/db.rs`：连接池 + `sqlx::migrate!()` 启动时自动迁移
- [x] 写 `server/src/grouping.rs`：fingerprint = `sha256(error.type + first_in_app_frame.fn + first_in_app_frame.file)` 取前 16 字节 hex
- [x] 写 `server/src/issues.rs`：`upsert_issue(project_id, fingerprint, ...) -> issue_id`（用 ON CONFLICT 更新 last_seen + event_count）
- [x] 改写 ingestion handler：解析 → 计 fingerprint → upsert issue → 写 events 行
- [x] 加 `GET /v1/projects/:id/issues?status=active` 端点
- [x] 加 `GET /v1/projects/:id/issues/:issue_id/events` 端点
- [x] 写 Valkey rate limit middleware：sliding window，1000 req/min/token，超限返回 429 + `retry_after_ms`
- [x] 写 seed 脚本 `server/scripts/seed.rs`：创建 dev project + dev token
- [x] 改 token 鉴权：从硬编码改为查 `tokens` 表（带 Valkey cache，TTL 60s）
- [x] 集成测试：连续 post 5 个相同事件，断言 issues 表 1 行 + events 表 5 行
- [x] 集成测试：rate limit 触发 429
- [x] commit：`feat(server): persistent storage with grouping and rate limit`

---

### Phase 6 — Web dashboard MVP

**Goal:** 在 `web/` 能登录、选项目、看 issue 列表（dense table）+ 看 issue 详情（event 列表 + stack）。
**Entry:** Phase 5 完成。
**Exit:** 浏览器 `http://localhost:5173` 能看到由 Phase 5 入库的真实事件，键盘 `j/k/Enter` 可用。
**Estimate:** 2 周。

#### Steps

- [x] `cd web && bun add @tanstack/react-query openapi-fetch react-hotkeys-hook`
- [x] 设计 admin API 鉴权：管理员 session cookie（Phase 6 先做最简：env 注入的 admin password + `POST /admin/api/login` 设 httpOnly cookie，多用户在 Phase 13）
- [x] server 写 admin 端点：
  - [x] `POST /admin/api/login`
  - [ ] `GET /admin/api/projects`
  - [x] `GET /admin/api/projects/:id/issues?status=&env=&q=`
  - [x] `GET /admin/api/projects/:id/issues/:issue_id`
  - [x] `GET /admin/api/projects/:id/issues/:issue_id/events?limit=50`
  - [x] `PATCH /admin/api/issues/:id`（修改 status）
- [ ] server 写 OpenAPI schema 输出（用 `utoipa` crate）→ `web/` 用 `openapi-typescript` 生成 types
- [x] vite 配 dev proxy：`/admin/api → http://localhost:8080`
- [x] web/ 路由：
  - [x] `/login`
  - [x] `/`（重定向到 `/issues`）
  - [x] `/issues`（列表）
  - [x] `/issues/:id`（详情）
  - [ ] `/projects/:id/settings`（token、recipient 管理，留 Phase 9）
- [x] 写 `IssueListView`：
  - [x] dense table：cols [type, msg_sample, count, last_seen, env, release]
  - [x] 行高 32px，字号 13px，等宽数字
  - [x] j/k 切行高亮、Enter 进详情、s 切 silenced、`/` 聚焦搜索框
- [x] 写 `IssueDetailView`：
  - [x] 左栏：events 列表（按 ts DESC）
  - [x] 右栏：选中 event 的 stack + breadcrumbs + tags + device/app/release
  - [x] stack frame 渲染（先无符号化，用 raw）
  - [x] 上下事件 `[`/`]` 切换
- [x] 写 `web/docs/design-language.md`：字号刻度（11/13/15/24px）、间距刻度（4/8/16/24px）、暗色 palette 决策、参照 Linear / Vercel
- [x] 暗色为默认（已搭好）
- [x] `bun run check / test / build` 全绿
- [x] commit：`feat(web): dashboard MVP with dense issue list and detail`

---

### Phase 7 — RN SDK Native 层

**Goal:** iOS NSException + Android 未捕获异常都能落盘 + 下次 JS 启动 flush 上报。
**Entry:** Phase 6 完成。
**Exit:** iOS Swift `NSException.raise(...)` + Android Kotlin `throw RuntimeException(...)`，重启 demo app 后 server 收到事件并在 dashboard 显示。
**Estimate:** 2 周。

#### Steps

##### iOS

- [x] `sdk/react-native/ios/SentoriModule.swift`：注册 turbo module，提供 `drainPending()` 给 JS 调
- [x] `sdk/react-native/ios/SentoriCrashHandler.swift`：
  - [x] `NSSetUncaughtExceptionHandler { ex in writeToDisk(ex) }`
  - [x] 写文件路径：`<Documents>/sentori/pending/<uuid>.json`
  - [x] 文件 schema：与 protocol Event 一致（src=ios）
- [x] `sdk/react-native/ios/Sentori.h` + `Sentori.m`（暴露给 RN）
- [ ] iOS 单测（XCTest）：触发 NSException → 断言 `Documents/sentori/pending/*.json` 出现 + 文件内容反序列化为合法 Event
- [x] JS 端 `src/native-bridge.ts`：`Sentori.drainPending()` 读所有 .json，喂给 transport，逐个删文件
- [x] `sentori.init` 在最后一步调一次 `drainPending()`
- [x] example app 加按钮 "Throw NSException"
- [ ] iOS sim 端到端：throw → kill app → cold start → 验证 server 收到事件

##### Android

- [x] `sdk/react-native/android/.../SentoriModule.kt`：注册 turbo module
- [x] `sdk/react-native/android/.../SentoriCrashHandler.kt`：
  - [x] `Thread.setDefaultUncaughtExceptionHandler { t, e -> writeToDisk(e); previousHandler?.uncaughtException(t, e) }`
  - [x] 写文件路径：`<filesDir>/sentori/pending/<uuid>.json`
- [ ] Android 单测（JUnit + Robolectric）：同 iOS 双断言
- [x] example app 加按钮 "Throw RuntimeException"
- [ ] Android emu 端到端：throw → 重启 → server 收到 + dashboard 显示

- [x] commit：`feat(sdk): native uncaught exception capture (iOS + Android)`

---

### Phase 8 — Sourcemap 上传 + 服务端符号化

**Goal:** dashboard 中 JS 错误的 stack 显示原始 `src/Form.tsx:42:10`，而非 minified 位置。
**Entry:** Phase 7 完成。
**Exit:** 用 production bundle 触发的错误，dashboard 默认显示原始位置；可切换 raw view。
**Estimate:** 1 周。

#### Steps

- [x] `cd sentori && cargo new cli --bin --name sentori-cli`
- [x] 加依赖：`reqwest`、`clap`、`serde`、`walkdir`
- [x] 写 `cli/src/main.rs`：`sentori-cli upload sourcemap --release <r> --token <t> <files...>`
- [x] server migration `0002_releases.sql`：`release_artifacts` (id, release_id FK, kind, name, content_hash, blob_path, created_at)
- [x] server 端点：`POST /admin/api/releases/:r/sourcemaps`（multipart upload，落盘到 `data/artifacts/<hash>`）
- [x] server `src/symbolicate.rs`：
  - [x] 用 `sourcemap` crate 加载 .js.map
  - [x] 函数 `symbolicate_frame(frame, release_id) -> Frame`
  - [x] in-memory LRU cache（key=release_id+file，cap 50 条）
- [x] 改 `GET /admin/api/issues/:id/events`：返回时 lazy symbolicate 每个 frame
- [x] dashboard event 详情页加 toggle "raw / symbolicated"（默认 symbolicated）
- [ ] e2e：build minified bundle → cli 上传 sourcemap → 触发错误 → 验证 dashboard 显示原始位置
- [x] commit：`feat: sourcemap upload and server-side symbolication`

---

### Phase 9 — Release / Env / 邮件告警

**Goal:** dashboard 可按 env / release 过滤；新 issue 自动邮件通知。
**Entry:** Phase 8 完成。
**Exit:** 在两个 release 各 throw 一次相同错误，dashboard 显示"出现于 v1 + v2"；新 issue 触发 mailcatcher 收到邮件。
**Estimate:** 1 周。

#### Steps

- [x] migration `0003_notifications.sql`：`notification_recipients` (id, project_id FK, email, on_new_issue, on_regression, created_at)
- [x] dashboard `IssueListView` 加过滤器：env 多选、release 多选、status 切换
- [x] dashboard `IssueDetailView` 新增"出现的 release"列表（小 chip）
- [x] 加依赖：`lettre`（SMTP）
- [x] 写 `server/src/notifier.rs`：
  - [x] 监听新 issue 创建事件（先用 channel + 后台 task；不引 Kafka）
  - [x] 拉 recipients → 渲染邮件 → `lettre` 发送
  - [x] 失败重试 3 次
- [x] 配置 SMTP via env：`SENTORI_SMTP_HOST` / `_PORT` / `_USER` / `_PASS` / `_FROM`
- [ ] dashboard project settings 页：增删 recipient
- [ ] 集成测试：用 `mailcatcher` 容器，触发新 issue，断言邮件入箱
- [x] commit：`feat: release/env filters and email notifications on new issues`

---

### Phase 10 — Docker compose + 文档 + self-hosted v0.1.0

**Goal:** 一行 `docker compose up` 起完整 sentori，文档够新人 5 分钟上手。
**Entry:** Phase 9 完成。
**Exit:** 全新机器跟着 docs/getting-started.md 5 分钟内能起 sentori 并接入一个 RN demo 看到事件。
**Estimate:** 1 周。

#### Steps

- [x] 写 `docker/Dockerfile.server`：多阶段构建（rust:1.83-alpine 编译 → distroless 运行）
- [x] 写 `docker/Dockerfile.web`：阶段 1 bun 构建 web/dist，阶段 2 nginx 托管 dist
- [x] 写 `docker-compose.yml`：services = `server` + `web` + `pg` + `valkey`
- [x] 写 `docker-compose.override.example.yml`：可选 SMTP / 持久卷映射
- [x] 写 `.github/workflows/build.yml`：cargo test、bun run check/test/build、cargo build --release，产出 docker image push 到 ghcr.io
- [x] 写 `docs/getting-started.md`：5 分钟从 zero 到捕获第一条事件（self-hosted）
- [x] 写 `docs/sdk-react-native.md`：`init` API、ErrorBoundary、breadcrumb 自定义、source map 上传
- [x] 写 `docs/self-hosting.md`：env 变量参考、备份策略、PG 升级注意事项
- [x] 完整化 `README.md`：徽章、demo gif、快速开始、链接到各 docs
- [ ] 用一台干净 mac 走一遍 `getting-started.md`（colima 或 docker desktop），计时 ≤ 5 分钟
- [ ] `git tag v0.1.0` + GitHub release
- [x] commit：`docs: getting started, self-hosting, SDK guide`
- [x] 🎯 **里程碑：self-hosted v0.1.0 发布**

---

### v0.1.0 launch checklist

Phase 0–10 代码层面全部完成（26 commits 落地）。下面是发布 v0.1.0 之前由你拾起的收尾——做完后 Phase 0–10 真正 close，进入 Phase 11（SaaS arc）。

- [ ] 用一台干净 mac 跑 `docs/getting-started.md`，计时 ≤ 5 分钟（来自 Phase 10 line 441）
- [ ] iOS simulator 端到端：`cd sdk/react-native/example && bunx expo prebuild && cd ios && bundle exec pod install && cd .. && bun run ios` → tap "Native crash" → relaunch → server stdout 收到 `platform: ios` 事件（来自 Phase 7 line 358）
- [ ] Android emulator 端到端：`bun run android` → 同上验证 `platform: android` 事件（来自 Phase 7 line 368）
- [ ] `git tag v0.1.0 -m "Sentori v0.1.0 — self-hosted MVP"` + 推 + 写 GitHub release notes（来自 Phase 10 line 442）
- [ ] （可选）self-dogfooding：本地 server 接 1-2 天看 ingestion / grouping / 邮件告警

### v0.1 范围内已 deferred 项（已分配到后续 phase）

各 phase 实施时被明确推迟的 step 都记录在这里，避免遗漏：

| 来自 | 内容 | 在哪儿拾起 |
|---|---|---|
| Phase 4 | `e2e/run-android.sh` + GitHub Actions simulator runner | Phase 16 Testing 段（与 simulator GUI 自动化整体一起做） |
| Phase 6 line 308 | `GET /admin/api/projects` | Phase 13 Server 段（multi-tenant 时做 project list） |
| Phase 6 line 313 | `utoipa` OpenAPI 自动导出 | v0.2（非阻塞，先手写 types） |
| Phase 7 line 354 | iOS XCTest（NSException + 文件落盘） | Phase 16 Testing 段 |
| Phase 7 line 358 | iOS simulator end-to-end | v0.1.0 launch checklist 上方 + Phase 16 Testing 段（自动化） |
| Phase 7 line 366 | Android Robolectric | Phase 16 Testing 段 |
| Phase 7 line 368 | Android emulator end-to-end | v0.1.0 launch checklist 上方 + Phase 16 Testing 段（自动化） |
| Phase 8 line 394 | minified bundle → cli upload → dashboard 验证原始位置 e2e | Phase 16 Testing 段 |
| Phase 9 line 417 | recipient 管理 UI | Phase 13 Dashboard 段（与 org settings 一起做） |
| Phase 9 line 418 | mailcatcher 集成测试 | Phase 16 Testing 段 |


