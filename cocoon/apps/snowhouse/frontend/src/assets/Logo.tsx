import * as React from "react";
const Logo = (props: React.SVGProps<SVGSVGElement>) => (
  <svg viewBox="0 0 120 32" role="img" aria-label="Logo" {...props}><rect width="120" height="32" rx="4" fill="currentColor"/><text x="60" y="21" fontFamily="sans-serif" fontSize="14" fill="#fff" textAnchor="middle">App</text></svg>
);
export default Logo;
