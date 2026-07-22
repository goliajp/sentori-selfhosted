// Legal documents, kept as data rather than markup.
//
// Two reasons they are not in `src/i18n`. The catalogues are a flat map
// of short UI strings where a missing key is a compile error — useful
// for buttons, useless for a document that is not a translation of
// another document but a separate legal text with its own governing
// language. And they sit outside `src/pages`, so
// `check-hardcoded-text` keeps meaning what it says: prose *there* is
// a bug, prose here is the product.
//
// 特定商取引法に基づく表記 exists only in Japanese. It is a disclosure
// required of a Japanese seller under Japanese law, and an English
// rendering of it would be a courtesy translation presented as a legal
// notice — the wrong thing to put on the page a regulator reads.

export type Locale = 'ja' | 'en';

export interface Section {
  heading: string;
  /** Paragraphs. */
  body?: string[];
  /** Label/value rows, for the disclosure table. */
  rows?: [string, string][];
}

export interface LegalDoc {
  title: string;
  updated: string;
  intro?: string;
  sections: Section[];
}

const UPDATED = '2026-07-22';

const COMPANY_JA = 'GOLIA株式会社';
const COMPANY_EN = 'GOLIA K.K.';
const REP_JA = 'リ ハオ';
const REP_EN = 'LI HAO';
const ADDR_JA = '〒160-0021 東京都新宿区歌舞伎町2-19-13 ASKビル6F';
const ADDR_EN =
  '6F, ASK Building, 2-19-13 Kabukicho, Shinjuku City, Tokyo 160-0021, Japan';
const TEL = '+81 3-6687-6723';
const MAIL = 'contact@golia.jp';

// ── 特定商取引法に基づく表記 ─────────────────────────────

const TOKUSHOHO: LegalDoc = {
  title: '特定商取引法に基づく表記',
  updated: UPDATED,
  intro:
    '特定商取引法第11条（通信販売についての広告）に基づき、以下のとおり表示します。',
  sections: [
    {
      heading: '事業者情報',
      rows: [
        ['販売事業者名', COMPANY_JA],
        ['運営統括責任者', REP_JA],
        ['所在地', ADDR_JA],
        // 特商法は電話番号の表示を求めているため掲載する。ただし
        // 問い合わせ窓口はメールに一本化しており、電話での応対は
        // 行っていない。
        ['電話番号', `${TEL}（お問い合わせはメールにて承ります）`],
        ['メールアドレス', MAIL],
        [
          'お問い合わせ方法',
          `お問い合わせは ${MAIL} 宛のメールにて承ります。原則として2営業日以内にご返信します（土日祝日および年末年始を除く）。お電話でのサポートは行っておりません。`,
        ],
      ],
    },
    {
      heading: '販売条件',
      rows: [
        [
          '販売価格',
          'Sentori Pro：月額 4,900円（消費税込）。無料プランは0円です。価格は本サイトの料金ページおよび決済画面に表示します。',
        ],
        [
          '商品代金以外の必要料金',
          'インターネット接続に必要な通信料金は、お客様のご負担となります。海外で発行されたクレジットカードをご利用の場合、カード会社所定の為替手数料が発生することがあります。',
        ],
        ['支払方法', 'クレジットカード（Stripe による決済）。'],
        [
          '支払時期',
          'お申し込み手続きの完了時に初回のご請求を行い、以後は毎月同日に自動更新のうえご請求します。',
        ],
        [
          '役務の提供時期',
          '決済の完了後、ただちにご利用いただけます。',
        ],
        [
          '契約期間',
          '1か月単位の自動更新です。解約のお手続きがない限り、同一条件で更新されます。',
        ],
      ],
    },
    {
      heading: '解約・返金',
      rows: [
        [
          '解約の方法',
          'ダッシュボードの「請求」画面からいつでもお手続きいただけます。ご連絡やお電話は不要です。',
        ],
        [
          '解約の効力',
          '解約後も、お支払い済みの請求期間の末日まではご利用いただけます。期間の末日をもって無料プランに切り替わります。',
        ],
        [
          '返品・返金',
          '役務の性質上、提供開始後の返品はお受けしておりません。日割りでの返金も行っておりません。当社の責めに帰すべき事由により役務を提供できなかった場合は、個別に対応いたします。',
        ],
      ],
    },
    {
      heading: 'その他',
      rows: [
        [
          '動作環境',
          'Google Chrome、Safari、Microsoft Edge、Mozilla Firefox の最新版に対応しています。',
        ],
        [
          '表現および再現性の注意書き',
          '本サービスの説明に記載された数値は、記載時点の仕様に基づくものであり、特定の効果を保証するものではありません。',
        ],
      ],
    },
  ],
};

// ── 利用規約 / Terms ─────────────────────────────────────

const TERMS_JA: LegalDoc = {
  title: '利用規約',
  updated: UPDATED,
  intro: `本規約は、${COMPANY_JA}（以下「当社」）が提供する Sentori（以下「本サービス」）の利用条件を定めるものです。本サービスをご利用いただくことで、本規約に同意いただいたものとみなします。`,
  sections: [
    {
      heading: '第1条（アカウント）',
      body: [
        'ご登録時に提供いただく情報は、正確かつ最新のものである必要があります。',
        'アカウントの認証情報の管理はお客様の責任において行っていただきます。第三者による不正な利用を検知した場合は、速やかに当社までご連絡ください。',
      ],
    },
    {
      heading: '第2条（お客様のデータ）',
      body: [
        '本サービスを通じて当社に送信されたデータ（エラー情報、トレース、セッション記録等。以下「お客様データ」）の権利は、お客様に帰属します。',
        '当社は、本サービスの提供、維持および改善のために必要な範囲でのみお客様データを取り扱います。お客様データを第三者に販売することはありません。',
        'お客様データの保存期間は、ご利用のプランに応じて定まります。保存期間を経過したデータは削除されます。',
        'お客様は、お客様データに個人情報が含まれる場合、その取得および当社への提供について必要な法的根拠を確保する責任を負います。',
      ],
    },
    {
      heading: '第3条（禁止事項）',
      body: [
        '法令または公序良俗に違反する行為、本サービスの運営を妨害する行為、他者の権利を侵害する行為、および本サービスのリバースエンジニアリングを目的とした行為を禁止します。',
        '本サービスのセルフホスト版は、公開されているライセンスの条件に従ってご利用いただけます。本規約は当社が運用するホスティング版に適用されます。',
      ],
    },
    {
      heading: '第4条（利用の制限および停止）',
      body: [
        '当社は、お客様が本規約に違反した場合、または本サービスの安定した運営に支障が生じるおそれがあると判断した場合、事前の通知なく利用を制限または停止することがあります。',
        'ご利用のプランに定める上限を超えた場合、超過分のデータの受け入れを停止することがあります。',
      ],
    },
    {
      heading: '第5条（料金）',
      body: [
        '有料プランの料金、支払時期および解約の条件は、特定商取引法に基づく表記に定めるとおりです。',
        '料金を改定する場合は、適用開始日の30日前までにご登録のメールアドレス宛にご通知します。',
      ],
    },
    {
      heading: '第6条（免責）',
      body: [
        '当社は、本サービスが中断なく提供されること、または特定の目的に適合することを保証するものではありません。',
        '当社の責任は、故意または重過失による場合を除き、責任の原因となった事象が発生した月にお客様が当社に支払った金額を上限とします。',
        '本サービスは、お客様のアプリケーションの監視を補助するものであり、お客様自身による品質管理の責任を代替するものではありません。',
      ],
    },
    {
      heading: '第7条（準拠法および管轄）',
      body: [
        '本規約は日本法に準拠します。本サービスに関して紛争が生じた場合、東京地方裁判所を第一審の専属的合意管轄裁判所とします。',
      ],
    },
    {
      heading: '第8条（規約の変更）',
      body: [
        '当社は本規約を変更することがあります。重要な変更については、適用開始日の30日前までにご通知します。変更後も本サービスをご利用いただいた場合、変更に同意いただいたものとみなします。',
      ],
    },
  ],
};

const TERMS_EN: LegalDoc = {
  title: 'Terms of Service',
  updated: UPDATED,
  intro: `These terms govern your use of Sentori, operated by ${COMPANY_EN}. Using the service means you accept them. Where this English text and the Japanese 利用規約 differ, the Japanese text governs.`,
  sections: [
    {
      heading: '1. Accounts',
      body: [
        'The information you register with must be accurate and kept current.',
        'You are responsible for your credentials. Tell us promptly if you believe someone else is using your account.',
      ],
    },
    {
      heading: '2. Your data',
      body: [
        'Data you send us through the service — errors, traces, session recordings — remains yours.',
        'We process it only to run, maintain and improve the service. We do not sell it.',
        'How long we keep it is set by your plan. Past that window it is deleted.',
        'If that data contains personal information, securing a lawful basis for collecting it and sending it to us is your responsibility, not ours.',
      ],
    },
    {
      heading: '3. Acceptable use',
      body: [
        'Do not break the law, interfere with the service, infringe anyone else’s rights, or reverse-engineer the hosted service.',
        'The self-hosted build is governed by its published licence. These terms cover the hosted service we operate.',
      ],
    },
    {
      heading: '4. Suspension',
      body: [
        'We may limit or suspend an account without notice if it breaches these terms or threatens the stability of the service.',
        'If you exceed your plan’s limits we may stop accepting further data for that period.',
      ],
    },
    {
      heading: '5. Fees',
      body: [
        'Price, billing dates and cancellation are set out in the 特定商取引法に基づく表記 disclosure.',
        'We will give 30 days’ notice by email before any price change takes effect.',
      ],
    },
    {
      heading: '6. Liability',
      body: [
        'We do not warrant that the service will be uninterrupted or fit for a particular purpose.',
        'Except in cases of wilful misconduct or gross negligence, our liability is capped at what you paid us in the month the claim arose.',
        'Sentori helps you watch your application. It does not replace your own quality assurance.',
      ],
    },
    {
      heading: '7. Governing law',
      body: [
        'Japanese law applies. The Tokyo District Court has exclusive jurisdiction at first instance.',
      ],
    },
    {
      heading: '8. Changes',
      body: [
        'We may revise these terms, with 30 days’ notice for material changes. Continuing to use the service after that date means you accept them.',
      ],
    },
  ],
};

// ── プライバシーポリシー / Privacy ────────────────────────

const PRIVACY_JA: LegalDoc = {
  title: 'プライバシーポリシー',
  updated: UPDATED,
  intro: `${COMPANY_JA}（以下「当社」）は、Sentori（以下「本サービス」）の提供にあたり取得する情報について、以下のとおり取り扱います。`,
  sections: [
    {
      heading: '取得する情報',
      body: [
        'アカウント情報：メールアドレス、氏名、所属ワークスペース、認証に関する記録。',
        'お客様データ：本サービスに送信されたエラー情報、トレース、セッション記録、および付随する属性。これらはお客様のアプリケーションから送信されるものであり、その内容はお客様が決定します。',
        '利用状況：ログイン日時、IPアドレス、操作の記録（監査ログ）。',
        '決済情報：お支払いは Stripe が処理します。当社はクレジットカード番号を保持しません。',
      ],
    },
    {
      heading: '利用目的',
      body: [
        '本サービスの提供、認証、課金、不正利用の防止、障害対応、およびお問い合わせへの回答のために利用します。',
        'お客様データを、当社の製品開発のための学習用データとして利用することはありません。',
      ],
    },
    {
      heading: '第三者提供および委託',
      body: [
        '法令に基づく場合を除き、ご本人の同意なく第三者に提供することはありません。',
        '本サービスの運営に必要な範囲で、以下に取り扱いを委託しています：サーバー設備としてアマゾン ウェブ サービス（日本リージョン）、決済代行として Stripe。',
        'お客様データ（エラー情報、トレース、セッション記録等）の保存および処理は、日本国内のサーバー設備において行います。',
        '決済に関する情報は Stripe が処理します。同社の処理体制により、当該情報が日本国外で取り扱われる場合があります。',
      ],
    },
    {
      heading: '保存期間',
      body: [
        'お客様データは、ご利用のプランに定める保存期間の経過後に削除されます。',
        'アカウント情報は、アカウントの削除後、法令上の保存義務がある場合を除き削除します。',
      ],
    },
    {
      heading: 'お客様の権利',
      body: [
        '保有個人データの開示、訂正、利用停止および削除のご請求は、下記の窓口までご連絡ください。ご本人であることを確認のうえ、法令に従い対応いたします。',
        'お客様のアプリケーションの利用者（エンドユーザー）に関するご請求については、当社はお客様の委託を受けて取り扱っているため、まずお客様ご自身にお問い合わせいただくようご案内する場合があります。',
      ],
    },
    {
      heading: 'お問い合わせ窓口',
      rows: [
        ['事業者名', COMPANY_JA],
        ['個人情報保護管理者', REP_JA],
        ['所在地', ADDR_JA],
        ['メールアドレス', MAIL],
      ],
    },
  ],
};

const PRIVACY_EN: LegalDoc = {
  title: 'Privacy Policy',
  updated: UPDATED,
  intro: `How ${COMPANY_EN} handles information collected in the course of providing Sentori. Where this English text and the Japanese プライバシーポリシー differ, the Japanese text governs.`,
  sections: [
    {
      heading: 'What we collect',
      body: [
        'Account information: email address, name, workspace membership, and authentication records.',
        'Your data: the errors, traces, session recordings and attached attributes your application sends us. What they contain is determined by you, not by us.',
        'Usage: sign-in times, IP addresses, and an audit log of actions taken.',
        'Payment: handled by Stripe. We never hold your card number.',
      ],
    },
    {
      heading: 'Why we use it',
      body: [
        'To run the service, authenticate you, bill you, prevent abuse, investigate faults, and answer your questions.',
        'We do not use your data as training material for our own product.',
      ],
    },
    {
      heading: 'Who else sees it',
      body: [
        'Nobody, except where the law requires it or you have agreed.',
        'Server infrastructure: Amazon Web Services, Japan region. Payment processing: Stripe.',
        'Your telemetry — errors, traces, session recordings — is stored and processed on infrastructure located in Japan.',
        'Payment information is handled by Stripe, and may be processed outside Japan depending on their arrangements.',
      ],
    },
    {
      heading: 'How long we keep it',
      body: [
        'Your data is deleted once your plan’s retention window passes.',
        'Account information is deleted when the account is, unless a legal obligation requires us to keep it.',
      ],
    },
    {
      heading: 'Your rights',
      body: [
        'Write to the contact below to request disclosure, correction, suspension of use, or deletion. We will verify your identity and respond as the law requires.',
        'For requests about the end users of *your* application, we act on your instructions rather than our own, so we may direct the request back to you.',
      ],
    },
    {
      heading: 'Contact',
      rows: [
        ['Operator', COMPANY_EN],
        ['Data protection officer', REP_EN],
        ['Address', ADDR_EN],
        ['Email', MAIL],
      ],
    },
  ],
};

/** Slug → locale → document. `tokushoho` is Japanese only, by design. */
export const LEGAL_DOCS: Record<string, Partial<Record<Locale, LegalDoc>>> = {
  tokushoho: { ja: TOKUSHOHO },
  terms: { ja: TERMS_JA, en: TERMS_EN },
  privacy: { ja: PRIVACY_JA, en: PRIVACY_EN },
};

export const LEGAL_NAV: { slug: string; ja: string; en: string }[] = [
  { slug: 'terms', ja: '利用規約', en: 'Terms' },
  { slug: 'privacy', ja: 'プライバシーポリシー', en: 'Privacy' },
  { slug: 'tokushoho', ja: '特定商取引法に基づく表記', en: '特定商取引法に基づく表記' },
];
