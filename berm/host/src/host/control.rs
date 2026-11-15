use crate::{Denied, deny, env::Env, host::Host};
use axum::http::StatusCode;
use berm::{abi::workflow::ControlOutput, control};

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
                if error.is::<berm::MemoryLimit>() {
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
