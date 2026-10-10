import mark from '../../shared/site/assets/mark-blue.svg';
import markWhite from '../../shared/site/assets/mark-white.svg';
import lockup from '../../shared/site/assets/lockup-blue-ink.svg';
import lockupWhite from '../../shared/site/assets/lockup-white.svg';

export function SentoriMark({ className = '' }: { className?: string }) {
  return <span className={`inline-flex shrink-0 ${className}`} aria-hidden>
    <img src={mark} alt="" className="identity-light h-full w-full" />
    <img src={markWhite} alt="" className="identity-dark h-full w-full" />
  </span>;
}

export function SentoriIdentity() {
  return <>
    <img src={lockup} alt="" className="identity-light h-8 w-auto max-w-full" />
    <img src={lockupWhite} alt="" className="identity-dark h-8 w-auto max-w-full" />
  </>;
}
