/** The cardloader mark: a card being loaded onto a stack. Uses currentColor. */
export default function Logo({ className }: { className?: string }) {
  return (
    <svg viewBox="0 0 64 64" fill="none" className={className} aria-hidden>
      <rect
        x="9"
        y="12"
        width="28"
        height="40"
        rx="4"
        transform="rotate(-12 23 32)"
        stroke="currentColor"
        strokeWidth="3.5"
        opacity="0.55"
      />
      <rect x="24" y="8" width="30" height="42" rx="4.5" fill="currentColor" />
      <path
        d="M39 20v17m0 0-6.5-6.5M39 37l6.5-6.5"
        stroke="#0b2a5b"
        strokeWidth="3.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
      <rect x="31" y="42" width="16" height="3" rx="1.5" fill="#0b2a5b" />
    </svg>
  );
}
