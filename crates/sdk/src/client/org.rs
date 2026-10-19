//! `/orgs` — orgs, their members and invitations.

use crate::{
    account::org::{
        AddMemberRequest, CreateOrgRequest, CreateOrgResponse, Invite, InviteRequest, Member, Org,
        OrgView, UpdateMemberRequest, UpdateOrgRequest,
    },
    client::{Client, Error},
};
use reqwest::Method;

impl Client {
    /// The orgs the caller belongs to.
    pub async fn orgs(&self) -> Result<Vec<Org>, Error> {
        self.json(self.at(Method::GET, "/user/orgs")).await
    }

    /// `org`, as much of it as the caller may see.
    pub async fn org(&self, org: &str) -> Result<OrgView, Error> {
        self.json(self.at(Method::GET, &format!("/orgs/{org}")))
            .await
    }

    pub async fn create_org(&self, body: &CreateOrgRequest) -> Result<CreateOrgResponse, Error> {
        self.json(self.at(Method::POST, "/orgs").json(body)).await
    }

    pub async fn update_org(&self, org: &str, body: &UpdateOrgRequest) -> Result<(), Error> {
        self.ok(self.at(Method::PATCH, &format!("/orgs/{org}")).json(body))
            .await?;
        Ok(())
    }

    pub async fn delete_org(&self, org: &str) -> Result<(), Error> {
        self.ok(self.at(Method::DELETE, &format!("/orgs/{org}")))
            .await?;
        Ok(())
    }

    pub async fn members(&self, org: &str) -> Result<Vec<Member>, Error> {
        self.json(self.at(Method::GET, &format!("/orgs/{org}/members")))
            .await
    }

    pub async fn add_member(&self, org: &str, body: &AddMemberRequest) -> Result<(), Error> {
        let req = self.at(Method::POST, &format!("/orgs/{org}/members"));
        self.ok(req.json(body)).await?;
        Ok(())
    }

    /// Change `member_id`'s role in `org`.
    pub async fn update_member(
        &self,
        org: &str,
        member_id: &str,
        body: &UpdateMemberRequest,
    ) -> Result<(), Error> {
        let req = self.at(Method::PATCH, &format!("/orgs/{org}/members/{member_id}"));
        self.ok(req.json(body)).await?;
        Ok(())
    }

    /// Remove `member_id` from `org`, or leave it when it is the caller.
    pub async fn remove_member(&self, org: &str, member_id: &str) -> Result<(), Error> {
        let req = self.at(Method::DELETE, &format!("/orgs/{org}/members/{member_id}"));
        self.ok(req).await?;
        Ok(())
    }

    /// `org`'s pending invitations.
    pub async fn invites(&self, org: &str) -> Result<Vec<Invite>, Error> {
        self.json(self.at(Method::GET, &format!("/orgs/{org}/invites")))
            .await
    }

    pub async fn invite(&self, org: &str, body: &InviteRequest) -> Result<Invite, Error> {
        let req = self.at(Method::POST, &format!("/orgs/{org}/invites"));
        self.json(req.json(body)).await
    }

    pub async fn revoke_invite(&self, org: &str, token: &str) -> Result<(), Error> {
        let req = self.at(Method::DELETE, &format!("/orgs/{org}/invites/{token}"));
        self.ok(req).await?;
        Ok(())
    }

    /// The invitations the caller has not yet accepted.
    pub async fn my_invites(&self) -> Result<Vec<Invite>, Error> {
        self.json(self.at(Method::GET, "/user/invites")).await
    }

    pub async fn accept_invite(&self, token: &str) -> Result<(), Error> {
        let req = self.at(Method::POST, &format!("/user/invites/{token}/accept"));
        self.ok(req).await?;
        Ok(())
    }
}
