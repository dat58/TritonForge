/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ["./src/**/*.{rs,html,css}", "./assets/**/*.{html,css}"],
  safelist: [
    // Dynamic status classes from progress_bar.rs and job_card.rs
    "bg-slate-500", "text-slate-400",
    "bg-cyan-500", "bg-cyan-400", "text-cyan-400", "text-cyan-300",
    "animate-pulse",
    "bg-emerald-500", "text-emerald-400",
    "bg-rose-500", "text-rose-400",
    "bg-slate-700", "text-slate-300",
    { pattern: /^bg-(cyan|emerald|rose|amber)-(900)\/60$/ },
    { pattern: /^text-(cyan|emerald|rose|amber)-300$/ },
  ],
  theme: {
    extend: {
      colors: {
        ocean: {
          50:  "#ecfeff",
          100: "#cffafe",
          200: "#a5f3fc",
          300: "#67e8f9",
          400: "#22d3ee",
          500: "#06b6d4",
          600: "#0891b2",
          700: "#0e7490",
          800: "#155e75",
          900: "#164e63",
          950: "#083344",
        },
      },
      boxShadow: {
        "glow-cyan":    "0 0 24px rgba(6, 182, 212, 0.35)",
        "glow-cyan-sm": "0 0 10px rgba(6, 182, 212, 0.2)",
        "glow-teal":    "0 0 24px rgba(20, 184, 166, 0.35)",
        "glow-emerald": "0 0 24px rgba(52, 211, 153, 0.3)",
        "glow-rose":    "0 0 24px rgba(251, 113, 133, 0.3)",
        "card":         "0 4px 32px rgba(0, 0, 0, 0.5)",
      },
    },
  },
  plugins: [],
};
