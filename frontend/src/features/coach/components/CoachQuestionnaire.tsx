import { useMemo, useState } from 'react';

import type { CoachQuestion } from '../types';

type CoachQuestionnaireProps = {
  questions: CoachQuestion[];
  onSubmit: (content: string) => Promise<boolean>;
};

type AnswerState = Record<string, string | undefined>;

type FreeTextState = Record<string, string>;

type QuestionnaireHeaderProps = {
  title: string;
  description: string;
};

type QuestionCardProps = {
  question: CoachQuestion;
  index: number;
  selectedAnswer: string | undefined;
  freeTextAnswer: string;
  onSelectAnswer: (questionId: string, answer: string) => void;
  onChangeFreeText: (questionId: string, value: string) => void;
};

type AnswerButtonProps = {
  answer: string;
  selected: boolean;
  onClick: () => void;
};

type SubmitActionsProps = {
  canSubmit: boolean;
  isSubmitting: boolean;
  onSubmit: () => void;
};

function buildQuestionnaireMessage(
  questions: CoachQuestion[],
  selectedAnswers: AnswerState,
  freeTextAnswers: FreeTextState,
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

function QuestionnaireHeader({ title, description }: QuestionnaireHeaderProps) {
  return (
    <div className="mb-4 flex items-center justify-between gap-3">
      <div>
        <p className="text-[11px] font-black uppercase tracking-[0.24em] text-cyan-200/75">{title}</p>
        <p className="mt-1 text-sm text-slate-300">{description}</p>
      </div>
    </div>
  );
}

function QuestionCard({
  question,
  index,
  selectedAnswer,
  freeTextAnswer,
  onSelectAnswer,
  onChangeFreeText,
}: QuestionCardProps) {
  const freeTextLabel = question.freeTextLabel ?? 'Additional details';

  return (
    <section className="rounded-[1.15rem] border border-white/8 bg-white/[0.04] p-4">
      <div className="mb-3 flex items-start gap-3">
        <div className="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-full border border-cyan-200/30 bg-cyan-300/10 text-[11px] font-black text-cyan-100">
          {index + 1}
        </div>
        <p className="text-sm font-semibold leading-6 text-white">{question.question}</p>
      </div>

      <div className="flex flex-wrap gap-2">
        {question.answers.map((answer) => (
          <AnswerButton
            key={answer}
            answer={answer}
            selected={selectedAnswer === answer}
            onClick={() => onSelectAnswer(question.id, answer)}
          />
        ))}
      </div>

      <textarea
        aria-label={freeTextLabel}
        className="mt-3 min-h-24 w-full resize-y rounded-2xl border border-white/10 bg-black/25 px-4 py-3 text-sm text-slate-100 outline-none transition placeholder:text-slate-500 focus:border-cyan-300/40"
        placeholder={question.freeTextLabel ?? 'Add any detail that matters here'}
        value={freeTextAnswer}
        onChange={(event) => onChangeFreeText(question.id, event.target.value)}
      />
    </section>
  );
}

function AnswerButton({ answer, selected, onClick }: AnswerButtonProps) {
  return (
    <button
      type="button"
      aria-pressed={selected}
      className={[
        'rounded-full border px-3.5 py-2 text-sm font-semibold transition',
        selected
          ? 'border-cyan-200/60 bg-cyan-300/20 text-cyan-50 shadow-[0_0_0_1px_rgba(103,232,249,0.18)]'
          : 'border-white/10 bg-white/[0.03] text-slate-200 hover:border-white/20 hover:bg-white/[0.08]',
      ].join(' ')}
      onClick={onClick}
    >
      {answer}
    </button>
  );
}

function SubmitActions({ canSubmit, isSubmitting, onSubmit }: SubmitActionsProps) {
  return (
    <div className="mt-4 flex justify-end">
      <button
        type="button"
        className="rounded-full border border-cyan-200/30 bg-cyan-300 px-4 py-2 text-sm font-black text-slate-950 transition hover:bg-cyan-200 disabled:cursor-not-allowed disabled:border-white/10 disabled:bg-white/10 disabled:text-slate-400"
        disabled={!canSubmit || isSubmitting}
        onClick={onSubmit}
      >
        {isSubmitting ? 'Sending answers...' : 'Send answers to coach'}
      </button>
    </div>
  );
}

export function CoachQuestionnaire({ questions, onSubmit }: CoachQuestionnaireProps) {
  const [selectedAnswers, setSelectedAnswers] = useState<AnswerState>({});
  const [freeTextAnswers, setFreeTextAnswers] = useState<FreeTextState>({});
  const [isSubmitting, setIsSubmitting] = useState(false);

  const canSubmit = useMemo(
    () => questions.every((question) => Boolean(selectedAnswers[question.id])),
    [questions, selectedAnswers],
  );

  function handleSelectAnswer(questionId: string, answer: string) {
    setSelectedAnswers((current) => ({
      ...current,
      [questionId]: answer,
    }));
  }

  function handleChangeFreeText(questionId: string, value: string) {
    setFreeTextAnswers((current) => ({
      ...current,
      [questionId]: value,
    }));
  }

  async function submitAnswers() {
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
      <QuestionnaireHeader
        title="Coach check-in"
        description="Lock in the important context without turning this into a dry form."
      />

      <div className="space-y-4">
        {questions.map((question, index) => (
          <QuestionCard
            key={question.id}
            question={question}
            index={index}
            selectedAnswer={selectedAnswers[question.id]}
            freeTextAnswer={freeTextAnswers[question.id] ?? ''}
            onSelectAnswer={handleSelectAnswer}
            onChangeFreeText={handleChangeFreeText}
          />
        ))}
      </div>

      <SubmitActions
        canSubmit={canSubmit}
        isSubmitting={isSubmitting}
        onSubmit={() => {
          void submitAnswers();
        }}
      />
    </div>
  );
}
