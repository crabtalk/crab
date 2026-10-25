//! `crab org` — orgs, their members and invitations.

use crate::{format, utils};
use anyhow::Result;
use crabtalk_sdk::{
    account::org::{
        AddMemberRequest, CreateOrgRequest, InviteRequest, Role, UpdateMemberRequest,
        UpdateOrgRequest,
    },
    client::Client,
};

pub fn ls(client: &Client, json: bool) -> Result<()> {
    let orgs = utils::wait(client.orgs())?;
    if json {
        return utils::json(&orgs);
    }
    for org in &orgs {
        println!(
            "{:<24}  {:<24}  {}",
            org.handle.as_deref().unwrap_or(&org.id),
            org.name,
            org.role
        );
    }
    Ok(())
}

pub fn create(client: &Client, handle: &str, name: Option<&str>, json: bool) -> Result<()> {
    let created = utils::wait(client.create_org(&CreateOrgRequest {
        name: name.unwrap_or(handle).to_owned(),
        handle: handle.to_owned(),
        tier: None,
        client: None,
    }))?;
    if json {
        return utils::json(&created);
    }
    eprintln!("org {handle} made; you own it");
    Ok(())
}

pub fn info(client: &Client, org: &str, json: bool) -> Result<()> {
    let view = utils::wait(client.org(org))?;
    if json {
        return utils::json(&view);
    }
    let account = &view.account;
    println!(
        "{}  {}",
        account.handle.as_deref().unwrap_or(&account.id),
        account.name
    );
    println!("  since {}", format::date(account.created_at));
    if let Some(private) = &view.private {
        println!("  you are {}", private.role);
        println!("  plan {}", private.plan);
    }
    Ok(())
}

pub fn update(client: &Client, org: &str, handle: Option<&str>, name: Option<&str>) -> Result<()> {
    utils::wait(client.update_org(
        org,
        &UpdateOrgRequest {
            handle: handle.map(str::to_owned),
            name: name.map(str::to_owned),
            ..UpdateOrgRequest::default()
        },
    ))?;
    eprintln!("updated {org}");
    Ok(())
}

pub fn rm(client: &Client, org: &str, yes: bool) -> Result<()> {
    utils::confirm(yes, &format!("delete org {org} and everything it owns"))?;
    utils::wait(client.delete_org(org))?;
    eprintln!("deleted {org}");
    Ok(())
}

pub fn members(client: &Client, org: &str, json: bool) -> Result<()> {
    let members = utils::wait(client.members(org))?;
    if json {
        return utils::json(&members);
    }
    for member in &members {
        println!(
            "{:<26}  {:<24}  {:<8}  {}",
            member.user_id,
            member.handle.as_deref().unwrap_or(&member.name),
            member.role,
            member.email.as_deref().unwrap_or_default()
        );
    }
    Ok(())
}

/// The role a flag names: owner with `owner`, member otherwise.
fn role(owner: bool) -> Role {
    if owner { Role::Owner } else { Role::Member }
}

pub fn add(client: &Client, org: &str, email: &str, owner: bool) -> Result<()> {
    let role = role(owner);
    utils::wait(client.add_member(
        org,
        &AddMemberRequest {
            email: email.to_owned(),
            role,
        },
    ))?;
    eprintln!("added {email} to {org} as {role}");
    Ok(())
}

pub fn set_role(client: &Client, org: &str, member: &str, owner: bool) -> Result<()> {
    let role = role(owner);
    utils::wait(client.update_member(org, member, &UpdateMemberRequest { role }))?;
    eprintln!("{member} is {role} of {org}");
    Ok(())
}

pub fn remove(client: &Client, org: &str, member: &str, yes: bool) -> Result<()> {
    utils::confirm(yes, &format!("remove {member} from {org}"))?;
    utils::wait(client.remove_member(org, member))?;
    eprintln!("removed {member} from {org}");
    Ok(())
}

pub fn invite(client: &Client, org: &str, email: &str, owner: bool, json: bool) -> Result<()> {
    let invite = utils::wait(client.invite(
        org,
        &InviteRequest {
            email: email.to_owned(),
            role: role(owner),
        },
    ))?;
    if json {
        return utils::json(&invite);
    }
    println!("{}", invite.token);
    eprintln!("send {email} the token above; `crab invites accept <token>` joins {org}");
    Ok(())
}

pub fn invites(client: &Client, org: &str, json: bool) -> Result<()> {
    let invites = utils::wait(client.invites(org))?;
    if json {
        return utils::json(&invites);
    }
    for invite in &invites {
        println!(
            "{:<32}  {:<8}  expires {}  {}",
            invite.email,
            invite.role,
            format::date(invite.expires_at),
            invite.token
        );
    }
    Ok(())
}

pub fn uninvite(client: &Client, org: &str, token: &str) -> Result<()> {
    utils::wait(client.revoke_invite(org, token))?;
    eprintln!("revoked the invitation");
    Ok(())
}
