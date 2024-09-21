# CFR-based Mafia solver

A proof-of-concept solver for Mafia-like games (and a game tree exploration CLI tool) written in Rust.

Assumes that there is a "meta-player" (a secure multi-party computation protocol) that privately orders each player claiming to be a detective (with optimal play should be two of them: one real and one fake) whom to check and privately collects their next-day responses, and also chooses whom to vote out during the day. A social deduction game is thus converted into a two-player zero-sum game, where one meta-player controls the town (deciding whom to check and whom to vote out during the day) and another controls the mafia (deciding what the fake detective should respond and whom to kill during the night). This two-player game is then solved using counterfactual regret minimization (the same method commonly used for poker and other imperfect-information games).

For a high-level explaination, read "A theory of social deduction games" (the README of this repository).

## Usage

Game parameters are specified in the code, so you need to install Rust to run it.

By default, a 7-player game with 2 mafias is created. The town can vote out at most one player, requiring the majority of votes (if `n_mafias ≥ n_players / 2`, mafia automatically wins). Detectives killed during the night do not get a "last word" and can't report their checks. The game starts with day.

First, the game tree is built (or loaded from a checkpoint) and the solver prints out the number of states and information states (sets of states indistinguishable from the current player's perspective). To make the tree smaller, strategically equivalent game states are deduplicated (the two states are equivalent if you can rename the players and get the same history) and certain suboptimal or irrelevant actions are excluded right away (e.g., you should never kill yourself as the last mafia, it doesn't matter whom to check among 3 players, and it's always optimal to skip on the first day — the last option is toggleable).

Then, the strategy is trained either for a fixed number of iterations (`--epochs`) or until the exploitability gap becomes small enough (`--early-stopping`). It is regularly evaluated (`--eval-every`), reporting the range of worst-case winrates for the current strategy pair (which should eventually converge to the same number), and optionally saved (`--save-checkpoint`):

```
cargo run -- --eval-every=10 --save-checkpoint checkpoint.mafia
```

You can then explore the game tree:

```
cargo run -- --load-checkpoint checkpoint.mafia --explore
```

Starting at the root node, you can choose the action and one of its possible outcomes for the current player by inputing two space-separated numbers, and input "ret" or "new" to revert them.

## Notation

Exploration tool output looks like this (starting state for a 7-player game):

```
       ID: 0
    State: ,,,
Infostate: ,,,,
      Day: true
 Equity: 0.5448

 0 Player { alive: true, mafia: false, count: 1 }
 1 Player { alive: true, mafia: true, count: 1 }
 2 Player { alive: true, mafia: true, count: 1 }
 3 Player { alive: true, mafia: false, count: 4 }

Action 0 (0.0613): .,0,,0,
   0  0.2000  0.6285  .,2,2,
   1  0.8000  0.5232  .,3,3,

Action 1 (0.4316): .,0,,1,
   0  0.2000  0.5765  .,2,3,
   1  0.2000  0.5914  .,3,2,
   2  0.6000  0.5187  .,3,4,

Action 2 (0.5069): .,0,,b,
   0  0.1000  0.6245  .,1,2,
   1  0.4000  0.5648  .,1,3,
   2  0.1000  0.6118  .,2,0,
   3  0.4000  0.4878  .,3,0,

Action 3 (0.0002): .,a,,b,
   0  1.0000  0.5397  .,1,0,
```

`ID` is just state id (index of the game node), `Day` is whether it's the town's turn to act, and `Equity` is the expected win probability for the current player (assuming optimal play). The other fields are more complicated.

To deduplicate equivalent game states, we adopt a player numbering system based on the order in which something interesting happened to them. The real detective is always assigned number 0, the fake detective is always assigned number 1, but other mafias (if there are ≥3 of them) and villagers are initially indistinguishable. When such non-unique player gets killed or checked, they are assigned the next available number (these mappings and current player status are also displayed).

`State` is a comma-separated string specifying who got killed (in order; `.` means that nobody got killed during the day), which players were checked by the real detective, which players were "checked" by the fake detective, and what the fake detective responded (`+` means mafia, `-`means town)

`Infostate` is a comma-separated string describing what the current player knows. As not all information is available to the player, it uses a different numeration than the state.

For town, it similarly specifies who got killed and who got checked by the two detectives and their responses. The two detectives are named `a` and `b` and everyone else is assigned numbers starting from 0. Since detectives are indistinguishable from the town's perspective, the `a` detective is chosen as the one that produces lexicographically smaller string.

For mafia, it specifies who got killed, who which checks were requested for the fake detective, and what the fake detective reported. The fake detective is named `a`, other mafias are named `b`, `c` and so on in the order of introduction, and everyone else is assigned numbers starting from 0.

Finally, it lists the available actions indistinguishable from the current player's perspective (`Action 0 (0.0613): .,0,,0` means that the strategy chooses to vote one noone and order the two detectives to check the same player with probability 0.0613) and their possible outcomes (we will either pick mafia with probability 0.2 and then win with probability 0.6285 or pick a villager with probability 0.8 and then win with probability 0.5232).

## TODOs

The code is not high-quality and not particularly performant, but I'm ~90% sure it is at least correct. (CFR is hard to implement from scratch!)

The simplest improvement would be to make it parallel. It already splits the game tree nodes by levels, and in a language like C/C++ it would be enough to add a simple `#pragma omp parallel for`, but in Rust you have to fight the borrow checker. Each infostate can be processed in parallel, the only synchronization point being the levels, and it seems relatively easy to compute the updates on the GPU.

The training procedure currently serializes the whole tree into a JSON, which inflates it by a lot (7-player game requires 1.5GB). In theory, we only need to store one node index, transition probability and maybe node frequencies and equities (4+4+4+4 bytes) for each transition/node, requiring ~41MB.

Run time is quadratic in the number of information states. For 7 players, it converges in a few minutes. Adding a new player increases the number of information states by ~50x. It should be possible to perfectly solve the game for 8 players on a laptop and for 9 players on a decent server with some optimizations, but larger games are challenging.

The algorithm is just vanilla tabular CFR, but many modifications of CFR have been proposed. It woulld be especially interesting to solve the game approximately for higher dimensions (either through MCCFR or DeepCFR).

I also tried looking into Marc Lanctot's OpenSpiel (adding Mafia as a custom environment to use it with out-of-the-box algorithms and analysis tools) but found its API somewhat restrictive and its algorithm implementations not good enough (they are meant as a proof-of-concept too), and just decided that it would be too difficult. But this is definitely the way, and perhaps in a few years the library will get there.
