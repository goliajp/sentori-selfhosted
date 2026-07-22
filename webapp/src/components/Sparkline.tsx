// Inline SVG sparkline — no external chart dep.
// Takes a flat array of numbers and renders a polyline scaled
// to the parent's width × height.

import { useT } from '../i18n';

interface Props {
  values: number[];
  width?: number;
  height?: number;
  className?: string;
  stroke?: string;
  fill?: string;
}

export function Sparkline({
  values,
  width = 200,
  height = 40,
  className,
  // Defaults read the live accent off the document rather than naming
  // a hex. These were `#10b981` — emerald — which is why sparklines
  // stayed green after the palette moved to blue, and why they would
  // not have followed a theme change either. SVG cannot take a
  // Tailwind class, so the custom property is the seam.
  stroke = 'var(--gds-accent)',
  fill = 'color-mix(in oklab, var(--gds-accent) 15%, transparent)',
}: Props) {
  const t = useT();
  if (values.length < 2) {
    return (
      <div
        className={`flex items-center justify-center text-xs text-fg-subtle ${className ?? ''}`}
        style={{ width, height }}
      >
        {t('overview.notEnoughData')}
      </div>
    );
  }

  const max = Math.max(...values, 1);
  const step = width / (values.length - 1);

  const points = values
    .map((v, i) => {
      const x = i * step;
      const y = height - (v / max) * height;
      return `${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(' ');

  const areaPoints = `0,${height} ${points} ${width},${height}`;

  return (
    <svg
      width={width}
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      className={className}
      preserveAspectRatio="none"
    >
      <polygon points={areaPoints} fill={fill} />
      <polyline points={points} fill="none" stroke={stroke} strokeWidth={1.5} />
    </svg>
  );
}
