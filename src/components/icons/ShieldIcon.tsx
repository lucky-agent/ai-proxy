interface Props {
  className?: string
}

export function ShieldIcon({ className }: Props) {
  return (
    <svg
      className={className}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
    >
      <path d="M12 2l8 4v6c0 5.5-3.8 10-8 12-4.2-2-8-6.5-8-12V6l8-4z" />
      <line x1="9" y1="12" x2="15" y2="12" />
    </svg>
  )
}
