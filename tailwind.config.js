/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      fontFamily: {
        sans: ["\"Space Grotesk\"", "Inter", "ui-sans-serif", "system-ui", "sans-serif"],
      },
      boxShadow: {
        glow: "0 25px 80px rgba(17, 24, 39, 0.55)",
      },
      opacity: {
        2: "0.02",
        3: "0.03",
        7: "0.07",
      },
    },
  },
  plugins: [],
};
