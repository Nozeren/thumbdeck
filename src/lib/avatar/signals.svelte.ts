// What tabs tell the avatar in the top bar (src/lib/avatar): the page reads these along with its own state.
export const avatarSignals = $state({
  /** Logs being read right now (a count: several can be read at once) */
  logsReading: 0,
  /** PRs asking for your review, when a Pull requests tab has looked (null: none has) */
  prsToReview: null as number | null,
});
