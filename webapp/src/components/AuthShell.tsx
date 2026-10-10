import type { ReactNode } from 'react';
import { AuthLayout } from '../../shared/site/AuthLayout';
import { useSite } from '../../shared/site/runtime';
import { PublicLayout } from './PublicLayout';
function Content({ children }: { children: ReactNode }) {
  const { t, image } = useSite();
  return <AuthLayout title={t('auth.title')} description={t('auth.subtitle')} art={<img className="ui-illustration ui-illustration-scene" src={image('observatory.png')} alt="" />}>{children}</AuthLayout>;
}
export function AuthShell({ children }: { children: ReactNode }) { return <PublicLayout><Content>{children}</Content></PublicLayout>; }
