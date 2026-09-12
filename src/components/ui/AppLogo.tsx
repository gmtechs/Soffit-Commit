import React from "react";

export function AppLogo({ size = 28 }: { size?: number }) {
  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      width={size}
      height={size}
      viewBox="0 0 26 26"
      fill="currentColor"
      aria-label="Soffit Commit logo"
    >
      <rect x="1.5" y="1.5" width="9" height="9" rx="2.2" />
      <rect x="15.5" y="1.5" width="9" height="9" rx="2.2" />
      <rect x="1.5" y="15.5" width="9" height="9" rx="2.2" />
      <rect x="15.5" y="15.5" width="4" height="4" rx="1" />
      <rect x="21" y="21" width="4" height="4" rx="1" />
    </svg>
  );
}
