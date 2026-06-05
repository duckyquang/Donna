import type { Config } from "tailwindcss";

export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        donna: {
          bg: "#090909",
          panel: "#0f0d0b",
          surface: "#161311",
          "surface-raised": "#1e1a16",
          border: "#26263040",
          accent: "#c9742a",
          "accent-hover": "#a8611f",
          "accent-light": "#e8a55a",
          muted: "#9ca3af",
        },
      },
      fontFamily: {
        sans: ["Inter", "system-ui", "sans-serif"],
      },
    },
  },
  plugins: [],
} satisfies Config;
