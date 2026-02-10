import * as React from "react";
const SVGComponent = (props) => (
  <svg
    width={1024}
    height={1024}
    viewBox="0 0 1024 1024"
    xmlns="http://www.w3.org/2000/svg"
    {...props}
  >
    <defs>
      <linearGradient id="accentGrad" x1="0%" y1="0%" x2="100%" y2="100%">
        <stop
          offset="0%"
          style={{
            stopColor: "#5336E2",
            stopOpacity: 1,
          }}
        />
        <stop
          offset="100%"
          style={{
            stopColor: "#8B5CF6",
            stopOpacity: 1,
          }}
        />
      </linearGradient>
    </defs>
    <rect width={1024} height={1024} rx={180} ry={180} fill="#0f0f17" />
    <path
      d="M256 768 Q512 256 768 768"
      stroke="url(#accentGrad)"
      strokeWidth={64}
      fill="none"
      strokeLinecap="round"
    />
  </svg>
);
export default SVGComponent;
