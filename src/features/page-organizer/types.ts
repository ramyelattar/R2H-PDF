export interface PageInfo {
  index: number;
  rotation: number;
  hasOverlayObjects: boolean;
}

export type PageAction =
  | { type: "rotateCW"; pageIndex: number }
  | { type: "rotateCCW"; pageIndex: number }
  | { type: "delete"; pageIndex: number }
  | { type: "insertBlank"; atIndex: number; widthPts: number; heightPts: number }
  | { type: "move"; fromIndex: number; toIndex: number }
  | { type: "duplicate"; pageIndex: number };
