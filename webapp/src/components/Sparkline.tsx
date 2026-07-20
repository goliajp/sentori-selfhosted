// Inline SVG sparkline — no external chart dep.
// Takes a flat array of numbers and renders a polyline scaled
// to the parent's width × height.

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
  stroke = '#10b981',
  fill = 'rgba(16, 185, 129, 0.15)',
}: Props) {
  if (values.length < 2) {
    return (
      <div
        className={`flex items-center justify-center text-[10px] text-zinc-500 ${className ?? ''}`}
        style={{ width, height }}
      >
        not enough data
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
