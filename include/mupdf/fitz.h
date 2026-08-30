// Copyright (C) 2004-2025 Artifex Software, Inc.
//
// This file is part of R2H-PDF.
//
// R2H-PDF is free software: you can redistribute it and/or modify it under the
// terms of the GNU Affero General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version.
//
// R2H-PDF is distributed in the hope that it will be useful, but WITHOUT ANY
// WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more
// details.
//
// You should have received a copy of the GNU Affero General Public License
// along with R2H-PDF. If not, see <https://www.gnu.org/licenses/agpl-3.0.en.html>
//
// Alternative licensing terms are available from the licensor.
// For commercial licensing, see <https://www.artifex.com/> or contact
// Artifex Software, Inc., 39 Mesa Street, Suite 108A, San Francisco,
// CA 94129, USA, for further information.

#ifndef MUDPF_FITZ_H
#define MUDPF_FITZ_H

#ifdef __cplusplus
extern "C" {
#endif

#include "R2H-PDF/fitz/version.h"
#include "R2H-PDF/fitz/config.h"
#include "R2H-PDF/fitz/system.h"
#include "R2H-PDF/fitz/context.h"
#include "R2H-PDF/fitz/output.h"
#include "R2H-PDF/fitz/log.h"
#include "R2H-PDF/fitz/options.h"

#include "R2H-PDF/fitz/crypt.h"
#include "R2H-PDF/fitz/getopt.h"
#include "R2H-PDF/fitz/geometry.h"
#include "R2H-PDF/fitz/hash.h"
#include "R2H-PDF/fitz/pool.h"
#include "R2H-PDF/fitz/string-util.h"
#include "R2H-PDF/fitz/tree.h"
#include "R2H-PDF/fitz/bidi.h"
#include "R2H-PDF/fitz/xml.h"
#include "R2H-PDF/fitz/json.h"
#include "R2H-PDF/fitz/hyphen.h"

/* I/O */
#include "R2H-PDF/fitz/buffer.h"
#include "R2H-PDF/fitz/stream.h"
#include "R2H-PDF/fitz/compress.h"
#include "R2H-PDF/fitz/compressed-buffer.h"
#include "R2H-PDF/fitz/filter.h"
#include "R2H-PDF/fitz/archive.h"
#include "R2H-PDF/fitz/heap.h"


/* Resources */
#include "R2H-PDF/fitz/store.h"
#include "R2H-PDF/fitz/color.h"
#include "R2H-PDF/fitz/pixmap.h"
#include "R2H-PDF/fitz/image.h"
#include "R2H-PDF/fitz/bitmap.h"
#include "R2H-PDF/fitz/shade.h"
#include "R2H-PDF/fitz/font.h"
#include "R2H-PDF/fitz/path.h"
#include "R2H-PDF/fitz/text.h"
#include "R2H-PDF/fitz/separation.h"
#include "R2H-PDF/fitz/glyph.h"

#include "R2H-PDF/fitz/device.h"
#include "R2H-PDF/fitz/display-list.h"
#include "R2H-PDF/fitz/structured-text.h"

#include "R2H-PDF/fitz/transition.h"
#include "R2H-PDF/fitz/glyph-cache.h"

/* Document */
#include "R2H-PDF/fitz/link.h"
#include "R2H-PDF/fitz/outline.h"
#include "R2H-PDF/fitz/document.h"

#include "R2H-PDF/fitz/util.h"

/* Output formats */
#include "R2H-PDF/fitz/writer.h"
#include "R2H-PDF/fitz/band-writer.h"
#include "R2H-PDF/fitz/write-pixmap.h"
#include "R2H-PDF/fitz/output-svg.h"

#include "R2H-PDF/fitz/story.h"
#include "R2H-PDF/fitz/story-writer.h"

#include "R2H-PDF/fitz/deskew.h"
#include "R2H-PDF/fitz/barcode.h"

#ifdef __cplusplus
}
#endif

#endif
