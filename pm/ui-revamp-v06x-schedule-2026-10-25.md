# UI Revamp in v0.6.x: schedule (25/10/2026 23:10 UTC+7) [P0-UI-V06]
Source: Jang 23:04, "Đẩy UI revamp lên trong v06x luôn nhé". Epic #1118. Milestone #57 "v0.7.5 UI Revamp slice 1" is retired: it is empty and has a description. It was not deleted (deleting it is Jang's call).

| Milestone | Due | UI Revamp | Existing items |
|---|---|---|---|
| v0.6.3 | 26/10 | none (Slice 1 isn't ready by 26/10: L takes 2–3 days) | #1085 #1117 #1072 |
| v0.6.4 | 27/10 (was 26/10) | #1166 spec, #1126 Gate L | #1069 #993 #887 #875 |
| v0.6.5 | 28/10 (was 26/10) | #1142 Spec A (after L) | #1088 #1003 #983 |
| v0.6.6 (new #58) | 29/10 | #1143 Spec B (after A, same files), #1141 #1154 | — |
| v0.6.7 (new #59) | 30/10 | #1139 widget kit C, #1121 #1127 #1140 #1165 | — |
| v0.6.8 (new #60) | 31/10 | #1119 #1120 #1124 | — |
| v0.6.9 (new #61) | 01/11 | #1128 #1138 #1144 | — |
| v0.6.10 (new #62) | 02/11 | #1146 #1155 | — |
| v0.6.11 (new #63) | 03/11 | #1125 docs, #1122 research | — |
| v0.7 | 04/11 (was 27/10) | | unchanged |
| v0.7.1 / v0.7.2 | 05/11 (was 28/10) | | #1158 #1159 … |
| v0.7.3 / v0.7.4 | 06/11 (was 29/10) | | |
| v0.8 → v0.13 | 07/11 → 12/11 (+7 days) | | |

## Trade-offs
- **What slips:** v0.6.4 slips 1 day and v0.6.5 slips 2 days. v0.7 slips 8 days (27/10 → 04/11), v0.7.1–v0.7.4 slip 8 days, and v0.8–v0.13 slip 7 days.
- **Capacity:** the train can do 4–5 patches a day. In the UI phase it drops to about 1 release a day, because L → A → B is strictly serial (each takes 1.5–3 days, and A and B touch the same files). Slice 1 is therefore done on 29/10 at the earliest, and only if L is done by 27/10. Spare capacity goes to the parallel enablers and surfaces (Phase 1).
- **Risk:** if L takes 3 days, v0.6.4 slips to 28/10 and the chain moves with it (+1 day per release).

## Open decision for Jang
- **Windows red (#1158):** Jang has NOT decided whether to accept it for releases after v0.6.2. Until he does, the v0.6.3 gate is ALL green, and #1085/#1117 are blocked by Windows ❌.
  - Option A: accept it as a known issue.
  - Option B: pull #1158 into v0.6.3 and land it first. v0.6.3 would then probably slip to 27/10, and v0.6.4 onwards by +1 day.
- v0.6.2 is still waiting for Jang to tag it at 184e145.
