# Route markdown rewrites through EditPlan

Any command that rewrites or moves existing markdown files under the wiki root uses `EditPlan`. Direct file writes are simpler locally, but `EditPlan` centralizes dry-run output, apply ordering, moved-path display, and non-overlapping edit checks so future rewrite commands share the same invariants.
