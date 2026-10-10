import { fileMessages } from '../design/file-messages.ts';
import { pricingMessages } from "./pricing-messages.ts";
export const siteMessages = {
  ...pricingMessages,
  ...fileMessages,
  "nav.selfHosted": [
    "实例管理",
    "インスタンス管理",
    "Instance"
  ],
  "site.menu": [
    "菜单",
    "メニュー",
    "Menu"
  ],
  "site.navigation": [
    "官网导航",
    "サイトナビゲーション",
    "Site navigation"
  ],
  "nav.inbox": [
    "收件箱",
    "受信トレイ",
    "Inbox"
  ],
  "nav.tasks": [
    "任务与修复",
    "タスクと修正",
    "Tasks & fixes"
  ],
  "nav.status": [
    "可用性监控",
    "稼働監視",
    "Availability"
  ],
  "nav.integrations": [
    "通知与集成",
    "通知と連携",
    "Notifications & integrations"
  ],
  "auth.accessHint": [
    "使用受邀邮箱或组织配置的登录方式访问团队",
    "招待されたメールアドレスか、組織で設定した方法でチームにログインします",
    "Use your invited email or an organization-enabled sign-in method to access your team"
  ],
  "scene.example": [
    "产品界面 · 示例数据",
    "製品画面 · デモデータ",
    "Product view · demo data"
  ],
  "scene.impact.title": [
    "先判断影响，再决定优先级",
    "影響を把握して優先順位を決める",
    "Impact before priority"
  ],
  "scene.impact.alt": [
    "问题队列截图，展示事件次数、受影响用户与版本",
    "イベント件数・影響ユーザー・リリースを示す問題一覧",
    "Issue queue with event counts, affected users and release context"
  ],
  "scene.evidence.title": [
    "从事件定位到源码",
    "イベントからソースへ",
    "From event to source"
  ],
  "scene.evidence.alt": [
    "真实源码面板截图，事件、源码与回放关联到同一问题",
    "同じ問題のイベント・ソース・リプレイを結ぶソース画面",
    "Source panel linking an event, code and replay to the same issue"
  ],
  "scene.team.title": [
    "把处理过程留在问题里",
    "対応の経緯を問題に残す",
    "Keep the work with the issue"
  ],
  "scene.team.alt": [
    "问题活动记录截图，展示团队备注与处理状态",
    "チームのメモと対応状況を示す問題のアクティビティ画面",
    "Issue activity with team notes and status history"
  ],
  "scene.team.detail": [
    "备注、负责人和修复版本",
    "メモ・担当者・修正リリース",
    "Notes, owner and fix release"
  ],
  "scene.event": [
    "事件",
    "イベント",
    "Event"
  ],
  "scene.source": [
    "源码",
    "ソース",
    "Source"
  ],
  "scene.replay": [
    "回放",
    "リプレイ",
    "Replay"
  ],
  "auth.orEmail": [
    "或使用邮箱登录",
    "またはメールでログイン",
    "Or sign in with email"
  ],
  "auth.sso": [
    "企业单点登录",
    "企業 SSO",
    "Enterprise single sign-on"
  ],
  "auth.ssoHint": [
    "使用组织身份提供方 · SAML 2.0",
    "組織の IdP を使用 · SAML 2.0",
    "Use your organization’s identity provider · SAML 2.0"
  ],
  "auth.organizationDomain": [
    "组织域名",
    "組織のドメイン",
    "Organization domain"
  ],
  "auth.domainHint": [
    "输入组织配置 SSO 时使用的域名",
    "SSO に設定した組織のドメインを入力",
    "Enter the domain configured for your organization’s SSO"
  ],
  "auth.domainInvalid": [
    "请输入有效域名，例如 company.com",
    "有効なドメインを入力してください（例: company.com）",
    "Enter a valid domain, such as company.com"
  ],
  "auth.providerTitle": [
    "通过 {provider} 登录",
    "{provider} でログイン",
    "Sign in with {provider}"
  ],
  "auth.providerBody": [
    "正式环境会跳转至 {provider} 完成身份验证",
    "本番環境では {provider} へ移動して認証します",
    "In production, you will be redirected to {provider} to authenticate"
  ],
  "auth.demoContinue": [
    "继续演示登录",
    "デモログインを続ける",
    "Continue demo sign-in"
  ],
  "auth.providerFailure": [
    "本次登录未完成，可以重试或选择其他方式",
    "ログインが完了しませんでした。再試行するか別の方法を選んでください",
    "Sign-in did not complete. Retry or choose another method."
  ],
  "auth.ssoNotConfigured": [
    "该组织尚未配置 SAML 登录，请联系管理员或使用其他登录方式",
    "この組織は SAML を未設定です。管理者に連絡するか別の方法でログインしてください",
    "SAML is not configured for this organization. Contact an administrator or use another sign-in method."
  ],
  "auth.ssoContinue": [
    "查找组织 SSO",
    "組織の SSO を検索",
    "Find organization SSO"
  ],
  "auth.ssoOrganization": [
    "组织 · {domain}",
    "組織 · {domain}",
    "Organization · {domain}"
  ],
  "auth.loginMethods": [
    "选择登录方式",
    "ログイン方法を選択",
    "Choose a sign-in method"
  ],
  "site.productEyebrow": [
    "为调查而生的工作台",
    "調査のためのワークベンチ",
    "A workbench for investigation"
  ],
  "site.evidence": [
    "把线索放在一起",
    "手がかりを一か所に",
    "Bring the evidence together"
  ],
  "site.evidenceHint": [
    "从一条错误出发，跟随事件、源码和版本找到下一步",
    "一つのエラーから、イベント・ソース・リリースをたどって次の一手へ",
    "Start with an error. Follow the event, source and release to your next move."
  ],
  "site.own": [
    "你的应用，你的调查空间",
    "あなたのアプリ、あなたの調査空間",
    "Your app. Your investigation space."
  ],
  "site.faq": [
    "开始之前，你可能想知道",
    "始める前に知っておきたいこと",
    "A few things before you start"
  ],
  "site.faqHint": [
    "费用、平台差异与部署方式，我们把边界说清楚",
    "費用、プラットフォームの違い、運用方法を明確に",
    "Clear answers on costs, platform differences and deployment."
  ],
  "site.faqLink": [
    "阅读部署指南",
    "デプロイガイドを読む",
    "Read the deployment guide"
  ],
  "site.authEyebrow": [
    "继续你的调查",
    "調査を再開",
    "Back to your investigation"
  ],
  "site.authFootnote": [
    "从一个问题，到完整的来龙去脉",
    "一つの問題から、その背景まで",
    "From a single issue to the full story"
  ],
  "site.docsSearch": [
    "搜索文档、SDK 和指南",
    "ドキュメント・SDK・ガイドを検索",
    "Search documentation, SDKs and guides"
  ],
  "site.docsHome": [
    "文档首页",
    "ドキュメントホーム",
    "Documentation home"
  ],
  "site.docsStart": [
    "选择你的起点",
    "ここから始めよう",
    "Choose your starting point"
  ],
  "site.docsQuick": [
    "快速接入",
    "クイックスタート",
    "Quick start"
  ],
  "site.docsQuickHint": [
    "安装 SDK，发送你的第一条事件",
    "SDK を導入して最初のイベントを送信",
    "Install an SDK and send your first event"
  ],
  "site.docsInvestigate": [
    "调查一个问题",
    "問題を調査",
    "Investigate an issue"
  ],
  "site.docsInvestigateHint": [
    "阅读事件、源码与回放证据",
    "イベント・ソース・リプレイを確認",
    "Read event, source and replay evidence"
  ],
  "site.docsDeploy": [
    "自行部署",
    "セルフホスト",
    "Deploy on your infrastructure"
  ],
  "site.docsDeployHint": [
    "掌握数据、权限和运维方式",
    "データ・権限・運用を自分で管理",
    "Control your data, access and operations"
  ],
  "site.docsReference": [
    "SDK 与协议",
    "SDK とプロトコル",
    "SDKs and protocol"
  ],
  "site.docsReferenceHint": [
    "查阅接口、事件格式与错误码",
    "API・イベント形式・エラーコードを参照",
    "Look up APIs, event formats and error codes"
  ],
  "site.docsPrevious": [
    "上一篇",
    "前のページ",
    "Previous"
  ],
  "site.docsNext": [
    "下一篇",
    "次のページ",
    "Next"
  ],
  "site.docsOverview": [
    "产品与协议概览",
    "製品とプロトコルの概要",
    "Product and protocol overview"
  ],
  "nav.states": [
    "状态与落地页",
    "状態と案内ページ",
    "States and landing pages"
  ],
  "source.site.control.failure": [
    "事件 API 隔离上报失败，避免影响宿主应用",
    "イベント API は送信処理の失敗をアプリから分離",
    "The event APIs isolate reporting failures from your application."
  ],
  "source.site.control.privacy": [
    "开启回放前，检查适合应用的过滤与遮罩设置",
    "リプレイを有効にする前に、フィルタリングとマスキングを確認",
    "Review filtering and masking before enabling replay on your app."
  ],
  "source.site.control.source": [
    "可以审阅源码、运行自己的实例并设置数据保留策略",
    "ソースを確認し、自分の環境で運用して保存期間を設定",
    "Inspect the source, run your own instance and choose your retention policy."
  ],
  "source.site.controlTitle": [
    "知道采集了什么，也能控制它",
    "収集する情報を選ぶ",
    "Control what you collect"
  ],
  "source.site.deployment": [
    "了解自托管方式",
    "セルフホストの仕組みを見る",
    "See how self-hosting works"
  ],
  "source.site.trustDescription": [
    "监测应该帮助被监测的应用；Sentori 的事件上报调用保持同步，缓冲区有明确边界，可选采集能力由你决定是否开启",
    "監視はアプリを支えるもの。イベント記録の呼び出しは同期式で、バッファーには上限があります。任意の収集機能はチームが選択できます",
    "Monitoring should help the app it observes. Sentori keeps its reporting calls synchronous, bounds its buffers and gives you control over optional capture."
  ],
  "source.site.trustTitle": [
    "始终把你的应用放在第一位",
    "主役は、あなたのアプリ",
    "Your app comes first."
  ],
  "source.story.coverage": [
    "暂未复发是观测结果，不代表已经覆盖所有用户",
    "再発なしは観測結果であり、全ユーザーを確認した証明ではありません",
    "No recurrence is an observation, not proof that every user is covered."
  ],
  "source.story.description.0": [
    "先看与问题直接相关的源码位置",
    "まず、問題に関わるソース位置を確認",
    "The first useful source frame is right here."
  ],
  "source.story.description.1": [
    "把用户操作与网络请求放在同一条时间线上",
    "操作とネットワークリクエストを同じ時間軸で確認",
    "The user’s actions and the request tell the same story."
  ],
  "source.story.description.2": [
    "版本 1.4.3 已加入客户会话校验",
    "リリース 1.4.3 に顧客セッションの検証を追加",
    "Release 1.4.3 includes the customer-session check."
  ],
  "source.story.event.cart": [
    "进入支付页面",
    "決済画面を開く",
    "Checkout opened"
  ],
  "source.story.event.error": [
    "从空会话中读取客户 ID",
    "空のセッションから顧客 ID を取得",
    "Customer ID read from an empty session"
  ],
  "source.story.event.network": [
    "会话请求返回 401",
    "セッション取得が 401 を返す",
    "Session request returned 401"
  ],
  "source.story.event.tap": [
    "点击支付按钮",
    "支払いボタンをタップ",
    "Pay button tapped"
  ],
  "source.story.eventHint.cart": [
    "购物车 → 支付",
    "カート → 決済",
    "Cart → Checkout"
  ],
  "source.story.eventHint.error": [
    "submit.ts:48 · TypeError",
    "submit.ts:48 · TypeError",
    "submit.ts:48 · TypeError"
  ],
  "source.story.eventHint.network": [
    "GET /api/session · 184 ms",
    "GET /api/session · 184 ms",
    "GET /api/session · 184 ms"
  ],
  "source.story.eventHint.tap": [
    "支付页面 · PayButton",
    "決済画面 · PayButton",
    "Checkout · PayButton"
  ],
  "source.story.example": [
    "交互示例 · 演示数据",
    "インタラクティブデモ · サンプルデータ",
    "Interactive example · sample data"
  ],
  "source.story.impact": [
    "影响 12 位用户",
    "影響を受けたユーザー 12 人",
    "12 affected users"
  ],
  "source.story.release": [
    "正在观察版本 1.4.3",
    "リリース 1.4.3 を観測中",
    "Observing release 1.4.3"
  ],
  "source.story.result": [
    "在这个示例的已观测会话中，问题暂未再次出现；下一步继续关注受影响的代码路径",
    "この例の観測済みセッションでは再発していません。引き続き、影響を受けたコードパスを監視します",
    "This example shows no recurrence in the observed sessions. The next step is to keep monitoring the affected path."
  ],
  "source.story.sessions": [
    "个已观测会话",
    "観測済みセッション",
    "observed sessions"
  ],
  "source.story.step.0": [
    "定位错误",
    "エラーを確認",
    "Find the error"
  ],
  "source.story.step.1": [
    "追溯过程",
    "前後の流れ",
    "Read the context"
  ],
  "source.story.step.2": [
    "观察修复",
    "修正後を確認",
    "Follow the fix"
  ],
  "source.story.steps": [
    "排查步骤",
    "調査のステップ",
    "Investigation steps"
  ],
  "source.story.title.0": [
    "客户信息缺失，中断了支付",
    "顧客情報がなく、決済が停止",
    "A missing customer stopped checkout"
  ],
  "source.story.title.1": [
    "错误发生之前，发生了什么",
    "エラーの直前に何が起きたか",
    "What happened before the error?"
  ],
  "source.story.title.2": [
    "修复已经发布，观察仍在继续",
    "修正をリリース。引き続き確認",
    "The fix shipped. Keep watching."
  ],
  "nav.prototypes": [
    "完整原型",
    "プロトタイプ",
    "Prototypes"
  ],
  "nav.demo": [
    "产品演示",
    "製品デモ",
    "Product demo"
  ],
  "nav.notFound": [
    "页面不存在",
    "ページが見つかりません",
    "Page not found"
  ],
  "site.available": [
    "现在可用",
    "提供中",
    "Available now"
  ],
  "site.license": [
    "许可费用",
    "ライセンス料金",
    "license cost"
  ],
  "site.selfDescription": [
    "运行在自己的基础设施上，掌控应用数据与访问权限；服务器、存储和运维由你负责",
    "自分のインフラで稼働し、データとアクセスを管理できます。サーバー、ストレージ、運用はご自身で用意します",
    "Run on your infrastructure and control your application data and access. You provide the servers, storage and operations."
  ],
  "site.planned": [
    "规划中",
    "計画中",
    "Planned"
  ],
  "site.cloudDescription": [
    "托管服务尚未开放，当前没有在线购买或注册入口；可以先体验演示，或部署自托管版本",
    "ホスティングサービスは未提供です。現在、購入や登録はできません。デモを試すか、セルフホスト版をご利用ください",
    "The hosted service is not available yet. Online purchase and signup are not open. Explore the demo or deploy the self-hosted edition."
  ],
  "site.deploy": [
    "部署自托管版",
    "セルフホストを始める",
    "Deploy self-hosted"
  ],
  "site.workflow": [
    "让每一条线索，都有清楚的下一步",
    "すべての手がかりに、次の一歩を",
    "A clear next step for every clue"
  ],
  "site.workflowHint": [
    "接入应用、定位问题、验证修复；让工具承担复杂，让团队专注判断",
    "アプリを接続し、原因を調べ、修正を確認。複雑な作業はツールに任せ、チームは判断に集中できます",
    "Connect an app, investigate a problem and verify the fix. Let the tools handle the detail so your team can focus on decisions."
  ],
  "site.closing": [
    "工作告一段落，安心回到生活",
    "ひと区切りつけて、日常へ",
    "Close the loop. Get back to your day."
  ],
  "site.closingHint": [
    "从第一条事件开始，把分散的线索整理成可以行动的答案",
    "最初のイベントから、散らばった手がかりを次の行動へつなげましょう",
    "Start with your first event and turn scattered clues into something you can act on."
  ],
  "site.demoHint": [
    "这是可以实际操作的本地演示；选择一个流程开始，也可以自由探索产品",
    "操作できるローカルデモです。フローを選ぶか、自由に製品をご覧ください",
    "This is a working local demo. Start with a guided journey or explore the product freely."
  ],
  "site.noSignup": [
    "账号由部署管理员创建；忘记密码时，可以请求重置邮件",
    "アカウントは管理者が作成します。パスワードを忘れた場合は、再設定メールを申請できます",
    "Your deployment administrator creates accounts. If you forget your password, request a reset email."
  ],
  "site.docsLanguage": [
    "界面支持三种语言；以下技术文档引用仓库中的英文原文",
    "UIは3言語対応です。以下の技術文書はリポジトリの英語原文です",
    "The interface supports three languages. Technical articles below use the repository’s original English documentation."
  ],
  "site.notFoundHint": [
    "这个地址没有对应的内容；返回首页，或在文档中继续查找",
    "このアドレスに対応する内容がありません。ホームに戻るか、ドキュメントで検索してください",
    "There is no content at this address. Return home or continue searching the documentation."
  ],
  "site.authFailure": [
    "邮箱或密码不正确，请检查后重试",
    "メールアドレスまたはパスワードが正しくありません。確認して再試行してください",
    "The email or password is incorrect. Check them and try again."
  ],
  "site.expired": [
    "重置链接已失效，请重新请求邮件",
    "再設定リンクの有効期限が切れています。新しいメールを申請してください",
    "This reset link has expired. Request a new email."
  ],
  "site.mailUnavailable": [
    "邮件服务暂不可用，请稍后重试或联系部署管理员",
    "メールサービスを利用できません。後で再試行するか、管理者に連絡してください",
    "Email is temporarily unavailable. Try again later or contact your deployment administrator."
  ],
  "nav.brand-system": [
    "品牌设定",
    "ブランド設定集",
    "Brand world"
  ],
  "nav.selections": [
    "选择控件",
    "選択コントロール",
    "Selection controls"
  ],
  "nav.spacing": [
    "间距与布局",
    "余白と配置",
    "Spacing & rhythm"
  ],
  "nav.overview": [
    "总览",
    "概要",
    "Overview"
  ],
  "nav.brand": [
    "品牌识别",
    "ブランド",
    "Identity"
  ],
  "nav.foundations": [
    "基础规范",
    "基本ルール",
    "Foundations"
  ],
  "nav.components": [
    "控件与交互",
    "コンポーネント",
    "Components"
  ],
  "nav.layouts": [
    "布局模式",
    "レイアウト",
    "Layouts"
  ],
  "nav.charts": [
    "图表与数据",
    "チャートとデータ",
    "Charts"
  ],
  "nav.coverage": [
    "功能覆盖与接入",
    "機能対応と連携",
    "Coverage & integration"
  ],
  "nav.projects": [
    "项目",
    "プロジェクト",
    "Projects"
  ],
  "nav.issues": [
    "问题",
    "問題",
    "Issues"
  ],
  "nav.issue": [
    "问题详情",
    "問題の詳細",
    "Issue detail"
  ],
  "nav.releases": [
    "版本与调试文件",
    "リリースとデバッグファイル",
    "Releases & debug files"
  ],
  "nav.instruments": [
    "埋点与性能",
    "計測とパフォーマンス",
    "Instrumentation"
  ],
  "nav.setup": [
    "接入引导",
    "セットアップ",
    "Setup"
  ],
  "nav.push": [
    "推送",
    "プッシュ通知",
    "Push"
  ],
  "nav.settings": [
    "设置",
    "設定",
    "Settings"
  ],
  "nav.docs": [
    "文档",
    "ドキュメント",
    "Documentation"
  ],
  "nav.welcome": [
    "产品首页",
    "製品トップ",
    "Product home"
  ],
  "nav.pricing": [
    "方案",
    "プラン",
    "Plans"
  ],
  "nav.login": [
    "登录",
    "ログイン",
    "Sign in"
  ],
  "nav.forgot": [
    "找回密码",
    "パスワード再発行",
    "Forgot password"
  ],
  "nav.resetPassword": [
    "重设密码",
    "パスワード再設定",
    "Reset password"
  ],
  "common.next": [
    "下一步",
    "次へ",
    "Next"
  ],
  "common.details": [
    "查看详情",
    "詳細を見る",
    "View details"
  ],
  "issue.events": [
    "事件次数",
    "イベント数",
    "Events"
  ],
  "issue.users": [
    "用户数",
    "ユーザー数",
    "Users"
  ],
  "auth.title": [
    "继续一起找线索",
    "手がかりを一緒に探そう",
    "Pick up the next clue"
  ],
  "auth.subtitle": [
    "登录后查看项目、问题和调查证据",
    "ログインしてプロジェクト、問題、調査データを確認します",
    "Sign in to review projects, issues, and evidence."
  ],
  "auth.demo": [
    "使用示例账号进入",
    "デモアカウントで開く",
    "Continue with demo account"
  ],
  "auth.forgot": [
    "忘记密码？",
    "パスワードをお忘れですか？",
    "Forgot your password?"
  ],
  "auth.sendReset": [
    "发送重设链接",
    "再設定リンクを送信",
    "Send reset link"
  ],
  "auth.resetSent": [
    "示例已记录请求；真实环境会以相同提示保护账号信息",
    "デモにリクエストを記録しました。本番ではアカウントの存在を明かさず同じ案内を表示します",
    "Request recorded in the demo. In production, the same response protects account existence."
  ],
  "auth.resetTitle": [
    "设置新密码",
    "新しいパスワードを設定",
    "Set a new password"
  ],
  "auth.token": [
    "重设令牌",
    "再設定トークン",
    "Reset token"
  ],
  "auth.oauth": [
    "通过 {provider} 继续",
    "{provider} で続ける",
    "Continue with {provider}"
  ],
  "auth.oauthDemo": [
    "这是登录跳转的本地交互示例",
    "これはログイン遷移のローカルデモです",
    "This is a local example of the sign-in redirect flow."
  ],
  "auth.agreement": [
    "进入后即使用本地示例数据",
    "ローカルのデモデータで開きます",
    "You will enter a local sample workspace."
  ],
  "public.eyebrow": [
    "SENTORI / 千鸟",
    "SENTORI / 千鳥",
    "SENTORI"
  ],
  "public.title": [
    "每一条线索，\n都带你靠近答案",
    "一つひとつの手がかりが、\n答えへとつながる",
    "Every clue brings\nyou closer."
  ],
  "public.subtitle": [
    "把错误、版本、源码与事件前后的过程放在一起，让团队从同一份证据开始调查",
    "エラー、リリース、ソース、イベント前後の経緯をまとめ、チームが同じ証拠から調査を始められます",
    "Bring errors, releases, source, and the moments around an event together so the team starts with shared evidence."
  ],
  "public.start": [
    "开始接入",
    "セットアップを始める",
    "Start setup"
  ],
  "public.demo": [
    "探索示例项目",
    "デモプロジェクトを見る",
    "Explore the demo"
  ],
  "public.product": [
    "产品",
    "製品",
    "Product"
  ],
  "public.features": [
    "从影响到原因，有序调查",
    "影響から原因へ、順序立てて調査",
    "Investigate from impact to cause"
  ],
  "public.feature1": [
    "看清影响范围",
    "影響範囲を把握",
    "Understand impact"
  ],
  "public.feature1Text": [
    "分别查看事件、用户和版本，保留未知与缺失的含义",
    "イベント、ユーザー、リリースを分けて確認し、不明と欠損を明示します",
    "Inspect events, users, and releases separately, keeping unknown and missing explicit."
  ],
  "public.feature2": [
    "读懂发生过程",
    "発生までの経緯を読む",
    "Read the sequence"
  ],
  "public.feature2Text": [
    "源码、上下文与回放共用一个事件锚点",
    "ソース、コンテキスト、リプレイを一つのイベントに結び付けます",
    "Source, context, and replay share the same event anchor."
  ],
  "public.feature3": [
    "共同推进处理",
    "対応を一緒に進める",
    "Move the work forward"
  ],
  "public.feature3Text": [
    "负责人、备注、版本和处理记录在同一处",
    "担当者、メモ、リリース、対応履歴を一か所にまとめます",
    "Keep assignees, notes, releases, and decisions together."
  ],
  "site.skip": [
    "跳到内容",
    "コンテンツへ移動",
    "Skip to content"
  ],
  "site.language": [
    "语言",
    "言語",
    "Language"
  ],
  "inspect.openSource": [
    "打开发布版本源码",
    "リリースのソースを開く",
    "Open release source"
  ],
  "inspect.altHint": [
    "Alt / Option + 点击跳转定义",
    "Alt / Option + クリックで定義へ",
    "Alt / Option + click to go to definition"
  ],
  "code.collapseAll": [
    "全部折叠",
    "すべて折りたたむ",
    "Collapse all"
  ],
  "code.expandAll": [
    "全部展开",
    "すべて展開",
    "Expand all"
  ],
  "code.collapseLine": [
    "折叠第 {line} 行",
    "{line} 行目を折りたたむ",
    "Collapse line {line}"
  ],
  "code.expandLine": [
    "展开第 {line} 行",
    "{line} 行目を展開",
    "Expand line {line}"
  ],
  "code.properties": [
    "个字段",
    "フィールド",
    "properties"
  ],
  "code.items": [
    "项",
    "項目",
    "items"
  ],
  "code.line.error": [
    "报错行",
    "エラー行",
    "Error line"
  ],
  "code.line.warning": [
    "警告行",
    "警告行",
    "Warning line"
  ],
  "code.line.success": [
    "已确认行",
    "確認済み行",
    "Verified line"
  ],
  "code.errorAt": [
    "报错位置 · 第 {line} 行",
    "エラー箇所 · {line} 行目",
    "Error location · line {line}"
  ],
  "docs.sectionLink": [
    "链接到此章节",
    "この節へのリンク",
    "Link to section"
  ],
  "docs.browse": [
    "浏览文档",
    "ドキュメント一覧",
    "Browse documentation"
  ],
  "docs.breadcrumb": [
    "文档路径",
    "ドキュメントのパス",
    "Documentation breadcrumb"
  ],
  "docs.searchResults": [
    "文档搜索结果",
    "検索結果",
    "Documentation search results"
  ],
  "docs.resultCount": [
    "找到 {count} 篇文档",
    "{count} 件のドキュメント",
    "{count} documents found"
  ],
  "docs.copyLink": [
    "复制链接",
    "リンクをコピー",
    "Copy link"
  ],
  "docs.viewSource": [
    "查看源文件",
    "ソースを表示",
    "View source"
  ],
  "docs.downloadMarkdown": [
    "下载 Markdown",
    "Markdown を保存",
    "Download Markdown"
  ],
  "docs.version": [
    "文档版本",
    "ドキュメントのバージョン",
    "Documentation version"
  ],
  "docs.latest": [
    "最新发布",
    "最新リリース",
    "Latest release"
  ],
  "docs.archive": [
    "历史版本",
    "過去のリリース",
    "Archived release"
  ],
  "docs.development": [
    "开发文档",
    "開発版",
    "Development"
  ],
  "docs.versions": [
    "版本与支持周期",
    "バージョンとサポート期間",
    "Versions and support lifecycle"
  ],
  "docs.changes": [
    "版本变更",
    "変更履歴",
    "Changes"
  ],
  "docs.releaseDate": [
    "发布日期",
    "リリース日",
    "Released"
  ],
  "docs.maintenance": [
    "维护状态",
    "メンテナンス状況",
    "Maintenance"
  ],
  "docs.support.lts": [
    "LTS · 长期支持",
    "LTS · 長期サポート",
    "LTS · long-term support"
  ],
  "docs.support.active": [
    "维护中",
    "メンテナンス中",
    "Maintained"
  ],
  "docs.support.eol": [
    "已停止维护",
    "サポート終了",
    "End of support"
  ],
  "docs.support.unconfirmed": [
    "支持政策待确认",
    "サポート方針は未確定",
    "Support policy unconfirmed"
  ],
  "docs.ltsUnassigned": [
    "尚未指定 LTS 版本",
    "LTS バージョンは未指定",
    "No LTS release designated"
  ],
  "docs.endOfSupport": [
    "维护截止",
    "サポート終了日",
    "Support until"
  ],
  "docs.notAnnounced": [
    "尚未公布",
    "未公開",
    "Not announced"
  ],
  "docs.supportNote": [
    "发布记录已核对；LTS 与停止维护日期尚未公布，历史版本不等于已停止维护",
    "リリース履歴は確認済みです。LTS とサポート終了日は未公開で、過去の版がサポート終了とは限りません",
    "Release history is verified. LTS and end-of-support dates are not announced; an archived release is not necessarily unsupported."
  ],
  "docs.archiveNote": [
    "正在阅读 {version} 的文档快照",
    "{version} のドキュメントを表示中",
    "You are reading the {version} documentation snapshot"
  ],
  "docs.switchLatest": [
    "切换到最新版本",
    "最新版に切り替える",
    "Switch to latest"
  ],
  "docs.versionMissing": [
    "此页面不在 {version} 中",
    "このページは {version} にありません",
    "This page is not in {version}"
  ],
  "docs.versionMissingHint": [
    "保留当前版本浏览目录，或切换到最新版本查看此页",
    "現在の版の目次を確認するか、最新版でこのページを開いてください",
    "Browse this version’s contents or open this page in the latest release"
  ],
  "docs.versionUnknown": [
    "找不到请求的文档版本",
    "指定したバージョンが見つかりません",
    "Requested documentation version not found"
  ],
  "docs.sourceTag": [
    "对应发布标签",
    "対応するリリースタグ",
    "Release tag"
  ],
  "docs.policy.active": [
    "接收常规修复与更新",
    "通常の修正と更新を提供",
    "Receives regular fixes and updates"
  ],
  "docs.policy.lts": [
    "在承诺周期内提供长期维护",
    "公表した期間にわたり長期サポートを提供",
    "Long-term maintenance for a declared support period"
  ],
  "docs.policy.eol": [
    "不再提供维护更新，建议迁移",
    "更新の提供を終了。移行を推奨",
    "No further maintenance updates; migration recommended"
  ],
  "docs.section.start": [
    "开始使用",
    "はじめに",
    "Get started"
  ],
  "docs.section.sdk": [
    "SDK 参考",
    "SDK リファレンス",
    "SDK reference"
  ],
  "docs.section.wire": [
    "协议与数据",
    "プロトコルとデータ",
    "Protocol and data"
  ],
  "docs.section.run": [
    "部署与管理",
    "デプロイと管理",
    "Deploy and manage"
  ],
  "docs.section.recipes": [
    "实践指南",
    "実践ガイド",
    "Recipes"
  ],
  "docs.section.help": [
    "帮助与排查",
    "ヘルプと診断",
    "Help and troubleshooting"
  ],
  "docs.section.more": [
    "更多参考",
    "その他の資料",
    "More reference"
  ],
  "code.themeNote": [
    "代码区域随外观切换浅色 / 深色；保留 Menlo 字体与语法结构；所有语法颜色均检查 4.5:1 对比度",
    "コードは外観に合わせてライト・ダークを切り替え。Menlo と構文を維持し、すべての構文色を 4.5:1 で確認",
    "Code surfaces follow light/dark appearance. Menlo typography and grammar are retained; every syntax color is checked at 4.5:1 contrast."
  ],
  "code.copyFailed": [
    "复制失败",
    "コピー失敗",
    "Copy failed"
  ],
  "code.language": [
    "示例语言",
    "サンプル言語",
    "Sample language"
  ],
  "code.legend": [
    "语法颜色",
    "構文の色",
    "Syntax colors"
  ],
  "code.token.keyword": [
    "关键字",
    "キーワード",
    "Keywords"
  ],
  "code.token.string": [
    "字符串",
    "文字列",
    "Strings"
  ],
  "code.token.number": [
    "数字",
    "数値",
    "Numbers"
  ],
  "code.token.literal": [
    "布尔值 / 空值",
    "真偽値 / null",
    "Booleans / null"
  ],
  "code.token.comment": [
    "注释",
    "コメント",
    "Comments"
  ],
  "code.token.function": [
    "函数",
    "関数",
    "Functions"
  ],
  "code.token.type": [
    "类型",
    "型",
    "Types"
  ],
  "code.token.attribute": [
    "属性",
    "属性",
    "Attributes"
  ],
  "code.comment.summary": [
    "事件计数的最小示例",
    "イベント数の最小サンプル",
    "A minimal event-count example"
  ],
  "code.comment.detail": [
    "先读取原始值，再按显示规则格式化",
    "元の値を読み取り、表示ルールに従って整形します",
    "Read the raw value, then format it for display."
  ],
  "code.comment.missing": [
    "缺失值显示 —；数值 0 仍显示 0",
    "欠損値は —、数値 0 は 0 と表示します",
    "Missing values show —; a numeric zero stays zero."
  ],
  "code.read": [
    "代码：{name}",
    "コード：{name}",
    "Code: {name}"
  ],
  "code.sampleNote": [
    "24 种语言与格式示例；注释、字符串、关键字和类型按语法着色；复制保留原始文本，不包含行号",
    "24 種の言語・形式を用意しています。コメント・文字列・キーワード・型を構文に沿って色分けし、コピーには行番号を含めません",
    "24 language and format examples. Comments, strings, keywords and types follow the grammar. Copy preserves the source text without line numbers."
  ],
  "code.jsonNote": [
    "标准 JSON 不支持注释；这个示例可直接解析；需要注释时请选择 JSONC",
    "標準 JSON はコメントをサポートしません。この例はそのまま解析できます。コメント付きの例は JSONC を選んでください",
    "Standard JSON has no comments. This sample is valid JSON; choose JSONC for a commented example."
  ],
  "code.textNote": [
    "纯文本保留原样，不猜测语法；未知语言也按纯文本安全显示",
    "プレーンテキストは構文を推測せずそのまま表示します。未知の言語も安全なテキストとして扱います",
    "Plain text stays unchanged. Unknown languages also render as safely escaped text without guessing a grammar."
  ],
  "code.diffNote": [
    "绿色表示新增，红色表示删除；同时保留 + / − 标记，不仅依靠颜色区分",
    "追加は緑、削除は赤で表示し、色だけに頼らず + / − 記号も保持します",
    "Green marks additions and red marks deletions. The + / − markers remain, so the change is also clear without color."
  ],
  "common.close": [
    "关闭",
    "閉じる",
    "Close"
  ],
  "common.copy": [
    "复制",
    "コピー",
    "Copy"
  ],
  "common.copied": [
    "已复制",
    "コピーしました",
    "Copied"
  ],
  "common.copyFailed": [
    "无法复制，请手动选择文本",
    "コピーできません。テキストを選択してください",
    "Copy failed. Select the text manually."
  ],
  "common.noResults": [
    "没有符合条件的结果",
    "条件に一致する結果がありません",
    "No matching results"
  ],
  "docs.title": [
    "从第一条事件开始",
    "最初のイベントから",
    "Start with the first event"
  ],
  "docs.intro": [
    "接入、调查、回放与投递的产品使用指南",
    "接続、調査、リプレイ、配信の使い方ガイドです",
    "Guides for setup, investigation, replay, and delivery."
  ],
  "docs.gettingStarted": [
    "快速接入",
    "クイックスタート",
    "Quick start"
  ],
  "docs.investigation": [
    "调查问题",
    "問題を調査",
    "Investigate issues"
  ],
  "docs.replay": [
    "读取回放",
    "リプレイを読む",
    "Read replay"
  ],
  "docs.releases": [
    "核对调试文件",
    "デバッグファイルの照合",
    "Match debug files"
  ],
  "docs.push": [
    "准备推送",
    "プッシュ通知の準備",
    "Prepare push delivery"
  ],
  "docs.permissions": [
    "权限与密钥",
    "権限とキー",
    "Permissions & keys"
  ],
  "docs.design": [
    "设计与实现",
    "デザインと実装",
    "Design & implementation"
  ],
  "docs.outline": [
    "本页目录",
    "目次",
    "On this page"
  ],
  "docs.step1": [
    "确认范围",
    "対象範囲を確認",
    "Confirm the scope"
  ],
  "docs.step2": [
    "执行并核对",
    "実行して確認",
    "Act and verify"
  ],
  "docs.step3": [
    "处理缺失与失败",
    "不足と失敗への対応",
    "Handle missing data and failure"
  ],
  "docs.setupBody": [
    "选择平台、创建项目和接入密钥，按示例初始化 SDK；收到测试事件后，单独核对版本与源码是否可读",
    "プラットフォームを選び、プロジェクトと取り込みキーを作成して SDK を初期化します。受信後、リリースとソース参照可否を別々に確認します",
    "Choose a platform, create a project and ingest key, and initialize the SDK. After receiving a test event, separately verify its release and readable source."
  ],
  "docs.issueBody": [
    "先确认环境、版本和受影响用户，再选择一条事件查看源码、上下文与回放；处理决定应附有说明，并可追溯到负责人",
    "環境、リリース、影響ユーザーを確認してから、イベントを選び、ソース、コンテキスト、リプレイを調べます。判断にはメモと担当者を残します",
    "Confirm environment, release, and affected users, then select an event to inspect source, context, and replay. Record decisions and an accountable assignee."
  ],
  "docs.replayBody": [
    "按时间轴定位操作，用逐帧或播放观察过程；结构回放与画面回放是不同数据；未采集时明确说明，不生成替代画面",
    "タイムラインから操作へ移動し、コマ送りや再生で確認します。ワイヤーフレームと画面キャプチャは別のデータです。未収集時に代替の録画を作りません",
    "Seek from the timeline and inspect frames or playback. Wireframes and screen captures are different data. When capture is absent, say so rather than inventing a recording."
  ],
  "docs.releaseBody": [
    "按构建标识匹配 dSYM、R8 mapping 或 source map；已上传不代表已匹配，样本不足时不把无崩溃比例写成 100%",
    "ビルド ID で dSYM、R8 mapping、source map を照合します。アップロード済みと照合済みは異なり、サンプル不足を 100% と表示しません",
    "Match dSYM, R8 mapping, or source maps by build identifier. Uploaded does not mean matched, and insufficient samples do not mean 100% crash-free."
  ],
  "docs.pushBody": [
    "添加并验证凭据，确认设备可投递；预览受众后再发送；条件或设备变化时重新预览；排队、送达与失败分别跟踪",
    "認証情報を追加・検証し、デバイスへの送信可否を確認します。対象をプレビューしてから送り、条件や端末が変われば再確認します。待機、配信済み、失敗を区別します",
    "Add and probe credentials, then check device addressability. Preview the audience before sending and repeat the preview after changes. Track queued, delivered, and failed separately."
  ],
  "docs.permissionsBody": [
    "接入密钥只负责事件写入，管理密钥具有不同权限；密钥创建时只显示一次，撤销与成员访问变更进入审计记录",
    "取り込みキーと管理キーの権限を分離します。キーは作成時に一度だけ表示し、失効やメンバー権限の変更を監査ログに残します",
    "Separate ingestion and management key scopes. Show secrets once, and record revocations and access changes in the audit log."
  ],
  "docs.designBody": [
    "从语义 token 和原生控件开始，通过 React 组合页面；所有可见文案、校验、时间和图表信息都经过三语目录",
    "セマンティックトークンとネイティブ要素を基盤に、React で画面を構成します。表示文、検証、日時、チャートを三言語で扱います",
    "Build from semantic tokens and native controls, then compose screens in React. UI copy, validation, time, and chart information use the three-language catalogue."
  ],
  "docs.workingCopy": [
    "开发中文档",
    "開発中のドキュメント",
    "Development docs"
  ],
  "docs.unreleased": [
    "未发布",
    "未リリース",
    "Unreleased"
  ]
} as const;
