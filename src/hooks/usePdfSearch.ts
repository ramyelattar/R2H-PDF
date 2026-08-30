import { useCallback, useEffect, useRef, useState } from "react";
import { goToPage, searchQuery as searchQueryIpc, type SearchMatch } from "../lib/ipc";
import { useSearchNavigation } from "./useSearchNavigation";
import { isBackendPdfSession, sessionStateToTabPatch } from "../lib/pdfSession";
import type { AppendDiagnostic } from "../lib/diagnostics";
import type { DocumentTab } from "../types/shell";

interface UsePdfSearchDeps {
  activeTab: DocumentTab | null;
  updateTab: (tabId: string, patch: Partial<DocumentTab>) => void;
  appendDiagnostic: AppendDiagnostic;
}

export function usePdfSearch(deps: UsePdfSearchDeps) {
  const depsRef = useRef(deps);
  depsRef.current = deps;

  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<SearchMatch[]>([]);
  const [searchBusy, setSearchBusy] = useState(false);
  const [searchMessage, setSearchMessage] = useState<string | null>(null);
  const [searchCaseSensitive, setSearchCaseSensitive] = useState(false);
  const [searchWholeWords, setSearchWholeWords] = useState(false);
  const [searchUseRegex, setSearchUseRegex] = useState(false);

  const { activeIndex, next: nextMatch, previous: previousMatch, activeMatch } = useSearchNavigation(searchResults);

  const runSearch = useCallback(async () => {
    const { activeTab, appendDiagnostic } = depsRef.current;
    if (!isBackendPdfSession(activeTab)) {
      setSearchResults([]);
      setSearchMessage("Search is available only for opened backend PDF sessions.");
      return;
    }

    const query = searchQuery.trim();
    if (!query) {
      setSearchResults([]);
      setSearchMessage("Enter text to search the active document.");
      return;
    }

    setSearchBusy(true);
    setSearchMessage(null);
    const result = await searchQueryIpc({
      session_id: activeTab.id,
      query,
      scope: "AllPages",
      case_sensitive: searchCaseSensitive,
      whole_words: searchWholeWords,
      use_regex: searchUseRegex,
      max_results: 200,
    });
    setSearchBusy(false);

    if (!result.ok) {
      setSearchResults([]);
      setSearchMessage(result.error.message);
      appendDiagnostic({ level: "ERROR", source: "ipc", message: `Search failed: ${result.error.message}` });
      return;
    }

    setSearchResults(result.data.matches);
    setSearchMessage(
      result.data.total
        ? `${result.data.total} match${result.data.total === 1 ? "" : "es"}`
        : "No matches found.",
    );
  }, [searchQuery, searchCaseSensitive, searchWholeWords, searchUseRegex]);

  const jumpToSearchMatch = useCallback(async (match: SearchMatch) => {
    const { activeTab, updateTab, appendDiagnostic } = depsRef.current;
    if (!isBackendPdfSession(activeTab)) return;

    const moved = await goToPage(activeTab.id, match.page_index);
    if (!moved.ok) {
      appendDiagnostic({ level: "ERROR", source: "ipc", message: `Jump to search result failed: ${moved.error.message}` });
      return;
    }
    updateTab(activeTab.id, sessionStateToTabPatch(moved.data));
    appendDiagnostic({ level: "INFO", source: "state", message: `Search jump moved to page ${moved.data.current_page + 1}` });
  }, []);

  // Navigate to the page of the active search match whenever it changes.
  useEffect(() => {
    if (!activeMatch) return;
    const { activeTab, updateTab, appendDiagnostic } = depsRef.current;
    if (!activeTab || activeTab.kind !== "pdf" || !activeTab.id.startsWith("doc-session-")) return;
    if (activeTab.page - 1 === activeMatch.page_index) return;

    void goToPage(activeTab.id, activeMatch.page_index).then((moved) => {
      if (moved.ok) {
        updateTab(activeTab.id, sessionStateToTabPatch(moved.data));
        appendDiagnostic({ level: "INFO", source: "state", message: `Search navigation moved to page ${moved.data.current_page + 1}` });
      } else {
        appendDiagnostic({ level: "ERROR", source: "ipc", message: `Search navigation failed: ${moved.error.message}` });
      }
    });
  }, [activeMatch]);

  return {
    searchQuery,
    setSearchQuery,
    searchResults,
    searchBusy,
    searchMessage,
    searchCaseSensitive,
    setSearchCaseSensitive,
    searchWholeWords,
    setSearchWholeWords,
    searchUseRegex,
    setSearchUseRegex,
    activeIndex,
    nextMatch,
    previousMatch,
    runSearch,
    jumpToSearchMatch,
  };
}
