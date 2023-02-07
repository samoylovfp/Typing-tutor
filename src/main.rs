use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet, VecDeque},
};

use gloo_storage::{LocalStorage, Storage};
use gloo_utils::body;
use itertools::Itertools;
use rand::{distributions::WeightedIndex, prelude::Distribution};
use serde::{Deserialize, Serialize};
use wasm_bindgen::{prelude::Closure, JsCast};
use yew::prelude::*;

struct Practice {
    prompt: String,
    correctness: Vec<bool>,
    expected_chars: HashSet<char>,
    mistyped: VecDeque<(char, char)>,
    error_stats: TypingErrors,
}

#[derive(Serialize, Deserialize, Default)]
struct TypingErrors {
    error_score: HashMap<char, usize>,
    error_stats: HashMap<String, usize>,
}
impl TypingErrors {
    fn account(&mut self, expected_c: char, typed_char: char) {
        let correct = expected_c == typed_char;
        let score = self.error_score.entry(expected_c).or_default();
        if correct {
            *score = score.saturating_sub(1);
            self.error_stats
                .iter_mut()
                .filter(|(k, _v)| k.starts_with(expected_c))
                .for_each(|(_k, v)| *v = v.saturating_sub(1));
        } else {
            *score += ERROR_SCORE_INCR;
            *self.error_score.entry(typed_char).or_default() += 1;
            let stat_score = self
                .error_stats
                .entry(chars_to_key(expected_c, typed_char))
                .or_default();
            *stat_score += STAT_SCORE_INCR;
        }
        LocalStorage::set(ERROR_STORAGE_KEY, self).unwrap();
    }
}

fn chars_to_key(ex: char, ty: char) -> String {
    format!("{ex} -> {ty}")
}

const ERROR_SCORE_INCR: usize = 10;
const STAT_SCORE_INCR: usize = 50;
const ERROR_STORAGE_KEY: &str = "typing_errors";

enum Msg {
    KeyPress(KeyboardEvent),
}

impl Practice {
    fn render_chars(&self) -> Html {
        self.prompt
            .chars()
            .enumerate()
            .map(|(i, c)| {
                let class = match (i, self.correctness.get(i)) {
                    (i, _) if self.correctness.len() == i => "cursor",
                    (_, Some(true)) => "correct",
                    (_, Some(false)) => "incorrect",
                    (_, None) => "",
                };
                html!(
                    <span class = {class}>{c}</span>
                )
            })
            .collect()
    }

    fn render_error_stats(&self) -> Html {
        self.error_stats
            .error_stats
            .iter()
            .sorted_by_key(|(_k, v)| Reverse(*v))
            .map(|(k, v)| format!("{k} ({})\n", div_ceil(*v, STAT_SCORE_INCR)))
            .collect()
    }
}

impl yew::Component for Practice {
    type Message = Msg;
    type Properties = ();

    fn create(ctx: &Context<Self>) -> Self {
        let link = ctx.link().clone();
        let cb: Closure<dyn Fn(Event)> = Closure::new(move |e: Event| {
            let e = e.dyn_into::<KeyboardEvent>().unwrap();
            link.send_message(Msg::KeyPress(e));
        });

        body()
            .add_event_listener_with_callback("keydown", cb.into_js_value().unchecked_ref())
            .unwrap();

        let stats = LocalStorage::get(ERROR_STORAGE_KEY).unwrap_or_default();

        Practice {
            prompt: generate_random_str(&stats),
            correctness: vec![],
            expected_chars: default_symbols().into_iter().collect(),
            mistyped: Default::default(),
            error_stats: stats,
        }
    }
    fn view(&self, _ctx: &Context<Self>) -> Html {
        html!(
            <>
            <a href="https://github.com/samoylovfp/Typing-tutor/">{"GitHub"}</a>
                <pre>{self.render_chars()}
                {(self.correctness.len() == self.prompt.chars().count()).then_some(
                    "\nEnter to continue\n"
                )}
                </pre>
                {"Last mistakes:"}
                <pre>{
                    self.mistyped
                    .iter()
                    .rev()
                    .map(|(ex,ty)|format!("{ex} -> {ty}\n"))
                    .collect::<String>()
                }</pre>

                {"Error stats:"}
                <pre>{self.render_error_stats()}</pre>
            </>
        )
    }

    fn update(&mut self, _ctx: &Context<Self>, msg: Self::Message) -> bool {
        let reset = |s: &mut Self| {
            s.prompt = generate_random_str(&s.error_stats);
            s.correctness.clear();
        };

        match msg {
            Msg::KeyPress(ev) if ev.key() == "Backspace" => {
                self.correctness.pop();
            }
            Msg::KeyPress(ev)
                if ev.key() == "Enter" && self.prompt.chars().count() == self.correctness.len() =>
            {
                reset(self)
            }
            Msg::KeyPress(ev) => {
                let key = ev.key();
                let mut chars = key.chars();
                let char = chars.next().unwrap();
                if chars.next().is_some() {
                    return false;
                }

                ev.prevent_default();

                if !self.expected_chars.contains(&char) {
                    return false;
                }

                match self.prompt.chars().nth(self.correctness.len()) {
                    None => {}
                    Some(expected_c) => {
                        let correct = expected_c == char;
                        self.correctness.push(correct);
                        self.error_stats.account(expected_c, char);
                        if !correct {
                            self.mistyped.push_back((expected_c, char));
                            if self.mistyped.len() > 10 {
                                self.mistyped.pop_front();
                            }
                        }
                    }
                }
            }
        }
        true
    }
}

fn main() {
    tracing_wasm::set_as_global_default();
    yew::Renderer::<Practice>::new().render();
}

fn default_symbols() -> Vec<char> {
    (0x21..=0x7e_u8).into_iter().map(|b| b as char).collect()
}

fn generate_random_str(stats: &TypingErrors) -> String {
    let chars = default_symbols();
    let weights = WeightedIndex::new(chars.iter().map(|c| {
        let score = stats.error_score.get(c).copied().unwrap_or_default();
        div_ceil(score, ERROR_SCORE_INCR) + 1
    }))
    .unwrap();
    let mut rng = rand::thread_rng();
    (0..50).map(|_| chars[weights.sample(&mut rng)]).collect()
}

fn div_ceil(divident: usize, divisor: usize) -> usize {
    (divident + (divisor - 1)) / divisor
}
