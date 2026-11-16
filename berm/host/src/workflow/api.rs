use crate::{
    deny,
    env::{Env, Images},
    host::{self, Host},
    workflow::StorageGrant,
};
use anyhow::Result;
use axum::http::StatusCode;
use berm_api::{
    Access, digest,
    host::Caller,
    workflow::{
        Offer,
        run::{Binding, Run, Trigger},
        view::{Inspection, Record, Start},
    },
};
use uuid::Uuid;

impl<E: Env> Host<E> {
    pub fn inspect_workflow(
        &self,
        service: &str,
        workflow: &str,
        caller: Option<&Caller>,
    ) -> Result<Inspection> {
        let offer = self.workflow_offer(service, workflow)?;
        let access = offer
            .spec
            .config
            .access
            .ok_or_else(|| deny(StatusCode::NOT_FOUND, "no such public workflow"))?;
        if access == Access::Services {
            return Err(deny(
                StatusCode::FORBIDDEN,
                "workflow takes calls from services only",
            )
            .explained(Run::SERVICES_ONLY)
            .into());
        }
        if access != Access::Anonymous && caller.is_none_or(Caller::anonymous) {
            return Err(deny(
                StatusCode::UNAUTHORIZED,
                "workflow requires an authenticated caller",
            )
            .into());
        }
        Self::workflow_inspection(service, offer, None)
    }

    pub fn workflow_inspection(
        service: &str,
        offer: Offer,
        quote: Option<String>,
    ) -> Result<Inspection> {
        let quote = quote.unwrap_or(digest::json_hex(&serde_json::to_value(&offer)?));
        Ok(Inspection {
            version: offer.spec.entry.version,
            service: service.into(),
            entry: offer.spec.entry,
            title: offer.spec.config.title,
            source: offer.source,
            quote,
            task_price: offer.spec.config.price.map(|_| 0),
            deadline_ms: offer.deadline_ms,
            retention_ms: offer.spec.config.retain.as_millis().try_into()?,
        })
    }

    pub async fn workflow_image(&self, service: &str) -> Result<Vec<u8>> {
        let (scope, name) = host::split(service);
        let row = crate::state::service::row(&self.store, scope, name)
            .await?
            .ok_or_else(|| deny(StatusCode::NOT_FOUND, "no such workflow service"))?;
        self.images.pull(&row.reference, &row.digest).await
    }

    pub async fn workflow_record(&self, service: &str, workflow: &str, id: &str) -> Result<Record> {
        let execution = self
            .workflows
            .view(id, (self.now)())
            .await?
            .ok_or_else(|| {
                deny(StatusCode::NOT_FOUND, "no such workflow run").explained(Run::REMOVED)
            })?;
        if execution.service != service || execution.workflow != workflow {
            return Err(deny(StatusCode::NOT_FOUND, "no such workflow run").into());
        }
        let accepted = if let Some((inspection, _)) = self.workflows.admission(id).await? {
            inspection
        } else {
            let image = self.workflows.image(id).await?;
            let manifest = berm_api::Manifest::try_from(image.as_slice())?;
            let spec = manifest
                .workflows
                .iter()
                .find(|spec| spec.entry.name == workflow)
                .ok_or_else(|| deny(StatusCode::NOT_FOUND, "no such retained workflow"))?;
            Inspection {
                version: execution.version,
                service: service.into(),
                entry: spec.entry.clone(),
                title: spec.config.title.clone(),
                source: execution.source.clone(),
                quote: execution.source.graph.clone(),
                task_price: spec.config.price.map(|_| 0),
                deadline_ms: self
                    .workflows
                    .get(id)
                    .await?
                    .ok_or(crate::workflow::Refused::Missing)?
                    .accepted
                    .deadline_ms,
                retention_ms: spec.config.retain.as_millis().try_into()?,
            }
        };
        Ok(Record {
            execution,
            accepted,
        })
    }

    pub async fn start_local_workflow(
        &self,
        service: &str,
        workflow: &str,
        request: Start,
        caller: Caller,
    ) -> Result<Record> {
        if caller.id.is_empty() || caller.anonymous() {
            return Err(deny(
                StatusCode::UNAUTHORIZED,
                "workflow admission requires an authenticated account or a verified app user",
            )
            .explained(Run::INITIATOR)
            .into());
        }
        Uuid::parse_str(&request.id)
            .map_err(|_| deny(StatusCode::BAD_REQUEST, "workflow run id must be a UUID"))?;
        let hash = digest::json_hex(
            &serde_json::json!({"service":service,"workflow":workflow,"input":request.input,"quote":request.quote,"caller":{"id":caller.id,"key":caller.key,"app":caller.app,"user":caller.user}}),
        );
        if let Some((_, original)) = self.workflows.admission(&request.id).await? {
            if original != hash {
                return Err(deny(
                    StatusCode::CONFLICT,
                    "workflow run id was reused with different arguments",
                )
                .explained(Binding::CHANGED)
                .into());
            }
            return self.workflow_record(service, workflow, &request.id).await;
        }
        let inspected = self.inspect_workflow(service, workflow, Some(&caller))?;
        if inspected.quote != request.quote {
            return Err(deny(StatusCode::CONFLICT, "workflow terms changed")
                .explained(Binding::CHANGED)
                .into());
        }
        let mut run =
            self.prepare_workflow(service, workflow, request.input, Trigger::Manual { caller })?;
        run.id = request.id;
        let image = self.workflow_image(service).await?;
        self.workflows
            .admit(
                run.clone(),
                image,
                StorageGrant {
                    scope: host::split(service).0.into(),
                    bytes: u64::MAX,
                },
                inspected,
                hash,
            )
            .await?;
        match self
            .claim_workflow(&run.id, crate::workflow::Intent::Start)
            .await
        {
            Ok(claim) => self.launch_workflow(claim, None).await?,
            Err(error)
                if error
                    .downcast_ref::<crate::workflow::Refused>()
                    .is_some_and(|error| {
                        matches!(
                            error,
                            crate::workflow::Refused::Busy | crate::workflow::Refused::Ineligible
                        )
                    }) => {}
            Err(error) => return Err(error),
        }
        self.workflow_record(service, workflow, &run.id).await
    }
}
