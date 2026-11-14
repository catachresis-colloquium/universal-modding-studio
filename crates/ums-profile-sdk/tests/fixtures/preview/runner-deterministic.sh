#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
# Test stub for crates/ums-profile-sdk/tests/process_preview.rs. Uses shell
# builtins only, because the adapter clears the environment (no PATH).
# Deterministic: prints one runner-output document and one mind trace.
printf '%s\n' '{"package":{"beliefs":[{"belief_id":1,"holder":"agent-a","proposition":{"frame":"door","hypothesis":"north"},"source_events":[17],"status":"Believed"}],"events":[{"event_id":17,"kind":"Observation","observer":"agent-a","sequence":0,"tick":40}]},"world_truth":[{"frame":"door","hypothesis":"south"}]}' '{"contract":"enaction-mind-trace/1","producer":{"abi":"1.1","kernel":"stub"},"profile":{"id":"stub","version":"0.0.0"},"ticks":[{"affect":{"attention":7500,"caused_by":[17],"token":"angry"},"agent":"agent-a","conation":{"incumbent":"engage","ranking":["evade","engage"],"selected":"engage","target":"contact-1"},"tells":["fixate"],"tick":41}],"vocabulary":{"affect_tokens":["calm","angry"],"candidates":["evade","engage"],"tells":["fixate"]}}'
