import type { NextConfig } from "next"

const nextConfig: NextConfig = {
  output: "export",
  images: {
    unoptimized: true,
    remotePatterns: [
      { protocol: "https", hostname: "i.ytimg.com", pathname: "/**" },
      { protocol: "https", hostname: "image.animecube.live", pathname: "/**" },
      { protocol: "https", hostname: "animekhor.org", pathname: "/**" },
      { protocol: "https", hostname: "donghuastream.org", pathname: "/**" },
    ],
  },
  turbopack: {
    root: process.cwd(),
  },
}

export default nextConfig
