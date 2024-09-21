use std::collections::HashMap;
use std::collections::VecDeque;
use itertools::Itertools;
use clap::Parser;
use serde::{Deserialize, Serialize};
use std::io;

// todo: pass compilation parameters
// todo: some assertions

const N_PLAYERS: usize = 7;           // total number of players
const N_MAFIAS: usize = 2;            // total number of mafias
const SKIP_FIRST_DAY: bool = true;    // whether to always skip on the first day
//const n_detectives: usize = 1;      // number of real detectives
//const n_doctors: usize = 0;         // number of real doctors
//const n_fake_detectives: usize = 1; // number of mafias pretending to be detectives
//const n_fake_doctors: usize = 0;    // number of mafias pretending to be doctor
//const n_bosses: usize = 0;

//const mafias_forced: bool = true;   // whether mafias have to kill during night
//const save_self: bool = false;      // whether the doctor can save themselves
//const save_twice: bool = false;     // whether doctor can save themselves twice in a row
//const talk_killed: bool = false;    // whether night killed can communicate before leaving
//const reveal_day: bool = false;     // whether roles of day kills are revealed
//const reveal_night: bool = false;   // whether roles of night kills are revealed
//const split_votes: bool = false;    // whether more than one person can be voted out in a tie

const SKIP: usize = N_PLAYERS;

// struct for storing currently indistinguishable players
#[derive(Serialize, Deserialize, Debug, Clone, Copy)]
struct Player {
    alive: bool,
    mafia: bool,
    count: usize
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct State {
    kills: Vec<usize>,
    real_requests: Vec<usize>,
    real_responses: Vec<bool>,
    fake_requests: Vec<usize>,
    fake_responses: Vec<bool>,
    players: Vec<Player>
}

impl State {
    // convert player id to char for printing
    fn to_char(x: usize) -> char {
        if x == N_PLAYERS {
            return '.';
        } else {
            return char::from_u32(x as u32 + 48).unwrap();
        }
    }

    fn inc(c: char) -> char { char::from_u32(c as u32 + 1).unwrap() }

    fn players_string(values: &Vec<usize>) -> String {
        return values.iter().map(|x| Self::to_char(*x)).collect();
    }

    fn responses_string(values: &Vec<bool>) -> String {
        return values.iter().map(|x| if *x {'+'} else {'-'}).collect();
    }

    fn is_day(&self) -> bool {
        self.kills.len() % 2 == 0
    }

    fn state_key(&self) -> String {
        // kills real_requests fake_requests fake_responses
        // '+' means mafia, '-' means villager
        let mut s = String::with_capacity(self.kills.len() + self.real_requests.len() + self.fake_requests.len() + self.fake_responses.len() + 3);

        s.push_str(&Self::players_string(&self.kills));
        s.push(',');
        s.push_str(&Self::players_string(&self.real_requests));
        s.push(',');
        s.push_str(&Self::players_string(&self.fake_requests));
        s.push(',');
        s.push_str(&Self::responses_string(&self.fake_responses));
    
        return s;
    }

    fn infostate_key_town(&self) -> String {
        // ab = detectives
        // kills requests1 responses1 requests2 responses2
        // whichever gives lexicographically minimal key

        let permutation = |swap: bool| {
            let mut s = String::with_capacity(self.kills.len() + self.real_requests.len() + self.real_responses.len() + self.fake_requests.len() + self.fake_responses.len() + 4);

            let mut m = {
                let mut array = ['?'; N_PLAYERS + 1];
                array[N_PLAYERS] = '.';
                array[0] = if swap { 'b' } else { 'a' };
                array[1] = if swap { 'a' } else { 'b' };
                array
            };

            let mut next_player = '0';

            let mut f = |x: &usize| {
                if m[*x] == '?' {
                    m[*x] = next_player;
                    next_player = Self::inc(next_player);
                }
                return m[*x];
            };

            s.push_str(&self.kills.iter().map(&mut f).collect::<String>());
            s.push(',');

            let (requests1, responses1, requests2, responses2) = if swap {
                (&self.real_requests, &self.real_responses, &self.fake_requests, &self.fake_responses)
            } else {
                (&self.fake_requests, &self.fake_responses, &self.real_requests, &self.real_responses)
            };

            s.push_str(&requests1.into_iter().map(&mut f).collect::<String>());
            s.push(',');
            s.push_str(&Self::responses_string(&responses1));
            s.push(',');
            s.push_str(&requests2.iter().map(&mut f).collect::<String>());
            s.push(',');
            s.push_str(&Self::responses_string(&responses2));

            return s;
        };

        return std::cmp::min(permutation(false), permutation(true));
    }

    fn infostate_key_mafia(&self) -> String {
        // a = fake detective
        // bcd... = mafias
        // kills requests responses
        let mut s = String::with_capacity(self.kills.len() + self.fake_requests.len() + self.fake_responses.len() + 2);

        let mut m = {
            let mut array = ['?'; N_PLAYERS + 1];
            array[N_PLAYERS] = '.';
            array[1] = 'a';
            array
        };

        let mut next_mafia = 'b';
        let mut next_villager = '0';

        let mut f = |x: &usize| {
            if m[*x] == '?' {
                if self.players[*x].mafia {
                    m[*x] = next_mafia;
                    next_mafia = Self::inc(next_mafia);
                } else {
                    m[*x] = next_villager;
                    next_villager = Self::inc(next_villager);
                }
            }
            return m[*x];
        };

        s.push_str(&self.kills.iter().map(&mut f).collect::<String>());
        s.push(',');
        s.push_str(&self.fake_requests.iter().map(&mut f).collect::<String>());
        s.push(',');
        s.push_str(&Self::responses_string(&self.fake_responses));
        
        return s;
    }

    fn infostate_key(&self) -> String {
        if self.is_day() {
            return self.infostate_key_town();
        } else {
            return self.infostate_key_mafia();
        }
    }

    // return a new state where a player was possibly assigned a number
    // all extra copies are reserved a new number
    fn touch(&self, player_id: usize) -> State {
        let mut t = self.clone();
        if player_id != SKIP {
            let player = self.players[player_id];
            if player.count > 1 {
                t.players.push(Player {
                    alive: true,
                    mafia: player.mafia,
                    count: player.count - 1
                });
                t.players[player_id].count = 1;
            }
        }

        return t;
    }

    // list of people for mafia to kill
    fn kill_candidates(&self) -> Vec<(usize, usize)> {
        let mut results = Vec::new();

        for (i, player) in self.players.iter().enumerate() {
            if player.alive {
                results.push((i, player.count));
            }
        }

        if !self.is_day() && self.alive_mafias() == 1 { // never kill self if last mafia
            results = results.into_iter().filter(|(i, _)| !self.players[*i].mafia).collect();
        }

        return results;
    }

    // list of people to vote out (or skip)
    fn vote_candidates(&self) -> Vec<(usize, usize)> {
        // todo: never/always kill if detectives in agreement?
        // todo: detective reporting more than mafias (including other detective)
        // todo: maybe skip on all even?
        if self.alive_total() == 4 || (SKIP_FIRST_DAY && self.kills.is_empty()) { // always skip on 4
            return vec![(SKIP, 1)];
        }
        let mut results = self.kill_candidates();
        if self.alive_total() > 3 {
            results.push((SKIP, 1));
        }
        // todo: on 3, if both "detectives" are alive, we need to kill one of them
        // also kill either detective or their checked mafia
        return results;
    }

    fn check_candidates(&self, detective: usize, requests: &Vec<usize>) -> Vec<(usize, usize)> {
        // candidate must be alive, not checked, and not self
        if !self.players[detective].alive || self.alive_total() == 2 { // doesn't matter on 3
            return vec![(SKIP, 1)]; // skip might mean detective is dead or no valid candidates
        }

        let mut results = Vec::new();

        for (i, player) in self.players.iter().enumerate() {
            if player.alive && !requests.contains(&i) && i != detective {
                results.push((i, player.count));
            }
        }

        if results.is_empty() {
            results.push((SKIP, 1));
        }

        return results;
    }

    fn alive_total(&self) -> usize {
        self.players.iter().map(|player| if player.alive { player.count } else { 0 }).sum::<usize>()
    }

    fn alive_mafias(&self) -> usize {
        self.players.iter().map(|player| if player.alive && player.mafia { player.count } else { 0 }).sum()
    }

    fn is_terminal(&self) -> bool {
        self.alive_total() - if self.is_day() { 0usize } else { 1usize } <= 2 * self.alive_mafias() || self.alive_mafias() == 0
    }
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct Node {
    state: State,
    infostate: usize,
    actions: Vec< Vec<(usize, f32)> >, // possible transition nodes for each infostate action
    equity: f32, // win probability for the current team
    frequency: f32, // how often we're in this state
}

#[derive(Clone, Serialize, Deserialize, Debug)]
struct Infostate {
    strategy: Vec<f32>, // current strategy
    strategy_sum: Vec<f32>, // average strategy (converges to Nash equilibrium)
    regret_sum: Vec<f32>
}

fn normalize(regrets: &Vec<f32>) -> Vec<f32> {
    let mut strategy: Vec<f32> = regrets.iter().map(|&x| if x > 0.0 { x } else { 0.0 }).collect();
    let sum: f32 = strategy.iter().sum();
    if sum > 0.0 {
        strategy = strategy.iter().map(|&x| x / sum).collect();
    } else {
        // is this ever called?
        strategy = vec![1.0 / regrets.len() as f32; regrets.len()];
    }
    return strategy;
}

// Mafia game solver
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    #[arg(long, default_value_t = String::new())]
    load_checkpoint: String,

    #[arg(long, default_value_t = String::new())]
    save_checkpoint: String,

    #[arg(long, default_value_t = 10000)]
    epochs: usize,

    #[arg(long, default_value_t = 10)]
    eval_every: usize,

    #[arg(long, default_value_t = 0.005)]
    early_stopping: f32,

    //#[arg(long, default_value_t = String::new())]
    //sample_rollouts_file: String,

    //#[arg(long, default_value_t = 10)]
    //n_sample_rollouts: usize,

    #[arg(long, default_value_t = false)]
    explore: bool,
}


fn main() {
    let args = Args::parse();

    println!("{}", args.explore);

    println!("players = {}, mafias = {}", N_PLAYERS, N_MAFIAS);

    let mut nodes: Vec<Node> = Vec::new(); // game tree
    let mut infostates: Vec<Infostate> = Vec::new(); // infostates and CFR stuff
    let levels: Vec< Vec<(usize, Vec<usize>)> >; // ordering of non-terminal nodes used while training

    if !args.load_checkpoint.is_empty() {
        println!("Loading checkpoint from {}", args.load_checkpoint);
        let data = std::fs::read_to_string(&args.load_checkpoint).unwrap();
        (nodes, infostates, levels) = serde_json::from_str(&data).unwrap();
    } else {
        println!("Building game graph...");

        let initial_state = State {
            kills: vec![],
            real_requests: vec![],
            real_responses: vec![],
            fake_requests: vec![],
            fake_responses: vec![],
            players: if N_MAFIAS == 1 {
                vec![
                    Player {alive: true, mafia: false, count: 1},
                    Player {alive: true, mafia: true, count: 1},
                    Player {alive: true, mafia: false, count: N_PLAYERS - 2}
                ]
            } else {
                vec![
                    Player {alive: true, mafia: false, count: 1},
                    Player {alive: true, mafia: true, count: 1},
                    Player {alive: true, mafia: true, count: N_MAFIAS - 1},
                    Player {alive: true, mafia: false, count: N_PLAYERS - N_MAFIAS - 1}
                ]
            }
        };

        let mut queue = VecDeque::from([initial_state.clone()]);
        let mut map_states = HashMap::from([(initial_state.state_key(), 0usize)]); // todo: we can get rid of it because the game is a tree now
        let mut map_infostates = HashMap::new();

        let mut last_level = 0;
        
        while !queue.is_empty() {
            let s = queue.pop_back().unwrap();

            if s.kills.len() > last_level {
                last_level += 1;
                println!("level {}, {} nodes", last_level, nodes.len());
            }

            // todo: if there is only one action, prune?

            if s.is_terminal() {
                nodes.push(Node {
                    equity: if (s.alive_mafias() == 0) == s.is_day() { 1.0 } else { 0.0 },
                    state: s,
                    infostate: 0,
                    actions: vec![],
                    frequency: 0.0,
                });
                continue;
            }

            struct RawAction {
                infostate: String,
                to: usize,
                count: usize
            }

            let mut raw_actions: Vec<RawAction> = Vec::new();

            if s.is_day() {
                // select vote and checks
                for (p1, c1) in s.vote_candidates() {
                    let mut s1 = s.touch(p1);
                    s1.kills.push(p1);
                    if p1 != SKIP {
                        s1.players[p1].alive = false;
                    }

                    for (p2, c2) in s1.check_candidates(0, &s1.real_requests) {
                        let mut s2 = s1.touch(p2);
                        s2.real_requests.push(p2);

                        for (p3, c3) in s2.check_candidates(1, &s2.fake_requests) {
                            let mut s3 = s2.touch(p3);
                            s3.fake_requests.push(p3);
                            let state_key = s3.state_key();
                            
                            if !map_states.contains_key(&state_key) {
                                queue.push_front(s3.clone());
                                map_states.insert(state_key.clone(), map_states.len());
                            }

                            let infostate_key = s3.infostate_key_town();
                            raw_actions.push(RawAction {
                                infostate: infostate_key,
                                to: map_states[&state_key],
                                count: c1 * c2 * c3
                            });
                        }
                    }
                }
            } else {
                // select kill and response
                for (p1, c1) in s.kill_candidates() {
                    let mut s1 = s.touch(p1);
                    s1.kills.push(p1);
                    s1.players[p1].alive = false;

                    // if real detective isn't killed, they should respond
                    let real_request = *s1.real_requests.last().unwrap();
                    if s1.players[0].alive && real_request != SKIP {
                        // ^ remove to allow report on death
                        s1.real_responses.push(s1.players[real_request].mafia);
                    }
                    
                    let mut process_transition = |t: State| {
                        let state_key = t.state_key();
                        if !map_states.contains_key(&state_key) {
                            queue.push_front(t.clone());
                            map_states.insert(state_key.clone(), map_states.len());
                        }

                        let infostate_key = t.infostate_key_mafia();
                        raw_actions.push(RawAction {
                            infostate: infostate_key,
                            to: map_states[&state_key],
                            count: c1
                        });
                    };

                    // if fake detective isn't killed, they should report
                    let fake_request = *s1.fake_requests.last().unwrap();
                    if s1.players[1].alive && fake_request != SKIP {
                        // ^ remove to allow report on death
                        for response in [false, true] {
                            // todo: can't report more mafias than there are in the game?
                            let mut s2 = s1.clone();
                            s2.fake_responses.push(response);
                            process_transition(s2);
                        }
                    } else {
                        process_transition(s1);
                    }
                }
            }

            let mut actions: Vec< Vec<(usize, f32)> > = Vec::new();

            raw_actions.sort_by_key(|action| action.infostate.clone());
            
            raw_actions.iter().group_by(|raw_action| raw_action.infostate.clone()).into_iter().for_each(|(_, group)| {
                // todo: get rid of temporary structures
                let transition_counts: Vec<(usize, usize)> = group.map(|raw_action| (raw_action.to, raw_action.count)).collect();
                let sum: usize = transition_counts.iter().map(|(_, count)| count).sum();
                let transition_probabilities: Vec<(usize, f32)> = transition_counts.iter().map(|(to, count)| (*to, *count as f32 / sum as f32)).collect();
                actions.push(transition_probabilities);
            });
            
            let infostate_key = s.infostate_key();
            
            if !map_infostates.contains_key(&infostate_key) {
                map_infostates.insert(infostate_key.clone(), map_infostates.len());
                infostates.push(Infostate {
                    strategy: vec![1.0 / actions.len() as f32; actions.len()],
                    strategy_sum: vec![0.0; actions.len()],
                    regret_sum: vec![0.0; actions.len()]
                });
            }
        
            nodes.push(Node {
                state: s,
                equity: 0.0,
                frequency: 0.0,
                infostate: map_infostates[&infostate_key],
                actions
            });
        }

        nodes[0].frequency = 1.0;

        // we want to sort by length and infostates to split workload and improve cache locality
        let mut node_indices: Vec<usize> = nodes.iter().enumerate().filter(|(_, node)| !node.actions.is_empty()).map(|(i, _)| i).collect();
        node_indices.sort_by_key(|&idx| (nodes[idx].state.kills.len(), nodes[idx].infostate));
        
        levels = node_indices.iter()
            .group_by(|&&idx| nodes[idx].state.kills.len())
            .into_iter()
            .map(|(_, group_by_level)| {
            group_by_level.group_by(|&&idx| nodes[idx].infostate)
                .into_iter()
                .map(|(infostate, group_by_infostate)| {
                (infostate, group_by_infostate.into_iter().map(|&idx| idx).collect())
            }).collect()
        }).collect();

        // todo: ^ this looks like shit
    }

    println!("States: {}", nodes.len()); // including terminal
    println!("Infostates: {}", infostates.len());

    for e in 0..args.epochs {
        if e % args.eval_every == 0 {
            println!("Epoch {}", e);

            let mut perfect_play = |player: usize| {
                // calculate winrate for player if the opponent's strategy is fixed

                // calculate counterfactual frequencies for each state
                // (that is, what is the probability of ending up in this state given our actions)
                for (i, level) in levels.iter().enumerate() {
                    // todo: this can be done in parallel
                    for (infostate_idx, matching_nodes) in level {
                        let strategy = normalize(&infostates[*infostate_idx].strategy_sum);
                        for node_idx in matching_nodes {
                            let actions = nodes[*node_idx].actions.clone();
                            for (transitions, prob_action) in actions.iter().zip_eq(strategy.iter()) {
                                for (to, prob_transition) in transitions {
                                    let counterfactual_prob = if i % 2 == player { 1.0 } else { *prob_action };
                                    nodes[*to].frequency = nodes[*node_idx].frequency * counterfactual_prob * prob_transition;
                                }
                            }
                        }
                    }
                }

                // calculate utilities for each state
                for (i, level) in levels.iter().enumerate().rev() {
                    // todo: this can be done in parallel
                    for (infostate_idx, matching_nodes) in level {
                        if i % 2 == player {
                            // choose perfect response for infostate
                            let infostate = &infostates[*infostate_idx];
                            let mut utilities = vec![0f32; infostate.strategy.len()];
                            for node_idx in matching_nodes {
                                let actions = nodes[*node_idx].actions.clone();
                                // node -> actions -> transitions
                                for (utility, transitions) in utilities.iter_mut().zip_eq(actions.iter()) {
                                    for (to, prob_transition) in transitions {
                                        *utility += (1.0 - nodes[*to].equity) * prob_transition * nodes[*node_idx].frequency;
                                    }
                                }
                            }

                            //let best_action = utilities.iter().max_by_key(|x| x).unwrap();
                            let mut best_action = 0;
                            for (i, u) in utilities.iter().enumerate() {
                                if *u > utilities[best_action] {
                                    best_action = i;
                                }
                            }

                            for node_idx in matching_nodes {
                                nodes[*node_idx].equity = nodes[*node_idx].actions[best_action].iter().map(|(to, prob)| prob * (1.0 - nodes[*to].equity)).sum();
                            }
                        } else {
                            // calculate equity for opponent nodes
                            let strategy = normalize(&infostates[*infostate_idx].strategy_sum); // todo: this can be saved and reused
                            for node_idx in matching_nodes {
                                nodes[*node_idx].equity = 0.0;
                                let actions = nodes[*node_idx].actions.clone();
                                for (transitions, prob_action) in actions.iter().zip_eq(strategy.iter()) {
                                    for (to, prob_transition) in transitions {
                                        nodes[*node_idx].equity += (1.0 - nodes[*to].equity) * prob_action * prob_transition;
                                    }
                                }
                            }
                        }    
                    }
                }

                return nodes[0].equity;
            };

            let (min_winrate, max_winrate) = (perfect_play(1), perfect_play(0));
            println!("Equilibrium range: ({:.4}, {:.4})", min_winrate, max_winrate);

            if !args.save_checkpoint.is_empty() {
                println!("Saving checkpoint to {}", args.save_checkpoint);
                let data = (nodes.clone(), infostates.clone(), levels.clone()); // todo: get rid of extra memory
                std::fs::write(&args.save_checkpoint, serde_json::to_string(&data).unwrap()).unwrap();    
            }

            if max_winrate - min_winrate < args.early_stopping {
                println!("Training converged early");
                break;
            }
        }

        let mut update_regrets = |player: usize| {
            // calculates counterfactual regrets for player and updates them

            // todo: this same as before, maybe combine somehow
            for (i, level) in levels.iter().enumerate() {
                // todo: this can be done in parallel
                for (infostate_idx, matching_nodes) in level {
                    let strategy = &infostates[*infostate_idx].strategy;
                    for node_idx in matching_nodes {
                        let actions = nodes[*node_idx].actions.clone();
                        for (transitions, prob_action) in actions.iter().zip_eq(strategy.iter()) {
                            for (to, prob_transition) in transitions {
                                let counterfactual_prob = if i % 2 == player { 1.0 } else { *prob_action };
                                nodes[*to].frequency = nodes[*node_idx].frequency * counterfactual_prob * prob_transition;
                            }
                        }
                    }
                }
            }

            // calculate equities bottom-up and update regrets
            for (i, level) in levels.iter().enumerate().rev() {
                // todo: this can be done in parallel
                for (infostate_idx, matching_nodes) in level {
                    let infostate = &mut infostates[*infostate_idx];

                    let mut regrets = vec![0f32; infostate.strategy.len()];
                    
                    for node_idx in matching_nodes {
                        nodes[*node_idx].equity = 0.0;
                        let actions = nodes[*node_idx].actions.clone();
                        for (regret, (transitions, prob_action)) in regrets.iter_mut().zip_eq(actions.iter().zip_eq(infostate.strategy.iter())) {
                            for (to, prob_transition) in transitions.iter() {
                                let winrate = 1.0 - nodes[*to].equity;
                                nodes[*node_idx].equity += winrate * prob_action * prob_transition;
                                *regret += winrate * prob_transition * nodes[*node_idx].frequency;
                            }
                        }
                        for regret in regrets.iter_mut() {
                            *regret -= nodes[*node_idx].equity * nodes[*node_idx].frequency;
                        }
                    }

                    //let infostate_frequency: f32 = matching_nodes.iter().map(|node| nodes[*node].frequency).sum();
                    //let scaling_factor = if infostate_frequency > 1e-6 { 1.0 / infostate_frequency } else { 1e6 };

                    // CFR+
                    // regrets = regrets.iter().map(|&x| if x > 0.0 { x } else { 0.0 }).collect();

                    // weighting
                    //regrets = regrets.iter().map(|&x| x * f32::ln(1.0 + e as f32)).collect();
                    //regrets = regrets.iter().map(|&x| x * 1.2 as f32).collect();

                    if i % 2 == player {
                        // calculate regrets and add to regret_sum
                        infostate.regret_sum = infostate.regret_sum.iter()
                            .zip_eq(regrets.iter())
                            .map(|(s, r)| s + r) // can add a discount factor here
                            .collect();
                    
                        // update strategy and strategy_sum
                        infostate.strategy = normalize(&infostate.regret_sum);
                        infostate.strategy_sum = infostate.strategy_sum.iter()
                            .zip_eq(infostate.strategy.iter())
                            .map(|(s, x)| s + x)
                            .collect();
                    }
                }
            }
        };

        update_regrets(0);
        update_regrets(1);
    }

    if args.explore {
        println!("Entered interactive mode");
        println!("Input \"n, k\" to select action, \"ret\" to revert, \"new\" to start over");

        // commands: n k, new, ret
        let stdin = io::stdin();
        let mut stack: Vec<usize> = vec![0];

        loop {
            println!();

            let node_id = stack.last().unwrap();
            let node = &nodes[*node_id];
            let strategy = normalize(&infostates[node.infostate].strategy_sum);

            println!("       ID: {}", node_id);
            println!("    State: {}", node.state.state_key());
            println!("Infostate: {}", node.state.infostate_key());
            println!("      Day: {}", node.state.is_day());
            println!(" Equity: {:.4}", node.equity);
            //println!(" Terminal: {} ({} {} {})", node.state.is_terminal(), node.state.alive_total(), node.state.alive_mafias(), node.equity);

            println!();
            
            for (i, player) in node.state.players.iter().enumerate() {
                println!(" {} {:?}", i, player);
            }

            println!();

            if node.state.is_terminal() {
                println!("Game over");
                println!();
            } else {
                for (i, (action, action_prob)) in node.actions.iter().zip_eq(strategy).enumerate() {
                    let infostate_key = if node.state.is_day() {
                        nodes[action[0].0].state.infostate_key_town()
                    } else {
                        nodes[action[0].0].state.infostate_key_mafia()
                    };

                    println!("Action {} ({:.4}): {}", i, action_prob, infostate_key);
                    for (j, (transition, transition_prob)) in action.iter().enumerate() {
                        println!("  {:>2}  {:.4}  {:.4}  {}", j, transition_prob,  1.0 - nodes[*transition].equity, nodes[*transition].state.state_key());
                    }
                    println!();
                }
            }

            let mut buffer = String::new();
            stdin.read_line(&mut buffer).unwrap();
            buffer = buffer.trim().to_string();

            if buffer == "new" {
                stack.truncate(1);
            } else if buffer == "ret" {
                if stack.len() > 1 {
                    stack.pop();
                }
            } else {
                let (a, b) = buffer.split_once(' ').unwrap();
                let action_id: usize = a.parse().unwrap();
                let transition_id: usize = b.parse().unwrap();
                stack.push(node.actions[action_id][transition_id].0);
            }

            // todo: error handling
            // todo: list other possible nodes for this infostate
        }
    }
}
