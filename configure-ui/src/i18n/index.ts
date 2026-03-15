import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'
import { resources } from './resources'

void i18n.use(initReactI18next).init({
  resources,
  fallbackLng: 'zh-CN',
  lng: 'zh-CN',
  defaultNS: 'translation',
  interpolation: {
    escapeValue: true,
  },
})

export default i18n
