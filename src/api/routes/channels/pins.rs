use chorus::types::{jwt::Claims, Snowflake};
use poem::{
    handler,
    http::StatusCode,
    web::{Data, Json, Path},
    IntoResponse, Response,
};
use sqlx::MySqlPool;

use crate::{
    database::entities::{Config, Message},
    errors::{ChannelError, Error},
};

#[handler]
pub async fn add_pinned_message(
    Data(db): Data<&MySqlPool>,
    Data(config): Data<&Config>,
    Data(claims): Data<&Claims>,
    Path((channel_id, message_id)): Path<(Snowflake, Snowflake)>,
) -> poem::Result<impl IntoResponse> {
    let mut message = Message::get_by_id(db, channel_id, message_id)
        .await?
        .ok_or(Error::Channel(ChannelError::InvalidMessage))?;

    if message.guild_id.is_some() {
        // TOOD: Check permission 'MANAGE_MESSAGES'
    }

    let pinned_count = Message::count_pinned(db, channel_id).await?;
    if pinned_count >= config.limits.channel.max_pins as i32 {
        return Err(Error::Channel(ChannelError::MaxPinsReached).into());
    }

    message.set_pinned(db, true).await?;
    // TODO: Emit events 'MESSAGE_UPDATE' AND 'CHANNEL_PINS_UPDATE'

    Ok(Response::builder().status(StatusCode::NO_CONTENT).finish())
}

#[handler]
pub async fn remove_pinned_message(
    Data(db): Data<&MySqlPool>,
    Data(claims): Data<&Claims>,
    Path((channel_id, message_id)): Path<(Snowflake, Snowflake)>,
) -> poem::Result<impl IntoResponse> {
    let mut message = Message::get_by_id(db, channel_id, message_id)
        .await?
        .ok_or(Error::Channel(ChannelError::InvalidMessage))?;

    if message.guild_id.is_some() {
        // TOOD: Check permission 'MANAGE_MESSAGES'
    }

    message.set_pinned(db, false).await?;
    // TODO: Emit events 'MESSAGE_UPDATE' AND 'CHANNEL_PINS_UPDATE'

    Ok(Response::builder().status(StatusCode::NO_CONTENT).finish())
}

#[handler]
pub async fn get_pinned_messages(
    Data(db): Data<&MySqlPool>,
    Path(channel_id): Path<Snowflake>,
) -> poem::Result<impl IntoResponse> {
    // TODO: Check permission 'READ_MESSAGE_HISTORY'
    let messages = Message::get_pinned(db, channel_id).await?;

    Ok(Json(messages))
}
