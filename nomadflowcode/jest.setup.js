global.fetch = jest.fn();

// Mock expo-localization for i18n
jest.mock('expo-localization', () => ({
  getLocales: () => [{ languageCode: 'en' }],
}));

// Initialize i18next for tests
const i18n = require('i18next');
const { initReactI18next } = require('react-i18next');
const en = require('./locales/en.json');

i18n.use(initReactI18next).init({
  resources: { en: { translation: en } },
  lng: 'en',
  fallbackLng: 'en',
  interpolation: { escapeValue: false },
  react: { useSuspense: false },
});
