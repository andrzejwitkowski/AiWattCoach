import { useMemo, useState } from 'react';

import type { CoachQuestion } from '../types';

type CoachQuestionnaireProps = {
  questions: CoachQuestion[];
  onSubmit: (content: string) => Promise<boolean>;
};

function buildQuestionnaireMessage(
  questions: CoachQuestion[],
  selectedAnswers: Record<string, string | undefined>,
  freeTextAnswers: Record<string, string>,
): string {
  const lines = ['Answers to the coach questionnaire:'];

  questions.forEach((question, index) => {
    const selected = selectedAnswers[question.id];
    const freeText = freeTextAnswers[question.id]?.trim();

    lines.push(`${index + 1}. ${question.question}`);
    if (selected) {
      lines.push(`Selected: ${selected}`);
    }
    if (freeText) {
      lines.push(`Details: ${freeText}`);
    }
  });

  return lines.join('\n');
}

export function CoachQuestionnaire({ questions, onSubmit }: CoachQuestionnaireProps) {
  const [selectedAnswers, setSelectedAnswers] = useState<Record<string, string | undefined>>({});
  const [freeTextAnswers, setFreeTextAnswers] = useState<Record<string, string>>({});
  const [isSubmitting, setIsSubmitting] = useState(false);

  const canSubmit = useMemo(
    () => questions.every((question) => Boolean(selectedAnswers[question.id])),
    [questions, selectedAnswers],
  );

  async function handleSubmit() {
    if (!canSubmit || isSubmitting) {
      return;
    }

    setIsSubmitting(true);
    const content = buildQuestionnaireMessage(questions, selectedAnswers, freeTextAnswers);
    try {
      const sent = await onSubmit(content);
      if (!sent) {
        setIsSubmitting(false);
      }
    } catch {
      setIsSubmitting(false);
    }
  }

  return (
    <div className="mt-5 rounded-[1.35rem] border border-cyan-300/15 bg-[linear-gradient(180deg,rgba(9,16,22,0.92),rgba(13,22,29,0.82))] p-4 shadow-[0_18px_40px_rgba(0,0,0,0.18)]">
      <div className="mb-4 flex items-center justify-between gap-3">
        <div>
          <p className="text-[11px] font-black uppercase tracking-[0.24em] text-cyan-200/75">Coach check-in</p>
          <p className="mt-1 text-sm text-slate-300">Lock in the important context without turning this into a dry form.</p>
        </div>
      </div>

      <div className="space-y-4">
        {questions.map((question, index) => (
          <section key={question.id} className="rounded-[1.15rem] border border-white/8 bg-white/[0.04] p-4">
            <div className="mb-3 flex items-start gap-3">
              <div className="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-full border border-cyan-200/30 bg-cyan-300/10 text-[11px] font-black text-cyan-100">
                {index + 1}
              </div>
              <p className="text-sm font-semibold leading-6 text-white">{question.question}</p>
            </div>

            <div className="flex flex-wrap gap-2">
              {question.answers.map((answer) => {
                const selected = selectedAnswers[question.id] === answer;

                return (
                  <button
                    key={answer}
                    type="button"
                    aria-pressed={selected}
                    className={[
                      'rounded-full border px-3.5 py-2 text-sm font-semibold transition',
                      selected
                        ? 'border-cyan-200/60 bg-cyan-300/20 text-cyan-50 shadow-[0_0_0_1px_rgba(103,232,249,0.18)]'
                        : 'border-white/10 bg-white/[0.03] text-slate-200 hover:border-white/20 hover:bg-white/[0.08]',
                    ].join(' ')}
                    onClick={() => {
                      setSelectedAnswers((current) => ({
                        ...current,
                        [question.id]: answer,
                      }));
                    }}
                  >
                    {answer}
                  </button>
                );
              })}
            </div>

            <textarea
              aria-label={question.freeTextLabel ?? 'Additional details'}
              className="mt-3 min-h-24 w-full resize-y rounded-2xl border border-white/10 bg-black/25 px-4 py-3 text-sm text-slate-100 outline-none transition placeholder:text-slate-500 focus:border-cyan-300/40"
              placeholder={question.freeTextLabel ?? 'Add any detail that matters here'}
              value={freeTextAnswers[question.id] ?? ''}
              onChange={(event) => {
                const value = event.target.value;
                setFreeTextAnswers((current) => ({
                  ...current,
                  [question.id]: value,
                }));
              }}
            />
          </section>
        ))}
      </div>

      <div className="mt-4 flex justify-end">
        <button
          type="button"
          className="rounded-full border border-cyan-200/30 bg-cyan-300 px-4 py-2 text-sm font-black text-slate-950 transition hover:bg-cyan-200 disabled:cursor-not-allowed disabled:border-white/10 disabled:bg-white/10 disabled:text-slate-400"
          disabled={!canSubmit || isSubmitting}
          onClick={() => {
            void handleSubmit();
          }}
        >
          {isSubmitting ? 'Sending answers...' : 'Send answers to coach'}
        </button>
      </div>
    </div>
  );
}
