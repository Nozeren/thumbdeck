// What tabs tell the avatar in the top bar (src/lib/avatar): the page reads these along with its own state.
export const avatarSignals = $state({
  /** Logs being read right now (a count: several can be read at once) */
  logsReading: 0,
});
