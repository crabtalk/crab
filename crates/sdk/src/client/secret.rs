use crate::{
    account::secret::{self, Create, Grant, Resource},
    client::{Client, Error},
    services::service::SetSecret,
};
use reqwest::Method;
impl Client {
    pub async fn secret_resources(&self) -> Result<Vec<Resource>, Error> {
        self.json(self.at(Method::GET, &self.secret_resources_path()))
            .await
    }
    pub async fn create_secret_resource(&self, input: &Create) -> Result<Resource, Error> {
        self.json(
            self.at(Method::POST, &self.secret_resources_path())
                .json(input),
        )
        .await
    }
    pub async fn rotate_secret_resource(&self, id: &str, value: &str) -> Result<(), Error> {
        self.ok(self
            .at(
                Method::PUT,
                &format!("{}/{id}", self.secret_resources_path()),
            )
            .json(&SetSecret {
                value: value.into(),
            }))
        .await?;
        Ok(())
    }
    pub async fn delete_secret_resource(&self, id: &str) -> Result<(), Error> {
        self.ok(self.at(
            Method::DELETE,
            &format!("{}/{id}", self.secret_resources_path()),
        ))
        .await?;
        Ok(())
    }
    pub async fn grant_secret_resource(&self, id: &str, grant: &Grant) -> Result<(), Error> {
        self.ok(self
            .at(
                Method::PUT,
                &format!("{}/{id}/grants", self.secret_resources_path()),
            )
            .json(grant))
            .await?;
        Ok(())
    }
    pub async fn revoke_secret_resource(&self, id: &str, service: &str) -> Result<(), Error> {
        self.ok(self.at(
            Method::DELETE,
            &format!("{}/{id}/grants/{service}", self.secret_resources_path()),
        ))
        .await?;
        Ok(())
    }
    fn secret_resources_path(&self) -> String {
        format!("{}/{}", secret::PATH, self.addressed().name())
    }
}
