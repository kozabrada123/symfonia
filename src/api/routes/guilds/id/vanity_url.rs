/* 
 *  This Source Code Form is subject to the terms of the Mozilla Public
 *  License, v. 2.0. If a copy of the MPL was not distributed with this
 *  file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

use chorus::types::{
    jwt::Claims, types::guild_configuration::GuildFeatures, GuildCreateVanitySchema,
    GuildVanityInviteResponse, Snowflake,
};
use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json, Path},
    IntoResponse,
};
use sqlx::MySqlPool;

use crate::{
    database::entities::{Guild, Invite},
    errors::{Error, GuildError},
};

#[handler]
pub async fn get_vanity(
    Data(db): Data<&MySqlPool>,
    Data(claims): Data<&Claims>,
    Path(guild_id): Path<Snowflake>,
) -> poem::Result<impl IntoResponse> {
    let guild = Guild::get_by_id(db, guild_id)
        .await?
        .ok_or(Error::Guild(GuildError::InvalidGuild))?;

    if !guild.has_member(db, claims.id).await? {
        return Err(Error::Guild(GuildError::MemberNotFound).into());
    }

    // tODO: Check permissions

    if !guild.features.contains(&GuildFeatures::AliasableNames) {
        if let Some(invite) = Invite::get_by_guild_vanity(db, guild.id).await? {
            return Ok(Json(GuildVanityInviteResponse {
                code: invite.code.to_owned(),
                uses: invite.uses,
            })
            .with_status(StatusCode::OK));
        }
    }
    Ok(Json(GuildVanityInviteResponse {
        code: "".to_string(),
        uses: None,
    })
    .with_status(StatusCode::NOT_FOUND))
}

#[handler]
pub async fn set_vanity(
    Data(db): Data<&MySqlPool>,
    Data(claims): Data<&Claims>,
    Path(guild_id): Path<Snowflake>,
    Json(payload): Json<GuildCreateVanitySchema>,
) -> poem::Result<impl IntoResponse> {
    let guild = Guild::get_by_id(db, guild_id)
        .await?
        .ok_or(Error::Guild(GuildError::InvalidGuild))?;

    if !guild.has_member(db, claims.id).await? {
        return Err(Error::Guild(GuildError::MemberNotFound).into());
    }

    // TODO: Check permissions

    if let Some(mut current_vanity) = Invite::get_by_guild_vanity(db, guild.id).await? {
        current_vanity.set_code(db, &payload.code).await?;
    } else {
        Invite::create_vanity(db, guild.id, &payload.code).await?;
    }

    Ok(Json(GuildVanityInviteResponse {
        code: payload.code,
        uses: None,
    }))
}
