import { lazy, Suspense } from 'react';
import { Navigate, Route, Routes } from 'react-router-dom';

import { App } from './App';
import { useT } from './i18n';

const ForgotPassword = lazy(() => import('./pages/ForgotPassword'));
const Instruments = lazy(() => import('./pages/Instruments'));
const ProjectsPage = lazy(() => import('./pages/Projects'));
const TriageView = lazy(() => import('./pages/TriageView'));
const Docs = lazy(() => import('./pages/Docs'));
const LoginPage = lazy(() => import('./pages/Login').then((page) => ({ default: page.LoginPage })));
const PushPage = lazy(() => import('./pages/Push'));
const Releases = lazy(() => import('./pages/Releases'));
const ResetPassword = lazy(() => import('./pages/ResetPassword'));
const Transfers = lazy(() => import('./pages/Transfers'));
const SelfHosted = lazy(() => import('./pages/SelfHosted'));
const Settings = lazy(() => import('./pages/Settings'));
const Setup = lazy(() => import('./pages/Setup'));
const Welcome = lazy(() => import('./pages/Welcome'));
const Demo = lazy(() => import('./pages/Welcome').then((page) => ({ default: page.Demo })));
const Tasks = lazy(() => import('./pages/Tasks'));
const InboxPage = lazy(() => import('./pages/Inbox'));
const MonitorsPage = lazy(() => import('./pages/Monitors'));
const IntegrationsPage = lazy(() => import('./pages/Integrations'));
const Pricing = lazy(() => import('./pages/Pricing'));

const CloudConsole = lazy(() => import('./pages/CloudConsole'));
const CloudAuth = lazy(() => import('./pages/CloudAuth'));
const CloudAccess = lazy(() => import('./pages/CloudAccess'));

export function AppRoutes() {
  return (
    <Suspense fallback={<RouteLoading />}>
      <Routes>
        {/* Outside <App>: a reader arriving from npm or GitHub has
            no account here, and the docs are public. */}
        {['login','register','forgot-password','verify'].map(mode => <Route key={mode} path={`/cloud/${mode}`} element={<CloudAuth key={mode}/>}/>)}
        <Route path="/cloud" element={<CloudConsole/>}/>
        <Route path="/cloud/workspaces/:workspaceId" element={<CloudConsole/>}/>
        <Route path="/cloud/account" element={<CloudConsole/>}/>
        <Route path="/cloud/access" element={<CloudAccess/>}/>
        <Route path="/docs/*" element={<Docs />} />
        <Route path="/welcome" element={<Welcome />} />
        <Route path="/demo" element={<Demo />} />
        <Route path="/pricing" element={<Pricing />} />
        <Route path="/login" element={<LoginPage />} />
        <Route path="/forgot-password" element={<ForgotPassword />} />
        <Route path="/reset-password" element={<ResetPassword />} />
        <Route element={<App />}>
          <Route path="/" element={<TriageView />} />
          <Route path="/issues" element={<TriageView />} />
          <Route path="/issues/:issueId" element={<TriageView />} />
          <Route path="/projects" element={<ProjectsPage />} />
          <Route path="/setup" element={<Setup />} />
          <Route path="/instruments" element={<Instruments />} />
          <Route path="/releases" element={<Releases />} />
          <Route path="/tasks" element={<Tasks />} />
          <Route path="/inbox" element={<InboxPage />} />
          <Route path="/status" element={<MonitorsPage />} />
          <Route path="/integrations" element={<IntegrationsPage />} />
          <Route path="/push" element={<PushPage />} />
          <Route path="/transfers" element={<Transfers />} />
          <Route path="/self-hosted" element={<SelfHosted />} />
          <Route path="/settings" element={<Settings />} />
          <Route path="/settings/:section" element={<Settings />} />
          <Route path="*" element={<Navigate to="/" replace />} />
        </Route>
      </Routes>
    </Suspense>
  );
}

function RouteLoading() {
  const t = useT();
  return <div role="status" className="grid min-h-48 place-items-center p-8 text-sm text-fg-muted">{t('shell.loading')}</div>;
}
