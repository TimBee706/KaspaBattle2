/** @type {import('tailwindcss').Config} */
export default {
    content: ['./index.html', './src/**/*.{ts,tsx}'],
    theme: {
        extend: {
            colors: {
                kaspa: {
                    primary:   '#49EACB',
                    secondary: '#2D7A6C',
                    accent:    '#7C3AED',
                    dark:      '#0a0e17',
                    card:      '#111827',
                    border:    '#1f2937',
                    surface:   '#0D1B2A',
                },
                status: {
                    success: '#22C55E',
                    warning: '#F59E0B',
                    danger:  '#EF4444',
                    info:    '#3B82F6',
                },
            },
            fontFamily: {
                sans: ['Inter', 'system-ui', 'sans-serif'],
                mono: ['JetBrains Mono', 'monospace'],
            },
            boxShadow: {
                'glow-primary': '0 0 32px rgba(73,234,203,0.45)',
                'glow-primary-lg': '0 0 64px rgba(73,234,203,0.3)',
                'glow-card': '0 18px 60px rgba(0,0,0,0.65)',
                'glow-accent': '0 0 32px rgba(124,58,237,0.35)',
            },
            backdropBlur: {
                glass: '12px',
            },
            animation: {
                'pulse-slow': 'pulse 3s cubic-bezier(0.4, 0, 0.6, 1) infinite',
            },
        },
    },
    plugins: [],
};
