// SPDX-License-Identifier: MPL-2.0
//
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0.  If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright 2024 MonetDB Foundation

use time::UtcOffset;

pub fn timezone_offset_east_of_utc() -> i32 {
    if let Ok(offset) = UtcOffset::current_local_offset() {
        offset.whole_seconds()
    } else {
        0
    }
}
