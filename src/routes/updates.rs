use std::collections::HashMap;

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Serialize;
use sqlx::PgPool;

use crate::database;
use crate::models::projects::{Version, VersionType};
use crate::util::auth::{get_user_from_headers, is_authorized};

use super::ApiError;

#[get("{id}/forge_updates.json")]
pub async fn forge_updates(
    req: HttpRequest,
    info: web::Path<(String,)>,
    pool: web::Data<PgPool>,
) -> Result<HttpResponse, ApiError> {
    const ERROR: &str = "The specified project does not exist!";

    let (id,) = info.into_inner();

    let project = database::models::Project::get_full_from_slug_or_project_id(
        &id, &**pool,
    )
    .await?
    .ok_or_else(|| ApiError::InvalidInputError(ERROR.to_string()))?;

    let user_option = get_user_from_headers(req.headers(), &**pool).await.ok();

    if !is_authorized(&project, &user_option, &pool).await? {
        return Err(ApiError::InvalidInputError(ERROR.to_string()));
    }

    let version_ids = database::models::Version::get_project_versions(
        project.inner.id,
        None,
        Some(vec!["forge".to_string()]),
        &**pool,
    )
    .await?;

    let mut versions =
        database::models::Version::get_many_full(version_ids, &**pool).await?;
    versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));

    #[derive(Serialize)]
    struct ForgeUpdates {
        homepage: String,
        promos: HashMap<String, String>,
    }

    let mut response = ForgeUpdates {
        homepage: format!(
            "{}/mod/{}",
            dotenv::var("SITE_URL").unwrap_or_default(),
            id
        ),
        promos: HashMap::new(),
    };

    for version in versions {
        let version = Version::from(version);

        if version.version_type == VersionType::Release {
            for game_version in &version.game_versions {
                response
                    .promos
                    .entry(format!("{}-recommended", game_version.0))
                    .or_insert_with(|| version.version_number.clone());
            }
        }

        for game_version in &version.game_versions {
            response
                .promos
                .entry(format!("{}-latest", game_version.0))
                .or_insert_with(|| version.version_number.clone());
        }
    }

    Ok(HttpResponse::Ok().json(response))
}