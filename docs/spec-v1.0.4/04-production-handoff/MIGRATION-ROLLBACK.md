# Migration & Rollback v1.0.4

## Migration

1. Keep v1.0.3 schema/data intact.
2. Add Trigger event / analytics storage only where required.
3. Add runtime-state and trigger config.
4. Run replay comparison before enabling event-driven production behavior.
5. Enable event-driven runtime first in OBSERVE/PAPER.
6. After acceptance, use the same runtime for CONFIRM/AUTO.

No existing Prediction Episode or Trade history should be rewritten.

## Rollback

If trigger runtime causes missed opportunities, trigger storms, duplicate Episodes, or safety regression:

- disable event-driven production policy via deployment/config rollback;
- return to the last known v1.0.3 full-evaluation runtime temporarily;
- preserve Trigger logs for analysis;
- do not alter historical Prediction/Trade Outcomes.

Rollback mode is an operational recovery mechanism, not a permanent user-facing product mode.
