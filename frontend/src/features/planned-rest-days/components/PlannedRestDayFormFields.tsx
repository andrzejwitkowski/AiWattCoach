import type { Dispatch, SetStateAction } from 'react';
import { useTranslation } from 'react-i18next';

import {
  PlannedRestDayFormField,
  PlannedRestDayModeButton,
  plannedRestDayFormInputClassName,
} from './PlannedRestDayFormField';
import type { PlannedRestDayDraft } from './plannedRestDayFormTypes';

type PlannedRestDayFormFieldsProps = {
  draft: PlannedRestDayDraft;
  error: string | null;
  onDraftChange: Dispatch<SetStateAction<PlannedRestDayDraft>>;
};

export function PlannedRestDayFormFields({ draft, error, onDraftChange }: PlannedRestDayFormFieldsProps) {
  const { t } = useTranslation();
  const inputClassName = plannedRestDayFormInputClassName;

  return (
    <div className="mt-6 space-y-5">
      <PlannedRestDayFormField label={t('plannedRestDays.form.modeLabel')}>
        <div className="grid grid-cols-2 gap-2">
          <PlannedRestDayModeButton
            active={draft.mode === 'single'}
            label={t('plannedRestDays.form.singleDay')}
            onClick={() => onDraftChange((current) => ({
              ...current,
              mode: 'single',
              endDate: current.startDate,
            }))}
          />
          <PlannedRestDayModeButton
            active={draft.mode === 'range'}
            label={t('plannedRestDays.form.dateRange')}
            onClick={() => onDraftChange((current) => ({ ...current, mode: 'range' }))}
          />
        </div>
      </PlannedRestDayFormField>

      <PlannedRestDayFormField
        label={draft.mode === 'single' ? t('plannedRestDays.form.date') : t('plannedRestDays.form.startDate')}
      >
        <input
          type="date"
          value={draft.startDate}
          onChange={(event) => {
            const startDate = event.target.value;
            onDraftChange((current) => ({
              ...current,
              startDate,
              endDate: current.mode === 'single' ? startDate : current.endDate,
            }));
          }}
          className={inputClassName}
        />
      </PlannedRestDayFormField>

      {draft.mode === 'range' ? (
        <PlannedRestDayFormField label={t('plannedRestDays.form.endDate')}>
          <input
            type="date"
            value={draft.endDate}
            min={draft.startDate || undefined}
            onChange={(event) => onDraftChange((current) => ({ ...current, endDate: event.target.value }))}
            className={inputClassName}
          />
        </PlannedRestDayFormField>
      ) : null}

      <PlannedRestDayFormField label={t('plannedRestDays.form.title')}>
        <input
          type="text"
          value={draft.title}
          maxLength={120}
          onChange={(event) => onDraftChange((current) => ({ ...current, title: event.target.value }))}
          placeholder={t('plannedRestDays.form.titlePlaceholder')}
          className={inputClassName}
        />
      </PlannedRestDayFormField>

      <PlannedRestDayFormField label={t('plannedRestDays.form.note')}>
        <textarea
          value={draft.note}
          maxLength={2000}
          rows={4}
          onChange={(event) => onDraftChange((current) => ({ ...current, note: event.target.value }))}
          placeholder={t('plannedRestDays.form.notePlaceholder')}
          className={inputClassName}
        />
      </PlannedRestDayFormField>

      {error ? <p className="text-sm text-rose-300">{error}</p> : null}
    </div>
  );
}
