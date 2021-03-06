// Copyright 2012 Mozilla Corporation. All rights reserved.
// This code is governed by the BSD license found in the LICENSE file.

/**
 * @description Tests that the behavior of a Record is not affected by adversarial
 *     changes to Object.prototype.
 * @author Norbert Lindenberg
 */

$INCLUDE("testIntl.js");

taintProperties(["weekday", "era", "year", "month", "day", "hour", "minute", "second", "timeZone"]);

var locale = new Intl.DateTimeFormat(undefined, {localeMatcher: "lookup"}).resolvedOptions().locale;
if (!isCanonicalizedStructurallyValidLanguageTag(locale)) {
    $ERROR("DateTimeFormat returns invalid locale " + locale + ".");
}

