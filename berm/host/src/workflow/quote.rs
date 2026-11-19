use crate::{deny, env::Env, host::Host};
use anyhow::{Context, Result, bail};
use axum::http::StatusCode;
use berm::{
    Meters,
    abi::workflow::{ControlInput, ControlOutput, ControlValue},
    control,
};
use berm_api::{
    schema,
    workflow::{
        graph::{Kind, Producer},
        view::Quote,
    },
};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    sync::Arc,
    time::{Duration, Instant},
};

impl<E: Env> Host<E> {
    /// What `input` settles for a run of `workflow`: each exact priced step's
    /// count and each input-only branch's case. Runs only controls that read
    /// the input alone and branches that do; controls are pure, so a run with
    /// the same input prepares the same lists.
    pub async fn quote_workflow(
        &self,
        service: &str,
        workflow: &str,
        input: Value,
    ) -> Result<Quote> {
        let owner = self.deployed(service)?;
        let spec = owner
            .manifest
            .workflows
            .iter()
            .find(|spec| spec.entry.name == workflow)
            .ok_or_else(|| deny(StatusCode::NOT_FOUND, "no such workflow"))?;
        schema::validate(&spec.entry.graph.input, &input)?;
        let graph = &spec.entry.graph;
        let deadline = Instant::now() + self.deadline.min(Duration::from_secs(60));
        let meters = Arc::new(Meters::default());
        let mut values: BTreeMap<String, Value> = BTreeMap::new();
        let mut quote = Quote::default();
        for node in &graph.nodes {
            let priced = matches!(node.kind, Kind::Step { price: Some(_), .. });
            let branch = matches!(node.kind, Kind::Branch { .. });
            if !(priced || branch) || !graph.settled_by_input(node) {
                continue;
            }
            let selected = node
                .guards
                .iter()
                .all(|guard| guard.selection.selects(values.get(&guard.producer)));
            if !selected {
                if priced {
                    quote.steps.insert(node.id.clone(), 0);
                }
                continue;
            }
            let control_input = ControlInput {
                input: input.clone(),
                nodes: node
                    .dependencies
                    .iter()
                    .filter_map(|dependency| match &dependency.source {
                        Producer::Input => None,
                        Producer::Node { id } => Some((
                            id.clone(),
                            values.get(id).map_or(ControlValue::Skipped, |value| {
                                ControlValue::Present {
                                    value: value.clone(),
                                }
                            }),
                        )),
                    })
                    .collect(),
            };
            let output = owner
                .control(control::Request {
                    workflow,
                    node: &node.id,
                    input: &control_input,
                    deadline,
                    meters: meters.clone(),
                })
                .await?;
            match (&node.kind, output) {
                (Kind::Step { mode, .. }, ControlOutput::Calls { inputs }) => {
                    if inputs.len() > mode.maximum() as usize {
                        bail!("workflow builder exceeded its declared call bound");
                    }
                    quote.steps.insert(node.id.clone(), inputs.len() as u32);
                }
                (Kind::Step { .. }, ControlOutput::Skipped) => {
                    quote.steps.insert(node.id.clone(), 0);
                }
                (Kind::Branch { cases }, ControlOutput::Branch { case, value }) => {
                    cases
                        .iter()
                        .find(|candidate| candidate.name == case)
                        .context("control selected an undeclared branch")?;
                    quote.branches.insert(node.id.clone(), case);
                    values.insert(node.id.clone(), value);
                }
                _ => bail!("control result does not match its declared node kind"),
            }
        }
        quote.meter = self.metered(meters.take());
        Ok(quote)
    }
}
