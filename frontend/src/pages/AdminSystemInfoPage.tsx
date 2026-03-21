import { useEffect, useState } from 'react';

import { loadAdminSystemInfo, type AdminSystemInfo } from '../features/auth/api/auth';
import { ApiConfigCard } from '../features/admin-system-info/components/ApiConfigCard';
import { BackendStatusCard } from '../features/admin-system-info/components/BackendStatusCard';
import { ProtectedSystemInfoCard } from '../features/admin-system-info/components/ProtectedSystemInfoCard';
import { SystemStatusHero } from '../features/admin-system-info/components/SystemStatusHero';
import type { BackendStatus } from '../lib/api/system';

type AdminSystemInfoPageProps = {
  apiBaseUrl: string;
  apiBaseUrlLabel: string;
  backendStatus: BackendStatus;
  onRefresh: () => void;
  isRefreshing: boolean;
};

export function AdminSystemInfoPage({
  apiBaseUrl,
  apiBaseUrlLabel,
  backendStatus,
  onRefresh,
  isRefreshing
}: AdminSystemInfoPageProps) {
  const [systemInfo, setSystemInfo] = useState<AdminSystemInfo | null>(null);

  useEffect(() => {
    let cancelled = false;

    void loadAdminSystemInfo(apiBaseUrl)
      .then((payload) => {
        if (!cancelled) {
          setSystemInfo(payload);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setSystemInfo(null);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [apiBaseUrl]);

  return (
    <section className="grid gap-6 xl:grid-cols-[minmax(0,1.3fr)_minmax(18rem,1fr)]">
      <div className="space-y-6">
        <SystemStatusHero />
        <ProtectedSystemInfoCard systemInfo={systemInfo} />
        <ApiConfigCard
          apiBaseUrlLabel={apiBaseUrlLabel}
          backendStatus={backendStatus}
          isRefreshing={isRefreshing}
          onRefresh={onRefresh}
        />
      </div>
      <BackendStatusCard backendStatus={backendStatus} />
    </section>
  );
}
