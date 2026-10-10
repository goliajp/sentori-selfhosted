import { marked } from "marked";
import { type DocPage, headingId } from "./types";
type DocHref = (page:string, params?:Record<string,string>) => string;

const escape = (text: string) => text.replaceAll("&", "&amp;").replaceAll('"', "&quot;").replaceAll("<", "&lt;");
export function renderArticle(page: DocPage, version: string | undefined, options: { omitTitle?: boolean; linkLabel?: string; href: DocHref }) {
  const { href } = options;
  const codes: { code: string; language: string }[] = [];
  const headings: { id: string; text: string; depth: number }[] = [];
  const ids = new Map<string, number>();
  let section = page.title, titleSeen = false;
  const renderer = new marked.Renderer();
  renderer.code = ({ text, lang }) => {
    const index = codes.push({ code: text, language: lang?.split(" ")[0] || "text" }) - 1;
    return `<!--VIS-CODE:${index}-->`;
  };
  renderer.heading = function ({ tokens, depth }) {
    const text = this.parser.parseInline(tokens);
    const plain = text.replace(/<[^>]*>/g, "");
    if (depth === 1 && !titleSeen) { titleSeen = true; if (options.omitTitle) return ''; }
    const base = headingId(plain) || 'section';
    const count = ids.get(base) ?? 0;
    ids.set(base, count + 1);
    const id = count ? `${base}-${count}` : base;
    section = plain;
    if (depth === 2 || depth === 3) headings.push({ id, text: plain, depth });
    const target = href('docs', { topic: page.slug, ...(version ? { version } : {}), anchor: id });
    return `<h${depth} id="${escape(id)}" tabindex="-1">${text}<a class="docs-heading-anchor" href="${escape(target)}" aria-label="${escape(options.linkLabel ?? 'Link to section')}: ${escape(plain)}">#</a></h${depth}>`;
  };
  renderer.table = function (token) {
    const table = marked.Renderer.prototype.table.call(this, token);
    return `<div class="document-table-scroll" role="region" tabindex="0" aria-label="${escape(section)}">${table}</div>`;
  };
  renderer.link = function ({ href: target = "", tokens, title }) {
    const label = this.parser.parseInline(tokens);
    if (/^(javascript|data|vbscript):/i.test(target)) return label;
    let resolved = target;
    if (target.startsWith("//")) resolved = `https:${target}`;
    if (!/^[a-z]+:/i.test(target) && !target.startsWith("#") && !target.startsWith("//")) {
      const base = `https://docs.example.invalid/docs/${page.slug || "README"}.md`;
      const url = new URL(target, base);
      if (url.pathname.startsWith("/docs/")) {
        const slug = url.pathname.slice(6).replace(/\.md$/, "").replace(/^README$/, "");
        resolved = href("docs", { topic: slug, ...(version ? { version } : {}), ...(url.hash ? { anchor: url.hash.slice(1) } : {}) });
      } else resolved = `https://github.com/goliajp/sentori-selfhosted/blob/${version && version !== "current" ? encodeURIComponent(version) : "master"}${url.pathname}${url.hash}`;
    }
    const external = /^https?:/.test(resolved);
    return `<a href="${escape(resolved)}"${title ? ` title="${escape(title)}"` : ""}${external ? ' target="_blank" rel="noreferrer"' : ""}>${label}</a>`;
  };
  return { html: marked.parse(page.markdown, { renderer, async: false }) as string, codes, headings };
}
export function scrollToHeading(id: string) {
  const heading = document.getElementById(id);
  heading?.scrollIntoView({ behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "instant" : "smooth", block: "start" });
  heading?.focus({ preventScroll: true });
}
