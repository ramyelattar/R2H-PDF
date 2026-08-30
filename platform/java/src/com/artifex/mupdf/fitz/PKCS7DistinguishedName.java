// Copyright (C) 2004-2021 Artifex Software, Inc.
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

package com.artifex.R2H-PDF.fitz;

// PKCS7DistinguishedName provides a friendly representation of the
// main descriptive fields of a PKCS7 encoded certificate used
// to sign a document
public class PKCS7DistinguishedName
{
	public String cn;       // common name
	public String o;        // organization
	public String ou;       // organizational unit
	public String email;    // email address of signer
	public String c;        // country
}
