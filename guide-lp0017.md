# Recording the LP-0017 (Whistleblower / logoz) demo — split terminal

Runbook for the **narrated video** in a **two-pane** terminal, so proof
generation and the live sequencer are both on screen. Runs in `~/logos/logoz`.

Binding prize requirements:
- show terminal output **including proof generation** to confirm
  `RISC0_DEV_MODE=0`;
- the builder **narrates** (a silent screencast is not sufficient);
- demonstrate: a file **uploaded and findable** on the Logos Delivery topic, the
  **batch-anchor** tool picking up the CID and anchoring it, and the **registry
  confirming** the CID.

> Difference vs LP-0013: here the `index_batch` proof is generated **client-side**
> by `batch-anchor`, so its `CU index_batch … cycles=…` line and the ~5–7 min
> proof pause print **inline in the demo pane** (right). The left pane (live
> sequencer) is supplementary — it shows a real local chain producing blocks and
> confirming the anchor. So proof generation is visible even if you never look
> left; the split just makes the "real sequencer" claim self-evident.

---

## Part 0 — Pre-warm (BEFORE you hit record)

Goal: **nothing compiles on camera.** A first run on a clean clone builds the
whole Logos stack (codex/waku, sequencer, guest) — **~1 hour**. Warm it once:

```bash
ssh <you>@<vps>
cd ~/logos/logoz

make build                       # build the guest ELF once (reproduces the pinned ProgramId)
nix run .#smoke-storage          # first run builds the codex/waku stack — SLOW, do it now
nix run .#smoke-broadcast
nix run .#smoke-publish
DEMO_RESET=1 ./scripts/demo.sh   # full dry run (~16 min): rehearsal + warms caches
```

Ready when the dry run ends with the lookup **found** / bogus **not-found**
lines. After this the real take only spends time *proving*, not building.

---

## Part 1 — Recorder + split terminal

1. On Windows, start **OBS Studio** (or **Win+G** Game Bar) capturing the
   terminal window **+ microphone**. Record a 10s test clip and **confirm audio**
   — a silent screencast fails the prize.
2. SSH in and start `tmux`, then split into two panes (`Ctrl-b %`):

   - **LEFT — live sequencer** (proves a real local chain is producing blocks):
     ```bash
     cd ~/logos/logoz
     watch -n 2 'lgs localnet logs --tail 20'
     ```
   - **RIGHT — the demo** (`Ctrl-b →` to switch):
     ```bash
     cd ~/logos/logoz
     export RISC0_DEV_MODE=0 && echo "RISC0_DEV_MODE=$RISC0_DEV_MODE"
     ```

   (The left pane fills once the sequencer is up in phase 5 of the demo.)

---

## Part 2 — The take (one recording, two segments)

### Segment A — anchor pipeline (the proof), RIGHT pane

```bash
DEMO_RESET=1 ./scripts/demo.sh
```

Prints labelled cyan phases `══ N/9 …`. The slow phases are **6** (shielding —
a privacy-preserving proving tx) and **8** (the `index_batch` anchor) — each
~5–7 min of **real Groth16 proof generation**. That pause *is* the evidence;
narrate over it. Phase 8 prints `CU index_batch … cycles=…` then `confirmed`;
phase 9 does `lookup` (found) and a bogus CID (not-found, non-zero exit).
Meanwhile the **left pane** shows the sequencer producing blocks and confirming.

### Segment B — publish half (upload → findable), RIGHT pane

```bash
nix run .#smoke-storage     # upload a file → CID, size, title (not path)
nix run .#smoke-broadcast   # publish the envelope to /chronicle/1/document-index/json
nix run .#smoke-publish     # upload + broadcast in one; ledger survives restart
```

---

## Part 3 — Narration (say it in your words, ~6–9 min)

> **Say early:** the first build from a clean clone compiles the full stack and
> can take ~an hour; this is pre-warmed, so the only pauses are real proofs.

| When | On screen | Say roughly |
|------|-----------|-------------|
| Intro | repo root, `README.md` | "Whistleblower — upload a document, broadcast its CID over Logos Delivery so it's instantly discoverable, and anchor it on-chain so it's permanently indexed. No central server, the publisher needs no tokens, and *anyone* can do the anchoring permissionlessly." |
| First-run cost | pre-warmed shell | "Anyone reproducing this: the first build compiles the whole stack, ~an hour. Pre-warmed here, so what you see are real proofs." |
| Architecture | `README` requirement map | "Three pieces: `logos-chronicle`, a reusable upload→broadcast→anchor module; `batch-anchor`, a permissionless CLI that gathers broadcast CIDs and anchors them in bulk; and a SPEL registry program on LEZ. Key choice: the anchorer is a *private* account — the index is public but the anchorer stays anonymous." |
| Dev-mode | `echo $RISC0_DEV_MODE` → 0 | "Real proofs are on — no dev-mode shortcut." |
| Seg A 1–5 | sequencer banners (+ left pane) | "A real local LEZ sequencer — the public testnet this was first verified on was wiped in the v0.2.0 migration, so we run the same code against a local chain. Left pane is the sequencer producing blocks." |
| Seg A phase 6 | shielding pause | "Funding the anonymous anchorer. Moving funds public→private is itself a privacy-preserving proof — this pause is real proof generation." |
| Seg A phase 8 | `CU index_batch … cycles=…`, `confirmed` | "The anchor. The CU line is the measured compute cost; this pause is the Groth16 proof for `index_batch` at dev-mode-off — the proof generation the prize asks to see. …confirmed in a block (watch it land in the left pane)." |
| Seg A phase 9 | lookup found / bogus not-found | "Querying the registry by CID returns the record — `anchored_by` is an anonymous key, not the signer. An un-anchored CID returns not-found and a non-zero exit." |
| Seg B | smoke-storage/broadcast/publish | "The other half: a file to Logos Storage → a CID; the metadata envelope published to the Delivery topic, immediately findable; the combined ledger survives a restart." |
| Wrap | — | "Upload, broadcast, permissionless batch anchor, privacy-preserving on-chain index — all on the Logos stack. Module and CLI are both in the repo. Thanks." |

---

## Part 4 — After recording

- Upload **unlisted** (YouTube / Loom / file link); copy the URL.
- Paste the URL into the submission write-up (`SUBMISSION_PR.md`, kept locally).
- Double-check the recording: `RISC0_DEV_MODE=0` visible, the phase-6/8 proof
  pauses present, and **both** the upload-findable and anchor-confirm halves shown.

---

## Troubleshooting

| Symptom | Fix |
|---|---|
| Left pane empty | Sequencer starts in phase 5; it fills once `lgs localnet` is up. |
| `lgs not on PATH` | `cargo install --git https://github.com/logos-co/scaffold logos-scaffold`. |
| A phase confirms in <2s | `RISC0_DEV_MODE` wasn't `0`. `export RISC0_DEV_MODE=0` and re-run. |
| Want a clean chain | `DEMO_RESET=1 ./scripts/demo.sh` wipes localnet + wallet, starts from genesis. |
| Sequencer stuck | `lgs localnet reset` (stops, wipes db, restarts, verifies block production). |
