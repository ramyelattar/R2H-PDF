import { useCallback, useEffect, useState } from "react";
import {
  pdfGetPageFontRegistry,
  type ContentObject,
  type FontResourceInfo,
} from "../../lib/ipc";
import type { InlineMethodPreview } from "./InlineTextEditor";

/**
 * Phase 29E — frontend mirror of `classify_text_edit_strategy`. Loads the
 * page font registry once and returns a synchronous strategy resolver
 * the inline editor uses to render the right method badge before the
 * user applies an edit.
 *
 * This deliberately mirrors the backend rules; the backend remains the
 * authoritative gate.
 */
export function useTextEditStrategy(sessionId: string, pageIndex: number) {
  const [registry, setRegistry] = useState<FontResourceInfo[]>([]);
  const [registryLoaded, setRegistryLoaded] = useState(false);

  useEffect(() => {
    let cancelled = false;
    if (!sessionId) return;
    (async () => {
      const res = await pdfGetPageFontRegistry(sessionId, pageIndex);
      if (cancelled) return;
      if (res.ok) {
        setRegistry(res.data.fonts);
      } else {
        setRegistry([]);
      }
      setRegistryLoaded(true);
    })();
    return () => {
      cancelled = true;
    };
  }, [sessionId, pageIndex]);

  const resolve = useCallback(
    (obj: ContentObject | null, replacement: string): InlineMethodPreview => {
      if (!obj) {
        return {
          strategy: "read_only",
          fontPreserved: false,
          reasons: ["No content object selected."],
        };
      }
      if (obj.editable_level === "read_only") {
        return {
          strategy: "read_only",
          fontPreserved: false,
          reasons: ["This text object is marked read-only by the analysis layer."],
        };
      }
      const fontName = obj.text_info?.font_name;
      const info = fontName ? registry.find((f) => f.resource_name === fontName) : undefined;
      const hasNonAscii = [...replacement].some((c) => c.charCodeAt(0) > 0x7f);
      const outsideLatin1 = [...replacement].some((c) => c.charCodeAt(0) > 0xff);
      const hasCjk = [...replacement].some((c) => {
        const cp = c.charCodeAt(0);
        return (
          (cp >= 0x4e00 && cp <= 0x9fff) ||
          (cp >= 0x3040 && cp <= 0x30ff) ||
          (cp >= 0xac00 && cp <= 0xd7af)
        );
      });
      const hasArabic = [...replacement].some((c) => {
        const cp = c.charCodeAt(0);
        return (cp >= 0x0600 && cp <= 0x06ff) || (cp >= 0xfb50 && cp <= 0xfdff);
      });

      if (info) {
        if (info.is_type3) {
          return {
            strategy: "read_only",
            fontPreserved: false,
            reasons: ["Type3 font — native edit is not available for this object."],
          };
        }
        if (hasCjk) {
          return {
            strategy: "safe_visual_replacement",
            fontPreserved: false,
            reasons: ["CJK shaping is not supported for native text editing."],
          };
        }
        if (hasArabic) {
          return {
            strategy: "safe_visual_replacement",
            fontPreserved: false,
            reasons: ["Arabic shaping is not supported for native text editing."],
          };
        }
        if (info.is_subset) {
          return {
            strategy: "safe_visual_replacement",
            fontPreserved: false,
            reasons: ["Subset font detected — replacement glyphs may not be embedded; using visual replacement."],
          };
        }
        if (info.is_type0 || info.encoding_kind === "identity_h" || info.encoding_kind === "identity_v") {
          return {
            strategy: "safe_visual_replacement",
            fontPreserved: false,
            reasons: ["Type0 / Identity-H font — native CID reverse mapping not implemented."],
          };
        }
        if (!hasNonAscii) {
          if (info.can_native_edit_ascii) {
            return { strategy: "native_in_place", fontPreserved: true, reasons: [] };
          }
          return {
            strategy: "safe_visual_replacement",
            fontPreserved: false,
            reasons: [`Font ${info.resource_name} cannot encode ASCII (encoding ${info.encoding_kind}).`],
          };
        }
        if (hasNonAscii && !outsideLatin1) {
          if (info.can_native_edit_latin1) {
            return {
              strategy: "native_in_place",
              fontPreserved: true,
              reasons: ["Non-ASCII Latin-1 replacement encoded via WinAnsi/MacRoman."],
            };
          }
          return {
            strategy: "safe_visual_replacement",
            fontPreserved: false,
            reasons: [`Font ${info.resource_name} (encoding ${info.encoding_kind}) cannot natively encode Latin-1.`],
          };
        }
        return {
          strategy: "safe_visual_replacement",
          fontPreserved: false,
          reasons: ["Replacement contains characters outside Latin-1 and the font does not support a wider encoding."],
        };
      }

      // No registry entry — conservative fallback. Native editing is only
      // allowed when the font/encoding target is known and can be verified.
      if (hasCjk || hasArabic) {
        return {
          strategy: "safe_visual_replacement",
          fontPreserved: false,
          reasons: ["Shaping required — visual replacement only."],
        };
      }
      if (!hasNonAscii) {
        return {
          strategy: "safe_visual_replacement",
          fontPreserved: false,
          reasons: ["Font not found in page registry — native edit cannot be verified; using visual replacement."],
        };
      }
      return {
        strategy: "safe_visual_replacement",
        fontPreserved: false,
        reasons: ["Non-ASCII replacement with unknown font — falling back to visual replacement."],
      };
    },
    [registry],
  );

  const fontInfoFor = useCallback(
    (obj: ContentObject | null): FontResourceInfo | undefined => {
      if (!obj?.text_info) return undefined;
      return registry.find((f) => f.resource_name === obj.text_info?.font_name);
    },
    [registry],
  );

  return { registry, registryLoaded, resolve, fontInfoFor };
}
