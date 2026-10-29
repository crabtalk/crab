//! A host's services, at `/services/{owner}/{name}`: on a local host, under
//! [`LOCAL`](crate::client::LOCAL).

use crate::{
    client::{Client, Error},
    services::service::{
        Billing, Costed, DeployRequest, DeploySetRequest, Deployment, Hosts, MeteredDay, Secret,
        SetSecret, Terms, ToolCosted, Upload, UploadRequest, Visibility,
    },
};
use berm_api::{
    Budget, Cron, Deployed, Listing, Logged, Meter, Output, Runtime, Service, Setting, VarValue,
    host::{self, CallInput, Caller, Crons, Live, ToolUpdate, Wakes},
};
use reqwest::{Body, Method, RequestBuilder};
use std::collections::BTreeMap;

impl Client {
    /// Every service the account runs.
    pub async fn list(&self) -> Result<Vec<Service>, Error> {
        self.json(self.at(Method::GET, &self.addressed().services()))
            .await
    }

    /// `name` as anyone it is visible to sees it: its IDL, what it served,
    /// its project and the registry reference it runs.
    pub async fn listing(&self, name: &str) -> Result<Listing, Error> {
        self.json(self.at(Method::GET, &self.path_of(name))).await
    }

    pub async fn inspect(&self, name: &str) -> Result<Service, Error> {
        let path = format!("{}/idl", self.path_of(name));
        self.json(self.at(Method::GET, &path)).await
    }

    /// How `name` runs: what its IDL leaves out, for its owners.
    pub async fn runtime(&self, name: &str) -> Result<Runtime, Error> {
        let path = format!("{}/runtime", self.path_of(name));
        self.json(self.at(Method::GET, &path)).await
    }

    /// Deploy as `name` what `deploy` names.
    pub async fn deploy(&self, name: &str, deploy: &DeployRequest) -> Result<Deployed, Error> {
        let request = self.at(Method::PUT, &self.path_of(name)).json(deploy);
        self.json(request).await
    }

    /// Deploy the services `set` names together: every one is checked before
    /// any switches, and one refused refuses the set.
    pub async fn deploy_set(
        &self,
        set: &DeploySetRequest,
    ) -> Result<BTreeMap<String, Deployed>, Error> {
        let path = format!("{}/deploys", self.addressed().services());
        self.json(self.at(Method::POST, &path).json(set)).await
    }

    /// `name`'s last 100 deploys on Cloud, newest first, refused and failed
    /// ones included.
    pub async fn deploys(&self, name: &str) -> Result<Vec<Deployment>, Error> {
        let path = format!("{}/deploys", self.service(name)?);
        self.json(self.at(Method::GET, &path)).await
    }

    /// Where to upload `seed` for a deploy of `name` on Cloud to name.
    pub async fn seed(&self, name: &str, seed: &UploadRequest) -> Result<Upload, Error> {
        self.account()?;
        let path = format!("{}/seed", self.path_of(name));
        self.json(self.at(Method::POST, &path).json(seed)).await
    }

    /// Where to upload `image` for a deploy of `name` to name.
    pub async fn image(&self, name: &str, image: &UploadRequest) -> Result<Upload, Error> {
        let path = format!("{}/image", self.path_of(name));
        self.json(self.at(Method::POST, &path).json(image)).await
    }

    /// Upload the `size` bytes of `body` to `url`, an upload [`Client::seed`]
    /// or [`Client::image`] answered.
    pub async fn upload(&self, url: &str, body: impl Into<Body>, size: u64) -> Result<(), Error> {
        let request = self
            .http
            .put(url)
            .header(reqwest::header::CONTENT_LENGTH, size)
            .body(body);
        self.ok(request).await.map(drop)
    }

    /// Call `tool` of `service`, as `caller` on a local host, which takes it
    /// unverified.
    pub async fn call(
        &self,
        service: &str,
        tool: &str,
        arguments: Vec<u8>,
        caller: Option<&Caller>,
    ) -> Result<(Output, Option<Meter>), Error> {
        if self.owner.is_none() && service.contains('/') {
            return Err(Error::Host(
                "the local host has no accounts: call its services by name alone",
            ));
        }
        let path = format!("{}/tools/{tool}", self.path_of(service));
        let request = self.as_caller(self.at(Method::POST, &path).body(arguments), caller)?;
        self.answered(request).await
    }

    /// What a call of each of `name`'s public tools is held to and costs.
    pub async fn terms(&self, name: &str) -> Result<Terms, Error> {
        let path = format!("{}/terms", self.path_of(name));
        self.json(self.at(Method::GET, &path)).await
    }

    /// What each of `name`'s tools used over the last 30 days, by tool, for
    /// its owners. A tool not called is left out.
    pub async fn tools(&self, name: &str) -> Result<BTreeMap<String, ToolCosted>, Error> {
        let path = format!("{}/calls/tools", self.path_of(name));
        self.json(self.at(Method::GET, &path)).await
    }

    /// Set what one call of `tool` of `name` may use.
    pub async fn budget(&self, name: &str, tool: &str, budget: Budget) -> Result<(), Error> {
        let path = format!("{}/tools/{tool}/budget", self.path_of(name));
        self.ok(self.at(Method::PUT, &path).json(&budget)).await?;
        Ok(())
    }

    /// `name`'s meters over the last 30 days, a day each.
    pub async fn meters(&self, name: &str) -> Result<Vec<MeteredDay>, Error> {
        let path = format!("{}/meters", self.path_of(name));
        self.json(self.at(Method::GET, &path)).await
    }

    /// `name`'s cron tools, for its owners.
    pub async fn crons(&self, name: &str) -> Result<Crons, Error> {
        let path = format!("{}/crons", self.path_of(name));
        self.json(self.at(Method::GET, &path)).await
    }

    /// The wakes armed to call `name`'s tools that have not run, for its owners.
    pub async fn wakes(&self, name: &str) -> Result<Wakes, Error> {
        let path = format!("{}/wakes", self.path_of(name));
        self.json(self.at(Method::GET, &path)).await
    }

    /// Set `name`'s cron tool `tool` to `cron`, from its next firing; `None`
    /// stops it.
    pub async fn set_cron(&self, name: &str, tool: &str, cron: Option<&str>) -> Result<(), Error> {
        let path = format!("{}/tools/{tool}/cron", self.path_of(name));
        let body = Cron {
            cron: cron.map(str::to_owned),
        };
        self.ok(self.at(Method::PUT, &path).json(&body)).await?;
        Ok(())
    }

    /// Each var of `name`'s `[vars]`, with the value it holds, for its owners.
    pub async fn vars(&self, name: &str) -> Result<Vec<Setting>, Error> {
        let path = format!("{}/vars", self.path_of(name));
        self.json(self.at(Method::GET, &path)).await
    }

    /// Set `name`'s var `var` to `value`, read from its next call; `None`
    /// returns it to its default.
    pub async fn set_var(&self, name: &str, var: &str, value: Option<&str>) -> Result<(), Error> {
        let path = format!("{}/vars/{var}", self.path_of(name));
        let body = VarValue {
            value: value.map(str::to_owned),
        };
        self.ok(self.at(Method::PUT, &path).json(&body)).await?;
        Ok(())
    }

    /// When `name`'s deprecated tool `tool` stops being served, in epoch
    /// seconds; `None` clears it.
    pub async fn sunset(&self, name: &str, tool: &str, sunset: Option<i64>) -> Result<(), Error> {
        let path = format!("{}/tools/{tool}", self.path_of(name));
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
        let path = format!("{}/calls/{id}/input", self.path_of(name));
        self.json(self.at(Method::GET, &path)).await
    }

    /// `name`'s call `id`'s tape, as berm encodes one. Only a call that failed
    /// keeps it.
    pub async fn call_tape(&self, name: &str, id: u64) -> Result<Vec<u8>, Error> {
        let path = format!("{}/calls/{id}/tape", self.path_of(name));
        self.bytes(self.at(Method::GET, &path)).await
    }

    /// The image `name`'s call `id` ran. Only a call that failed names one.
    pub async fn call_image(&self, name: &str, id: u64) -> Result<Vec<u8>, Error> {
        let path = format!("{}/calls/{id}/image", self.path_of(name));
        self.bytes(self.at(Method::GET, &path)).await
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

    /// `name`'s secrets, by name.
    pub async fn secrets(&self, name: &str) -> Result<Vec<Secret>, Error> {
        let path = self.secrets_path(name);
        self.json(self.at(Method::GET, &path)).await
    }

    /// Set `name`'s secret `secret` to `value`, sent only to `hosts`.
    pub async fn put_secret(
        &self,
        name: &str,
        secret: &str,
        value: &str,
        hosts: &[String],
    ) -> Result<(), Error> {
        let path = format!("{}/{secret}", self.secrets_path(name));
        self.ok(self.at(Method::PUT, &path).json(&SetSecret {
            value: value.to_owned(),
            hosts: hosts.to_vec(),
        }))
        .await?;
        Ok(())
    }

    /// Send `name`'s secret `secret` only to `hosts` from now on.
    pub async fn put_secret_hosts(
        &self,
        name: &str,
        secret: &str,
        hosts: &[String],
    ) -> Result<(), Error> {
        let path = format!("{}/{secret}/hosts", self.secrets_path(name));
        self.ok(self.at(Method::PUT, &path).json(&Hosts {
            hosts: hosts.to_vec(),
        }))
        .await?;
        Ok(())
    }

    /// Revoke `name`'s secret `secret`.
    pub async fn delete_secret(&self, name: &str, secret: &str) -> Result<(), Error> {
        let path = format!("{}/{secret}", self.secrets_path(name));
        self.ok(self.at(Method::DELETE, &path)).await?;
        Ok(())
    }

    /// Serve `name` again, stopped or deployed stopped.
    pub async fn start(&self, name: &str) -> Result<(), Error> {
        let path = format!("{}/start", self.path_of(name));
        self.ok(self.at(Method::POST, &path)).await?;
        Ok(())
    }

    /// Stop serving `name`, keeping what it committed.
    pub async fn stop(&self, name: &str) -> Result<(), Error> {
        let path = format!("{}/stop", self.path_of(name));
        self.ok(self.at(Method::POST, &path)).await?;
        Ok(())
    }

    pub async fn undeploy(&self, name: &str) -> Result<(), Error> {
        self.ok(self.at(Method::DELETE, &self.path_of(name)))
            .await?;
        Ok(())
    }

    /// `name`'s logs after `after`, oldest first, at most `limit`.
    pub async fn logs(&self, name: &str, after: u64, limit: u64) -> Result<Vec<Logged>, Error> {
        let path = format!("{}/logs", self.path_of(name));
        let request = self
            .at(Method::GET, &path)
            .query(&[("after", after), ("limit", limit)]);
        self.json(request).await
    }

    fn secrets_path(&self, name: &str) -> String {
        format!("{}/secrets", self.path_of(name))
    }

    /// `name`'s path, for a route only Cloud serves.
    fn service(&self, name: &str) -> Result<String, Error> {
        self.account()?;
        Ok(self.path_of(name))
    }

    /// The path of `name`: `owner/name` names another account's service, and
    /// a bare name the context's own.
    fn path_of(&self, name: &str) -> String {
        match name.contains('/') {
            true => format!("/services/{name}"),
            false => format!("/services/{}/{name}", self.addressed().name()),
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
