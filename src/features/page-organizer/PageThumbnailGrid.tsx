import { useMemo } from "react";
import { PageThumbnailItem } from "./PageThumbnailItem";
import type { EditorObject } from "../pdf-editor/types";

interface PageThumbnailGridProps {
  sessionId: string;
  totalPages: number;
  selectedPages: number[];
  editorObjects: Map<string, EditorObject>;
  onSelectPage: (pageIndex: number) => void;
}

/**
 * Grid of page thumbnails for the Page Organizer.
 * Uses lazy rendering — only renders thumbnails that are in the DOM.
 * For very large documents (>100 pages), consider adding virtualization.
 */
export const PageThumbnailGrid = ({
  sessionId,
  totalPages,
  selectedPages,
  editorObjects,
  onSelectPage,
}: PageThumbnailGridProps) => {
  // Compute which pages have overlay objects.
  const pagesWithObjects = useMemo(() => {
    const set = new Set<number>();
    for (const obj of editorObjects.values()) {
      if (obj.sessionId === sessionId) set.add(obj.pageIndex);
    }
    return set;
  }, [editorObjects, sessionId]);

  const pages = useMemo(() => {
    return Array.from({ length: totalPages }, (_, i) => i);
  }, [totalPages]);

  return (
    <div className="page-thumb-grid">
      {pages.map((pageIndex) => (
        <PageThumbnailItem
          key={pageIndex}
          sessionId={sessionId}
          pageIndex={pageIndex}
          rotation={0}
          isSelected={selectedPages.includes(pageIndex)}
          hasOverlayObjects={pagesWithObjects.has(pageIndex)}
          onClick={onSelectPage}
        />
      ))}
    </div>
  );
};
