import { describe, it, expect } from "vitest";
import {
  remapObjectsAfterDeletePage,
  remapObjectsAfterInsertPage,
  remapObjectsAfterMovePage,
  remapObjectsAfterRotatePage,
} from "./pageObjectRemap";
import type { TextBoxObject } from "../pdf-editor/types";

function makeObj(id: string, pageIndex: number): TextBoxObject {
  return {
    id, sessionId: "s1", pageIndex, type: "textBox",
    rect: { x: 0, y: 0, width: 100, height: 40 },
    rotation: 0, zIndex: 1, locked: false, hidden: false,
    createdAt: 0, updatedAt: 0, metadata: {},
    text: "T", fontSize: 12, fontFamily: "Helvetica",
    color: "#000", backgroundColor: "#fff", borderColor: "#ccc",
  };
}

describe("remapObjectsAfterDeletePage", () => {
  it("removes objects on the deleted page", () => {
    const objects = [makeObj("a", 0), makeObj("b", 1), makeObj("c", 2)];
    const result = remapObjectsAfterDeletePage(objects, 1);
    expect(result.find((o) => o.id === "b")).toBeUndefined();
  });

  it("shifts objects after deleted page down by 1", () => {
    const objects = [makeObj("a", 0), makeObj("b", 1), makeObj("c", 2), makeObj("d", 3)];
    const result = remapObjectsAfterDeletePage(objects, 1);
    expect(result.find((o) => o.id === "a")!.pageIndex).toBe(0);
    expect(result.find((o) => o.id === "c")!.pageIndex).toBe(1);
    expect(result.find((o) => o.id === "d")!.pageIndex).toBe(2);
  });

  it("does not shift objects before deleted page", () => {
    const objects = [makeObj("a", 0), makeObj("b", 2)];
    const result = remapObjectsAfterDeletePage(objects, 2);
    expect(result.find((o) => o.id === "a")!.pageIndex).toBe(0);
  });
});

describe("remapObjectsAfterInsertPage", () => {
  it("shifts objects at insertion index up by 1", () => {
    const objects = [makeObj("a", 0), makeObj("b", 1), makeObj("c", 2)];
    const result = remapObjectsAfterInsertPage(objects, 1);
    expect(result.find((o) => o.id === "a")!.pageIndex).toBe(0);
    expect(result.find((o) => o.id === "b")!.pageIndex).toBe(2);
    expect(result.find((o) => o.id === "c")!.pageIndex).toBe(3);
  });

  it("shifts all objects when inserting at page 0", () => {
    const objects = [makeObj("a", 0), makeObj("b", 1)];
    const result = remapObjectsAfterInsertPage(objects, 0);
    expect(result.find((o) => o.id === "a")!.pageIndex).toBe(1);
    expect(result.find((o) => o.id === "b")!.pageIndex).toBe(2);
  });
});

describe("remapObjectsAfterMovePage", () => {
  it("moves page forward correctly", () => {
    // Pages: [0, 1, 2, 3]. Move page 1 → 3.
    // Objects on page 1 → page 3.
    // Objects on pages 2, 3 shift down by 1 → pages 1, 2.
    const objects = [makeObj("a", 1), makeObj("b", 2), makeObj("c", 3)];
    const result = remapObjectsAfterMovePage(objects, 1, 3);
    expect(result.find((o) => o.id === "a")!.pageIndex).toBe(3); // moved page
    expect(result.find((o) => o.id === "b")!.pageIndex).toBe(1); // shifted down
    expect(result.find((o) => o.id === "c")!.pageIndex).toBe(2); // shifted down
  });

  it("moves page backward correctly", () => {
    // Pages: [0, 1, 2, 3]. Move page 3 → 1.
    // Objects on page 3 → page 1.
    // Objects on pages 1, 2 shift up by 1 → pages 2, 3.
    const objects = [makeObj("a", 1), makeObj("b", 2), makeObj("c", 3)];
    const result = remapObjectsAfterMovePage(objects, 3, 1);
    expect(result.find((o) => o.id === "c")!.pageIndex).toBe(1); // moved page
    expect(result.find((o) => o.id === "a")!.pageIndex).toBe(2); // shifted up
    expect(result.find((o) => o.id === "b")!.pageIndex).toBe(3); // shifted up
  });

  it("no-op when from equals to", () => {
    const objects = [makeObj("a", 1)];
    const result = remapObjectsAfterMovePage(objects, 1, 1);
    expect(result[0].pageIndex).toBe(1);
  });
});

describe("remapObjectsAfterRotatePage", () => {
  it("keeps page index unchanged", () => {
    const objects = [makeObj("a", 2), makeObj("b", 2)];
    const result = remapObjectsAfterRotatePage(objects, 2, 90);
    expect(result[0].pageIndex).toBe(2);
    expect(result[1].pageIndex).toBe(2);
  });

  it("returns same objects (coordinates not rotated in Phase 5)", () => {
    const objects = [makeObj("a", 0)];
    const result = remapObjectsAfterRotatePage(objects, 0, 90);
    expect(result[0].rect).toEqual(objects[0].rect);
  });
});
