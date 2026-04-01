import { cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import '../../../i18n';
import { WorkoutActionButtons } from './WorkoutActionButtons';

afterEach(() => {
  cleanup();
});

describe('WorkoutActionButtons', () => {
  it('disables save and enables edit when summary is already saved', () => {
    const onSave = vi.fn();
    const onEdit = vi.fn();

    render(
      <WorkoutActionButtons
        disabled={false}
        isSaving={false}
        isEditing={false}
        onSave={onSave}
        onEdit={onEdit}
      />,
    );

    expect(screen.getByRole('button', { name: /save as workout summary/i })).toBeDisabled();
    expect(screen.getByRole('button', { name: /edit/i })).toBeEnabled();

    fireEvent.click(screen.getByRole('button', { name: /edit/i }));
    expect(onEdit).toHaveBeenCalledTimes(1);
  });

  it('disables save until rpe is selected', () => {
    const onSave = vi.fn();

    render(
      <WorkoutActionButtons
        disabled={false}
        isSaving={false}
        isEditing={true}
        canSave={false}
        onSave={onSave}
        onEdit={() => undefined}
      />,
    );

    const saveButton = screen.getByRole('button', { name: /save as workout summary/i });
    expect(saveButton).toBeDisabled();

    fireEvent.click(saveButton);
    expect(onSave).not.toHaveBeenCalled();
  });
});
