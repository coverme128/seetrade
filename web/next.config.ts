import type { NextConfig } from "next";
const config: NextConfig = {
  devIndicators: false,
  async rewrites() {
    return [
      {
        source: "/api/:path*",
        destination: `${process.env.MICRO_ALPHA_CORE_URL || "http://127.0.0.1:8787"}/api/:path*`,
      },
    ];
  },
};
export default config;
