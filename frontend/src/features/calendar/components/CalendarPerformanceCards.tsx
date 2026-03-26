import { Activity, Bolt, Gauge, TrendingUp } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import type { ReactNode } from 'react';

export function CalendarPerformanceCards() {
  const { t } = useTranslation();

  return (
    <div className="grid gap-6 border-t border-white/5 pt-8 md:grid-cols-2 xl:grid-cols-4">
      <PlaceholderCard
        title={t('calendar.fitness')}
        detail={t('calendar.comingSoon')}
        description={t('calendar.performanceInsightsPlaceholder')}
        accent="text-[#d2ff9a]"
        barClass="bg-[#d2ff9a]"
        icon={<TrendingUp className="text-[#d2ff9a]" size={20} />}
      />
      <PlaceholderCard
        title={t('calendar.fatigue')}
        detail={t('calendar.comingSoon')}
        description={t('calendar.performanceInsightsPlaceholder')}
        accent="text-[#ff7351]"
        barClass="bg-[#ff7351]"
        icon={<Bolt className="text-[#ff7351]" size={20} />}
      />
      <PlaceholderCard
        title={t('calendar.form')}
        detail={t('calendar.comingSoon')}
        description={t('calendar.performanceInsightsPlaceholder')}
        accent="text-[#00e3fd]"
        barClass="bg-[#00e3fd]"
        icon={<Gauge className="text-[#00e3fd]" size={20} />}
      />
      <div className="relative overflow-hidden rounded-xl border border-white/5 bg-[#1d2024] p-5">
        <div className="absolute inset-0 bg-[radial-gradient(circle_at_top_right,rgba(210,255,154,0.18),transparent_55%)]" />
        <div className="relative flex h-full min-h-[140px] flex-col justify-end gap-3">
          <Activity className="text-slate-500" size={22} />
          <p className="text-[10px] font-black uppercase tracking-widest text-slate-500">{t('calendar.comingSoon')}</p>
          <p className="max-w-[18rem] text-sm font-semibold text-[#f9f9fd]">{t('calendar.performanceInsightsPlaceholder')}</p>
        </div>
      </div>
    </div>
  );
}

type CardProps = {
  title: string;
  detail: string;
  description: string;
  accent: string;
  barClass: string;
  icon: ReactNode;
};

function PlaceholderCard({ title, detail, description, accent, barClass, icon }: CardProps) {
  return (
    <div className="rounded-xl border border-white/5 bg-[#1d2024] p-5">
      <div className="mb-4 flex items-center justify-between">
        <span className="text-[10px] font-black uppercase tracking-widest text-slate-400">{title}</span>
        {icon}
      </div>
      <p className={`text-[10px] font-bold uppercase tracking-widest ${accent}`}>{detail}</p>
      <p className="mt-2 max-w-[18rem] text-sm font-semibold text-[#f9f9fd]">{description}</p>
      <div className="mt-4 h-1 w-full overflow-hidden rounded-full bg-white/5">
        <div className={`h-full ${barClass} opacity-40`} style={{ width: '100%' }} />
      </div>
    </div>
  );
}
