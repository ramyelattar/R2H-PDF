/**
 * Phase UI-3 — Premium empty state shown when no document is open.
 *
 * Every action here is real: the primary CTA opens a real file dialog,
 * "Validate Local AI" jumps to the Models tab in the inspector, and each
 * workflow card both opens the file dialog and remembers which inspector
 * tab to switch to once a PDF is loaded.
 *
 * The component never claims to do anything it can't — when an action
 * needs a document, the card explains that need rather than rendering a
 * fake/dead button.
 */
import type { InspectorTab } from "./RightInspector";

interface WorkflowCard {
  id: string;
  testId: string;
  icon: string;
  title: string;
  desc: string;
  tab: InspectorTab;
}

const CARDS: ReadonlyArray<WorkflowCard> = [
  {
    id: "edit",
    testId: "welcome-card-edit",
    icon: "✎",
    title: "Edit PDF",
    desc: "Replace text natively or with a safe visual layer.",
    tab: "edit",
  },
  {
    id: "ai",
    testId: "welcome-card-ai",
    icon: "✶",
    title: "Review with AI",
    desc: "Summarize, ask questions, and surface action items.",
    tab: "ai",
  },
  {
    id: "export",
    testId: "welcome-card-export",
    icon: "⤓",
    title: "Export report",
    desc: "Bundle edits, redlines, and AI notes into a deliverable.",
    tab: "export",
  },
];

export interface WelcomeScreenProps {
  /** Triggered when the user wants to open a PDF (primary CTA + cards). */
  onOpenFile: () => void;
  /** Opens the separate optional BentoPDF utility surface. */
  onOpenBentoPdf: () => void;
  /** Switches the right inspector to a specific tab (used by Validate Local
   *  AI to jump to Models, and by workflow cards to set a pending tab). */
  onShowInspectorTab: (tab: InspectorTab) => void;
  /**
   * Optional — when a workflow card is clicked, we record which tab the
   * user wanted so the parent can switch the inspector to it after the
   * document opens. Implementation detail of the parent.
   */
  onPickWorkflow?: (tab: InspectorTab) => void;
}

export const WelcomeScreen = ({
  onOpenFile,
  onOpenBentoPdf,
  onShowInspectorTab,
  onPickWorkflow,
}: WelcomeScreenProps) => {
  const handleCardClick = (card: WorkflowCard) => {
    onPickWorkflow?.(card.tab);
    onOpenFile();
  };

  return (
    <div className="welcome-screen" data-testid="welcome-screen">
      <div className="welcome-screen__inner">
        <header className="welcome-hero">
          <span className="welcome-hero__eyebrow">R2H PDF</span>
          <h1 className="welcome-hero__title">R2H PDF AI Workstation</h1>
          <p className="welcome-hero__subtitle">
            Edit, review, compare, OCR, and analyze PDFs locally — without
            sending documents to the cloud.
          </p>
          <div className="welcome-cta-row">
            <button
              className="btn btn--primary btn--lg"
              onClick={onOpenFile}
              data-testid="welcome-open-pdf"
            >
              Open PDF
            </button>
            <button
              className="btn btn--secondary btn--lg"
              onClick={onOpenBentoPdf}
              data-testid="welcome-open-bentopdf"
              title="Open the local BentoPDF toolkit in a dedicated window"
            >
              BentoPDF Tools
            </button>            <button
              className="btn btn--secondary btn--lg"
              onClick={() => onShowInspectorTab("models")}
              data-testid="welcome-validate-ai"
              title="Verify the local AI model is installed and ready"
            >
              Validate Local AI
            </button>
          </div>
        </header>

        <section aria-labelledby="welcome-workflows-heading">
          <div className="section-header">
            <h2
              id="welcome-workflows-heading"
              className="section-header__title"
            >
              Workflows
            </h2>
            <span className="section-header__hint">
              Pick one — we&apos;ll open a PDF and land you on the right panel
            </span>
          </div>
          <div className="welcome-cards" data-testid="welcome-cards">
            {CARDS.map((card) => (
              <button
                key={card.id}
                type="button"
                className="welcome-card"
                onClick={() => handleCardClick(card)}
                data-testid={card.testId}
                aria-label={`${card.title}: ${card.desc}`}
              >
                <span className="welcome-card__icon" aria-hidden="true">
                  {card.icon}
                </span>
                <h3 className="welcome-card__title">{card.title}</h3>
                <p className="welcome-card__desc">{card.desc}</p>
              </button>
            ))}
          </div>
        </section>

        <footer className="welcome-meta" data-testid="welcome-meta">
          <span>
            <span className="welcome-meta__dot" aria-hidden="true" />
            {" "}Local-first · works offline
          </span>
          <span>·</span>
          <span>Native PDF editing</span>
          <span>·</span>
          <span>OCR · AI · Compare · Forms · Sign</span>
        </footer>
      </div>
    </div>
  );
};
