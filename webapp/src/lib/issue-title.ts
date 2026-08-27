// A grouping title that is only a class name is a label, not a
// headline.
//
// Measured on production: five of nine error issues in the dogfood
// project are titled "Error", while their messages read "pinning
// mismatch on identity.focusai.com (mode=report-only)" and "The API
// method must be called from the main thread". A triage queue exists
// to answer "which of these do I open", and five identical headlines
// answer nothing while the sentence that does sits underneath in
// 12px grey.
//
// The crash view has demoted these since A13; the queue did not, so
// the same issue read one way in the list and another way once
// opened.

/** A single TitleCase token — `Error`, `TypeError`,
 *  `SentoriTestException`. Not `Cannot read property 'id'`. */
export function isBareTypeTitle(title: string): boolean {
  return /^[A-Z][A-Za-z0-9]*$/.test(title);
}

/** What to lead with, and what to demote beside it. `type` is the
 *  class name when it has given up the headline, else null. */
export function issueHeadline(issue: {
  title: string;
  messageSample: string;
}): { headline: string; type: null | string } {
  const demote =
    isBareTypeTitle(issue.title) &&
    issue.messageSample.length > 0 &&
    issue.messageSample !== issue.title;
  return demote
    ? { headline: issue.messageSample, type: issue.title }
    : { headline: issue.title, type: null };
}
