/**
 * Pure utility functions for remapping editor overlay objects
 * when page operations change page indices.
 */

import type { EditorObject } from "../pdf-editor/types";

/**
 * After deleting a page:
 * - Remove all objects on the deleted page.
 * - Shift objects on pages after the deleted page down by 1.
 */
export function remapObjectsAfterDeletePage(
  objects: EditorObject[],
  deletedPageIndex: number,
): EditorObject[] {
  return objects
    .filter((obj) => obj.pageIndex !== deletedPageIndex)
    .map((obj) => {
      if (obj.pageIndex > deletedPageIndex) {
        return { ...obj, pageIndex: obj.pageIndex - 1 };
      }
      return obj;
    });
}

/**
 * After inserting a blank page at `insertPageIndex`:
 * - Shift objects at or after the insertion index up by 1.
 */
export function remapObjectsAfterInsertPage(
  objects: EditorObject[],
  insertPageIndex: number,
): EditorObject[] {
  return objects.map((obj) => {
    if (obj.pageIndex >= insertPageIndex) {
      return { ...obj, pageIndex: obj.pageIndex + 1 };
    }
    return obj;
  });
}

/**
 * After moving a page from `fromIndex` to `toIndex`:
 * - Objects on the moved page get the new index.
 * - Objects in the affected range shift by ±1.
 */
export function remapObjectsAfterMovePage(
  objects: EditorObject[],
  fromIndex: number,
  toIndex: number,
): EditorObject[] {
  if (fromIndex === toIndex) return objects;

  return objects.map((obj) => {
    if (obj.pageIndex === fromIndex) {
      // Object was on the moved page → goes to toIndex.
      return { ...obj, pageIndex: toIndex };
    }

    if (fromIndex < toIndex) {
      // Page moved forward: pages in (from, to] shift down by 1.
      if (obj.pageIndex > fromIndex && obj.pageIndex <= toIndex) {
        return { ...obj, pageIndex: obj.pageIndex - 1 };
      }
    } else {
      // Page moved backward: pages in [to, from) shift up by 1.
      if (obj.pageIndex >= toIndex && obj.pageIndex < fromIndex) {
        return { ...obj, pageIndex: obj.pageIndex + 1 };
      }
    }

    return obj;
  });
}

/**
 * After rotating a page:
 * - Objects stay on the same page index.
 * - Coordinate rotation is deferred (Phase 5 limitation).
 * - Returns objects unchanged.
 */
export function remapObjectsAfterRotatePage(
  objects: EditorObject[],
  _pageIndex: number, // eslint-disable-line @typescript-eslint/no-unused-vars
  _rotation: number, // eslint-disable-line @typescript-eslint/no-unused-vars
): EditorObject[] {
  // Phase 5 limitation: object coordinates are not rotated.
  // Objects remain at their PDF-space positions. Visual alignment
  // may be off after rotation until coordinate transform is added.
  return objects;
}
