- A “session” is wall-clock from session start until user chooses “Disable until I turn back on” or the app quits/crashes.
- “Disable for X minutes” does NOT end the session; it’s a within-session state. We need to analyze active vs temporarily disabled time.

Verify session/state data is sufficient for metrics. We must be able to compute:

- activation: ≥60 seconds active time within 48 hours of first run
- retention: active on 2+ different calendar days within 14 days
- companion intensity: long active day (≥60 active minutes), 3+ active days, % active time in sessions
  Therefore confirm we can derive or directly capture:
- a session_id that ties all within-session records together
- session start timestamp
- session end timestamp OR duration
- active time within session (preferred) OR enough state-change events to derive active vs disabled minutes
- whether “Disable for X minutes” is logged as a state change with duration and timestamps (without ending the session)

If any of the above cannot be computed from existing logs, propose the smallest change to existing events (or a new one if necessary) to be able to compute.
