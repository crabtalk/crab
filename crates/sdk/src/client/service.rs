//! A host's programs: `/services/{owner}/{name}` on Cloud, `/programs/{name}`
//! on a local host.

use crate::{
    client::{Client, Error},
    sandbox::service::{
        Billing, Costed, DeployRequest, MeteredDay, Secret, SeedRequest, SeedUpload, SetSecret,
        Visibility,
    },
};
use berm_api::{
    Budget, Cron, Deployed, Logged, Meter, Output, Program, Runtime, Terms,
    host::{self, CallInput, Caller, Crons, Live, ToolUpdate, Wakes},
};
use reqwest::{Method, RequestBuilder};
use std::collections::BTreeMap;
use tokio::fs::File;

impl Client {
    /// Every service the owner runs on Cloud, or every program on a local host.
    pub async fn list(&self) -> Result<Vec<Program>, Error> {
        let path = match &self.owner {
            Some(owner) => owner.services(),
            None => "/programs".to_owned(),
        };
        self.json(self.at(Method::GET, &path)).await
    }

    pub async fn inspect(&self, name: &str) -> Result<Program, Error> {
        let path = format!("{}/idl", self.program(name));
        self.json(self.at(Method::GET, &path)).await
    }

    /// How `name` runs: what its IDL leaves out, for its owners.
    pub async fn runtime(&self, name: &str) -> Result<Runtime, Error> {
        let path = format!("{}/runtime", self.program(name));
        self.json(self.at(Method::GET, &path)).await
    }

    /// Deploy `image` as `name` on a local host, confirming the tools it may make
    /// public, setting each cron tool's expression when `crons` is given, and
    /// holding its database to `size` bytes.
    pub async fn deploy_image(
        &self,
        name: &str,
        image: Vec<u8>,
        public: &[String],
        force: bool,
        crons: Option<&BTreeMap<String, String>>,
        size: u64,
    ) -> Result<Deployed, Error> {
        if self.owner.is_some() {
            return Err(Error::Host("Cloud deploys from a registry, not from bytes"));
        }
        let mut request = self
            .at(Method::PUT, &self.program(name))
            .query(&[("public", public.join(","))])
            .query(&[("force", force)])
            .query(&[("size", size)]);
        if let Some(crons) = crons {
            let crons =
                serde_json::to_string(crons).map_err(|_| Error::Host("crons do not serialize"))?;
            request = request.query(&[("crons", crons)]);
        }
        let request = request.body(image);
        self.json(request).await
    }

    /// Deploy as `name` on Cloud what `deploy` names.
    pub async fn deploy(&self, name: &str, deploy: &DeployRequest) -> Result<Deployed, Error> {
        let request = self.at(Method::PUT, &self.program(name)).json(deploy);
        self.json(request).await
    }

    /// Where to upload `seed` for a deploy of `name` on Cloud to name.
    pub async fn seed(&self, name: &str, seed: &SeedRequest) -> Result<SeedUpload, Error> {
        self.account()?;
        let path = format!("{}/seed", self.program(name));
        self.json(self.at(Method::POST, &path).json(seed)).await
    }

    /// Upload the `size` bytes of `file` to `url`, an upload [`Client::seed`]
    /// answered.
    pub async fn upload_seed(&self, url: &str, file: File, size: u64) -> Result<(), Error> {
        let request = self
            .http
            .put(url)
            .header(reqwest::header::CONTENT_LENGTH, size)
            .body(file);
        self.ok(request).await.map(drop)
    }

    /// Call `tool` of `program`, as `caller` on a local host, which takes it
    /// unverified.
    pub async fn call(
        &self,
        program: &str,
        tool: &str,
        arguments: Vec<u8>,
        caller: Option<&Caller>,
    ) -> Result<(Output, Option<Meter>), Error> {
        let path = format!("{}/tools/{tool}", self.program(program));
        let request = self.as_caller(self.at(Method::POST, &path).body(arguments), caller)?;
        self.answered(request).await
    }

    /// What a call of each of `name`'s public tools is held to.
    pub async fn terms(&self, name: &str) -> Result<Terms, Error> {
        let path = format!("{}/terms", self.program(name));
        self.json(self.at(Method::GET, &path)).await
    }

    /// Set what one call of `tool` of `name` may use.
    pub async fn budget(&self, name: &str, tool: &str, budget: Budget) -> Result<(), Error> {
        let path = format!("{}/tools/{tool}/budget", self.program(name));
        self.ok(self.at(Method::PUT, &path).json(&budget)).await?;
        Ok(())
    }

    /// `name`'s meters over the last 30 days, a day each.
    pub async fn meters(&self, name: &str) -> Result<Vec<MeteredDay>, Error> {
        let path = format!("{}/meters", self.program(name));
        self.json(self.at(Method::GET, &path)).await
    }

    /// `name`'s cron tools, for its owners.
    pub async fn crons(&self, name: &str) -> Result<Crons, Error> {
        let path = format!("{}/crons", self.program(name));
        self.json(self.at(Method::GET, &path)).await
    }

    /// The wakes armed to call `name`'s tools that have not run, for its owners.
    pub async fn wakes(&self, name: &str) -> Result<Wakes, Error> {
        let path = format!("{}/wakes", self.program(name));
        self.json(self.at(Method::GET, &path)).await
    }

    /// Set `name`'s cron tool `tool` to `cron`, from its next firing; `None`
    /// stops it.
    pub async fn set_cron(&self, name: &str, tool: &str, cron: Option<&str>) -> Result<(), Error> {
        let path = format!("{}/tools/{tool}/cron", self.program(name));
        let body = Cron {
            cron: cron.map(str::to_owned),
        };
        self.ok(self.at(Method::PUT, &path).json(&body)).await?;
        Ok(())
    }

    /// When `name`'s deprecated tool `tool` stops being served, in epoch
    /// seconds; `None` clears it.
    pub async fn sunset(&self, name: &str, tool: &str, sunset: Option<i64>) -> Result<(), Error> {
        let path = format!("{}/tools/{tool}", self.program(name));
        let body = ToolUpdate { sunset };
        self.ok(self.at(Method::PATCH, &path).json(&body)).await?;
        Ok(())
    }

    /// Who `name` answers. Cloud only.
    pub async fn visibility(&self, name: &str) -> Result<Visibility, Error> {
        let path = format!("{}/visibility", self.service(name)?);
        self.json(self.at(Method::GET, &path)).await
    }

    /// Make `name` answer only its account and members, or anyone. Cloud only.
    pub async fn set_visibility(&self, name: &str, private: bool) -> Result<(), Error> {
        let path = format!("{}/visibility", self.service(name)?);
        let req = self.at(Method::PUT, &path).json(&Visibility { private });
        self.ok(req).await?;
        Ok(())
    }

    /// `name`'s calls over the last 30 days, newest first, at most `limit`,
    /// only those of `tool` or ended as `outcome` when given. Cloud only.
    pub async fn calls(
        &self,
        name: &str,
        tool: Option<&str>,
        outcome: Option<&str>,
        limit: u32,
    ) -> Result<Vec<Costed>, Error> {
        let path = format!("{}/calls", self.service(name)?);
        let mut req = self.at(Method::GET, &path).query(&[("limit", limit)]);
        if let Some(tool) = tool {
            req = req.query(&[("tool", tool)]);
        }
        if let Some(outcome) = outcome {
            req = req.query(&[("outcome", outcome)]);
        }
        self.json(req).await
    }

    /// What `name`'s call `id` ran with. Only a call that failed keeps it.
    pub async fn call_input(&self, name: &str, id: u64) -> Result<CallInput, Error> {
        let path = format!("{}/calls/{id}/input", self.program(name));
        self.json(self.at(Method::GET, &path)).await
    }

    /// What `name` used in UTC month `month`, `YYYY-MM`, or this one, and what
    /// it cost. Cloud only.
    pub async fn billing(&self, name: &str, month: Option<&str>) -> Result<Billing, Error> {
        let path = format!("{}/billing", self.service(name)?);
        let mut req = self.at(Method::GET, &path);
        if let Some(month) = month {
            req = req.query(&[("month", month)]);
        }
        self.json(req).await
    }

    /// What `name` is using now, read from the node running it. Cloud only.
    pub async fn live(&self, name: &str) -> Result<Live, Error> {
        let path = format!("{}/usage", self.service(name)?);
        self.json(self.at(Method::GET, &path)).await
    }

    /// `name`'s secrets, by name. Cloud only: a local host reads its secrets from
    /// its own file.
    pub async fn secrets(&self, name: &str) -> Result<Vec<Secret>, Error> {
        let path = self.secrets_path(name)?;
        self.json(self.at(Method::GET, &path)).await
    }

    /// Set `name`'s secret `secret` to `value`. Cloud only.
    pub async fn put_secret(&self, name: &str, secret: &str, value: &str) -> Result<(), Error> {
        let path = format!("{}/{secret}", self.secrets_path(name)?);
        self.ok(self.at(Method::PUT, &path).json(&SetSecret {
            value: value.to_owned(),
        }))
        .await?;
        Ok(())
    }

    /// Revoke `name`'s secret `secret`. Cloud only.
    pub async fn delete_secret(&self, name: &str, secret: &str) -> Result<(), Error> {
        let path = format!("{}/{secret}", self.secrets_path(name)?);
        self.ok(self.at(Method::DELETE, &path)).await?;
        Ok(())
    }

    pub async fn undeploy(&self, name: &str) -> Result<(), Error> {
        self.ok(self.at(Method::DELETE, &self.program(name)))
            .await?;
        Ok(())
    }

    /// `name`'s logs after `after`, oldest first, at most `limit`.
    pub async fn logs(&self, name: &str, after: u64, limit: u64) -> Result<Vec<Logged>, Error> {
        let path = format!("{}/logs", self.program(name));
        let request = self
            .at(Method::GET, &path)
            .query(&[("after", after), ("limit", limit)]);
        self.json(request).await
    }

    fn secrets_path(&self, name: &str) -> Result<String, Error> {
        match &self.owner {
            Some(_) => Ok(format!("{}/secrets", self.program(name))),
            None => Err(Error::Host(
                "the local host reads secrets from secrets.toml under its root",
            )),
        }
    }

    /// `name`'s path on Cloud, which alone serves it.
    fn service(&self, name: &str) -> Result<String, Error> {
        Ok(format!("/services/{}/{name}", self.account()?.name()))
    }

    fn program(&self, name: &str) -> String {
        match &self.owner {
            Some(owner) => format!("/services/{}/{name}", owner.name()),
            None => format!("/programs/{name}"),
        }
    }

    fn as_caller(
        &self,
        request: RequestBuilder,
        caller: Option<&Caller>,
    ) -> Result<RequestBuilder, Error> {
        let Some(caller) = caller else {
            return Ok(request);
        };
        if self.owner.is_some() {
            return Err(Error::Host(
                "Cloud calls as the token; a caller is for the local host",
            ));
        }
        let caller = serde_json::to_string(caller).map_err(|e| Error::Decode(e.to_string()))?;
        Ok(request.header(host::CALLER, caller))
    }

    /// A tool's answer and the meter its headers carry.
    async fn answered(&self, request: RequestBuilder) -> Result<(Output, Option<Meter>), Error> {
        let response = request.send().await?;
        let status = response.status();
        let meter = response
            .headers()
            .get(host::METER)
            .and_then(|value| serde_json::from_slice(value.as_bytes()).ok());
        let body = response.text().await?;
        if !status.is_success() {
            return Err(Error::refusal(status, body));
        }
        let output = serde_json::from_str(&body).map_err(|e| Error::Decode(e.to_string()))?;
        Ok((output, meter))
    }
}
