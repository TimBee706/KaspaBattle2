# Internationalization (i18n) Documentation

This project uses `react-i18next` for internationalization.

## Translation Files

Translations are located in `src/locales/`:

- `de/common.json`: German (default)
- `en/common.json`: English

## Usage in Components

### 1. Basic usage with `useTranslation` hook

```tsx
import { useTranslation } from 'react-i18next';

export function MyComponent() {
  const { t } = useTranslation();
  return <div>{t('common.loading')}</div>;
}
```

### 2. Usage with variables

```tsx
// Translation: "Welcome back, {{name}}!"
<div>{t('auth.welcome', { name: 'Player1' })}</div>
```

### 3. Usage with complex strings (components inside)

```tsx
import { Trans } from 'react-i18next';

<Trans i18nKey="footer.terms">
  By continuing you agree to our <Link to="/terms">Terms of Service</Link>.
</Trans>
```

## Adding New Translations

1. Add the key and its German text to `src/locales/de/common.json`.
2. Add the same key and its English translation to `src/locales/en/common.json`.
3. Use the key in your component.

## Language Selection Logic

1. **Initial Load**: The system checks `localStorage` for `preferred_language`.
2. **Detection**: If not found, it uses the browser's language (`navigator.language`).
3. **Fallback**: If the browser language is not supported, it defaults to `de`.
4. **Persistence**: Any manual language change via the `LanguageToggle` is saved to `localStorage`.
