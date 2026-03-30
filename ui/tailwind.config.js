/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{vue,js,ts,jsx,tsx}",
  ],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        "primary": "#3c83f6",
        "primary-hover": "#2563eb",
        "background-light": "#f5f7f8",
        "background-dark": "#101722",
        "surface-dark": "#1e293b",
        "border-dark": "#314668",
        "text-secondary": "#90a7cb",
        "success": "#10b981",
        "error": "#ef4444",
        "warning": "#f59e0b",
        "info": "#0ea5e9",
        "purple": "#8b5cf6",
        "quality-gold": "#fbbf24",
        "quality-silver": "#e2e8f0",
        "quality-gray": "#64748b",
        "sidebar": "#0d121c",
        "surface": "#1e293b",
        "surface-highlight": "#2d3b55"
      },
      fontFamily: {
        "display": ["Inter", "sans-serif"],
        "mono": ["ui-monospace", "SFMono-Regular", "Menlo", "Monaco", "Consolas", "Liberation Mono", "Courier New", "monospace"]
      },
      borderRadius: {
        "DEFAULT": "0.25rem",
        "lg": "0.5rem",
        "xl": "0.75rem",
        "2xl": "1rem",
        "full": "9999px"
      },
      boxShadow: {
        "glow": "0 0 20px rgba(60, 131, 246, 0.15)",
      },
      keyframes: {
        shimmer: {
          '100%': { transform: 'translateX(100%)' },
        }
      },
      animation: {
        shimmer: 'shimmer 2s infinite linear',
      }
    },
  },
  plugins: [],
}
