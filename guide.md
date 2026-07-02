# Running the LP-0013 Token-Authority Demo (and recording the video)

End-to-end guide to run `scripts/demo.sh` against a **real local LEZ sequencer**
at `RISC0_DEV_MODE=0` (real ZK proofs), and to record the narrated video the
prize requires.

The demo runs the full mint-authority lifecycle **through the sequencer**:

```
create → mint → rotate authority → mint by rotated authority
       → revoke → post-revoke mint REJECTED (error 1003)
```

Each positive step submits a real transaction and prints its tx hash; the
post-revoke mint is rejected on-chain by the authority guard.

---

## 1. Prerequisites (one-time, per machine)

On this VPS these are already built. On a clean machine:

```bash
# Rust + RISC-Zero toolchain (Docker needed to rebuild the guest ELF)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
curl -L https://risczero.com/install | bash && rzup install
sudo apt-get install -y clang libclang-dev unzip python3-dev

# LEZ node — sequencer + wallet, built from the SAME commit (a58fbce2)
git clone https://github.com/logos-blockchain/logos-execution-zone ~/lez-node
cd ~/lez-node && git checkout a58fbce2
cargo build --release --features standalone -p sequencer_service
cargo build --release -p wallet

# SPEL CLI (vendored in this repo at the pinned tag)
cd ~/token-authority
cargo build --release --manifest-path vendor/spel-framework/spel-cli/Cargo.toml
```

> IMPORTANT: `sequencer_service` and `wallet` must be built from the **same**
> lez-node commit. If they differ, native ops fail with `Unknown program`
> ("Local ID … different from remote"). Verify with:
> `~/lez-node/target/release/wallet check-health` (needs a sequencer running).

The pre-built guest ELF `token_authority.bin` is committed; its deterministic
Program ID is `63a29a4ec2b24402807c319d14e5d9a6bd5b26a49088cb3c6c2c8cd6187d2a60`
(reproduce: `vendor/…/spel -- inspect token_authority.bin`).

---

## 2. Clean slate (free the ports)

The demo starts a sequencer on a port (default 3040). Make sure nothing else is
holding it — stray sequencers/wallets from earlier runs will collide:

```bash
pkill -f sequencer_service ; pkill -f 'release/wallet' ; pkill -f 'release/spel'
ss -ltn | grep -E ':30(4|5)[0-9]'      # should print nothing → ports free
```

If a port is genuinely busy and you can't clear it, just pick another with
`SEQ_PORT=<n>` (below). `demo.sh` now **fails loudly** if its sequencer can't
bind, instead of silently talking to a foreign one.

---

## 3. Run the demo

```bash
cd ~/token-authority
SEQ_PORT=3040 ./scripts/demo.sh
```

Useful env overrides:

| Var | Default | Use |
|---|---|---|
| `SEQ_PORT` | `3040` | pick a free port if 3040 is taken |
| `STEP_TIMEOUT` | `600` (via demo.sh) | max seconds to confirm each *proving* tx |
| `REJECT_TIMEOUT` | `60` | seconds to declare the post-revoke rejection |
| `WALLET_HOME` | `.demo-wallet` | isolate the wallet dir for this run |
| `SPEL` | vendored build | path to the spel CLI if built elsewhere |

Runtime is ~4–5 min: each real proof takes ~30–60s (that latency is itself the
proof that `RISC0_DEV_MODE=0` — dev mode would be near-instant).

### Expected output (tail)

```
[3/4] Deploying token_authority.bin …  program_id = 63a29a4e…
[4/4] Running the mint-authority lifecycle on the local sequencer …
=== 1. create token (supply 1000, mint authority = creator) ===
  tx: …  CONFIRMED
=== 2. mint 500 (creator authority) -> supply 1500 ===        tx: …  CONFIRMED
=== 3. rotate authority creator -> auth2 ===                  tx: …  CONFIRMED
=== 4. mint 300 (auth2 authority) -> supply 1800 ===          tx: …  CONFIRMED
=== 5. revoke authority (auth2) -> fixed supply ===           tx: …  CONFIRMED
=== 6. mint 100 after revoke -> MUST be rejected (error 1003) ===
  REJECTED as expected (submitted tx … did not confirm; guest guard 1003)
Demo complete
```

Final state reads back `total_supply = 1800`, `mint_authority = None` — the
post-revoke mint did **not** change supply. Full record is written to
`demo-lifecycle-evidence.txt`.

---

## 4. Showing proof generation (for the video)

`demo.sh`'s own output shows the tx confirmations but **not** the proving lines —
those (`R0VM … CU <instr> cycles=N`) come from the sequencer, which `demo.sh`
redirects to `.seq.log`. The prize wants proof generation **on screen**, so run
it in **two panes** (e.g. `tmux`, `Ctrl-b %`):

- **Left:**  `cd ~/token-authority && touch .seq.log && tail -F .seq.log`
  → streams the live proving/CU lines.
- **Right:** `export RISC0_DEV_MODE=0 && echo $RISC0_DEV_MODE && SEQ_PORT=3040 ./scripts/demo.sh`

Now a single recording shows: `RISC0_DEV_MODE=0`, the per-tx confirmations, and
the matching proof lines + the real ~40s/tx latency.

---

## 5. Record the narrated video

The prize requires a **narrated** walkthrough (a silent screencast is not
enough): explain what you built and why, the architecture, key decisions, and
demonstrate the flow.

1. From your PC, screen-record the terminal window **with microphone** — OBS
   Studio, or Windows **Win+G** Game Bar.
2. Start recording, then run the two-pane demo above and **read the narration
   below** — talk *through* each proof pause; the `…confirmed` lands as you finish.
3. Stop recording → upload to YouTube (unlisted is fine) → paste the link into
   `solutions/LP-0013.md` (the Video section is stubbed and waiting).

### Narration — read aloud

Cues in *italics* tell you where you are on screen; the plain text is what you
say. Numbers are written the way you'd speak them.

***[As it's starting up]***
"Alright, so this is my submission for LP-0013 — it adds a mint-authority model
to the LEZ token program. The idea's pretty simple: whoever creates a token gets
to control its supply. They can mint more, they can hand that control to somebody
else, or they can give it up completely — and once they give it up, the supply's
locked forever. That last part is what lets you make a genuinely fixed-supply
token."

***[During the setup / deploy banners — the `63a29a4e` line]***
"The way it's built, there are three layers. At the bottom there's a small
admin-authority library — that's the RFP-001 piece — and it's completely
standalone, it doesn't depend on anything LEZ-specific, so you could drop it into
any project. On top of that there's a types layer for the token, and then the
actual on-chain program that ties them together. And you can see it deploying
right now — that program ID, the 63a29a4e one, comes out of the source
deterministically, so anyone who builds it themselves gets the exact same ID."

***[Point at the `RISC0_DEV_MODE=0` line]***
"One thing I want to call out before we get going — see up here, RISC0 dev-mode
is set to zero. That means these are real zero-knowledge proofs; there's no
dev-mode shortcut faking them. So every step is going to pause for maybe thirty
seconds to a minute, and that pause is the proof actually being generated. Over
on the left you can watch the cycle counts come through as each one proves —
that's the real work happening."

***[Step 1 — create, during the pause]***
"Okay, first step, we're creating a brand-new token. I'm calling it AUTHDEMO,
starting it at a supply of a thousand, and right at creation the mint authority
gets set to my key, the creator. So it's working on it now… generating the
proof… and there it goes — confirmed on chain."

***[Step 2 — mint 500]***
"Now that I'm the authority, I can mint. So here I'm minting another five hundred,
which takes the supply from a thousand up to fifteen hundred. And the key thing
is, only the current authority is allowed to do this — the program checks that
before it changes anything. Give it a second to prove… and confirmed."

***[Step 3 — rotate]***
"Here's where it gets interesting. Instead of holding onto control, I'm going to
rotate the authority over to a second key — I'll call it auth-two. It's an atomic
hand-off, so the moment this lands, my original key can't mint anymore — only the
new one can. Proving… and confirmed."

***[Step 4 — mint by auth-two]***
"And to actually show that the rotation worked, now the *new* authority, auth-two,
mints three hundred — that takes us up to eighteen hundred. If the rotation hadn't
really taken effect, this would get rejected. But… there it is, confirmed. So the
new key genuinely has control now."

***[Step 5 — revoke]***
"Next, auth-two permanently revokes the authority — basically sets it to nobody.
And this one's a one-way door. Once it's revoked, the supply is fixed at eighteen
hundred forever; there's no key anywhere that can ever mint again. Proving…
confirmed."

***[Step 6 — the rejection, longer pause]***
"Alright, this last step is really the whole point. I'm going to try to mint a
hundred more tokens *after* the authority's been revoked — and this has to fail.
The program hits the guard, sees there's no authority left, and rejects it with
error one-thousand-three. And you can see the sequencer just skips it — it never
confirms — so the supply doesn't move at all."

***[Wrap — final state on screen]***
"And there's the final state, read straight off the chain — supply's still
eighteen hundred, authority is None. That failed mint didn't change a thing. So
that's the whole lifecycle: create it, mint, hand off control, revoke, and once
it's revoked the supply is genuinely locked. And all of it ran against a real
local sequencer with real proofs. The library, the SDK, the examples, the IDL —
it's all in the repo. Thanks for watching."

---

## 6. Troubleshooting

| Symptom | Cause / fix |
|---|---|
| `ERROR: Port 3040 is already in use` | Another sequencer is running. Free it (section 2) or `SEQ_PORT=<free>`. |
| `Unknown program` / `check-health` panics `Local ID … different from remote` | `wallet` and `sequencer_service` built from different commits. Rebuild both from `a58fbce2`. |
| Step confirms very fast (<2s) | `RISC0_DEV_MODE` was not `0`. `export RISC0_DEV_MODE=0` before running. |
| `token_authority.bin not found` | Rebuild: `cargo risczero build --manifest-path methods/guest/Cargo.toml`. |
| `spel CLI not found` | Build it (section 1) or set `SPEL=/path/to/spel`. |

---

## 7. Re-anchor on the public testnet (optional)

The same lifecycle runs against the live testnet by pointing the wallet home at
it (the public testnet is periodically reset, so re-run to refresh evidence):

```bash
SPEL=vendor/spel-framework/spel-cli/target/release/spel \
WALLET=~/lez-node/target/release/wallet \
LEE_WALLET_HOME_DIR=<wallet home pointed at testnet.lez.logos.co> \
./scripts/testnet-lifecycle.sh
```
