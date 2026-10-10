export type DocPage = {
  /** Path under /docs, without extension. The index is ''. */
  slug: string
  title: string
  description?: string
  markdown: string
}

export function headingId(text: string): string {
  return text
    .toLowerCase()
    .replace(/[^\w\s-]/g, '')
    .trim()
    .replace(/\s+/g, '-')
}

