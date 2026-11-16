use crate::{Denied, deny, env::Env, host::Host};
use axum::http::StatusCode;
use berm::{MemoryLimit, abi::workflow::ControlOutput, control};
use berm_api::workflow::run::Failure;

impl<E: Env> Host<E> {
    pub async fn control(
        &self,
        service: &str,
        request: control::Request<'_>,
    ) -> Result<ControlOutput, Denied> {
        self.deployed(service)?
            .control(request)
            .await
            .map_err(|error| {
                if error.is::<MemoryLimit>() || error.is::<Failure>() || error.is::<Denied>() {
                    return Denied::from(error);
                }
                let explain = control::Effect::explanation(&error);
                let denied = deny(StatusCode::UNPROCESSABLE_ENTITY, format!("{error:#}"));
                match explain {
                    Some(code) => denied.explained(code),
                    None => denied,
                }
            })
    }
}
