const images = import.meta.glob<string>('./assets/*.{png,svg}', { eager: true, query: '?url', import: 'default' });
export function siteImage(path: string): string {
  const file = path.split('/').at(-1)!;
  const url = images[`./assets/${file}`];
  if (!url) throw new Error(`Unknown site image: ${path}`);
  return url;
}
