import { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';

import { useDialogFocusTrap } from '../../../lib/useDialogFocusTrap';
import { usePlannedRestDaysApi } from '../api/plannedRestDays';
import type { PlannedRestDay } from '../types';
import { PlannedRestDayFormActions } from './PlannedRestDayFormActions';
import { PlannedRestDayFormDialog } from './PlannedRestDayFormDialog';
import { PlannedRestDayFormFields } from './PlannedRestDayFormFields';
import {
  defaultPlannedRestDayDraft,
  isPlannedRestDayDraftValid,
  resolveDraftEndDate,
  type PlannedRestDayDraft,
} from './plannedRestDayFormTypes';

type PlannedRestDayFormProps = {
  entry: PlannedRestDay | null;
  onCancel: () => void;
  onSaved: () => void;
};

export function PlannedRestDayForm({ entry, onCancel, onSaved }: PlannedRestDayFormProps) {
  const { createPlannedRestDay, deletePlannedRestDay, updatePlannedRestDay } = usePlannedRestDaysApi();
  const { t } = useTranslation();
  const [draft, setDraft] = useState<PlannedRestDayDraft>(defaultPlannedRestDayDraft);
  const [isSaving, setIsSaving] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const dialogRef = useRef<HTMLElement | null>(null);
  const cancelButtonRef = useRef<HTMLButtonElement>(null);

  useDialogFocusTrap(true, dialogRef, cancelButtonRef);

  useEffect(() => {
    if (!entry) {
      setDraft(defaultPlannedRestDayDraft);
      setError(null);
      return;
    }

    const isSingle = entry.startDate === entry.endDate;
    setDraft({
      mode: isSingle ? 'single' : 'range',
      startDate: entry.startDate,
      endDate: entry.endDate,
      title: entry.title ?? '',
      note: entry.note ?? '',
    });
    setError(null);
  }, [entry]);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && !isSaving && !isDeleting) {
        onCancel();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [isDeleting, isSaving, onCancel]);

  const handleRequestClose = () => {
    if (isSaving || isDeleting) {
      return;
    }
    onCancel();
  };

  const isValid = isPlannedRestDayDraftValid(draft);

  const handleSave = async () => {
    const startDate = draft.startDate;
    const endDate = resolveDraftEndDate(draft);

    if (!isValid || !startDate || !endDate) {
      setError(t('plannedRestDays.form.dateRequired'));
      return;
    }

    setIsSaving(true);
    setError(null);

    try {
      const payload = {
        startDate,
        endDate,
        title: draft.title.trim() || null,
        note: draft.note.trim() || null,
      };

      if (entry) {
        await updatePlannedRestDay(entry.plannedRestDayId, payload);
      } else {
        await createPlannedRestDay(payload);
      }

      onSaved();
    } catch (err) {
      setError(err instanceof Error ? err.message : t('plannedRestDays.form.saveError'));
    } finally {
      setIsSaving(false);
    }
  };

  const handleDelete = async () => {
    if (!entry || !window.confirm(t('plannedRestDays.form.deleteConfirm'))) {
      return;
    }

    setIsDeleting(true);
    setError(null);

    try {
      await deletePlannedRestDay(entry.plannedRestDayId);
      onSaved();
    } catch (err) {
      setError(err instanceof Error ? err.message : t('plannedRestDays.form.deleteError'));
    } finally {
      setIsDeleting(false);
    }
  };

  return (
    <PlannedRestDayFormDialog
      title={entry ? t('plannedRestDays.editTitle') : t('plannedRestDays.addTitle')}
      dialogRef={dialogRef}
      onBackdropClick={handleRequestClose}
    >
      <PlannedRestDayFormFields draft={draft} error={error} onDraftChange={setDraft} />
      <PlannedRestDayFormActions
        cancelButtonRef={cancelButtonRef}
        isEditing={entry !== null}
        isValid={isValid}
        isSaving={isSaving}
        isDeleting={isDeleting}
        onCancel={handleRequestClose}
        onDelete={() => void handleDelete()}
        onSave={() => void handleSave()}
      />
    </PlannedRestDayFormDialog>
  );
}
