/**
 * Decode a base64 string to a `Uint8ClampedArray` of raw bytes.
 *
 * Implementation note (CRITICAL): we deliberately do NOT use
 * `fetch("data:...;base64,...")` here. The packaged Tauri webview enforces
 * the CSP from `tauri.conf.json`, whose `connect-src` directive does not
 * include `data:`. A previous implementation fell back to a data-URL fetch
 * for payloads >= 64 KB; under CSP that fetch is rejected, the surrounding
 * async render swallows the rejection, and the page silently never paints
 * (blank/white viewer with a stuck "Rendering page…" overlay).
 *
 * We use `atob` for all sizes. `String.fromCharCode(...binary)` is avoided
 * because it blows the call-stack on multi-megabyte payloads — instead we
 * walk the decoded string with `charCodeAt`.
 */
export function decodeBase64ToBytes(base64: string): Uint8ClampedArray {
  if (base64.length === 0) {
    return new Uint8ClampedArray(0);
  }
  const binary = atob(base64);
  const len = binary.length;
  const bytes = new Uint8ClampedArray(len);
  for (let i = 0; i < len; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}
