import type { Config } from "tailwindcss";

export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        donna: {
          bg: "#080808",
          panel: "#0d0d0d",
          surface: "#111111",
          "surface-raised": "#181818",
          "surface-hover": "#1c1c1c",
          border: "rgba(255,255,255,0.07)",
          "border-strong": "rgba(255,255,255,0.12)",
          accent: "#c9742a",
          "accent-hover": "#b5621f",
          "accent-light": "#e8a55a",
          "accent-dim": "rgba(201,116,42,0.12)",
          muted: "#666666",
          "muted-light": "#888888",
          text: "#ededed",
          "text-secondary": "#999999",
          success: "#22c55e",
          "success-dim": "rgba(34,197,94,0.12)",
          danger: "#ef4444",
          "danger-dim": "rgba(239,68,68,0.1)",
        },
      },
      fontFamily: {
        sans: ["Inter", "system-ui", "sans-serif"],
        mono: ["JetBrains Mono", "Fira Code", "Consolas", "monospace"],
      },
      borderRadius: {
        DEFAULT: "6px",
      },
      boxShadow: {
        "card": "0 1px 3px rgba(0,0,0,0.4), 0 0 0 1px rgba(255,255,255,0.06)",
        "elevated": "0 4px 16px rgba(0,0,0,0.6), 0 0 0 1px rgba(255,255,255,0.06)",
      },
    },
  },
  plugins: [],
} satisfies Config;
