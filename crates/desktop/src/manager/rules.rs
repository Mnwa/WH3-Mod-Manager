//! Load-order rules: user rules from the library, pack rules from `whmm\load_order.whmm`,
//! applied to the explicit order without moving pinned (hand-placed) mods.
use super::Manager;
use gpui_kit::*;
use std::{
    collections::HashSet,
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};
use wh3_core::{
    catalog::Catalog,
    load_order::{self, Resolution, Rule, RuleKey},
    message,
    storage::Settings,
};

#[derive(Default)]
pub(crate) struct State {
    /// Mods the user placed by hand; rules never move them (the original's pins).
    pub pinned: HashSet<usize>,
    pub pack_rules: Arc<Vec<Rule>>,
    pub resolution: Arc<Resolution>,
    /// Accepted rules that a pinned mod currently breaks.
    pub overridden: Vec<Rule>,
    pub task: Option<Task<()>>,
    pub scan_task: Option<Task<()>>,
    pub generation: u64,
}

/// Everything rule evaluation needs, cloned off the UI thread.
pub(super) struct Inputs {
    user: Vec<Rule>,
    pack: Arc<Vec<Rule>>,
    disabled: HashSet<RuleKey>,
    disabled_packs: HashSet<String>,
}

impl Inputs {
    pub(super) fn new(settings: &Settings, pack: Arc<Vec<Rule>>) -> Self {
        Self {
            user: settings.rules.clone(),
            pack,
            disabled: settings
                .disabled_rules
                .iter()
                .map(RuleKey::from_raw)
                .collect(),
            disabled_packs: settings.disabled_rule_packs.clone().into_iter().collect(),
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.user.is_empty() && self.pack.is_empty()
    }

    /// Resolve and apply; `present` is every installed pack, as in the original.
    pub(super) fn run(
        &self,
        catalog: &Catalog,
        order: &[usize],
        enabled: &HashSet<usize>,
        pinned: &HashSet<usize>,
    ) -> (Resolution, load_order::Applied) {
        let present: HashSet<String> = catalog
            .mods
            .iter()
            .map(|m| load_order::pack_key(&m.name))
            .collect();
        let resolution = load_order::resolve(
            &self.user,
            &self.pack,
            &self.disabled,
            &self.disabled_packs,
            &present,
        );
        let applied = load_order::apply(
            order,
            |i| &catalog.mods[i].name,
            enabled,
            &resolution.accepted,
            pinned,
        );
        (resolution, applied)
    }
}

impl Manager {
    /// Re-apply rules shortly after a change; newer requests replace pending ones.
    pub(super) fn apply_rules(&mut self, cx: &mut Context<Self>) {
        let inputs = Inputs::new(&self.settings, self.rules.pack_rules.clone());
        if inputs.is_empty()
            && self.rules.resolution.dropped.is_empty()
            && self.rules.resolution.accepted.is_empty()
        {
            return;
        }
        self.rules.generation += 1;
        let generation = self.rules.generation;
        let (catalog, order, enabled, pinned) = (
            self.catalog.clone(),
            self.order.clone(),
            self.enabled.clone(),
            self.rules.pinned.clone(),
        );
        let executor = cx.background_executor().clone();
        self.rules.task = Some(cx.spawn(async move |this, cx| {
            executor.timer(Duration::from_millis(120)).await;
            let (resolution, applied) = executor
                .spawn(async move { inputs.run(&catalog, &order, &enabled, &pinned) })
                .await;
            let _ = this.update(cx, |this, cx| {
                if generation != this.rules.generation {
                    return;
                }
                if !applied.moved.is_empty() && applied.order.len() == this.order.len() {
                    this.status = message!(
                        "Load-order rules moved {} mods",
                        "Правила порядка переместили модов: {}",
                        applied.moved.len()
                    );
                    this.order = Arc::new(applied.order);
                    this.rebuild_ranks();
                    this.dirty = true;
                    this.refresh_query(cx);
                }
                this.rules.resolution = Arc::new(resolution);
                this.rules.overridden = applied.overridden_by_pin;
                cx.notify();
            });
        }));
    }

    /// Read rules shipped inside packs after each scan.
    pub(super) fn scan_pack_rules(&mut self, cx: &mut Context<Self>) {
        if self.demo {
            return;
        }
        let catalog = self.catalog.clone();
        let task = cx.background_spawn(async move {
            load_order::pack_rules(&catalog, &AtomicBool::new(false))
        });
        self.rules.scan_task = Some(cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok((rules, warnings)) => {
                        this.rules.pack_rules = Arc::new(rules);
                        this.diagnostics.extend(warnings);
                    }
                    Err(error) => this.diagnostics.push(error.message()),
                }
                this.apply_rules(cx);
            });
        }));
    }

    pub(super) fn pin(&mut self, index: usize) {
        self.rules.pinned.insert(index);
    }

    pub(super) fn toggle_pin(&mut self, index: usize, cx: &mut Context<Self>) {
        if !self.rules.pinned.remove(&index) {
            self.rules.pinned.insert(index);
        }
        self.dirty = true;
        self.apply_rules(cx);
        cx.notify();
    }

    pub(super) fn clear_pins(&mut self, cx: &mut Context<Self>) {
        self.rules.pinned.clear();
        self.dirty = true;
        self.apply_rules(cx);
        cx.notify();
    }

    /// `index` loads before (or after) `other`; `index` is the mod that gives way.
    pub(super) fn add_rule(
        &mut self,
        index: usize,
        other: &str,
        before: bool,
        cx: &mut Context<Self>,
    ) {
        let own = self.catalog.mods[index].name.to_string();
        let rule = if before {
            Rule::user(&own, other, &own)
        } else {
            Rule::user(other, &own, &own)
        };
        let pair = |r: &Rule| {
            let mut pair = [
                load_order::pack_key(&r.before),
                load_order::pack_key(&r.after),
            ];
            pair.sort();
            pair
        };
        // A new rule about the same pair replaces the old one, as in the original.
        self.settings
            .rules
            .retain(|existing| pair(existing) != pair(&rule));
        self.settings.rules.push(rule);
        // Let the rule move the mod even if it was placed by hand earlier.
        self.rules.pinned.remove(&index);
        self.dirty = true;
        self.apply_rules(cx);
        cx.notify();
    }

    pub(super) fn remove_rule(&mut self, rule: &Rule, cx: &mut Context<Self>) {
        self.settings.rules.retain(|existing| existing != rule);
        self.dirty = true;
        self.apply_rules(cx);
        cx.notify();
    }

    pub(super) fn toggle_pack_rule(&mut self, rule: &Rule, cx: &mut Context<Self>) {
        let key = rule.key().as_str().to_owned();
        if !self.settings.disabled_rules.remove(&key) {
            self.settings.disabled_rules.insert(key);
        }
        self.dirty = true;
        self.apply_rules(cx);
        cx.notify();
    }
}
