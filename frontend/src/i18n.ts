import i18n from 'i18next';
import { initReactI18next } from 'react-i18next';
import LanguageDetector from 'i18next-browser-languagedetector';

import en from './locales/en/translation.json';
import pl from './locales/pl/translation.json';

const resources = {
  en: { translation: en },
  pl: { translation: pl },
};

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources,
    fallbackLng: 'en',
    supportedLngs: ['en', 'pl'],
    interpolation: {
      escapeValue: false,
    },
    detection: {
      order: ['htmlTag', 'path', 'navigator'],
      caches: [],
    },
  });

export default i18n;
