/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        bg0: "#252320",
        bg1: "#2E2B25",
        bg2: "#3A3731",
        bg3: "#4a4640",
        orange: "#E8622A",
        red: "#E24B4A",
        green: "#4caf7d",
        text: "#f0ece4",
        muted: "#a09890",
        dim: "#6A6460",
        border: "#4a4640",
      },
      fontFamily: {
        mono: ["'Fira Mono'", "monospace"],
      },
    },
  },
  plugins: [],
};
