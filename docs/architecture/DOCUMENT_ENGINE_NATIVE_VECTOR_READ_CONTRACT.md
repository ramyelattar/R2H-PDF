# DocumentEngine Native Vector Read Contract

Status: landed in R2H-PDF 2.1.0 host protocol `r2h-documentengine-host-v1`.

This document records the additive product boundary only. Drawing Intelligence
semantics remain owned by the consumer phase contracts.

## Capability and operation

`HELLO` continues to advertise `nativeText: true` and `pageRender: true` and
now additionally advertises `vectorRead: true`. Existing clients do not need
to request or understand the new capability.

The read-only operation is:

```text
EXTRACT_NATIVE_VECTORS
```

It uses the existing opaque `documentSessionId` and zero-based `pageIndex`:

```json
{
  "documentSessionId": "opaque-host-session",
  "pageIndex": 0,
  "profileVersion": "documentengine-native-vector-read-v1"
}
```

The operation accepts no source path, executable, model path, PID, or display
coordinate input. A missing handle/profile is rejected as
`VECTOR_INPUT_INVALID`; a closed handle retains the existing
`SESSION_CLOSED` behavior.

## Response

The response is page-bound and contains `pageIndex`, `pageGeometry`,
`coordinateSpace: "PDF_PAGE"`, `paths`, a versioned `profile`, bounded engine
identity, and warnings. A path contains native `sourceOrder`, preserved
commands (`MOVE_TO`, `LINE_TO`, `CURVE_TO`, `CLOSE_PATH`, or `RECT`), finite
page bounds, and optional stroke/fill metadata.

The response does not contain a source PDF path, host session ID, process ID,
temporary path, or internal FFI pointer. No stable PDF object ID is fabricated;
source order is the only source identity exposed by this version.

## Geometry and MuPDF behavior

The implementation reuses the vendored MuPDF `NativeDevice` callbacks and
`PathWalker`. It runs page contents with the identity matrix, preserves Bézier
control points, and does not flatten curves. MuPDF page/device coordinates are
converted once to the product PDF_PAGE convention by flipping Y using the
verified page height. Page rotation is reported as page metadata and is not
applied as a display transform.

The profile is `documentengine-native-vector-read-v1` version `1`. Ordering is
native device execution order. Text, images, shades, and other non-path device
operations are omitted. Clipping callbacks only produce the truthful
`CLIPPING_PROVENANCE_NOT_AVAILABLE` warning; no clip object identity is
invented. Stroke cap/join/width/miter/dash and fill rule/color are exposed only
when the binding provides finite values.

Path geometry is rejected with `VECTOR_GEOMETRY_INVALID` when non-finite,
empty, inverted, or outside page bounds. Defensive response limits are
100,000 path records and 2,000,000 commands. They are protection limits, not
an engineering-drawing performance SLA.

## Error and lifecycle behavior

Vector engine failures map to `VECTOR_ENGINE_UNAVAILABLE`, malformed model
responses cannot escape the host, response-limit failures map to
`VECTOR_RESPONSE_INVALID`, and invalid geometry maps to
`VECTOR_GEOMETRY_INVALID`. The existing open/close/document lifecycle is
reused; vector read after close fails without corrupting native text or page
render behavior. The host remains local-only and introduces no network or
remote fallback.

## Consumer boundary

R2H.AI-ELE does not yet have a production DrawingVectorEvidence adapter in
this phase. E6b1 ends at this governed R2H-PDF read boundary. E6b2 may add the
consumer-side typed adapter and provenance/cache integration; E6c/E6d remain
out of scope.
