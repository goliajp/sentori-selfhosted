export const fileMessages = {
"common.clear": ["清除", "クリア", "Clear"],
"common.loading": ["正在加载", "読み込み中", "Loading"],
"file.choose": ["选择文件", "ファイルを選択", "Choose file"],
"file.drop": [
    "拖放文件到这里，或点击选择",
    "ここにファイルをドロップ、または選択",
    "Drop a file here or choose one",
  ],
"file.tooLarge": [
    "文件不能超过 {size} MB",
    "ファイルは {size} MB 以下にしてください",
    "File must be no larger than {size} MB",
  ],
"file.added": [
    "已记录文件：{name}",
    "ファイルを記録しました：{name}",
    "File recorded: {name}",
  ],
'file.none': ['未选择文件', 'ファイル未選択', 'No files selected'],
'file.selected': ['已选择 {count} 个文件', '{count} 件を選択中', '{count} files selected'],
'file.limit': ['最多 {count} 个文件 · 单个不超过 {size}', '最大 {count} 件・1 件あたり {size} まで', 'Up to {count} files · {size} each'],
'file.limitCount': ['最多 {count} 个文件', '最大 {count} 件', 'Up to {count} files'],
'file.accepted': ['允许格式：{types}', '形式：{types}', 'Accepted formats: {types}'],
'file.reject.type': ['{name}：不支持此文件格式', '{name}：この形式は選択できません', '{name}: unsupported file format'],
'file.reject.size': ['{name}：超过单个文件大小限制', '{name}：ファイルサイズの上限を超えています', '{name}: exceeds the file size limit'],
'file.reject.count': ['{name}：已达到文件数量上限', '{name}：ファイル数の上限に達しています', '{name}: file count limit reached'],
'file.reject.duplicate': ['{name}：已在列表中', '{name}：すでに選択されています', '{name}: already selected'],
'file.preview': ['预览 {name}', '{name} をプレビュー', 'Preview {name}'],
'file.removeNamed': ['移除 {name}', '{name} を削除', 'Remove {name}'],
'file.previewUnavailable': ['此格式不提供内嵌预览', 'この形式はプレビューできません', 'An inline preview is unavailable for this format'],
'file.previewTruncated': ['仅预览前 64 KB', '先頭の 64 KB のみ表示', 'Showing the first 64 KB'],
'file.readFailed': ['无法读取预览，请重新选择文件', 'プレビューを読み込めません。選び直してください', 'Unable to read the preview. Select the file again']
} as const;
