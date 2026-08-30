import { describe, expect, it } from 'vitest';

import { getFontAssetFileName } from '../js/config/font-mappings';
import { resolveFontUrl } from '../js/utils/font-loader';

describe('font-loader', () => {
  it('uses the default public font URL when no offline font base URL is configured', () => {
    expect(resolveFontUrl('Noto Sans', {})).toBe(
      'https://rawcdn.githack.com/googlefonts/noto-fonts/ffebf8c1ee449e544955a7e813c54f9b73848eac/hinted/ttf/NotoSans/NotoSans-Regular.ttf'
    );
  });

  it('builds a self-hosted font URL when an OCR font base URL is configured', () => {
    expect(
      resolveFontUrl('Noto Naskh Arabic', {
        VITE_OCR_FONT_BASE_URL: 'https://internal.example.com/wasm/ocr/fonts/',
      })
    ).toBe(
      'https://internal.example.com/wasm/ocr/fonts/NotoNaskhArabic-Regular.ttf'
    );
  });

  it('derives the bundled font asset file name from the default font URL', () => {
    expect(getFontAssetFileName('Noto Sans SC')).toBe(
      'NotoSansCJKsc-Regular.otf'
    );
  });
});
