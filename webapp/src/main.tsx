import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { BrowserRouter, Navigate, Route, Routes } from 'react-router-dom';

import { App } from './App';
import { I18nProvider } from './i18n/provider';
import ForgotPassword from './pages/ForgotPassword';
import Instruments from './pages/Instruments';
import ProjectsPage from './pages/Projects';
import TriageView from './pages/TriageView';
import { LoginPage } from './pages/Login';
import Releases from './pages/Releases';
import ResetPassword from './pages/ResetPassword';
import Settings from './pages/Settings';
import { initTheme } from './lib/theme';
import './styles/index.css';

// Paint the persisted theme (dark by default — this is a triage
// tool) before React mounts, so no page flashes the wrong scheme.
initTheme();

const root = document.getElementById('root');
if (root) {
  createRoot(root).render(
    <StrictMode>
      <I18nProvider>
        <BrowserRouter>
          <Routes>
            <Route path="/login" element={<LoginPage />} />
            <Route path="/forgot-password" element={<ForgotPassword />} />
            <Route path="/reset-password" element={<ResetPassword />} />
            <Route element={<App />}>
              <Route path="/" element={<TriageView />} />
              <Route path="/issues/:issueId" element={<TriageView />} />
              <Route path="/projects" element={<ProjectsPage />} />
              <Route path="/instruments" element={<Instruments />} />
              <Route path="/releases" element={<Releases />} />
              <Route path="/settings" element={<Settings />} />
              <Route path="*" element={<Navigate to="/" replace />} />
            </Route>
          </Routes>
        </BrowserRouter>
      </I18nProvider>
    </StrictMode>,
  );
}
