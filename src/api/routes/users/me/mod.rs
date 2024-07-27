/*
 *  This Source Code Form is subject to the terms of the Mozilla Public
 *  License, v. 2.0. If a copy of the MPL was not distributed with this
 *  file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

mod settings;

use crate::database::entities::User;
use crate::errors::{Error, UserError};
use chorus::types::jwt::Claims;
use poem::web::{Data, Json};
use poem::{get, handler, IntoResponse, Route};
use settings::{get_settings, update_settings};
use sqlx::MySqlPool;

pub fn setup_routes() -> Route {
    Route::new()
        .at("/", get(get_data))
        .at("/settings", get(get_settings).patch(update_settings))
}

#[handler]
pub async fn get_data(
    Data(db): Data<&MySqlPool>,
    Data(claims): Data<&Claims>,
) -> poem::Result<impl IntoResponse> {
    let user = User::get_by_id(db, claims.id)
        .await
        .unwrap()
        .ok_or(Error::User(UserError::InvalidUser))
        .unwrap();

    Ok(Json(user))
}
