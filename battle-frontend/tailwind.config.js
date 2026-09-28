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
            fontSize: {
                // Consolidates the text-[10px]/text-[9px]/text-[11px] magic
                // numbers scattered across labels, badges and meta text into
                // one real token.
                '2xs': ['0.6875rem', { lineHeight: '0.875rem', letterSpacing: '0.04em' }],
            },
            boxShadow: {
                // Toned down from the original (0 0 32px/0.45, 0 0 64px/0.3):
                // smaller spread and lower opacity read as "refined depth"
                // rather than a neon floodlight — see UI polish pass notes.
                'glow-primary': '0 0 20px rgba(73,234,203,0.25)',
                'glow-primary-lg': '0 0 36px rgba(73,234,203,0.24)',
                'glow-card': '0 18px 60px rgba(0,0,0,0.65)',
                'glow-accent': '0 0 32px rgba(124,58,237,0.35)',
                // New: a quiet whisper of glow for elements that need only a
                // hint of emphasis (e.g. an active/selected state), and a
                // consistent focus indicator to replace bare outline-none.
                'glow-subtle': '0 0 14px rgba(73,234,203,0.14)',
                'focus-ring': '0 0 0 3px rgba(73,234,203,0.35)',
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
