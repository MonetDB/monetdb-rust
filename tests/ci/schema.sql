-- SPDX-License-Identifier: MPL-2.0
--
-- This Source Code Form is subject to the terms of the Mozilla Public
-- License, v. 2.0.  If a copy of the MPL was not distributed with this
-- file, You can obtain one at http://mozilla.org/MPL/2.0/.
--
-- Copyright 2024 MonetDB Foundation

START TRANSACTION;

DROP TABLE IF EXISTS temporal;
CREATE TABLE temporal(
    i       INT,
    tsz     TIMESTAMPTZ
);

-- The interval 87_654 seconds was chosen to get a nice variation
-- in seconds, minutes, hours, days, etc.
-- 1000 * 1000 goes back to about year -748.
INSERT INTO temporal
SELECT
    value AS i,
    NOW - value * value * INTERVAL '87654' SECOND
FROM
    sys.generate_series(0, 1000)
;
-- SELECT * FROM temporal;

COMMIT;