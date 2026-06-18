/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  darkMode: "class",
  theme: {
    extend: {
      colors: {
        bg: {
          900: "#0b0d12",
          800: "#11141b",
          700: "#171b24",
          600: "#1f2430",
        },
        accent: {
          DEFAULT: "#7c5cff",
          soft: "#3a2f6b",
        },
      },
      fontFamily: {
        sans: ["Inter", "system-ui", "sans-serif"],
        mono: ["JetBrains Mono", "ui-monospace", "monospace"],
      },
      boxShadow: {
        overlay: "0 12px 40px rgba(0,0,0,0.45)",
      },
    },
  },
  plugins: [],
};
