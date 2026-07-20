import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { BrowserRouter, Route, Routes } from 'react-router-dom';

import { App } from './App';
import { AlertsPage } from './pages/Alerts';
import { AuditPage } from './pages/Audit';
import { CertPage } from './pages/Cert';
import { EventsPage } from './pages/Events';
import { HealthPage } from './pages/Health';
import { IssuesPage } from './pages/Issues';
import IssueDetail from './pages/IssueDetail';
import { LoginPage } from './pages/Login';
import { OverviewPage } from './pages/Overview';
import ForgotPassword from './pages/ForgotPassword';
import Integrations from './pages/Integrations';
import Members from './pages/Members';
import Projects from './pages/Projects';
import PushCredentials from './pages/PushCredentials';
import PushSends from './pages/PushSends';
import Sessions from './pages/Sessions';
import Register from './pages/Register';
import Releases from './pages/Releases';
import SaasAdmin from './pages/SaasAdmin';
import SavedViews from './pages/SavedViews';
import EndpointProbes from './pages/EndpointProbes';
import Metrics from './pages/Metrics';
import Notifications from './pages/Notifications';
import Search from './pages/Search';
import Shortcuts from './pages/Shortcuts';
import Replays from './pages/Replays';
import ReplayDetail from './pages/ReplayDetail';
import Traces from './pages/Traces';
import TraceDetail from './pages/TraceDetail';
import { SettingsPage } from './pages/Settings';
import Tokens from './pages/Tokens';

import './styles/index.css';

const rootEl = document.getElementById('root');
if (!rootEl) {
  throw new Error('#root not found');
}

createRoot(rootEl).render(
  <StrictMode>
    <BrowserRouter>
      <Routes>
        <Route path="/login" element={<LoginPage />} />
        <Route path="/register" element={<Register />} />
        <Route path="/forgot-password" element={<ForgotPassword />} />
        <Route element={<App />}>
          <Route index element={<OverviewPage />} />
          <Route path="/alerts" element={<AlertsPage />} />
          <Route path="/audit" element={<AuditPage />} />
          <Route path="/settings" element={<SettingsPage />} />
          <Route path="/health" element={<HealthPage />} />
          <Route path="/projects/:id/issues" element={<IssuesPage />} />
          <Route
            path="/projects/:id/issues/:issueId"
            element={<IssueDetail />}
          />
          <Route path="/projects/:id/events" element={<EventsPage />} />
          <Route path="/projects/:id/cert" element={<CertPage />} />
          <Route path="/projects" element={<Projects />} />
          <Route path="/members" element={<Members />} />
          <Route path="/projects/:id/tokens" element={<Tokens />} />
          <Route path="/projects/:id/push" element={<PushCredentials />} />
          <Route path="/projects/:id/push-sends" element={<PushSends />} />
          <Route path="/projects/:id/integrations" element={<Integrations />} />
          <Route path="/projects/:id/releases" element={<Releases />} />
          <Route path="/saas" element={<SaasAdmin />} />
          <Route path="/saved-views" element={<SavedViews />} />
          <Route path="/notifications" element={<Notifications />} />
          <Route path="/shortcuts" element={<Shortcuts />} />
          <Route path="/search" element={<Search />} />
          <Route path="/sessions" element={<Sessions />} />
          <Route path="/projects/:id/traces" element={<Traces />} />
          <Route
            path="/projects/:id/traces/:traceId"
            element={<TraceDetail />}
          />
          <Route path="/projects/:id/metrics" element={<Metrics />} />
          <Route path="/projects/:id/replays" element={<Replays />} />
          <Route path="/projects/:id/probes" element={<EndpointProbes />} />
          <Route
            path="/projects/:id/replays/:replayId"
            element={<ReplayDetail />}
          />
        </Route>
      </Routes>
    </BrowserRouter>
  </StrictMode>
);
