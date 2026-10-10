import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { BrowserRouter } from 'react-router-dom';

import { AppRoutes } from './AppRoutes';
import { I18nProvider } from './i18n/provider';
import { initTheme } from './lib/theme';
import './styles/index.css';

initTheme();
const root = document.getElementById('root');
if (root) {
  createRoot(root).render(
    <StrictMode>
      <I18nProvider>
        <BrowserRouter><AppRoutes /></BrowserRouter>
      </I18nProvider>
    </StrictMode>,
  );
}
